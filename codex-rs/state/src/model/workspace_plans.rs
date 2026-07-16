use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

use super::WorkspaceTaskPriority;
use super::epoch_millis_to_datetime;

fn content_sha256(content: &str) -> String {
    format!("{:x}", Sha256::digest(content.as_bytes()))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WorkspaceGuideModelToolMode {
    #[default]
    Disabled,
    WorkspacePlanningOnly,
}

impl WorkspaceGuideModelToolMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::WorkspacePlanningOnly => "workspace_planning_only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspacePlanSessionStatus {
    Active,
    Closed,
}

impl WorkspacePlanSessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Closed => "closed",
        }
    }

    fn from_stored(value: &str) -> Result<Self> {
        match value {
            "active" => Ok(Self::Active),
            "closed" => Ok(Self::Closed),
            other => anyhow::bail!("unknown stored workspace plan session status `{other}`"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePlanSession {
    pub id: String,
    pub client_id: String,
    pub source_thread_id: Option<String>,
    pub status: WorkspacePlanSessionStatus,
    pub latest_revision: i64,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub replayed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePlanSessionOpen {
    pub client_id: String,
    pub created_by: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePlanSessionThreadBind {
    pub session_id: String,
    pub client_id: String,
    pub expected_thread_id: Option<String>,
    pub source_thread_id: String,
    pub actor: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePlanSessionClose {
    pub session_id: String,
    pub client_id: String,
    pub actor: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspacePlanMessageRole {
    Human,
    Assistant,
    Question,
    Answer,
    Error,
    SystemStatus,
}

impl WorkspacePlanMessageRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Assistant => "assistant",
            Self::Question => "question",
            Self::Answer => "answer",
            Self::Error => "error",
            Self::SystemStatus => "system_status",
        }
    }

    fn from_stored(value: &str) -> Result<Self> {
        match value {
            "human" => Ok(Self::Human),
            "assistant" => Ok(Self::Assistant),
            "question" => Ok(Self::Question),
            "answer" => Ok(Self::Answer),
            "error" => Ok(Self::Error),
            "system_status" => Ok(Self::SystemStatus),
            other => anyhow::bail!("unknown stored workspace plan message role `{other}`"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePlanMessage {
    pub id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub guide_run_id: String,
    pub sequence: i64,
    pub role: WorkspacePlanMessageRole,
    pub content: String,
    pub content_sha256: String,
    pub idempotency_key: String,
    pub source_checkpoint_id: String,
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePlanMessageAppend {
    pub plan_session_id: String,
    pub client_id: String,
    pub guide_run_id: String,
    pub role: WorkspacePlanMessageRole,
    pub content: String,
    pub idempotency_key: String,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePlanMessageFilter {
    pub plan_session_id: String,
    pub client_id: String,
    pub after_sequence: Option<i64>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspacePlanRevisionStatus {
    Current,
    Outdated,
    Submitted,
}

impl WorkspacePlanRevisionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Outdated => "outdated",
            Self::Submitted => "submitted",
        }
    }

    fn from_stored(value: &str) -> Result<Self> {
        match value {
            "current" => Ok(Self::Current),
            "outdated" => Ok(Self::Outdated),
            "submitted" => Ok(Self::Submitted),
            other => anyhow::bail!("unknown stored workspace plan revision status `{other}`"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePlanRevision {
    pub id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub guide_run_id: String,
    pub revision: i64,
    pub plan_markdown: String,
    pub decisions_json: String,
    pub open_questions_json: String,
    pub content_sha256: String,
    pub evidence_manifest_json: String,
    pub evidence_manifest_sha256: String,
    pub evidence_read_count: u32,
    pub idempotency_key: String,
    pub status: WorkspacePlanRevisionStatus,
    pub source_checkpoint_id: String,
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub source_thread_id: String,
    pub source_turn_id: String,
    pub created_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub replayed: bool,
}

/// Immutable proof that one submitted Plan revision was bound to one exact
/// context packet and one exact master-agent run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePlanSubmissionReceipt {
    pub plan_revision_id: String,
    pub packet_id: String,
    pub agent_run_id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub plan_content_sha256: String,
    pub evidence_manifest_sha256: String,
    pub submitted_by: String,
    pub submitted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePlanRevisionCreate {
    pub plan_session_id: String,
    pub client_id: String,
    pub guide_run_id: String,
    pub plan_markdown: String,
    pub decisions_json: String,
    pub open_questions_json: String,
    pub idempotency_key: String,
    pub source_thread_id: String,
    pub source_turn_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePlanRevisionSubmit {
    pub revision_id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub packet_id: String,
    pub agent_run_id: String,
    pub source_checkpoint_id: String,
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub content_sha256: String,
    pub actor: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePlanRevisionOutdate {
    pub revision_id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub content_sha256: String,
    pub actor: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePlanRevisionFilter {
    pub plan_session_id: String,
    pub client_id: String,
    pub before_revision: Option<i64>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspacePlanProposalKind {
    NoteRevision,
    NoteAddendum,
    TaskDraft,
}

impl WorkspacePlanProposalKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoteRevision => "note_revision",
            Self::NoteAddendum => "note_addendum",
            Self::TaskDraft => "task_draft",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkspacePlanProposalPayload {
    NoteRevision {
        note_id: String,
        base_revision: i64,
        proposed_body: String,
    },
    NoteAddendum {
        note_id: String,
        base_revision: i64,
        body: String,
    },
    TaskDraft {
        title: String,
        details: String,
        task_kind: String,
        priority: WorkspaceTaskPriority,
        due_date: Option<String>,
        assigned_to: Option<String>,
    },
}

impl WorkspacePlanProposalPayload {
    pub fn kind(&self) -> WorkspacePlanProposalKind {
        match self {
            Self::NoteRevision { .. } => WorkspacePlanProposalKind::NoteRevision,
            Self::NoteAddendum { .. } => WorkspacePlanProposalKind::NoteAddendum,
            Self::TaskDraft { .. } => WorkspacePlanProposalKind::TaskDraft,
        }
    }

    pub fn target_note(&self) -> (Option<&str>, Option<i64>) {
        match self {
            Self::NoteRevision {
                note_id,
                base_revision,
                ..
            }
            | Self::NoteAddendum {
                note_id,
                base_revision,
                ..
            } => (Some(note_id), Some(*base_revision)),
            Self::TaskDraft { .. } => (None, None),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspacePlanProposalStatus {
    Pending,
    Accepted,
    Declined,
    Outdated,
}

impl WorkspacePlanProposalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Declined => "declined",
            Self::Outdated => "outdated",
        }
    }

    fn from_stored(value: &str) -> Result<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "accepted" => Ok(Self::Accepted),
            "declined" => Ok(Self::Declined),
            "outdated" => Ok(Self::Outdated),
            other => anyhow::bail!("unknown stored workspace plan proposal status `{other}`"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePlanProposal {
    pub id: String,
    pub plan_session_id: String,
    pub plan_revision_id: String,
    pub client_id: String,
    pub guide_run_id: String,
    pub kind: WorkspacePlanProposalKind,
    pub payload: WorkspacePlanProposalPayload,
    pub payload_sha256: String,
    pub summary: String,
    pub rationale: String,
    pub idempotency_key: String,
    pub status: WorkspacePlanProposalStatus,
    pub source_checkpoint_id: String,
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub source_thread_id: String,
    pub source_turn_id: String,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<String>,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePlanProposalCreate {
    pub plan_session_id: String,
    pub plan_revision_id: String,
    pub client_id: String,
    pub guide_run_id: String,
    pub payload: WorkspacePlanProposalPayload,
    pub summary: String,
    pub rationale: String,
    pub idempotency_key: String,
    pub source_thread_id: String,
    pub source_turn_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspacePlanProposalResolution {
    Accept,
    Decline,
}

impl WorkspacePlanProposalResolution {
    pub fn status(self) -> WorkspacePlanProposalStatus {
        match self {
            Self::Accept => WorkspacePlanProposalStatus::Accepted,
            Self::Decline => WorkspacePlanProposalStatus::Declined,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePlanProposalResolve {
    pub proposal_id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub resolution: WorkspacePlanProposalResolution,
    pub actor: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePlanProposalFilter {
    pub plan_session_id: String,
    pub client_id: String,
    pub status: Option<WorkspacePlanProposalStatus>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspacePlanError {
    Validation { message: String },
    NotFound { message: String },
    IdempotencyConflict { message: String },
    TerminalConflict { message: String },
    Stale { message: String },
    Transition { message: String },
    Storage { message: String },
}

impl WorkspacePlanError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Validation { .. } => "validation",
            Self::NotFound { .. } => "notFound",
            Self::IdempotencyConflict { .. } => "idempotencyConflict",
            Self::TerminalConflict { .. } => "terminalConflict",
            Self::Stale { .. } => "stale",
            Self::Transition { .. } => "transition",
            Self::Storage { .. } => "storage",
        }
    }
}

impl std::fmt::Display for WorkspacePlanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation { message }
            | Self::NotFound { message }
            | Self::IdempotencyConflict { message }
            | Self::TerminalConflict { message }
            | Self::Stale { message }
            | Self::Transition { message }
            | Self::Storage { message } => formatter.write_str(message),
        }
    }
}

impl std::error::Error for WorkspacePlanError {}

impl From<anyhow::Error> for WorkspacePlanError {
    fn from(error: anyhow::Error) -> Self {
        Self::Storage {
            message: error.to_string(),
        }
    }
}

impl From<sqlx::Error> for WorkspacePlanError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage {
            message: error.to_string(),
        }
    }
}

impl From<serde_json::Error> for WorkspacePlanError {
    fn from(error: serde_json::Error) -> Self {
        Self::Storage {
            message: error.to_string(),
        }
    }
}

#[derive(sqlx::FromRow)]
pub(crate) struct WorkspacePlanSessionRow {
    pub id: String,
    pub client_id: String,
    pub source_thread_id: Option<String>,
    pub status: String,
    pub latest_revision: i64,
    pub created_by: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub closed_at_ms: Option<i64>,
}

impl WorkspacePlanSessionRow {
    pub(crate) fn try_into_model(self, replayed: bool) -> Result<WorkspacePlanSession> {
        Ok(WorkspacePlanSession {
            id: self.id,
            client_id: self.client_id,
            source_thread_id: self.source_thread_id,
            status: WorkspacePlanSessionStatus::from_stored(&self.status)?,
            latest_revision: self.latest_revision,
            created_by: self.created_by,
            created_at: epoch_millis_to_datetime(self.created_at_ms)?,
            updated_at: epoch_millis_to_datetime(self.updated_at_ms)?,
            closed_at: self
                .closed_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
            replayed,
        })
    }
}

#[derive(sqlx::FromRow)]
pub(crate) struct WorkspacePlanMessageRow {
    pub id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub guide_run_id: String,
    pub sequence: i64,
    pub role: String,
    pub content: String,
    pub content_sha256: String,
    pub idempotency_key: String,
    pub source_checkpoint_id: String,
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub created_at_ms: i64,
}

impl WorkspacePlanMessageRow {
    pub(crate) fn try_into_model(self, replayed: bool) -> Result<WorkspacePlanMessage> {
        if content_sha256(&self.content) != self.content_sha256 {
            anyhow::bail!(
                "workspace plan message `{}` failed its content hash check",
                self.id
            );
        }
        Ok(WorkspacePlanMessage {
            id: self.id,
            plan_session_id: self.plan_session_id,
            client_id: self.client_id,
            guide_run_id: self.guide_run_id,
            sequence: self.sequence,
            role: WorkspacePlanMessageRole::from_stored(&self.role)?,
            content: self.content,
            content_sha256: self.content_sha256,
            idempotency_key: self.idempotency_key,
            source_checkpoint_id: self.source_checkpoint_id,
            source_checkpoint_revision: self.source_checkpoint_revision,
            source_checkpoint_sha256: self.source_checkpoint_sha256,
            encounter_id: self.encounter_id,
            note_id: self.note_id,
            source_thread_id: self.source_thread_id,
            source_turn_id: self.source_turn_id,
            created_at: epoch_millis_to_datetime(self.created_at_ms)?,
            replayed,
        })
    }
}

#[derive(sqlx::FromRow)]
pub(crate) struct WorkspacePlanRevisionRow {
    pub id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub guide_run_id: String,
    pub revision: i64,
    pub plan_markdown: String,
    pub decisions_json: String,
    pub open_questions_json: String,
    pub content_sha256: String,
    pub evidence_manifest_json: String,
    pub evidence_manifest_sha256: String,
    pub evidence_read_count: i64,
    pub idempotency_key: String,
    pub status: String,
    pub source_checkpoint_id: String,
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub source_thread_id: String,
    pub source_turn_id: String,
    pub created_at_ms: i64,
    pub submitted_at_ms: Option<i64>,
}

impl WorkspacePlanRevisionRow {
    pub(crate) fn try_into_model(self, replayed: bool) -> Result<WorkspacePlanRevision> {
        let decisions: Vec<String> = serde_json::from_str(&self.decisions_json)?;
        let open_questions: Vec<String> = serde_json::from_str(&self.open_questions_json)?;
        let canonical_content = serde_json::to_string(&serde_json::json!({
            "planMarkdown": &self.plan_markdown,
            "decisions": decisions,
            "openQuestions": open_questions,
        }))?;
        let evidence_manifest: Vec<serde_json::Value> =
            serde_json::from_str(&self.evidence_manifest_json)?;
        if content_sha256(&canonical_content) != self.content_sha256
            || content_sha256(&self.evidence_manifest_json) != self.evidence_manifest_sha256
            || evidence_manifest.len() != usize::try_from(self.evidence_read_count)?
        {
            anyhow::bail!(
                "workspace plan revision `{}` failed its content hash checks",
                self.id
            );
        }
        Ok(WorkspacePlanRevision {
            id: self.id,
            plan_session_id: self.plan_session_id,
            client_id: self.client_id,
            guide_run_id: self.guide_run_id,
            revision: self.revision,
            plan_markdown: self.plan_markdown,
            decisions_json: self.decisions_json,
            open_questions_json: self.open_questions_json,
            content_sha256: self.content_sha256,
            evidence_manifest_json: self.evidence_manifest_json,
            evidence_manifest_sha256: self.evidence_manifest_sha256,
            evidence_read_count: u32::try_from(self.evidence_read_count)?,
            idempotency_key: self.idempotency_key,
            status: WorkspacePlanRevisionStatus::from_stored(&self.status)?,
            source_checkpoint_id: self.source_checkpoint_id,
            source_checkpoint_revision: self.source_checkpoint_revision,
            source_checkpoint_sha256: self.source_checkpoint_sha256,
            encounter_id: self.encounter_id,
            note_id: self.note_id,
            source_thread_id: self.source_thread_id,
            source_turn_id: self.source_turn_id,
            created_at: epoch_millis_to_datetime(self.created_at_ms)?,
            submitted_at: self
                .submitted_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
            replayed,
        })
    }
}

#[derive(sqlx::FromRow)]
pub(crate) struct WorkspacePlanSubmissionReceiptRow {
    pub plan_revision_id: String,
    pub packet_id: String,
    pub agent_run_id: String,
    pub plan_session_id: String,
    pub client_id: String,
    pub plan_content_sha256: String,
    pub evidence_manifest_sha256: String,
    pub submitted_by: String,
    pub submitted_at_ms: i64,
}

impl WorkspacePlanSubmissionReceiptRow {
    pub(crate) fn try_into_model(self) -> Result<WorkspacePlanSubmissionReceipt> {
        Ok(WorkspacePlanSubmissionReceipt {
            plan_revision_id: self.plan_revision_id,
            packet_id: self.packet_id,
            agent_run_id: self.agent_run_id,
            plan_session_id: self.plan_session_id,
            client_id: self.client_id,
            plan_content_sha256: self.plan_content_sha256,
            evidence_manifest_sha256: self.evidence_manifest_sha256,
            submitted_by: self.submitted_by,
            submitted_at: epoch_millis_to_datetime(self.submitted_at_ms)?,
        })
    }
}

#[derive(sqlx::FromRow)]
pub(crate) struct WorkspacePlanProposalRow {
    pub id: String,
    pub plan_session_id: String,
    pub plan_revision_id: String,
    pub client_id: String,
    pub guide_run_id: String,
    pub proposal_kind: String,
    pub payload_json: String,
    pub payload_sha256: String,
    pub summary: String,
    pub rationale: String,
    pub idempotency_key: String,
    pub status: String,
    pub source_checkpoint_id: String,
    pub source_checkpoint_revision: i64,
    pub source_checkpoint_sha256: String,
    pub source_thread_id: String,
    pub source_turn_id: String,
    pub created_at_ms: i64,
    pub resolved_at_ms: Option<i64>,
    pub resolved_by: Option<String>,
}

impl WorkspacePlanProposalRow {
    pub(crate) fn try_into_model(self, replayed: bool) -> Result<WorkspacePlanProposal> {
        if content_sha256(&self.payload_json) != self.payload_sha256 {
            anyhow::bail!(
                "workspace plan proposal `{}` failed its payload hash check",
                self.id
            );
        }
        let payload: WorkspacePlanProposalPayload = serde_json::from_str(&self.payload_json)?;
        let kind = payload.kind();
        if kind.as_str() != self.proposal_kind {
            anyhow::bail!(
                "workspace plan proposal kind `{}` does not match payload kind `{}`",
                self.proposal_kind,
                kind.as_str()
            );
        }
        Ok(WorkspacePlanProposal {
            id: self.id,
            plan_session_id: self.plan_session_id,
            plan_revision_id: self.plan_revision_id,
            client_id: self.client_id,
            guide_run_id: self.guide_run_id,
            kind,
            payload,
            payload_sha256: self.payload_sha256,
            summary: self.summary,
            rationale: self.rationale,
            idempotency_key: self.idempotency_key,
            status: WorkspacePlanProposalStatus::from_stored(&self.status)?,
            source_checkpoint_id: self.source_checkpoint_id,
            source_checkpoint_revision: self.source_checkpoint_revision,
            source_checkpoint_sha256: self.source_checkpoint_sha256,
            source_thread_id: self.source_thread_id,
            source_turn_id: self.source_turn_id,
            created_at: epoch_millis_to_datetime(self.created_at_ms)?,
            resolved_at: self
                .resolved_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
            resolved_by: self.resolved_by,
            replayed,
        })
    }
}

#[cfg(test)]
#[path = "workspace_plans_integrity_tests.rs"]
mod tests;
