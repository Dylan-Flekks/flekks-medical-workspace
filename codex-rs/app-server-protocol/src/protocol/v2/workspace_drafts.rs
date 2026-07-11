use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspaceDraftSessionStatus {
    Active,
    Closed,
    Discarded,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspaceDraftSessionCloseStatus {
    Closed,
    Discarded,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDraftCheckpoint {
    pub id: String,
    pub session_id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    #[ts(type = "number | null")]
    pub base_note_revision: Option<i64>,
    #[ts(type = "number")]
    pub schema_version: i64,
    #[ts(type = "number")]
    pub revision: i64,
    pub draft: JsonValue,
    pub content_sha256: String,
    pub trigger: String,
    pub actor: String,
    #[ts(type = "number")]
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDraftSession {
    pub id: String,
    pub client_id: String,
    pub status: WorkspaceDraftSessionStatus,
    #[ts(type = "number")]
    pub current_revision: i64,
    pub current_checkpoint: WorkspaceDraftCheckpoint,
    pub created_by: String,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
    #[ts(type = "number | null")]
    pub closed_at: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDraftCheckpointCreateParams {
    #[ts(optional = nullable)]
    pub session_id: Option<String>,
    #[ts(optional = nullable)]
    pub session_creation_key: Option<String>,
    pub client_id: String,
    #[ts(optional = nullable)]
    pub encounter_id: Option<String>,
    #[ts(optional = nullable)]
    pub note_id: Option<String>,
    #[ts(type = "number | null")]
    #[ts(optional = nullable)]
    pub base_note_revision: Option<i64>,
    pub draft: JsonValue,
    pub trigger: String,
    pub actor: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDraftCheckpointCreateResponse {
    pub checkpoint: WorkspaceDraftCheckpoint,
    pub replayed: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDraftCheckpointListParams {
    pub client_id: String,
    pub session_id: String,
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDraftCheckpointListResponse {
    pub data: Vec<WorkspaceDraftCheckpoint>,
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDraftSessionListParams {
    #[ts(optional = nullable)]
    pub client_id: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub all_clients: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub include_closed: bool,
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDraftSessionListResponse {
    pub data: Vec<WorkspaceDraftSession>,
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDraftSessionCloseParams {
    pub session_id: String,
    pub client_id: String,
    pub status: WorkspaceDraftSessionCloseStatus,
    #[ts(optional = nullable)]
    pub expected_current_checkpoint_id: Option<String>,
    #[ts(type = "number | null")]
    #[ts(optional = nullable)]
    pub expected_current_checkpoint_revision: Option<i64>,
    #[ts(optional = nullable)]
    pub expected_current_checkpoint_sha256: Option<String>,
    pub actor: String,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceDraftSessionCloseResponse {
    pub session: WorkspaceDraftSession,
}

#[cfg(test)]
#[path = "workspace_drafts_tests.rs"]
mod tests;
