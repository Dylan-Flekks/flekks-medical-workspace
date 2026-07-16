use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use super::WorkspaceArtifactDerivative;
use super::WorkspaceArtifactDerivativeUpsert;
use super::WorkspaceBillingReadiness;
use super::WorkspaceClient;
use super::WorkspaceClientUpsert;
use super::WorkspaceContextClip;
use super::WorkspaceContextClipUpsert;
use super::WorkspaceCoverage;
use super::WorkspaceCoverageUpsert;
use super::WorkspaceDocument;
use super::WorkspaceDocumentUpsert;
use super::WorkspaceEncounter;
use super::WorkspaceEncounterUpsert;
use super::WorkspaceNote;
use super::WorkspaceNoteUpsert;
use super::WorkspacePatientSafetyItem;
use super::WorkspacePatientSafetyItemUpsert;
use super::WorkspaceTask;
use super::WorkspaceTaskUpsert;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceChartNoteChange {
    pub upsert: WorkspaceNoteUpsert,
    pub expected_base_revision: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceChartCommitRequest {
    pub idempotency_key: String,
    pub actor: String,
    pub reason: String,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub client_id: Option<String>,
    pub client: Option<WorkspaceClientUpsert>,
    #[serde(default)]
    pub coverage: Option<WorkspaceCoverageUpsert>,
    pub expected_versions: WorkspaceChartExpectedVersions,
    pub safety_item: Option<WorkspacePatientSafetyItemUpsert>,
    pub encounter: Option<WorkspaceEncounterUpsert>,
    pub note: Option<WorkspaceChartNoteChange>,
    pub document: Option<WorkspaceDocumentUpsert>,
    pub artifact_derivative: Option<WorkspaceArtifactDerivativeUpsert>,
    pub context_clip: Option<WorkspaceContextClipUpsert>,
    pub task: Option<WorkspaceTaskUpsert>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceChartExpectedVersions {
    pub client: Option<String>,
    #[serde(default)]
    pub coverage: Option<String>,
    pub safety_item: Option<String>,
    pub encounter: Option<String>,
    pub document: Option<String>,
    pub artifact_derivative: Option<String>,
    pub context_clip: Option<String>,
    pub task: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

impl WorkspaceChartEntityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Coverage => "coverage",
            Self::SafetyItem => "safety_item",
            Self::Encounter => "encounter",
            Self::Note => "note",
            Self::Document => "document",
            Self::ArtifactDerivative => "artifact_derivative",
            Self::ContextClip => "context_clip",
            Self::Task => "task",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceChartCommitResult {
    pub commit_id: String,
    pub idempotency_key: String,
    pub replayed: bool,
    pub changed_entity_kinds: Vec<WorkspaceChartEntityKind>,
    pub client: WorkspaceClient,
    #[serde(default)]
    pub coverage: Option<WorkspaceCoverage>,
    #[serde(default)]
    pub coverage_billing_readiness: Option<WorkspaceBillingReadiness>,
    pub safety_item: Option<WorkspacePatientSafetyItem>,
    pub encounter: Option<WorkspaceEncounter>,
    pub note: Option<WorkspaceNote>,
    pub document: Option<WorkspaceDocument>,
    pub artifact_derivative: Option<WorkspaceArtifactDerivative>,
    pub context_clip: Option<WorkspaceContextClip>,
    pub task: Option<WorkspaceTask>,
    pub resulting_note_revision: Option<i64>,
    pub committed_at: DateTime<Utc>,
}

#[derive(Debug)]
pub enum WorkspaceChartCommitError {
    IdempotencyConflict {
        idempotency_key: String,
    },
    Validation {
        message: String,
    },
    StaleNoteRevision {
        note_id: String,
        expected: i64,
        actual: i64,
    },
    StaleEntityVersion {
        entity_kind: WorkspaceChartEntityKind,
        entity_id: String,
        expected: String,
        actual: String,
    },
    Storage {
        message: String,
    },
}

impl std::fmt::Display for WorkspaceChartCommitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IdempotencyConflict { idempotency_key } => write!(
                formatter,
                "workspace chart idempotency key `{idempotency_key}` was reused with different content"
            ),
            Self::Validation { message } => formatter.write_str(message),
            Self::StaleNoteRevision {
                note_id,
                expected,
                actual,
            } => write!(
                formatter,
                "workspace note `{note_id}` has revision {actual}, expected {expected}"
            ),
            Self::StaleEntityVersion {
                entity_kind,
                entity_id,
                expected,
                actual,
            } => write!(
                formatter,
                "workspace {} `{entity_id}` has version {actual}, expected {expected}",
                entity_kind.as_str()
            ),
            Self::Storage { message } => formatter.write_str(message),
        }
    }
}

impl std::error::Error for WorkspaceChartCommitError {}

impl From<anyhow::Error> for WorkspaceChartCommitError {
    fn from(error: anyhow::Error) -> Self {
        Self::Storage {
            message: error.to_string(),
        }
    }
}

impl From<sqlx::Error> for WorkspaceChartCommitError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage {
            message: error.to_string(),
        }
    }
}

impl From<serde_json::Error> for WorkspaceChartCommitError {
    fn from(error: serde_json::Error) -> Self {
        Self::Storage {
            message: error.to_string(),
        }
    }
}
