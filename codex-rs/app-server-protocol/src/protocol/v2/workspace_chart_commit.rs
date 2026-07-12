use super::workspace::WorkspaceArtifactDerivative;
use super::workspace::WorkspaceArtifactDerivativeUpsertParams;
use super::workspace::WorkspaceClient;
use super::workspace::WorkspaceClientUpsertParams;
use super::workspace::WorkspaceContextClip;
use super::workspace::WorkspaceContextClipUpsertParams;
use super::workspace_coverage::WorkspaceCoverage;
use super::workspace_coverage::WorkspaceCoverageUpsertParams;
use super::workspace::WorkspaceDocument;
use super::workspace::WorkspaceDocumentUpsertParams;
use super::workspace::WorkspaceEncounter;
use super::workspace::WorkspaceEncounterUpsertParams;
use super::workspace::WorkspaceNote;
use super::workspace::WorkspaceNoteUpsertParams;
use super::workspace::WorkspacePatientSafetyItem;
use super::workspace::WorkspacePatientSafetyItemUpsertParams;
use super::workspace::WorkspaceTask;
use super::workspace::WorkspaceTaskUpsertParams;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceChartNoteChange {
    pub upsert: WorkspaceNoteUpsertParams,
    #[ts(type = "number | null", optional = nullable)]
    #[serde(default)]
    pub expected_base_revision: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceChartExpectedVersions {
    #[ts(optional = nullable)]
    #[serde(default)]
    pub client: Option<String>,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub coverage: Option<String>,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub safety_item: Option<String>,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub encounter: Option<String>,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub document: Option<String>,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub artifact_derivative: Option<String>,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub context_clip: Option<String>,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub task: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceChartCommitParams {
    pub idempotency_key: String,
    pub actor: String,
    pub reason: String,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub source_thread_id: Option<String>,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub source_turn_id: Option<String>,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub client_id: Option<String>,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub client: Option<WorkspaceClientUpsertParams>,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub coverage: Option<WorkspaceCoverageUpsertParams>,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub expected_versions: Option<WorkspaceChartExpectedVersions>,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub safety_item: Option<WorkspacePatientSafetyItemUpsertParams>,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub encounter: Option<WorkspaceEncounterUpsertParams>,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub note: Option<WorkspaceChartNoteChange>,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub document: Option<WorkspaceDocumentUpsertParams>,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub artifact_derivative: Option<WorkspaceArtifactDerivativeUpsertParams>,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub context_clip: Option<WorkspaceContextClipUpsertParams>,
    #[ts(optional = nullable)]
    #[serde(default)]
    pub task: Option<WorkspaceTaskUpsertParams>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum WorkspaceChartEntityKind {
    Client,
    Coverage,
    SafetyItem,
    Encounter,
    Note,
    Document,
    ArtifactDerivative,
    ContextClip,
    Task,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(tag = "kind", export_to = "v2/")]
pub enum WorkspaceChartCommitErrorData {
    Validation,
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    StaleNoteRevision {
        note_id: String,
        #[ts(type = "number")]
        expected_revision: i64,
        #[ts(type = "number")]
        actual_revision: i64,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    StaleEntityVersion {
        entity_kind: WorkspaceChartEntityKind,
        entity_id: String,
        expected_version: String,
        actual_version: String,
    },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    IdempotencyConflict {
        idempotency_key: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct WorkspaceChartCommitResponse {
    pub commit_id: String,
    pub idempotency_key: String,
    pub replayed: bool,
    pub changed_entity_kinds: Vec<WorkspaceChartEntityKind>,
    pub client: WorkspaceClient,
    pub coverage: Option<WorkspaceCoverage>,
    pub safety_item: Option<WorkspacePatientSafetyItem>,
    pub encounter: Option<WorkspaceEncounter>,
    pub note: Option<WorkspaceNote>,
    pub document: Option<WorkspaceDocument>,
    pub artifact_derivative: Option<WorkspaceArtifactDerivative>,
    pub context_clip: Option<WorkspaceContextClip>,
    pub task: Option<WorkspaceTask>,
    #[ts(type = "number | null")]
    pub resulting_note_revision: Option<i64>,
    #[ts(type = "number")]
    pub committed_at: i64,
}

#[cfg(test)]
#[path = "workspace_chart_commit_tests.rs"]
mod tests;
