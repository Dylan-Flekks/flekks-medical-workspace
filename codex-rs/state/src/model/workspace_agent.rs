use anyhow::Result;
use anyhow::anyhow;
use chrono::DateTime;
use chrono::Utc;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use super::epoch_millis_to_datetime;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceContextPacketLifecycleUpdate {
    pub packet_id: String,
    pub client_id: String,
    pub expected_context_envelope_sha256: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceAgentRun {
    pub id: String,
    pub packet_id: String,
    pub client_id: String,
    pub note_id: Option<String>,
    pub base_note_revision: Option<i64>,
    pub context_envelope_sha256: String,
    pub workspace_plan_revision_id: Option<String>,
    pub workspace_plan_content_sha256: Option<String>,
    pub workspace_plan_evidence_manifest_sha256: Option<String>,
    pub run_kind: String,
    pub idempotency_key: String,
    pub provider: String,
    pub model: String,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub status: String,
    pub error_summary: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceAgentRunStart {
    pub packet_id: String,
    pub expected_client_id: String,
    pub expected_context_envelope_sha256: String,
    pub expected_workspace_plan_revision_id: Option<String>,
    pub expected_workspace_plan_content_sha256: Option<String>,
    pub expected_workspace_plan_evidence_manifest_sha256: Option<String>,
    pub run_kind: String,
    pub idempotency_key: String,
    pub provider: String,
    pub model: String,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub actor: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceAgentRunFilter {
    pub client_id: String,
    pub note_id: Option<String>,
    pub packet_id: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceAgentRunStatusUpdate {
    pub run_id: String,
    pub status: String,
    pub error_summary: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceAgentRunSource {
    pub id: String,
    pub run_id: String,
    pub source_entity_type: String,
    pub source_entity_id: String,
    pub source_revision: Option<i64>,
    pub display_label: String,
    pub snapshot_json: String,
    pub content_sha256: String,
    pub access_purpose: String,
    pub accessed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceAgentRunSourceCreate {
    pub run_id: String,
    pub source_entity_type: String,
    pub source_entity_id: String,
    pub source_revision: Option<i64>,
    pub display_label: String,
    pub snapshot_json: String,
    pub access_purpose: String,
}

/// A run-scoped request for additional chart context.
///
/// Patient and note identity are deliberately absent: the store derives both
/// from `run_id` so callers cannot widen an already-authorized packet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceAgentContextReadRequest {
    pub run_id: String,
    pub category: String,
    pub max_records: Option<u32>,
}

/// Immutable execution identity for one claimed medical agent turn.
///
/// This is checked again for every model-facing context read so a run id cannot be moved to a
/// different thread, turn, provider, or model after the claim succeeds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceAgentExecutionBinding {
    pub run_id: String,
    pub source_thread_id: String,
    pub source_turn_id: String,
    pub provider: String,
    pub model: String,
}

/// Exact generated prompt and execution identity claimed before a restricted turn may sample.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceAgentTurnClaim {
    pub execution: WorkspaceAgentExecutionBinding,
    pub prompt: String,
}

/// Exact state snapshots read for an authorized agent run.
///
/// Each source is an immutable access row containing the exact serialized
/// record and its SHA-256 digest. The identity fields are derived from the run,
/// never supplied by the reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceAgentContextRead {
    pub run_id: String,
    pub packet_id: String,
    pub client_id: String,
    pub note_id: Option<String>,
    pub category: String,
    pub max_records: u32,
    pub sources: Vec<WorkspaceAgentRunSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceNoteProposalDecisionKind {
    AcceptedAll,
    AcceptedEdited,
    RejectedAll,
    CopiedChange,
    RejectedChange,
}

impl WorkspaceNoteProposalDecisionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AcceptedAll => "accepted_all",
            Self::AcceptedEdited => "accepted_edited",
            Self::RejectedAll => "rejected_all",
            Self::CopiedChange => "copied_change",
            Self::RejectedChange => "rejected_change",
        }
    }
}

impl TryFrom<&str> for WorkspaceNoteProposalDecisionKind {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "accepted_all" => Ok(Self::AcceptedAll),
            "accepted_edited" => Ok(Self::AcceptedEdited),
            "rejected_all" => Ok(Self::RejectedAll),
            "copied_change" => Ok(Self::CopiedChange),
            "rejected_change" => Ok(Self::RejectedChange),
            other => Err(anyhow!(
                "unknown workspace note proposal decision kind `{other}`"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceNoteProposalDecision {
    pub id: String,
    pub proposal_id: String,
    pub agent_result_id: Option<String>,
    pub note_id: String,
    pub base_revision: i64,
    pub decision_kind: WorkspaceNoteProposalDecisionKind,
    pub change_id: Option<String>,
    pub applied_text: Option<String>,
    pub applied_text_sha256: Option<String>,
    pub resulting_note_revision: Option<i64>,
    pub actor: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceNoteProposalResolution {
    Accept,
    AcceptEdited { body: String },
    Decline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceNoteProposalResolve {
    pub proposal_id: String,
    pub resolution: WorkspaceNoteProposalResolution,
    pub actor: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceNoteProposalChangeDecisionCreate {
    pub proposal_id: String,
    pub decision_kind: WorkspaceNoteProposalDecisionKind,
    pub change_id: String,
    pub applied_text: Option<String>,
    pub actor: String,
    pub reason: String,
}

pub(crate) struct WorkspaceAgentRunRow {
    pub id: String,
    pub packet_id: String,
    pub client_id: String,
    pub note_id: Option<String>,
    pub base_note_revision: Option<i64>,
    pub context_envelope_sha256: String,
    pub workspace_plan_revision_id: Option<String>,
    pub workspace_plan_content_sha256: Option<String>,
    pub workspace_plan_evidence_manifest_sha256: Option<String>,
    pub run_kind: String,
    pub idempotency_key: String,
    pub provider: String,
    pub model: String,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub status: String,
    pub error_summary: String,
    pub started_at_ms: i64,
    pub completed_at_ms: Option<i64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl WorkspaceAgentRunRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            packet_id: row.try_get("packet_id")?,
            client_id: row.try_get("client_id")?,
            note_id: row.try_get("note_id")?,
            base_note_revision: row.try_get("base_note_revision")?,
            context_envelope_sha256: row.try_get("context_envelope_sha256")?,
            workspace_plan_revision_id: row.try_get("workspace_plan_revision_id")?,
            workspace_plan_content_sha256: row.try_get("workspace_plan_content_sha256")?,
            workspace_plan_evidence_manifest_sha256: row
                .try_get("workspace_plan_evidence_manifest_sha256")?,
            run_kind: row.try_get("run_kind")?,
            idempotency_key: row.try_get("idempotency_key")?,
            provider: row.try_get("provider")?,
            model: row.try_get("model")?,
            source_thread_id: row.try_get("source_thread_id")?,
            source_turn_id: row.try_get("source_turn_id")?,
            status: row.try_get("status")?,
            error_summary: row.try_get("error_summary")?,
            started_at_ms: row.try_get("started_at_ms")?,
            completed_at_ms: row.try_get("completed_at_ms")?,
            created_at_ms: row.try_get("created_at_ms")?,
            updated_at_ms: row.try_get("updated_at_ms")?,
        })
    }
}

impl TryFrom<WorkspaceAgentRunRow> for WorkspaceAgentRun {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceAgentRunRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            packet_id: row.packet_id,
            client_id: row.client_id,
            note_id: row.note_id,
            base_note_revision: row.base_note_revision,
            context_envelope_sha256: row.context_envelope_sha256,
            workspace_plan_revision_id: row.workspace_plan_revision_id,
            workspace_plan_content_sha256: row.workspace_plan_content_sha256,
            workspace_plan_evidence_manifest_sha256: row.workspace_plan_evidence_manifest_sha256,
            run_kind: row.run_kind,
            idempotency_key: row.idempotency_key,
            provider: row.provider,
            model: row.model,
            source_thread_id: row.source_thread_id,
            source_turn_id: row.source_turn_id,
            status: row.status,
            error_summary: row.error_summary,
            started_at: epoch_millis_to_datetime(row.started_at_ms)?,
            completed_at: row
                .completed_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
            created_at: epoch_millis_to_datetime(row.created_at_ms)?,
            updated_at: epoch_millis_to_datetime(row.updated_at_ms)?,
        })
    }
}

pub(crate) struct WorkspaceAgentRunSourceRow {
    pub id: String,
    pub run_id: String,
    pub source_entity_type: String,
    pub source_entity_id: String,
    pub source_revision: Option<i64>,
    pub display_label: String,
    pub snapshot_json: String,
    pub content_sha256: String,
    pub access_purpose: String,
    pub accessed_at_ms: i64,
}

impl WorkspaceAgentRunSourceRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            run_id: row.try_get("run_id")?,
            source_entity_type: row.try_get("source_entity_type")?,
            source_entity_id: row.try_get("source_entity_id")?,
            source_revision: row.try_get("source_revision")?,
            display_label: row.try_get("display_label")?,
            snapshot_json: row.try_get("snapshot_json")?,
            content_sha256: row.try_get("content_sha256")?,
            access_purpose: row.try_get("access_purpose")?,
            accessed_at_ms: row.try_get("accessed_at_ms")?,
        })
    }
}

impl TryFrom<WorkspaceAgentRunSourceRow> for WorkspaceAgentRunSource {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceAgentRunSourceRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            run_id: row.run_id,
            source_entity_type: row.source_entity_type,
            source_entity_id: row.source_entity_id,
            source_revision: row.source_revision,
            display_label: row.display_label,
            snapshot_json: row.snapshot_json,
            content_sha256: row.content_sha256,
            access_purpose: row.access_purpose,
            accessed_at: epoch_millis_to_datetime(row.accessed_at_ms)?,
        })
    }
}

pub(crate) struct WorkspaceNoteProposalDecisionRow {
    pub id: String,
    pub proposal_id: String,
    pub agent_result_id: Option<String>,
    pub note_id: String,
    pub base_revision: i64,
    pub decision_kind: String,
    pub change_id: Option<String>,
    pub applied_text: Option<String>,
    pub applied_text_sha256: Option<String>,
    pub resulting_note_revision: Option<i64>,
    pub actor: String,
    pub reason: String,
    pub created_at_ms: i64,
}

impl WorkspaceNoteProposalDecisionRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            proposal_id: row.try_get("proposal_id")?,
            agent_result_id: row.try_get("agent_result_id")?,
            note_id: row.try_get("note_id")?,
            base_revision: row.try_get("base_revision")?,
            decision_kind: row.try_get("decision_kind")?,
            change_id: row.try_get("change_id")?,
            applied_text: row.try_get("applied_text")?,
            applied_text_sha256: row.try_get("applied_text_sha256")?,
            resulting_note_revision: row.try_get("resulting_note_revision")?,
            actor: row.try_get("actor")?,
            reason: row.try_get("reason")?,
            created_at_ms: row.try_get("created_at_ms")?,
        })
    }
}

impl TryFrom<WorkspaceNoteProposalDecisionRow> for WorkspaceNoteProposalDecision {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceNoteProposalDecisionRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            proposal_id: row.proposal_id,
            agent_result_id: row.agent_result_id,
            note_id: row.note_id,
            base_revision: row.base_revision,
            decision_kind: WorkspaceNoteProposalDecisionKind::try_from(row.decision_kind.as_str())?,
            change_id: row.change_id,
            applied_text: row.applied_text,
            applied_text_sha256: row.applied_text_sha256,
            resulting_note_revision: row.resulting_note_revision,
            actor: row.actor,
            reason: row.reason,
            created_at: epoch_millis_to_datetime(row.created_at_ms)?,
        })
    }
}
