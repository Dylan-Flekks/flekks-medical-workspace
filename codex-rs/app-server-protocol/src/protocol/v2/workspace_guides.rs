use codex_protocol::config_types::ModelToolMode;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspaceGuideRunStatus {
    Running,
    Completed,
    Failed,
    Canceled,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceGuideRun {
    pub id: String,
    pub client_id: String,
    pub session_id: String,
    pub source_checkpoint_id: String,
    #[ts(type = "number")]
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    #[ts(type = "number")]
    pub request_schema_version: i64,
    pub request_envelope: JsonValue,
    pub request_envelope_sha256: String,
    pub idempotency_key: String,
    pub trigger: String,
    pub actor: String,
    pub provider: String,
    pub model: String,
    pub model_tool_mode: ModelToolMode,
    pub status: WorkspaceGuideRunStatus,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub terminal_envelope: Option<JsonValue>,
    pub terminal_envelope_sha256: Option<String>,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
    #[ts(type = "number | null")]
    pub terminal_at: Option<i64>,
    pub is_stale: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceGuideRunStartParams {
    pub client_id: String,
    pub session_id: String,
    pub source_checkpoint_id: String,
    #[ts(type = "number")]
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub request: JsonValue,
    pub idempotency_key: String,
    pub trigger: String,
    pub actor: String,
    pub provider: String,
    pub model: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceGuideRunStartResponse {
    pub run: WorkspaceGuideRun,
    pub replayed: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type", rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspaceGuideRunFinishOutcome {
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Completed { result: JsonValue },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Failed { error_summary: String },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Canceled { reason: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceGuideRunFinishParams {
    pub run_id: String,
    pub client_id: String,
    pub session_id: String,
    pub source_checkpoint_id: String,
    #[ts(type = "number")]
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub request_envelope_sha256: String,
    #[ts(optional = nullable)]
    pub source_thread_id: Option<String>,
    #[ts(optional = nullable)]
    pub source_turn_id: Option<String>,
    pub outcome: WorkspaceGuideRunFinishOutcome,
    pub actor: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceGuideRunFinishResponse {
    pub run: WorkspaceGuideRun,
    pub replayed: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceGuideRunListParams {
    pub client_id: String,
    #[ts(optional = nullable)]
    pub session_id: Option<String>,
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceGuideRunListResponse {
    pub data: Vec<WorkspaceGuideRun>,
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspaceGuideRunErrorData {
    Validation,
    StaleCheckpoint,
    IdempotencyConflict,
    ActiveRunConflict,
    TerminalConflict,
}

#[cfg(test)]
#[path = "workspace_guides_tests.rs"]
mod tests;
