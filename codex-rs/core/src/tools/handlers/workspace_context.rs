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

#[derive(Debug, Deserialize)]
struct WorkspaceContextReadArgs {
    client_id: String,
    note_id: Option<String>,
    include_documents: Option<bool>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct WorkspaceContextReadResult {
    context: Option<WorkspaceContextReadContext>,
    warnings: Vec<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct WorkspaceContextReadContext {
    client: WorkspaceContextClient,
    active_note: Option<WorkspaceContextNote>,
    recent_notes: Vec<WorkspaceContextNoteSummary>,
    documents: Vec<WorkspaceContextDocument>,
    tasks: Vec<WorkspaceContextTask>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct WorkspaceContextClient {
    id: String,
    display_name: String,
    preferred_name: Option<String>,
    date_of_birth: Option<String>,
    sex_or_gender: Option<String>,
    external_id: Option<String>,
    record_start_date: Option<String>,
    record_end_date: Option<String>,
    summary: String,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct WorkspaceContextNote {
    id: String,
    client_id: String,
    encounter_id: Option<String>,
    title: String,
    kind: String,
    body: String,
    status: String,
    current_revision: i64,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct WorkspaceContextNoteSummary {
    id: String,
    client_id: String,
    encounter_id: Option<String>,
    title: String,
    kind: String,
    status: String,
    current_revision: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct WorkspaceContextDocument {
    id: String,
    client_id: String,
    encounter_id: Option<String>,
    title: String,
    kind: String,
    local_path: String,
    notes: String,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct WorkspaceContextTask {
    id: String,
    client_id: String,
    encounter_id: Option<String>,
    note_id: Option<String>,
    document_id: Option<String>,
    title: String,
    kind: String,
    status: String,
    priority: String,
    due_date: Option<String>,
    assigned_to: Option<String>,
    updated_at: i64,
}

#[async_trait::async_trait]
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

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
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
    }
}

impl CoreToolRuntime for WorkspaceContextReadHandler {}

async fn read_workspace_context(
    state_db: crate::StateDbHandle,
    args: WorkspaceContextReadArgs,
) -> Result<WorkspaceContextReadResult, FunctionCallError> {
    let client_id = args.client_id.trim();
    if client_id.is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "client_id must not be empty".to_string(),
        ));
    }

    let workspace = state_db.workspace();
    let Some(client) = workspace.get_client(client_id).await.map_err(read_error)? else {
        return Ok(WorkspaceContextReadResult {
            context: None,
            warnings: vec![format!("workspace client `{client_id}` was not found")],
        });
    };

    let mut warnings = Vec::new();
    let active_note: Option<WorkspaceContextNote> = match args
        .note_id
        .as_deref()
        .map(str::trim)
        .filter(|note_id| !note_id.is_empty())
    {
        Some(note_id) => {
            let note = workspace.get_note(note_id).await.map_err(read_error)?;
            match note {
                Some(note) if note.client_id == client.id => Some(note.into()),
                Some(_) | None => {
                    warnings.push(format!(
                        "workspace note `{note_id}` was not found for client `{}`",
                        client.id
                    ));
                    None
                }
            }
        }
        None => None,
    };
    workspace
        .record_audit_event(codex_state::WorkspaceAuditEventCreate {
            entity_type: "client".to_string(),
            entity_id: client.id.clone(),
            action: "agent_read".to_string(),
            actor: "agent".to_string(),
            actor_kind: "agent".to_string(),
            source: "core-tool".to_string(),
            client_id: Some(client.id.clone()),
            encounter_id: active_note
                .as_ref()
                .and_then(|note| note.encounter_id.clone()),
            note_id: active_note.as_ref().map(|note| note.id.clone()),
            summary: "workspace_context_read".to_string(),
            metadata_json: Some(format!(
                "{{\"include_documents\":{}}}",
                args.include_documents.unwrap_or(true)
            )),
            ..Default::default()
        })
        .await
        .map_err(read_error)?;

    let recent_notes = workspace
        .list_notes(&client.id)
        .await
        .map_err(read_error)?
        .into_iter()
        .take(10)
        .map(WorkspaceContextNoteSummary::from)
        .collect();
    let documents = if args.include_documents.unwrap_or(true) {
        workspace
            .list_documents(&client.id)
            .await
            .map_err(read_error)?
            .into_iter()
            .map(WorkspaceContextDocument::from)
            .collect()
    } else {
        Vec::new()
    };
    let tasks = workspace
        .list_open_tasks(&client.id)
        .await
        .map_err(read_error)?
        .into_iter()
        .take(10)
        .map(WorkspaceContextTask::from)
        .collect();

    Ok(WorkspaceContextReadResult {
        context: Some(WorkspaceContextReadContext {
            client: client.into(),
            active_note,
            recent_notes,
            documents,
            tasks,
        }),
        warnings,
    })
}

fn read_error(err: anyhow::Error) -> FunctionCallError {
    FunctionCallError::RespondToModel(format!("failed to read workspace context: {err}"))
}

fn timestamp(value: DateTime<Utc>) -> i64 {
    value.timestamp()
}

impl From<codex_state::WorkspaceClient> for WorkspaceContextClient {
    fn from(value: codex_state::WorkspaceClient) -> Self {
        Self {
            id: value.id,
            display_name: value.display_name,
            preferred_name: value.preferred_name,
            date_of_birth: value.date_of_birth,
            sex_or_gender: value.sex_or_gender,
            external_id: value.external_id,
            record_start_date: value.record_start_date,
            record_end_date: value.record_end_date,
            summary: value.summary,
            created_at: timestamp(value.created_at),
            updated_at: timestamp(value.updated_at),
        }
    }
}

impl From<codex_state::WorkspaceNote> for WorkspaceContextNote {
    fn from(value: codex_state::WorkspaceNote) -> Self {
        Self {
            id: value.id,
            client_id: value.client_id,
            encounter_id: value.encounter_id,
            title: value.title,
            kind: value.kind,
            body: value.body,
            status: value.status,
            current_revision: value.current_revision,
            created_at: timestamp(value.created_at),
            updated_at: timestamp(value.updated_at),
        }
    }
}

impl From<codex_state::WorkspaceNote> for WorkspaceContextNoteSummary {
    fn from(value: codex_state::WorkspaceNote) -> Self {
        Self {
            id: value.id,
            client_id: value.client_id,
            encounter_id: value.encounter_id,
            title: value.title,
            kind: value.kind,
            status: value.status,
            current_revision: value.current_revision,
            updated_at: timestamp(value.updated_at),
        }
    }
}

impl From<codex_state::WorkspaceDocument> for WorkspaceContextDocument {
    fn from(value: codex_state::WorkspaceDocument) -> Self {
        Self {
            id: value.id,
            client_id: value.client_id,
            encounter_id: value.encounter_id,
            title: value.title,
            kind: value.kind,
            local_path: value.local_path,
            notes: value.notes,
            created_at: timestamp(value.created_at),
            updated_at: timestamp(value.updated_at),
        }
    }
}

impl From<codex_state::WorkspaceTask> for WorkspaceContextTask {
    fn from(value: codex_state::WorkspaceTask) -> Self {
        Self {
            id: value.id,
            client_id: value.client_id,
            encounter_id: value.encounter_id,
            note_id: value.note_id,
            document_id: value.document_id,
            title: value.title,
            kind: value.kind,
            status: value.status.as_str().to_string(),
            priority: value.priority.as_str().to_string(),
            due_date: value.due_date,
            assigned_to: value.assigned_to,
            updated_at: timestamp(value.updated_at),
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
        (temp, state_db)
    }

    async fn seed_workspace(
        state_db: &crate::StateDbHandle,
    ) -> (
        codex_state::WorkspaceClient,
        codex_state::WorkspaceNote,
        codex_state::WorkspaceDocument,
        codex_state::WorkspaceTask,
    ) {
        let client = state_db
            .workspace()
            .upsert_client(codex_state::WorkspaceClientUpsert {
                display_name: "Jordan Patient".to_string(),
                preferred_name: Some("Jordan".to_string()),
                date_of_birth: Some("1980-01-01".to_string()),
                sex_or_gender: Some("X".to_string()),
                external_id: Some("MRN-123".to_string()),
                record_start_date: Some("2026-01-01".to_string()),
                record_end_date: Some("2026-06-09".to_string()),
                summary: "Example workspace summary.".to_string(),
                ..Default::default()
            })
            .await
            .expect("client should save");
        let note = state_db
            .workspace()
            .upsert_note(codex_state::WorkspaceNoteUpsert {
                client_id: client.id.clone(),
                title: "Initial visit".to_string(),
                kind: "note".to_string(),
                body: "Human-entered note body.".to_string(),
                status: "draft".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("note should save");
        let document = state_db
            .workspace()
            .upsert_document(codex_state::WorkspaceDocumentUpsert {
                client_id: client.id.clone(),
                title: "Outside referral PDF".to_string(),
                kind: "referral".to_string(),
                local_path: "/tmp/outside-referral.pdf".to_string(),
                notes: "Metadata only.".to_string(),
                ..Default::default()
            })
            .await
            .expect("document should save");
        let task = state_db
            .workspace()
            .upsert_task(codex_state::WorkspaceTaskUpsert {
                client_id: client.id.clone(),
                note_id: Some(note.id.clone()),
                document_id: Some(document.id.clone()),
                title: "Request outside records".to_string(),
                details: "Call referring office.".to_string(),
                kind: "follow-up".to_string(),
                status: codex_state::WorkspaceTaskStatus::Open,
                priority: codex_state::WorkspaceTaskPriority::High,
                due_date: Some("2026-06-12".to_string()),
                assigned_to: Some("local staff".to_string()),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("task should save");
        (client, note, document, task)
    }

    #[tokio::test]
    async fn workspace_context_read_returns_selected_context() {
        let (_temp, state_db) = test_state_db().await;
        let (client, note, document, task) = seed_workspace(&state_db).await;

        let result = read_workspace_context(
            state_db.clone(),
            WorkspaceContextReadArgs {
                client_id: client.id.clone(),
                note_id: Some(note.id.clone()),
                include_documents: Some(true),
            },
        )
        .await
        .expect("context read should succeed");

        assert_eq!(
            result,
            WorkspaceContextReadResult {
                context: Some(WorkspaceContextReadContext {
                    client: client.clone().into(),
                    active_note: Some(note.clone().into()),
                    recent_notes: vec![note.clone().into()],
                    documents: vec![document.into()],
                    tasks: vec![task.into()],
                }),
                warnings: Vec::new(),
            }
        );

        let saved_note = state_db
            .workspace()
            .get_note(&note.id)
            .await
            .expect("saved note should be readable")
            .expect("saved note should still exist");
        assert_eq!(saved_note, note);
        let audit = state_db
            .workspace()
            .list_audit_events_filtered(codex_state::WorkspaceAuditEventFilter {
                client_id: Some(client.id.clone()),
                note_id: Some(note.id.clone()),
                ..Default::default()
            })
            .await
            .expect("audit should be readable");
        assert!(
            audit
                .iter()
                .any(|event| event.action == "agent_read" && event.actor_kind == "agent")
        );
    }

    #[tokio::test]
    async fn workspace_context_read_handles_missing_and_cross_client_ids() {
        let (_temp, state_db) = test_state_db().await;
        let (_client, cross_client_note, _document, _task) = seed_workspace(&state_db).await;
        let other_client = state_db
            .workspace()
            .upsert_client(codex_state::WorkspaceClientUpsert {
                display_name: "Other Client".to_string(),
                ..Default::default()
            })
            .await
            .expect("other client should save");

        let missing = read_workspace_context(
            state_db.clone(),
            WorkspaceContextReadArgs {
                client_id: "missing-client".to_string(),
                note_id: None,
                include_documents: Some(true),
            },
        )
        .await
        .expect("missing client should be a successful empty read");
        assert_eq!(
            missing,
            WorkspaceContextReadResult {
                context: None,
                warnings: vec!["workspace client `missing-client` was not found".to_string()],
            }
        );

        let cross_client = read_workspace_context(
            state_db,
            WorkspaceContextReadArgs {
                client_id: other_client.id.clone(),
                note_id: Some(cross_client_note.id.clone()),
                include_documents: Some(false),
            },
        )
        .await
        .expect("cross-client note should be safely ignored");
        assert_eq!(
            cross_client,
            WorkspaceContextReadResult {
                context: Some(WorkspaceContextReadContext {
                    client: other_client.clone().into(),
                    active_note: None,
                    recent_notes: Vec::new(),
                    documents: Vec::new(),
                    tasks: Vec::new(),
                }),
                warnings: vec![format!(
                    "workspace note `{}` was not found for client `{}`",
                    cross_client_note.id, other_client.id
                )],
            }
        );
    }

    #[tokio::test]
    async fn workspace_context_read_rejects_empty_client_id() {
        let (_temp, state_db) = test_state_db().await;
        let result = read_workspace_context(
            state_db,
            WorkspaceContextReadArgs {
                client_id: "  ".to_string(),
                note_id: None,
                include_documents: Some(true),
            },
        )
        .await;

        let Err(FunctionCallError::RespondToModel(message)) = result else {
            panic!("expected empty client_id to be rejected");
        };
        assert_eq!(message, "client_id must not be empty");
    }
}
