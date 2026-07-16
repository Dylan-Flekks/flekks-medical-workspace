use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::workspace_context_spec::WORKSPACE_CONTEXT_READ_TOOL_NAME;
use crate::tools::handlers::workspace_context_spec::create_workspace_context_read_tool;
use crate::tools::handlers::workspace_context_spec::create_workspace_planning_context_read_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_tools::ToolName;
use codex_tools::ToolOutput;
use codex_tools::ToolSpec;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct WorkspaceContextReadHandler {
    execution: WorkspaceContextExecution,
}

#[derive(Clone)]
enum WorkspaceContextExecution {
    Agent(codex_state::WorkspaceAgentExecutionBinding),
    Planning {
        execution: codex_state::WorkspacePlanningGuideExecutionBinding,
        evidence_read_ids: Arc<Mutex<Vec<String>>>,
    },
}

impl WorkspaceContextReadHandler {
    pub(crate) fn bound_to_execution(
        execution: codex_state::WorkspaceAgentExecutionBinding,
    ) -> Self {
        Self {
            execution: WorkspaceContextExecution::Agent(execution),
        }
    }

    pub(crate) fn bound_to_planning_execution(
        execution: codex_state::WorkspacePlanningGuideExecutionBinding,
        evidence_read_ids: Arc<Mutex<Vec<String>>>,
    ) -> Self {
        Self {
            execution: WorkspaceContextExecution::Planning {
                execution,
                evidence_read_ids,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum WorkspaceContextCategory {
    VisitHistory,
    ProgressNotes,
    PatientChart,
    SelectedContext,
}

impl WorkspaceContextCategory {
    fn as_str(self) -> &'static str {
        match self {
            Self::VisitHistory => "visit_history",
            Self::ProgressNotes => "progress_notes",
            Self::PatientChart => "patient_chart",
            Self::SelectedContext => "selected_context",
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct WorkspaceContextReadArgs {
    run_id: String,
    category: WorkspaceContextCategory,
    limit: Option<u32>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct WorkspaceContextReadResult {
    run_id: String,
    packet_id: String,
    client_id: String,
    note_id: Option<String>,
    category: String,
    max_records: u32,
    sources: Vec<WorkspaceContextSource>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct WorkspaceContextSource {
    id: String,
    run_id: String,
    source_entity_type: String,
    source_entity_id: String,
    source_revision: Option<i64>,
    display_label: String,
    snapshot_json: String,
    content_sha256: String,
    access_purpose: String,
    accessed_at: i64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct WorkspacePlanningContextReadResult {
    #[serde(skip)]
    id: String,
    run_id: String,
    plan_session_id: String,
    client_id: String,
    category: String,
    max_records: u32,
    response_sha256: String,
    sources: Vec<WorkspacePlanningContextSource>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct WorkspacePlanningContextSource {
    source_entity_type: String,
    source_entity_id: String,
    source_revision: Option<i64>,
    display_label: String,
    snapshot_json: String,
    content_sha256: String,
}

impl ToolExecutor<ToolInvocation> for WorkspaceContextReadHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(WORKSPACE_CONTEXT_READ_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        match &self.execution {
            WorkspaceContextExecution::Agent(_) => create_workspace_context_read_tool(),
            WorkspaceContextExecution::Planning { .. } => {
                create_workspace_planning_context_read_tool()
            }
        }
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        let execution = self.execution.clone();
        Box::pin(async move {
            let ToolInvocation {
                session,
                call_id,
                payload,
                ..
            } = invocation;
            let arguments = match payload {
                ToolPayload::Function { arguments } => arguments,
                _ => {
                    return Err(FunctionCallError::RespondToModel(
                        "workspace_context_read handler received unsupported payload".to_string(),
                    ));
                }
            };
            let args: WorkspaceContextReadArgs = parse_arguments(arguments.as_str())?;
            let state_db = session.state_db().ok_or_else(|| {
                FunctionCallError::RespondToModel(
                    "workspace store is unavailable for this session".to_string(),
                )
            })?;
            let (value, source_count) = match execution {
                WorkspaceContextExecution::Agent(execution) => {
                    let result = read_workspace_context(state_db, args, &execution).await?;
                    let source_count = result.sources.len();
                    let value = serde_json::to_value(result).map_err(|err| {
                        FunctionCallError::Fatal(format!(
                            "failed to serialize workspace context: {err}"
                        ))
                    })?;
                    (value, source_count)
                }
                WorkspaceContextExecution::Planning {
                    execution,
                    evidence_read_ids,
                } => {
                    let result = read_workspace_planning_context(
                        state_db,
                        args,
                        &execution,
                        call_id.as_str(),
                    )
                    .await?;
                    let read_id = result.id.clone();
                    let source_count = result.sources.len();
                    let value = serde_json::to_value(result).map_err(|err| {
                        FunctionCallError::Fatal(format!(
                            "failed to serialize workspace planning context: {err}"
                        ))
                    })?;
                    {
                        let mut ids = evidence_read_ids.lock().await;
                        if !ids.contains(&read_id) {
                            ids.push(read_id);
                        }
                    }
                    (value, source_count)
                }
            };
            Ok(boxed_tool_output(WorkspaceContextReadToolOutput {
                value,
                source_count,
            }))
        })
    }
}

impl CoreToolRuntime for WorkspaceContextReadHandler {}

struct WorkspaceContextReadToolOutput {
    value: serde_json::Value,
    source_count: usize,
}

impl ToolOutput for WorkspaceContextReadToolOutput {
    fn log_preview(&self) -> String {
        format!(
            "workspace_context_read completed with {} source snapshot(s); clinical content redacted",
            self.source_count
        )
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn contains_external_context(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output: FunctionCallOutputPayload {
                body: FunctionCallOutputBody::Text(self.value.to_string()),
                success: Some(true),
            },
        }
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> serde_json::Value {
        serde_json::json!({
            "source_count": self.source_count,
            "clinical_content_redacted": true,
        })
    }
}

async fn read_workspace_context(
    state_db: crate::StateDbHandle,
    args: WorkspaceContextReadArgs,
    execution: &codex_state::WorkspaceAgentExecutionBinding,
) -> Result<WorkspaceContextReadResult, FunctionCallError> {
    let run_id = args.run_id.trim();
    if run_id.is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "run_id must not be empty".to_string(),
        ));
    }
    if run_id != execution.run_id {
        return Err(FunctionCallError::RespondToModel(
            "workspace_context_read run_id does not match the current restricted turn".to_string(),
        ));
    }

    let request = codex_state::WorkspaceAgentContextReadRequest {
        run_id: run_id.to_string(),
        category: args.category.as_str().to_string(),
        max_records: args.limit,
    };
    let context = state_db
        .workspace()
        .read_authorized_agent_context_for_execution(request, execution.clone())
        .await
        .map_err(read_error)?;
    Ok(WorkspaceContextReadResult {
        run_id: context.run_id,
        packet_id: context.packet_id,
        client_id: context.client_id,
        note_id: context.note_id,
        category: context.category,
        max_records: context.max_records,
        sources: context
            .sources
            .into_iter()
            .map(WorkspaceContextSource::from)
            .collect(),
    })
}

async fn read_workspace_planning_context(
    state_db: crate::StateDbHandle,
    args: WorkspaceContextReadArgs,
    execution: &codex_state::WorkspacePlanningGuideExecutionBinding,
    idempotency_key: &str,
) -> Result<WorkspacePlanningContextReadResult, FunctionCallError> {
    let run_id = args.run_id.trim();
    if run_id.is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "run_id must not be empty".to_string(),
        ));
    }
    if run_id != execution.guide_run_id {
        return Err(FunctionCallError::RespondToModel(
            "workspace_context_read run_id does not match the current restricted planning turn"
                .to_string(),
        ));
    }
    let context = state_db
        .workspace()
        .read_authorized_planning_context(codex_state::WorkspacePlanningContextReadRequest {
            execution: execution.clone(),
            category: args.category.as_str().to_string(),
            max_records: args.limit,
            idempotency_key: idempotency_key.to_string(),
        })
        .await
        .map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "failed to read workspace planning context: {err}"
            ))
        })?;
    Ok(WorkspacePlanningContextReadResult {
        id: context.id,
        run_id: context.guide_run_id,
        plan_session_id: context.plan_session_id,
        client_id: context.client_id,
        category: context.category,
        max_records: context.max_records,
        response_sha256: context.response_sha256,
        sources: context
            .sources
            .into_iter()
            .map(WorkspacePlanningContextSource::from)
            .collect(),
    })
}

fn read_error(err: anyhow::Error) -> FunctionCallError {
    FunctionCallError::RespondToModel(format!("failed to read workspace context: {err}"))
}

fn timestamp(value: DateTime<Utc>) -> i64 {
    value.timestamp()
}

impl From<codex_state::WorkspaceAgentRunSource> for WorkspaceContextSource {
    fn from(value: codex_state::WorkspaceAgentRunSource) -> Self {
        Self {
            id: value.id,
            run_id: value.run_id,
            source_entity_type: value.source_entity_type,
            source_entity_id: value.source_entity_id,
            source_revision: value.source_revision,
            display_label: value.display_label,
            snapshot_json: value.snapshot_json,
            content_sha256: value.content_sha256,
            access_purpose: value.access_purpose,
            accessed_at: timestamp(value.accessed_at),
        }
    }
}

impl From<codex_state::WorkspacePlanningContextSource> for WorkspacePlanningContextSource {
    fn from(value: codex_state::WorkspacePlanningContextSource) -> Self {
        Self {
            source_entity_type: value.source_entity_type,
            source_entity_id: value.source_entity_id,
            source_revision: value.source_revision,
            display_label: value.display_label,
            snapshot_json: value.snapshot_json,
            content_sha256: value.content_sha256,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    async fn test_state_db() -> (tempfile::TempDir, crate::StateDbHandle) {
        let temp = tempfile::tempdir().expect("create temp dir");
        let state_db =
            codex_state::StateRuntime::init(temp.path().to_path_buf(), "test-provider".to_string())
                .await
                .expect("state db should initialize");
        state_db
            .workspace()
            .provision_synthetic_workspace("core workspace context test fixture")
            .await
            .expect("test workspace should be classified synthetic");
        (temp, state_db)
    }

    async fn seed_authorized_run(
        state_db: &crate::StateDbHandle,
        categories: &[&str],
    ) -> (
        codex_state::WorkspaceClient,
        codex_state::WorkspaceNote,
        codex_state::WorkspaceEncounter,
        codex_state::WorkspaceAgentRun,
        codex_state::WorkspaceAgentExecutionBinding,
    ) {
        let client = state_db
            .workspace()
            .upsert_client(codex_state::WorkspaceClientUpsert {
                display_name: "Jordan Patient".to_string(),
                summary: "Synthetic core-tool patient.".to_string(),
                ..Default::default()
            })
            .await
            .expect("client should save");
        let encounter = state_db
            .workspace()
            .upsert_encounter(codex_state::WorkspaceEncounterUpsert {
                client_id: client.id.clone(),
                kind: "therapy".to_string(),
                title: "Authorized synthetic visit".to_string(),
                status: "completed".to_string(),
                ..Default::default()
            })
            .await
            .expect("encounter should save");
        let note = state_db
            .workspace()
            .upsert_note(codex_state::WorkspaceNoteUpsert {
                client_id: client.id.clone(),
                encounter_id: Some(encounter.id.clone()),
                title: "Progress note".to_string(),
                kind: "progress".to_string(),
                body: "Exact human-authored synthetic note.".to_string(),
                status: "draft".to_string(),
                actor: "Clinician Example".to_string(),
                ..Default::default()
            })
            .await
            .expect("note should save");
        let envelope = serde_json::json!({
            "assemblyVersion": "core-tool-context-test-v1",
            "sourceMode": "agent_request",
            "includeDocuments": false,
            "humanRequest": "Read explicitly authorized chart context.",
            "ids": {
                "selectedArtifactIds": [],
                "selectedDerivativeIds": [],
                "selectedClipIds": [],
            },
            "note": { "revision": note.current_revision },
            "safety": [
                "read-only context packet; do not mutate workspace records",
                "do not sign notes, submit claims, send payer communications, or overwrite saved data",
            ],
            "promptSnapshot": "Synthetic packet without filesystem paths.",
        })
        .to_string();
        let packet = state_db
            .workspace()
            .prepare_context_packet(codex_state::WorkspaceContextPacketCreate {
                client_id: client.id.clone(),
                encounter_id: Some(encounter.id.clone()),
                note_id: Some(note.id.clone()),
                human_request: "Read explicitly authorized chart context.".to_string(),
                selected_artifact_ids_json: "[]".to_string(),
                selected_derivative_ids_json: "[]".to_string(),
                selected_clip_ids_json: "[]".to_string(),
                context_envelope_json: envelope,
                base_note_revision: Some(note.current_revision),
                authorized_scope_json: serde_json::json!({
                    "categories": categories,
                    "maxRecords": 5,
                })
                .to_string(),
                expected_output_kind: "note_proposal".to_string(),
                actor: "Clinician Example".to_string(),
                ..Default::default()
            })
            .await
            .expect("packet should prepare");
        let run = state_db
            .workspace()
            .start_agent_run(codex_state::WorkspaceAgentRunStart {
                packet_id: packet.id.clone(),
                expected_client_id: client.id.clone(),
                expected_context_envelope_sha256: packet.context_envelope_sha256.clone(),
                run_kind: "agent".to_string(),
                idempotency_key: format!("core-tool-context-test-{}", packet.id),
                provider: "test-provider".to_string(),
                model: "test-model".to_string(),
                source_thread_id: Some(format!("thread-core-context-{}", packet.id)),
                actor: "Clinician Example".to_string(),
                ..Default::default()
            })
            .await
            .expect("run should start");
        let execution = codex_state::WorkspaceAgentExecutionBinding {
            run_id: run.id.clone(),
            source_thread_id: run
                .source_thread_id
                .clone()
                .expect("agent run should preserve its source thread"),
            source_turn_id: format!("turn-core-context-{}", run.id),
            provider: run.provider.clone(),
            model: run.model.clone(),
        };
        state_db
            .workspace()
            .claim_agent_turn(codex_state::WorkspaceAgentTurnClaim {
                execution: execution.clone(),
                prompt: codex_state::render_workspace_agent_handoff_prompt(
                    &codex_state::WorkspaceAgentHandoffPromptInput::from(&packet),
                    Some(&run.id),
                ),
            })
            .await
            .expect("run should claim its execution binding");
        (client, note, encounter, run, execution)
    }

    #[test]
    fn workspace_context_read_args_require_run_and_category() {
        assert!(
            serde_json::from_str::<WorkspaceContextReadArgs>(r#"{"category":"visit_history"}"#)
                .is_err()
        );
        assert!(serde_json::from_str::<WorkspaceContextReadArgs>(r#"{"run_id":"run-1"}"#).is_err());
        assert!(
            serde_json::from_str::<WorkspaceContextReadArgs>(
                r#"{"run_id":"run-1","category":"documents"}"#
            )
            .is_err()
        );
    }

    #[test]
    fn workspace_context_read_tool_output_redacts_generic_observability_surfaces() {
        let clinical_marker = "synthetic-clinical-content-must-not-be-logged";
        let output = WorkspaceContextReadToolOutput {
            value: serde_json::json!({
                "run_id": "run-capability-must-not-be-logged",
                "sources": [{"snapshot_json": clinical_marker}],
            }),
            source_count: 1,
        };
        let payload = ToolPayload::Function {
            arguments: r#"{"run_id":"run-capability-must-not-be-logged"}"#.to_string(),
        };

        let preview = output.log_preview();
        assert_eq!(
            preview,
            "workspace_context_read completed with 1 source snapshot(s); clinical content redacted"
        );
        assert!(!preview.contains(clinical_marker));
        assert!(output.contains_external_context());
        assert_eq!(
            output.code_mode_result(&payload),
            serde_json::json!({
                "source_count": 1,
                "clinical_content_redacted": true,
            })
        );

        let response = output.to_response_item("call-1", &payload);
        let serialized = serde_json::to_string(&response).expect("response should serialize");
        assert!(serialized.contains(clinical_marker));
    }

    #[tokio::test]
    async fn workspace_context_read_returns_authorized_hashed_sources() {
        let (_temp, state_db) = test_state_db().await;
        let (client, note, encounter, run, execution) =
            seed_authorized_run(&state_db, &["visit_history", "progress_notes"]).await;

        let result = read_workspace_context(
            state_db.clone(),
            WorkspaceContextReadArgs {
                run_id: run.id.clone(),
                category: WorkspaceContextCategory::VisitHistory,
                limit: Some(4),
            },
            &execution,
        )
        .await
        .expect("authorized context read should succeed");

        assert_eq!(result.run_id, run.id);
        assert_eq!(result.client_id, client.id);
        assert_eq!(result.note_id.as_deref(), Some(note.id.as_str()));
        assert_eq!(result.category, "visit_history");
        assert_eq!(result.max_records, 4);
        assert_eq!(result.sources.len(), 1);
        assert_eq!(result.sources[0].source_entity_type, "encounter");
        assert_eq!(result.sources[0].source_entity_id, encounter.id);
        assert_eq!(result.sources[0].content_sha256.len(), 64);
        let snapshot: serde_json::Value =
            serde_json::from_str(&result.sources[0].snapshot_json).expect("snapshot should parse");
        assert_eq!(snapshot["client_id"], client.id);

        let persisted_sources = state_db
            .workspace()
            .list_agent_run_sources(&run.id)
            .await
            .expect("source manifest should list");
        assert!(
            persisted_sources.len() >= 2,
            "authoritative packet and authorized visit should persist"
        );
        let persisted_visit = persisted_sources
            .iter()
            .find(|source| source.source_entity_id == encounter.id)
            .expect("authorized visit source should persist");
        assert_eq!(
            result.sources[0].content_sha256,
            persisted_visit.content_sha256
        );
        assert_eq!(
            result.sources[0].snapshot_json,
            persisted_visit.snapshot_json
        );
    }

    #[tokio::test]
    async fn workspace_context_read_denies_missing_terminal_or_unscoped_run() {
        let (_temp, state_db) = test_state_db().await;
        let (_client, _note, _encounter, run, execution) =
            seed_authorized_run(&state_db, &["visit_history"]).await;

        let missing = read_workspace_context(
            state_db.clone(),
            WorkspaceContextReadArgs {
                run_id: "missing-run".to_string(),
                category: WorkspaceContextCategory::VisitHistory,
                limit: None,
            },
            &execution,
        )
        .await
        .expect_err("missing run should be denied");
        assert!(matches!(missing, FunctionCallError::RespondToModel(_)));

        let unscoped = read_workspace_context(
            state_db.clone(),
            WorkspaceContextReadArgs {
                run_id: run.id.clone(),
                category: WorkspaceContextCategory::ProgressNotes,
                limit: None,
            },
            &execution,
        )
        .await
        .expect_err("category omitted from packet must be denied");
        let FunctionCallError::RespondToModel(unscoped_message) = unscoped else {
            panic!("scope denial should be returned to the model");
        };
        assert!(unscoped_message.contains("does not explicitly authorize"));

        state_db
            .workspace()
            .update_agent_run_status(codex_state::WorkspaceAgentRunStatusUpdate {
                run_id: run.id.clone(),
                status: "canceled".to_string(),
                actor: "Clinician Example".to_string(),
                ..Default::default()
            })
            .await
            .expect("run cancellation should save");
        let terminal = read_workspace_context(
            state_db,
            WorkspaceContextReadArgs {
                run_id: run.id,
                category: WorkspaceContextCategory::VisitHistory,
                limit: None,
            },
            &execution,
        )
        .await
        .expect_err("terminal run should be denied");
        let FunctionCallError::RespondToModel(terminal_message) = terminal else {
            panic!("lifecycle denial should be returned to the model");
        };
        assert!(terminal_message.contains("cannot read additional context"));
    }

    #[tokio::test]
    async fn workspace_context_read_rejects_empty_run_id() {
        let (_temp, state_db) = test_state_db().await;
        let (_client, _note, _encounter, _run, execution) =
            seed_authorized_run(&state_db, &["visit_history"]).await;
        let result = read_workspace_context(
            state_db,
            WorkspaceContextReadArgs {
                run_id: "  ".to_string(),
                category: WorkspaceContextCategory::VisitHistory,
                limit: None,
            },
            &execution,
        )
        .await;

        let Err(FunctionCallError::RespondToModel(message)) = result else {
            panic!("expected empty run_id to be rejected");
        };
        assert_eq!(message, "run_id must not be empty");
    }

    #[tokio::test]
    async fn restricted_workspace_context_read_rejects_a_previous_authorized_run() {
        let (_temp, state_db) = test_state_db().await;
        let (
            _previous_client,
            _previous_note,
            _previous_encounter,
            previous_run,
            _previous_execution,
        ) = seed_authorized_run(&state_db, &["visit_history"]).await;
        let (_current_client, _current_note, _current_encounter, _current_run, current_execution) =
            seed_authorized_run(&state_db, &["visit_history"]).await;

        let result = read_workspace_context(
            state_db,
            WorkspaceContextReadArgs {
                run_id: previous_run.id,
                category: WorkspaceContextCategory::VisitHistory,
                limit: None,
            },
            &current_execution,
        )
        .await;

        let Err(FunctionCallError::RespondToModel(message)) = result else {
            panic!("expected cross-run context read to be rejected");
        };
        assert_eq!(
            message,
            "workspace_context_read run_id does not match the current restricted turn"
        );
    }
}
