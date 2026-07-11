use chrono::DateTime;
use chrono::Utc;

use super::epoch_millis_to_datetime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceGuideRunStatus {
    Running,
    Completed,
    Failed,
    Canceled,
}

impl WorkspaceGuideRunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
        }
    }

    fn from_stored(value: &str) -> anyhow::Result<Self> {
        match value {
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "canceled" => Ok(Self::Canceled),
            other => anyhow::bail!("unknown stored workspace guide run status `{other}`"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceGuideRun {
    pub id: String,
    pub client_id: String,
    pub session_id: String,
    pub source_checkpoint_id: String,
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub request_schema_version: i64,
    pub request_envelope_json: String,
    pub request_envelope_sha256: String,
    pub idempotency_key: String,
    pub trigger: String,
    pub actor: String,
    pub provider: String,
    pub model: String,
    pub status: WorkspaceGuideRunStatus,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub terminal_envelope_json: Option<String>,
    pub terminal_envelope_sha256: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub terminal_at: Option<DateTime<Utc>>,
    pub is_stale: bool,
    pub replayed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceGuideRunStart {
    pub client_id: String,
    pub session_id: String,
    pub source_checkpoint_id: String,
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub request_json: String,
    pub idempotency_key: String,
    pub trigger: String,
    pub actor: String,
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceGuideRunOutcome {
    Completed { result_json: String },
    Failed { error_summary: String },
    Canceled { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceGuideRunFinish {
    pub run_id: String,
    pub client_id: String,
    pub session_id: String,
    pub source_checkpoint_id: String,
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub request_envelope_sha256: String,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub outcome: WorkspaceGuideRunOutcome,
    pub actor: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceGuideRunFilter {
    pub client_id: String,
    pub session_id: Option<String>,
    pub limit: Option<u32>,
}

#[derive(sqlx::FromRow)]
pub(crate) struct WorkspaceGuideRunRow {
    pub id: String,
    pub client_id: String,
    pub session_id: String,
    pub source_checkpoint_id: String,
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub request_schema_version: i64,
    pub request_envelope_json: String,
    pub request_envelope_sha256: String,
    pub idempotency_key: String,
    pub trigger: String,
    pub actor: String,
    pub provider: String,
    pub model: String,
    pub status: String,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub terminal_envelope_json: Option<String>,
    pub terminal_envelope_sha256: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub terminal_at_ms: Option<i64>,
    pub is_stale: i64,
}

impl WorkspaceGuideRunRow {
    pub(crate) fn try_into_model(self, replayed: bool) -> anyhow::Result<WorkspaceGuideRun> {
        Ok(WorkspaceGuideRun {
            id: self.id,
            client_id: self.client_id,
            session_id: self.session_id,
            source_checkpoint_id: self.source_checkpoint_id,
            source_checkpoint_revision: self.source_checkpoint_revision,
            source_checkpoint_sha256: self.source_checkpoint_sha256,
            request_schema_version: self.request_schema_version,
            request_envelope_json: self.request_envelope_json,
            request_envelope_sha256: self.request_envelope_sha256,
            idempotency_key: self.idempotency_key,
            trigger: self.trigger,
            actor: self.actor,
            provider: self.provider,
            model: self.model,
            status: WorkspaceGuideRunStatus::from_stored(&self.status)?,
            source_thread_id: self.source_thread_id,
            source_turn_id: self.source_turn_id,
            terminal_envelope_json: self.terminal_envelope_json,
            terminal_envelope_sha256: self.terminal_envelope_sha256,
            created_at: epoch_millis_to_datetime(self.created_at_ms)?,
            updated_at: epoch_millis_to_datetime(self.updated_at_ms)?,
            terminal_at: self
                .terminal_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
            is_stale: self.is_stale != 0,
            replayed,
        })
    }
}
