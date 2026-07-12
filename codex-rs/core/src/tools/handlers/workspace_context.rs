use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::workspace_context_spec::WORKSPACE_CONTEXT_READ_TOOL_NAME;
use crate::tools::handlers::workspace_context_spec::create_workspace_context_read_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use chrono::DateTime;
use chrono::Utc;
use codex_tools::JsonToolOutput;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde::Deserialize;
use serde::Serialize;

pub struct WorkspaceContextReadHandler;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum WorkspaceContextCategory {
    VisitHistory,
    ProgressNotes,
}

impl WorkspaceContextCategory {
    fn as_str(self) -> &'static str {
        match self {
            Self::VisitHistory => "visit_history",
            Self::ProgressNotes => "progress_notes",
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

impl ToolExecutor<ToolInvocation> for WorkspaceContextReadHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(WORKSPACE_CONTEXT_READ_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        create_workspace_context_read_tool()
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(async move {
            let ToolInvocation {
                session, payload, ..
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
            let result = read_workspace_context(state_db, args).await?;
            let value = serde_json::to_value(result).map_err(|err| {
                FunctionCallError::Fatal(format!("failed to serialize workspace context: {err}"))
            })?;
            Ok(boxed_tool_output(JsonToolOutput::new(value)))
        })
    }
}

impl CoreToolRuntime for WorkspaceContextReadHandler {}

async fn read_workspace_context(
    state_db: crate::StateDbHandle,
    args: WorkspaceContextReadArgs,
) -> Result<WorkspaceContextReadResult, FunctionCallError> {
    let run_id = args.run_id.trim();
    if run_id.is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "run_id must not be empty".to_string(),
        ));
    }

    let context = state_db
        .workspace()
        .read_authorized_agent_context(codex_state::WorkspaceAgentContextReadRequest {
            run_id: run_id.to_string(),
            category: args.category.as_str().to_string(),
            max_records: args.limit,
        })
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
                expected_context_envelope_sha256: packet.context_envelope_sha256,
                run_kind: "agent".to_string(),
                idempotency_key: "core-tool-context-test".to_string(),
                provider: "test-provider".to_string(),
                model: "test-model".to_string(),
                actor: "Clinician Example".to_string(),
                ..Default::default()
            })
            .await
            .expect("run should start");
        (client, note, encounter, run)
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

    #[tokio::test]
    async fn workspace_context_read_returns_authorized_hashed_sources() {
        let (_temp, state_db) = test_state_db().await;
        let (client, note, encounter, run) =
            seed_authorized_run(&state_db, &["visit_history", "progress_notes"]).await;

        let result = read_workspace_context(
            state_db.clone(),
            WorkspaceContextReadArgs {
                run_id: run.id.clone(),
                category: WorkspaceContextCategory::VisitHistory,
                limit: Some(4),
            },
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
        let (_client, _note, _encounter, run) =
            seed_authorized_run(&state_db, &["visit_history"]).await;

        let missing = read_workspace_context(
            state_db.clone(),
            WorkspaceContextReadArgs {
                run_id: "missing-run".to_string(),
                category: WorkspaceContextCategory::VisitHistory,
                limit: None,
            },
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
        let result = read_workspace_context(
            state_db,
            WorkspaceContextReadArgs {
                run_id: "  ".to_string(),
                category: WorkspaceContextCategory::VisitHistory,
                limit: None,
            },
        )
        .await;

        let Err(FunctionCallError::RespondToModel(message)) = result else {
            panic!("expected empty run_id to be rejected");
        };
        assert_eq!(message, "run_id must not be empty");
    }
}
