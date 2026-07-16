use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use super::epoch_millis_to_datetime;

/// Optional decision-complete artifact published by a planning turn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePlanArtifact {
    pub plan_markdown: String,
    pub decisions_json: String,
    pub open_questions_json: String,
}

/// Atomic terminal write for one capability-bound workspace planning turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePlanTurnComplete {
    pub execution: crate::WorkspacePlanningGuideExecutionBinding,
    pub assistant_message_role: crate::WorkspacePlanMessageRole,
    pub assistant_message: String,
    pub plan: Option<WorkspacePlanArtifact>,
    /// Ordered immutable context-read ids used by the answer or published plan.
    pub evidence_read_ids: Vec<String>,
    pub idempotency_key: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePlanEvidenceRead {
    pub ordinal: u32,
    pub context_read_id: String,
    pub category: String,
    pub response_sha256: String,
    pub source_content_sha256: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePlanTurnCompletionReceipt {
    pub guide_run_id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub idempotency_key: String,
    pub assistant_message_id: String,
    pub plan_revision_id: Option<String>,
    pub completion_input_sha256: String,
    pub evidence_manifest_sha256: String,
    pub evidence_read_count: u32,
    pub terminal_envelope_sha256: String,
    pub source_checkpoint_id: String,
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub source_thread_id: String,
    pub source_turn_id: String,
    pub provider: String,
    pub model: String,
    pub prompt_sha256: String,
    pub completed_at: DateTime<Utc>,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePlanTurnCompletion {
    pub run: crate::WorkspaceGuideRun,
    pub assistant_message: crate::WorkspacePlanMessage,
    pub revision: Option<crate::WorkspacePlanRevision>,
    pub evidence_manifest: Vec<WorkspacePlanEvidenceRead>,
    pub evidence_manifest_sha256: String,
    pub receipt: WorkspacePlanTurnCompletionReceipt,
    pub replayed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePlanActiveRunFilter {
    pub plan_session_id: String,
    pub client_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePlanActiveRun {
    pub run: crate::WorkspaceGuideRun,
    pub plan_session_id: String,
    pub source_thread_id: String,
    pub source_turn_id: String,
    pub provider: String,
    pub model: String,
    pub prompt_sha256: String,
    pub context_read_count: u32,
    pub claimed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePlanPendingQuestionFilter {
    pub plan_session_id: String,
    pub client_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePlanRecoveryRequest {
    pub plan_session_id: String,
    pub client_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePlanRecoveryState {
    pub session: crate::WorkspacePlanSession,
    pub active_runs: Vec<WorkspacePlanActiveRun>,
    pub pending_questions: Vec<crate::WorkspacePlanMessage>,
    pub current_revision: Option<crate::WorkspacePlanRevision>,
    pub last_completion: Option<WorkspacePlanTurnCompletionReceipt>,
}

#[derive(sqlx::FromRow)]
pub(crate) struct WorkspacePlanTurnCompletionRow {
    pub guide_run_id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub idempotency_key: String,
    pub assistant_message_id: String,
    pub plan_revision_id: Option<String>,
    pub completion_input_sha256: String,
    pub evidence_manifest_json: String,
    pub evidence_manifest_sha256: String,
    pub evidence_read_count: i64,
    pub terminal_envelope_json: String,
    pub terminal_envelope_sha256: String,
    pub source_checkpoint_id: String,
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub source_thread_id: String,
    pub source_turn_id: String,
    pub provider: String,
    pub model: String,
    pub prompt_sha256: String,
    pub execution_token_sha256: String,
    pub completed_at_ms: i64,
}

impl WorkspacePlanTurnCompletionRow {
    pub(crate) fn evidence_manifest(&self) -> anyhow::Result<Vec<WorkspacePlanEvidenceRead>> {
        Ok(serde_json::from_str(&self.evidence_manifest_json)?)
    }

    pub(crate) fn try_into_receipt(
        &self,
        replayed: bool,
    ) -> anyhow::Result<WorkspacePlanTurnCompletionReceipt> {
        Ok(WorkspacePlanTurnCompletionReceipt {
            guide_run_id: self.guide_run_id.clone(),
            plan_session_id: self.plan_session_id.clone(),
            client_id: self.client_id.clone(),
            idempotency_key: self.idempotency_key.clone(),
            assistant_message_id: self.assistant_message_id.clone(),
            plan_revision_id: self.plan_revision_id.clone(),
            completion_input_sha256: self.completion_input_sha256.clone(),
            evidence_manifest_sha256: self.evidence_manifest_sha256.clone(),
            evidence_read_count: u32::try_from(self.evidence_read_count)?,
            terminal_envelope_sha256: self.terminal_envelope_sha256.clone(),
            source_checkpoint_id: self.source_checkpoint_id.clone(),
            source_checkpoint_revision: self.source_checkpoint_revision,
            source_checkpoint_sha256: self.source_checkpoint_sha256.clone(),
            source_thread_id: self.source_thread_id.clone(),
            source_turn_id: self.source_turn_id.clone(),
            provider: self.provider.clone(),
            model: self.model.clone(),
            prompt_sha256: self.prompt_sha256.clone(),
            completed_at: epoch_millis_to_datetime(self.completed_at_ms)?,
            replayed,
        })
    }
}
