use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;

use super::epoch_millis_to_datetime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDraftSession {
    pub id: String,
    pub client_id: String,
    pub status: String,
    pub current_revision: i64,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDraftCheckpoint {
    pub id: String,
    pub session_id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub base_note_revision: Option<i64>,
    pub schema_version: i64,
    pub revision: i64,
    pub draft_json: String,
    pub content_sha256: String,
    pub trigger: String,
    pub actor: String,
    pub created_at: DateTime<Utc>,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDraftSessionSnapshot {
    pub session: WorkspaceDraftSession,
    pub current_checkpoint: WorkspaceDraftCheckpoint,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceDraftCheckpointCreate {
    pub session_id: Option<String>,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub base_note_revision: Option<i64>,
    pub draft_json: String,
    pub trigger: String,
    pub actor: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceDraftCheckpointFilter {
    pub client_id: String,
    pub session_id: Option<String>,
    pub cursor_before_revision: Option<i64>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceDraftSessionFilter {
    pub client_id: String,
    pub include_closed: bool,
    pub cursor_updated_at_ms: Option<i64>,
    pub cursor_id: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceDraftError {
    Validation { message: String },
    Storage { message: String },
}

impl std::fmt::Display for WorkspaceDraftError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation { message } | Self::Storage { message } => {
                formatter.write_str(message)
            }
        }
    }
}

impl std::error::Error for WorkspaceDraftError {}

impl From<anyhow::Error> for WorkspaceDraftError {
    fn from(error: anyhow::Error) -> Self {
        Self::Storage {
            message: error.to_string(),
        }
    }
}

impl From<sqlx::Error> for WorkspaceDraftError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage {
            message: error.to_string(),
        }
    }
}

impl From<serde_json::Error> for WorkspaceDraftError {
    fn from(error: serde_json::Error) -> Self {
        Self::Storage {
            message: error.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceDraftSessionTerminalStatus {
    Closed,
    Discarded,
}

impl WorkspaceDraftSessionTerminalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Discarded => "discarded",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDraftSessionClose {
    pub session_id: String,
    pub client_id: String,
    pub status: WorkspaceDraftSessionTerminalStatus,
    pub expected_current_checkpoint_id: Option<String>,
    pub expected_current_checkpoint_revision: Option<i64>,
    pub expected_current_checkpoint_sha256: Option<String>,
    pub actor: String,
    pub reason: String,
}

#[derive(sqlx::FromRow)]
pub(crate) struct WorkspaceDraftSessionRow {
    pub id: String,
    pub client_id: String,
    pub status: String,
    pub current_revision: i64,
    pub created_by: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub closed_at_ms: Option<i64>,
}

impl TryFrom<WorkspaceDraftSessionRow> for WorkspaceDraftSession {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceDraftSessionRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            client_id: row.client_id,
            status: row.status,
            current_revision: row.current_revision,
            created_by: row.created_by,
            created_at: epoch_millis_to_datetime(row.created_at_ms)?,
            updated_at: epoch_millis_to_datetime(row.updated_at_ms)?,
            closed_at: row.closed_at_ms.map(epoch_millis_to_datetime).transpose()?,
        })
    }
}

#[derive(sqlx::FromRow)]
pub(crate) struct WorkspaceDraftCheckpointRow {
    pub id: String,
    pub session_id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub base_note_revision: Option<i64>,
    pub schema_version: i64,
    pub revision: i64,
    pub draft_json: String,
    pub content_sha256: String,
    pub trigger: String,
    pub actor: String,
    pub created_at_ms: i64,
}

#[derive(sqlx::FromRow)]
pub(crate) struct WorkspaceDraftSessionSnapshotRow {
    pub session_id: String,
    pub session_client_id: String,
    pub session_status: String,
    pub session_current_revision: i64,
    pub session_created_by: String,
    pub session_created_at_ms: i64,
    pub session_updated_at_ms: i64,
    pub session_closed_at_ms: Option<i64>,
    pub checkpoint_id: String,
    pub checkpoint_session_id: String,
    pub checkpoint_client_id: String,
    pub checkpoint_encounter_id: Option<String>,
    pub checkpoint_note_id: Option<String>,
    pub checkpoint_base_note_revision: Option<i64>,
    pub checkpoint_schema_version: i64,
    pub checkpoint_revision: i64,
    pub checkpoint_draft_json: String,
    pub checkpoint_content_sha256: String,
    pub checkpoint_trigger: String,
    pub checkpoint_actor: String,
    pub checkpoint_created_at_ms: i64,
}

impl WorkspaceDraftCheckpointRow {
    pub(crate) fn try_into_model(self, replayed: bool) -> Result<WorkspaceDraftCheckpoint> {
        Ok(WorkspaceDraftCheckpoint {
            id: self.id,
            session_id: self.session_id,
            client_id: self.client_id,
            encounter_id: self.encounter_id,
            note_id: self.note_id,
            base_note_revision: self.base_note_revision,
            schema_version: self.schema_version,
            revision: self.revision,
            draft_json: self.draft_json,
            content_sha256: self.content_sha256,
            trigger: self.trigger,
            actor: self.actor,
            created_at: epoch_millis_to_datetime(self.created_at_ms)?,
            replayed,
        })
    }
}

impl TryFrom<WorkspaceDraftSessionSnapshotRow> for WorkspaceDraftSessionSnapshot {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceDraftSessionSnapshotRow) -> Result<Self> {
        let session = WorkspaceDraftSessionRow {
            id: row.session_id,
            client_id: row.session_client_id,
            status: row.session_status,
            current_revision: row.session_current_revision,
            created_by: row.session_created_by,
            created_at_ms: row.session_created_at_ms,
            updated_at_ms: row.session_updated_at_ms,
            closed_at_ms: row.session_closed_at_ms,
        }
        .try_into()?;
        let current_checkpoint = WorkspaceDraftCheckpointRow {
            id: row.checkpoint_id,
            session_id: row.checkpoint_session_id,
            client_id: row.checkpoint_client_id,
            encounter_id: row.checkpoint_encounter_id,
            note_id: row.checkpoint_note_id,
            base_note_revision: row.checkpoint_base_note_revision,
            schema_version: row.checkpoint_schema_version,
            revision: row.checkpoint_revision,
            draft_json: row.checkpoint_draft_json,
            content_sha256: row.checkpoint_content_sha256,
            trigger: row.checkpoint_trigger,
            actor: row.checkpoint_actor,
            created_at_ms: row.checkpoint_created_at_ms,
        }
        .try_into_model(false)?;
        Ok(Self {
            session,
            current_checkpoint,
        })
    }
}
