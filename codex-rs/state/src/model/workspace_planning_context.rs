use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use super::epoch_millis_to_datetime;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePlanningGuideTurnClaimRequest {
    pub guide_run_id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub source_checkpoint_id: String,
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub source_thread_id: String,
    pub source_turn_id: String,
    pub provider: String,
    pub model: String,
    pub prompt: String,
}

/// Exact, capability-bearing identity for one patient planning model turn.
///
/// Callers must retain the opaque execution token in memory and present the complete binding for
/// every context read. Only the token digest is persisted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePlanningGuideExecutionBinding {
    pub guide_run_id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub source_checkpoint_id: String,
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub source_thread_id: String,
    pub source_turn_id: String,
    pub provider: String,
    pub model: String,
    pub prompt_sha256: String,
    pub execution_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePlanningContextReadRequest {
    pub execution: WorkspacePlanningGuideExecutionBinding,
    pub category: String,
    pub max_records: Option<u32>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspacePlanningContextSource {
    pub source_entity_type: String,
    pub source_entity_id: String,
    pub source_revision: Option<i64>,
    pub display_label: String,
    pub snapshot_json: String,
    pub content_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePlanningContextRead {
    pub id: String,
    pub guide_run_id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub category: String,
    pub max_records: u32,
    pub response_sha256: String,
    pub sources: Vec<WorkspacePlanningContextSource>,
    pub accessed_at: DateTime<Utc>,
    pub replayed: bool,
}

#[derive(sqlx::FromRow)]
pub(crate) struct WorkspacePlanningTurnClaimRow {
    pub guide_run_id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub source_checkpoint_id: String,
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub source_thread_id: String,
    pub source_turn_id: String,
    pub provider: String,
    pub model: String,
    pub prompt_sha256: String,
    pub execution_token_sha256: String,
}

#[derive(sqlx::FromRow)]
pub(crate) struct WorkspacePlanningContextReadRow {
    pub id: String,
    pub guide_run_id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub category: String,
    pub max_records: i64,
    pub response_json: String,
    pub response_sha256: String,
    pub accessed_at_ms: i64,
}

impl WorkspacePlanningContextReadRow {
    pub(crate) fn try_into_model(
        self,
        replayed: bool,
    ) -> anyhow::Result<WorkspacePlanningContextRead> {
        Ok(WorkspacePlanningContextRead {
            id: self.id,
            guide_run_id: self.guide_run_id,
            plan_session_id: self.plan_session_id,
            client_id: self.client_id,
            category: self.category,
            max_records: u32::try_from(self.max_records)?,
            response_sha256: self.response_sha256,
            sources: serde_json::from_str(&self.response_json)?,
            accessed_at: epoch_millis_to_datetime(self.accessed_at_ms)?,
            replayed,
        })
    }
}
