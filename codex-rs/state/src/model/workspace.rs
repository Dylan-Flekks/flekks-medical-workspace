use anyhow::Result;
use anyhow::anyhow;
use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use super::epoch_millis_to_datetime;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceClient {
    pub id: String,
    pub display_name: String,
    pub preferred_name: Option<String>,
    pub date_of_birth: Option<String>,
    pub sex_or_gender: Option<String>,
    pub external_id: Option<String>,
    pub record_start_date: Option<String>,
    pub record_end_date: Option<String>,
    pub summary: String,
    pub primary_phone: Option<String>,
    pub secondary_phone: Option<String>,
    pub email: Option<String>,
    pub preferred_contact_method: Option<String>,
    pub emergency_contact_name: Option<String>,
    pub emergency_contact_relationship: Option<String>,
    pub emergency_contact_phone: Option<String>,
    pub emergency_contact_email: Option<String>,
    pub contact_notes: Option<String>,
    pub payer_name: Option<String>,
    pub plan_name: Option<String>,
    pub member_id: Option<String>,
    pub group_number: Option<String>,
    pub coverage_type: Option<String>,
    pub coverage_status: Option<String>,
    pub coverage_notes: Option<String>,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceClientUpsert {
    pub id: Option<String>,
    pub display_name: String,
    pub preferred_name: Option<String>,
    pub date_of_birth: Option<String>,
    pub sex_or_gender: Option<String>,
    pub external_id: Option<String>,
    pub record_start_date: Option<String>,
    pub record_end_date: Option<String>,
    pub summary: String,
    pub primary_phone: Option<String>,
    pub secondary_phone: Option<String>,
    pub email: Option<String>,
    pub preferred_contact_method: Option<String>,
    pub emergency_contact_name: Option<String>,
    pub emergency_contact_relationship: Option<String>,
    pub emergency_contact_phone: Option<String>,
    pub emergency_contact_email: Option<String>,
    pub contact_notes: Option<String>,
    pub payer_name: Option<String>,
    pub plan_name: Option<String>,
    pub member_id: Option<String>,
    pub group_number: Option<String>,
    pub coverage_type: Option<String>,
    pub coverage_status: Option<String>,
    pub coverage_notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceDocument {
    pub id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub title: String,
    pub kind: String,
    pub local_path: String,
    pub notes: String,
    pub scope: String,
    pub detected_kind: String,
    pub mime_type: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub modified_at: Option<DateTime<Utc>>,
    pub sha256: Option<String>,
    pub tags: String,
    pub source_label: String,
    pub existence_status: String,
    pub metadata_json: String,
    pub original_path: String,
    pub reference_kind: String,
    pub vault_path: String,
    pub content_sha256: Option<String>,
    pub thumbnail_path: String,
    pub thumbnail_status: String,
    pub thumbnail_mime_type: Option<String>,
    pub intake_source: String,
    pub imported_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePracticeLibraryItem {
    pub document: WorkspaceDocument,
    pub owner_client_id: String,
    pub owner_display_name: String,
    pub linked_to_active_client: bool,
    pub linked_document_id: Option<String>,
    pub scope_reason: String,
    pub reviewed_text_count: i64,
    pub clip_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePracticeLibraryFilter {
    pub active_client_id: Option<String>,
    pub query: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceDocumentUpsert {
    pub id: Option<String>,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub title: String,
    pub kind: String,
    pub local_path: String,
    pub notes: String,
    pub scope: String,
    pub detected_kind: String,
    pub mime_type: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub modified_at: Option<DateTime<Utc>>,
    pub sha256: Option<String>,
    pub tags: String,
    pub source_label: String,
    pub existence_status: String,
    pub metadata_json: String,
    pub original_path: String,
    pub reference_kind: String,
    pub vault_path: String,
    pub content_sha256: Option<String>,
    pub thumbnail_path: String,
    pub thumbnail_status: String,
    pub thumbnail_mime_type: Option<String>,
    pub intake_source: String,
    pub imported_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspacePatientSafetyItem {
    pub id: String,
    pub client_id: String,
    pub category: String,
    pub name: String,
    pub reaction: Option<String>,
    pub severity: Option<String>,
    pub dose: Option<String>,
    pub route: Option<String>,
    pub frequency: Option<String>,
    pub status: Option<String>,
    pub recorded_date: Option<String>,
    pub notes: String,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspacePatientSafetyItemUpsert {
    pub id: Option<String>,
    pub client_id: String,
    pub category: String,
    pub name: String,
    pub reaction: Option<String>,
    pub severity: Option<String>,
    pub dose: Option<String>,
    pub route: Option<String>,
    pub frequency: Option<String>,
    pub status: Option<String>,
    pub recorded_date: Option<String>,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceArtifactDerivative {
    pub id: String,
    pub document_id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub review_status: String,
    pub source_method: String,
    pub page_range: String,
    pub timestamp_range: String,
    pub segment_label: String,
    pub tags: String,
    pub metadata_json: String,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceArtifactDerivativeUpsert {
    pub id: Option<String>,
    pub document_id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub review_status: String,
    pub source_method: String,
    pub page_range: String,
    pub timestamp_range: String,
    pub segment_label: String,
    pub tags: String,
    pub metadata_json: String,
    pub actor: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceArtifactDerivativeStatusUpdate {
    pub derivative_id: String,
    pub review_status: String,
    pub actor: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceArtifactDerivativeFilter {
    pub client_id: String,
    pub document_id: Option<String>,
    pub note_id: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceContextClip {
    pub id: String,
    pub derivative_id: String,
    pub document_id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub review_status: String,
    pub source_method: String,
    pub page_range: String,
    pub timestamp_range: String,
    pub line_range: String,
    pub segment_label: String,
    pub tags: String,
    pub metadata_json: String,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceContextClipUpsert {
    pub id: Option<String>,
    pub derivative_id: String,
    pub document_id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub review_status: String,
    pub source_method: String,
    pub page_range: String,
    pub timestamp_range: String,
    pub line_range: String,
    pub segment_label: String,
    pub tags: String,
    pub metadata_json: String,
    pub actor: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceContextClipStatusUpdate {
    pub clip_id: String,
    pub review_status: String,
    pub actor: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceContextClipFilter {
    pub client_id: String,
    pub derivative_id: Option<String>,
    pub document_id: Option<String>,
    pub note_id: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceTaskStatus {
    Open,
    InProgress,
    Blocked,
    Done,
    Canceled,
}

impl WorkspaceTaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::InProgress => "in_progress",
            Self::Blocked => "blocked",
            Self::Done => "done",
            Self::Canceled => "canceled",
        }
    }

    pub fn is_active(self) -> bool {
        matches!(self, Self::Open | Self::InProgress | Self::Blocked)
    }
}

impl TryFrom<&str> for WorkspaceTaskStatus {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "open" => Ok(Self::Open),
            "in_progress" => Ok(Self::InProgress),
            "blocked" => Ok(Self::Blocked),
            "done" => Ok(Self::Done),
            "canceled" => Ok(Self::Canceled),
            other => Err(anyhow!("unknown workspace task status `{other}`")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceTaskPriority {
    Low,
    Normal,
    High,
    Urgent,
}

impl WorkspaceTaskPriority {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Urgent => "urgent",
        }
    }
}

impl TryFrom<&str> for WorkspaceTaskPriority {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "low" => Ok(Self::Low),
            "normal" => Ok(Self::Normal),
            "high" => Ok(Self::High),
            "urgent" => Ok(Self::Urgent),
            other => Err(anyhow!("unknown workspace task priority `{other}`")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceTask {
    pub id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub document_id: Option<String>,
    pub title: String,
    pub details: String,
    pub kind: String,
    pub status: WorkspaceTaskStatus,
    pub priority: WorkspaceTaskPriority,
    pub due_date: Option<String>,
    pub assigned_to: Option<String>,
    pub completed_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceTaskUpsert {
    pub id: Option<String>,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub document_id: Option<String>,
    pub title: String,
    pub details: String,
    pub kind: String,
    pub status: WorkspaceTaskStatus,
    pub priority: WorkspaceTaskPriority,
    pub due_date: Option<String>,
    pub assigned_to: Option<String>,
    pub actor: String,
}

impl Default for WorkspaceTaskUpsert {
    fn default() -> Self {
        Self {
            id: None,
            client_id: String::new(),
            encounter_id: None,
            note_id: None,
            document_id: None,
            title: String::new(),
            details: String::new(),
            kind: "task".to_string(),
            status: WorkspaceTaskStatus::Open,
            priority: WorkspaceTaskPriority::Normal,
            due_date: None,
            assigned_to: None,
            actor: "human".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceTaskStatusUpdate {
    pub client_id: String,
    pub task_id: String,
    pub status: WorkspaceTaskStatus,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceEncounter {
    pub id: String,
    pub client_id: String,
    pub kind: String,
    pub title: String,
    pub status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceEncounterUpsert {
    pub id: Option<String>,
    pub client_id: String,
    pub kind: String,
    pub title: String,
    pub status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceNote {
    pub id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub title: String,
    pub kind: String,
    pub body: String,
    pub status: String,
    pub current_revision: i64,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceNoteUpsert {
    pub id: Option<String>,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub title: String,
    pub kind: String,
    pub body: String,
    pub status: String,
    pub actor: String,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceNoteProposalStatus {
    Pending,
    Accepted,
    Declined,
}

impl WorkspaceNoteProposalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Declined => "declined",
        }
    }
}

impl TryFrom<&str> for WorkspaceNoteProposalStatus {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "accepted" => Ok(Self::Accepted),
            "declined" => Ok(Self::Declined),
            other => Err(anyhow!("unknown workspace note proposal status `{other}`")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceNoteProposal {
    pub id: String,
    pub note_id: String,
    pub base_revision: i64,
    pub agent_result_id: Option<String>,
    pub proposed_body: String,
    pub summary: String,
    pub status: WorkspaceNoteProposalStatus,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceNoteProposalCreate {
    pub note_id: String,
    pub base_revision: i64,
    pub agent_result_id: Option<String>,
    pub proposed_body: String,
    pub summary: String,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceNoteSignature {
    pub id: String,
    pub note_id: String,
    pub revision: i64,
    pub signer: String,
    pub body_sha256: String,
    pub signed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceNoteSign {
    pub note_id: String,
    pub signer: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceNoteAddendum {
    pub id: String,
    pub note_id: String,
    pub base_revision: i64,
    pub body: String,
    pub author: String,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceNoteAddendumCreate {
    pub note_id: String,
    pub base_revision: i64,
    pub body: String,
    pub author: String,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceContextPacket {
    pub id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub source_draft_session_id: Option<String>,
    pub source_draft_checkpoint_id: Option<String>,
    pub source_draft_checkpoint_revision: Option<i64>,
    pub source_draft_checkpoint_sha256: Option<String>,
    pub human_request: String,
    pub selected_artifact_ids_json: String,
    pub selected_derivative_ids_json: String,
    pub selected_clip_ids_json: String,
    pub artifact_summary: String,
    pub derivative_summary: String,
    pub clip_summary: String,
    pub chart_context_summary: String,
    pub context_envelope_json: String,
    pub context_envelope_sha256: String,
    pub clinician_actor: String,
    pub base_note_revision: Option<i64>,
    pub authorized_scope_json: String,
    pub expected_output_kind: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub sent_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub canceled_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

/// Agent-visible context boundary:
/// - local workspace tables may contain broader patient, note, task, artifact, derivative, and clip rows
/// - agent-visible context is limited to an explicit context packet envelope
/// - the stored packet envelope is the authoritative sent snapshot
/// - replay is historical and read-only; current source rows must not expand an already sent packet
/// - unselected artifacts, derivatives, and clips must never appear in agent-visible payloads
/// - original local files are never uploaded or read automatically
/// - packet context grants no authority to write, sign, submit, or contact payers
/// - reviewed workspace handoffs bind all four draft-source fields to one current checkpoint
/// - legacy/manual callers may omit the entire draft-source tuple
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceContextPacketCreate {
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub source_draft_session_id: Option<String>,
    pub source_draft_checkpoint_id: Option<String>,
    pub source_draft_checkpoint_revision: Option<i64>,
    pub source_draft_checkpoint_sha256: Option<String>,
    pub human_request: String,
    pub selected_artifact_ids_json: String,
    pub selected_derivative_ids_json: String,
    pub selected_clip_ids_json: String,
    pub artifact_summary: String,
    pub derivative_summary: String,
    pub clip_summary: String,
    pub chart_context_summary: String,
    pub context_envelope_json: String,
    pub base_note_revision: Option<i64>,
    pub authorized_scope_json: String,
    pub expected_output_kind: String,
    pub status: String,
    pub actor: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceContextPacketFilter {
    pub client_id: String,
    pub note_id: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceContextPacketReplayFilter {
    pub client_id: String,
    pub packet_id: String,
    pub context_envelope_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceAgentResult {
    pub id: String,
    pub packet_id: String,
    pub client_id: String,
    pub note_id: Option<String>,
    pub run_id: Option<String>,
    pub base_note_revision: Option<i64>,
    pub context_envelope_sha256: String,
    pub packet_context_sha256: String,
    pub body: String,
    pub summary: String,
    pub result_kind: String,
    pub structured_changes_json: String,
    pub rationale_summary: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Agent-result provenance invariant: a saved result is a review-pending output
// from exactly one context packet. The packet id is authoritative for client,
// note, and context-envelope hash; conversions must verify the active workspace
// context before creating drafts/proposals and never sign, submit, contact, or
// overwrite records.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceAgentResultCreate {
    pub packet_id: String,
    pub run_id: Option<String>,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub body: String,
    pub summary: String,
    pub result_kind: String,
    pub structured_changes_json: String,
    pub rationale_summary: String,
    pub status: String,
    pub actor: String,
    pub expected_client_id: Option<String>,
    pub expected_note_id: Option<String>,
    pub expected_context_envelope_sha256: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceAgentResultStatusUpdate {
    pub result_id: String,
    pub status: String,
    pub actor: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceAgentResultFilter {
    pub client_id: String,
    pub note_id: Option<String>,
    pub packet_id: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceAuditEvent {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub action: String,
    pub actor: String,
    pub actor_kind: String,
    pub source: String,
    pub client_id: Option<String>,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub document_id: Option<String>,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub success: bool,
    pub summary: String,
    pub metadata_json: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceAuditEventCreate {
    pub entity_type: String,
    pub entity_id: String,
    pub action: String,
    pub actor: String,
    pub actor_kind: String,
    pub source: String,
    pub client_id: Option<String>,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub document_id: Option<String>,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub success: bool,
    pub summary: String,
    pub metadata_json: Option<String>,
}

impl Default for WorkspaceAuditEventCreate {
    fn default() -> Self {
        Self {
            entity_type: String::new(),
            entity_id: String::new(),
            action: String::new(),
            actor: "human".to_string(),
            actor_kind: "human".to_string(),
            source: "state".to_string(),
            client_id: None,
            encounter_id: None,
            note_id: None,
            document_id: None,
            source_thread_id: None,
            source_turn_id: None,
            success: true,
            summary: String::new(),
            metadata_json: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceAuditEventFilter {
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub client_id: Option<String>,
    pub note_id: Option<String>,
    pub cursor_created_at_ms: Option<i64>,
    pub limit: Option<u32>,
}

pub(crate) struct WorkspaceClientRow {
    pub id: String,
    pub display_name: String,
    pub preferred_name: Option<String>,
    pub date_of_birth: Option<String>,
    pub sex_or_gender: Option<String>,
    pub external_id: Option<String>,
    pub record_start_date: Option<String>,
    pub record_end_date: Option<String>,
    pub summary: String,
    pub contact_row_present: bool,
    pub primary_phone: Option<String>,
    pub secondary_phone: Option<String>,
    pub email: Option<String>,
    pub preferred_contact_method: Option<String>,
    pub emergency_contact_name: Option<String>,
    pub emergency_contact_relationship: Option<String>,
    pub emergency_contact_phone: Option<String>,
    pub emergency_contact_email: Option<String>,
    pub contact_notes: Option<String>,
    pub coverage_row_present: bool,
    pub payer_name: Option<String>,
    pub plan_name: Option<String>,
    pub member_id: Option<String>,
    pub group_number: Option<String>,
    pub coverage_type: Option<String>,
    pub coverage_status: Option<String>,
    pub coverage_notes: Option<String>,
    pub archived_at_ms: Option<i64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl WorkspaceClientRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            display_name: row.try_get("display_name")?,
            preferred_name: row.try_get("preferred_name")?,
            date_of_birth: row.try_get("date_of_birth")?,
            sex_or_gender: row.try_get("sex_or_gender")?,
            external_id: row.try_get("external_id")?,
            record_start_date: row.try_get("record_start_date")?,
            record_end_date: row.try_get("record_end_date")?,
            summary: row.try_get("summary")?,
            contact_row_present: row
                .try_get::<Option<String>, _>("contact_client_id")?
                .is_some(),
            primary_phone: row.try_get("primary_phone")?,
            secondary_phone: row.try_get("secondary_phone")?,
            email: row.try_get("email")?,
            preferred_contact_method: row.try_get("preferred_contact_method")?,
            emergency_contact_name: row.try_get("emergency_contact_name")?,
            emergency_contact_relationship: row.try_get("emergency_contact_relationship")?,
            emergency_contact_phone: row.try_get("emergency_contact_phone")?,
            emergency_contact_email: row.try_get("emergency_contact_email")?,
            contact_notes: row.try_get("contact_notes")?,
            coverage_row_present: row
                .try_get::<Option<String>, _>("coverage_client_id")?
                .is_some(),
            payer_name: row.try_get("payer_name")?,
            plan_name: row.try_get("plan_name")?,
            member_id: row.try_get("member_id")?,
            group_number: row.try_get("group_number")?,
            coverage_type: row.try_get("coverage_type")?,
            coverage_status: row.try_get("coverage_status")?,
            coverage_notes: row.try_get("coverage_notes")?,
            archived_at_ms: row.try_get("archived_at_ms")?,
            created_at_ms: row.try_get("created_at_ms")?,
            updated_at_ms: row.try_get("updated_at_ms")?,
        })
    }
}

impl TryFrom<WorkspaceClientRow> for WorkspaceClient {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceClientRow) -> Result<Self> {
        let legacy = legacy_client_admin_metadata_from_summary(&row.summary);
        let primary_phone = normalized_or_legacy(
            row.contact_row_present,
            row.primary_phone,
            legacy.primary_phone,
        );
        let secondary_phone = normalized_or_legacy(
            row.contact_row_present,
            row.secondary_phone,
            legacy.secondary_phone,
        );
        let email = normalized_or_legacy(row.contact_row_present, row.email, legacy.email);
        let preferred_contact_method = normalized_or_legacy(
            row.contact_row_present,
            row.preferred_contact_method,
            legacy.preferred_contact_method,
        );
        let emergency_contact_name = normalized_or_legacy(
            row.contact_row_present,
            row.emergency_contact_name,
            legacy.emergency_contact_name,
        );
        let emergency_contact_relationship = normalized_or_legacy(
            row.contact_row_present,
            row.emergency_contact_relationship,
            legacy.emergency_contact_relationship,
        );
        let emergency_contact_phone = normalized_or_legacy(
            row.contact_row_present,
            row.emergency_contact_phone,
            legacy.emergency_contact_phone,
        );
        let emergency_contact_email = normalized_or_legacy(
            row.contact_row_present,
            row.emergency_contact_email,
            legacy.emergency_contact_email,
        );
        let contact_notes = normalized_or_legacy(
            row.contact_row_present,
            row.contact_notes,
            legacy.contact_notes,
        );
        let payer_name =
            normalized_or_legacy(row.coverage_row_present, row.payer_name, legacy.payer_name);
        let plan_name =
            normalized_or_legacy(row.coverage_row_present, row.plan_name, legacy.plan_name);
        let member_id =
            normalized_or_legacy(row.coverage_row_present, row.member_id, legacy.member_id);
        let group_number = normalized_or_legacy(
            row.coverage_row_present,
            row.group_number,
            legacy.group_number,
        );
        let coverage_type = normalized_or_legacy(
            row.coverage_row_present,
            row.coverage_type,
            legacy.coverage_type,
        );
        let coverage_status = normalized_or_legacy(
            row.coverage_row_present,
            row.coverage_status,
            legacy.coverage_status,
        );
        let coverage_notes = normalized_or_legacy(
            row.coverage_row_present,
            row.coverage_notes,
            legacy.coverage_notes,
        );
        Ok(Self {
            id: row.id,
            display_name: row.display_name,
            preferred_name: row.preferred_name,
            date_of_birth: row.date_of_birth,
            sex_or_gender: row.sex_or_gender,
            external_id: row.external_id,
            record_start_date: row.record_start_date,
            record_end_date: row.record_end_date,
            summary: row.summary,
            primary_phone,
            secondary_phone,
            email,
            preferred_contact_method,
            emergency_contact_name,
            emergency_contact_relationship,
            emergency_contact_phone,
            emergency_contact_email,
            contact_notes,
            payer_name,
            plan_name,
            member_id,
            group_number,
            coverage_type,
            coverage_status,
            coverage_notes,
            archived_at: row
                .archived_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
            created_at: epoch_millis_to_datetime(row.created_at_ms)?,
            updated_at: epoch_millis_to_datetime(row.updated_at_ms)?,
        })
    }
}

fn normalized_or_legacy(
    normalized_row_present: bool,
    normalized: Option<String>,
    legacy: Option<String>,
) -> Option<String> {
    if normalized_row_present {
        normalized
    } else {
        legacy
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct WorkspaceClientAdminMetadata {
    pub primary_phone: Option<String>,
    pub secondary_phone: Option<String>,
    pub email: Option<String>,
    pub preferred_contact_method: Option<String>,
    pub emergency_contact_name: Option<String>,
    pub emergency_contact_relationship: Option<String>,
    pub emergency_contact_phone: Option<String>,
    pub emergency_contact_email: Option<String>,
    pub contact_notes: Option<String>,
    pub payer_name: Option<String>,
    pub plan_name: Option<String>,
    pub member_id: Option<String>,
    pub group_number: Option<String>,
    pub coverage_type: Option<String>,
    pub coverage_status: Option<String>,
    pub coverage_notes: Option<String>,
}

impl WorkspaceClientAdminMetadata {
    pub(crate) fn has_contact_values(&self) -> bool {
        [
            &self.primary_phone,
            &self.secondary_phone,
            &self.email,
            &self.preferred_contact_method,
            &self.emergency_contact_name,
            &self.emergency_contact_relationship,
            &self.emergency_contact_phone,
            &self.emergency_contact_email,
            &self.contact_notes,
        ]
        .iter()
        .any(|value| {
            value
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
        })
    }

    pub(crate) fn has_coverage_values(&self) -> bool {
        [
            &self.payer_name,
            &self.plan_name,
            &self.member_id,
            &self.group_number,
            &self.coverage_type,
            &self.coverage_status,
            &self.coverage_notes,
        ]
        .iter()
        .any(|value| {
            value
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
        })
    }
}

pub(crate) fn legacy_client_admin_metadata_from_summary(
    summary: &str,
) -> WorkspaceClientAdminMetadata {
    let fields = legacy_summary_field_map(summary);
    WorkspaceClientAdminMetadata {
        primary_phone: first_legacy_summary_value(
            &fields,
            &["primary phone", "phone", "mobile", "contact phone"],
        ),
        secondary_phone: first_legacy_summary_value(
            &fields,
            &["secondary phone", "alternate phone", "alt phone"],
        ),
        email: first_legacy_summary_value(&fields, &["email", "contact email"]),
        preferred_contact_method: first_legacy_summary_value(
            &fields,
            &[
                "preferred contact method",
                "preferred contact",
                "contact method",
            ],
        ),
        emergency_contact_name: first_legacy_summary_value(
            &fields,
            &["emergency contact name", "emergency contact", "ec name"],
        ),
        emergency_contact_relationship: first_legacy_summary_value(
            &fields,
            &[
                "emergency contact relationship",
                "emergency relationship",
                "ec relationship",
            ],
        ),
        emergency_contact_phone: first_legacy_summary_value(
            &fields,
            &["emergency contact phone", "emergency phone", "ec phone"],
        ),
        emergency_contact_email: first_legacy_summary_value(
            &fields,
            &["emergency contact email", "emergency email", "ec email"],
        ),
        contact_notes: first_legacy_summary_value(&fields, &["contact notes"]),
        payer_name: first_legacy_summary_value(
            &fields,
            &[
                "payer name",
                "payer",
                "insurance",
                "coverage payer",
                "medicare payer",
            ],
        ),
        plan_name: first_legacy_summary_value(&fields, &["plan name", "plan"]),
        member_id: first_legacy_summary_value(
            &fields,
            &[
                "member id",
                "member id / medicare id",
                "coverage member id",
                "medicare id",
                "insurance id",
            ],
        ),
        group_number: first_legacy_summary_value(&fields, &["group number", "group"]),
        coverage_type: first_legacy_summary_value(&fields, &["coverage type"]),
        coverage_status: first_legacy_summary_value(&fields, &["coverage status"]),
        coverage_notes: first_legacy_summary_value(&fields, &["coverage notes"]),
    }
}

fn legacy_summary_field_map(summary: &str) -> std::collections::BTreeMap<String, String> {
    summary
        .lines()
        .filter_map(|line| line.split_once(':'))
        .filter_map(|(label, value)| {
            let label = normalize_legacy_summary_label(label);
            let value = value.trim();
            if label.is_empty() || value.is_empty() {
                None
            } else {
                Some((label, value.to_string()))
            }
        })
        .collect()
}

fn first_legacy_summary_value(
    fields: &std::collections::BTreeMap<String, String>,
    labels: &[&str],
) -> Option<String> {
    labels
        .iter()
        .find_map(|label| fields.get(&normalize_legacy_summary_label(label)).cloned())
}

fn normalize_legacy_summary_label(label: &str) -> String {
    label
        .trim()
        .trim_matches(|ch: char| ch == '-' || ch == '*')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

pub(crate) struct WorkspaceDocumentRow {
    pub id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub title: String,
    pub kind: String,
    pub local_path: String,
    pub notes: String,
    pub scope: String,
    pub detected_kind: String,
    pub mime_type: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub modified_at_ms: Option<i64>,
    pub sha256: Option<String>,
    pub tags: String,
    pub source_label: String,
    pub existence_status: String,
    pub metadata_json: String,
    pub original_path: String,
    pub reference_kind: String,
    pub vault_path: String,
    pub content_sha256: Option<String>,
    pub thumbnail_path: String,
    pub thumbnail_status: String,
    pub thumbnail_mime_type: Option<String>,
    pub intake_source: String,
    pub imported_at_ms: Option<i64>,
    pub archived_at_ms: Option<i64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

pub(crate) struct WorkspacePatientSafetyItemRow {
    pub id: String,
    pub client_id: String,
    pub category: String,
    pub name: String,
    pub reaction: Option<String>,
    pub severity: Option<String>,
    pub dose: Option<String>,
    pub route: Option<String>,
    pub frequency: Option<String>,
    pub status: Option<String>,
    pub recorded_date: Option<String>,
    pub notes: String,
    pub archived_at_ms: Option<i64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

pub(crate) struct WorkspaceArtifactDerivativeRow {
    pub id: String,
    pub document_id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub review_status: String,
    pub source_method: String,
    pub page_range: String,
    pub timestamp_range: String,
    pub segment_label: String,
    pub tags: String,
    pub metadata_json: String,
    pub archived_at_ms: Option<i64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl WorkspaceArtifactDerivativeRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            document_id: row.try_get("document_id")?,
            client_id: row.try_get("client_id")?,
            encounter_id: row.try_get("encounter_id")?,
            note_id: row.try_get("note_id")?,
            kind: row.try_get("kind")?,
            title: row.try_get("title")?,
            body: row.try_get("body")?,
            review_status: row.try_get("review_status")?,
            source_method: row.try_get("source_method")?,
            page_range: row.try_get("page_range")?,
            timestamp_range: row.try_get("timestamp_range")?,
            segment_label: row.try_get("segment_label")?,
            tags: row.try_get("tags")?,
            metadata_json: row.try_get("metadata_json")?,
            archived_at_ms: row.try_get("archived_at_ms")?,
            created_at_ms: row.try_get("created_at_ms")?,
            updated_at_ms: row.try_get("updated_at_ms")?,
        })
    }
}

impl TryFrom<WorkspaceArtifactDerivativeRow> for WorkspaceArtifactDerivative {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceArtifactDerivativeRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            document_id: row.document_id,
            client_id: row.client_id,
            encounter_id: row.encounter_id,
            note_id: row.note_id,
            kind: row.kind,
            title: row.title,
            body: row.body,
            review_status: row.review_status,
            source_method: row.source_method,
            page_range: row.page_range,
            timestamp_range: row.timestamp_range,
            segment_label: row.segment_label,
            tags: row.tags,
            metadata_json: row.metadata_json,
            archived_at: row
                .archived_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
            created_at: epoch_millis_to_datetime(row.created_at_ms)?,
            updated_at: epoch_millis_to_datetime(row.updated_at_ms)?,
        })
    }
}

pub(crate) struct WorkspaceContextClipRow {
    pub id: String,
    pub derivative_id: String,
    pub document_id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub review_status: String,
    pub source_method: String,
    pub page_range: String,
    pub timestamp_range: String,
    pub line_range: String,
    pub segment_label: String,
    pub tags: String,
    pub metadata_json: String,
    pub archived_at_ms: Option<i64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl WorkspaceContextClipRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            derivative_id: row.try_get("derivative_id")?,
            document_id: row.try_get("document_id")?,
            client_id: row.try_get("client_id")?,
            encounter_id: row.try_get("encounter_id")?,
            note_id: row.try_get("note_id")?,
            kind: row.try_get("kind")?,
            title: row.try_get("title")?,
            body: row.try_get("body")?,
            review_status: row.try_get("review_status")?,
            source_method: row.try_get("source_method")?,
            page_range: row.try_get("page_range")?,
            timestamp_range: row.try_get("timestamp_range")?,
            line_range: row.try_get("line_range")?,
            segment_label: row.try_get("segment_label")?,
            tags: row.try_get("tags")?,
            metadata_json: row.try_get("metadata_json")?,
            archived_at_ms: row.try_get("archived_at_ms")?,
            created_at_ms: row.try_get("created_at_ms")?,
            updated_at_ms: row.try_get("updated_at_ms")?,
        })
    }
}

impl TryFrom<WorkspaceContextClipRow> for WorkspaceContextClip {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceContextClipRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            derivative_id: row.derivative_id,
            document_id: row.document_id,
            client_id: row.client_id,
            encounter_id: row.encounter_id,
            note_id: row.note_id,
            kind: row.kind,
            title: row.title,
            body: row.body,
            review_status: row.review_status,
            source_method: row.source_method,
            page_range: row.page_range,
            timestamp_range: row.timestamp_range,
            line_range: row.line_range,
            segment_label: row.segment_label,
            tags: row.tags,
            metadata_json: row.metadata_json,
            archived_at: row
                .archived_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
            created_at: epoch_millis_to_datetime(row.created_at_ms)?,
            updated_at: epoch_millis_to_datetime(row.updated_at_ms)?,
        })
    }
}

impl WorkspaceDocumentRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            client_id: row.try_get("client_id")?,
            encounter_id: row.try_get("encounter_id")?,
            title: row.try_get("title")?,
            kind: row.try_get("kind")?,
            local_path: row.try_get("local_path")?,
            notes: row.try_get("notes")?,
            scope: row.try_get("scope")?,
            detected_kind: row.try_get("detected_kind")?,
            mime_type: row.try_get("mime_type")?,
            file_size_bytes: row.try_get("file_size_bytes")?,
            modified_at_ms: row.try_get("modified_at_ms")?,
            sha256: row.try_get("sha256")?,
            tags: row.try_get("tags")?,
            source_label: row.try_get("source_label")?,
            existence_status: row.try_get("existence_status")?,
            metadata_json: row.try_get("metadata_json")?,
            original_path: row.try_get("original_path")?,
            reference_kind: row.try_get("reference_kind")?,
            vault_path: row.try_get("vault_path")?,
            content_sha256: row.try_get("content_sha256")?,
            thumbnail_path: row.try_get("thumbnail_path")?,
            thumbnail_status: row.try_get("thumbnail_status")?,
            thumbnail_mime_type: row.try_get("thumbnail_mime_type")?,
            intake_source: row.try_get("intake_source")?,
            imported_at_ms: row.try_get("imported_at_ms")?,
            archived_at_ms: row.try_get("archived_at_ms")?,
            created_at_ms: row.try_get("created_at_ms")?,
            updated_at_ms: row.try_get("updated_at_ms")?,
        })
    }
}

impl TryFrom<WorkspaceDocumentRow> for WorkspaceDocument {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceDocumentRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            client_id: row.client_id,
            encounter_id: row.encounter_id,
            title: row.title,
            kind: row.kind,
            local_path: row.local_path,
            notes: row.notes,
            scope: row.scope,
            detected_kind: row.detected_kind,
            mime_type: row.mime_type,
            file_size_bytes: row.file_size_bytes,
            modified_at: row
                .modified_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
            sha256: row.sha256,
            tags: row.tags,
            source_label: row.source_label,
            existence_status: row.existence_status,
            metadata_json: row.metadata_json,
            original_path: row.original_path,
            reference_kind: row.reference_kind,
            vault_path: row.vault_path,
            content_sha256: row.content_sha256,
            thumbnail_path: row.thumbnail_path,
            thumbnail_status: row.thumbnail_status,
            thumbnail_mime_type: row.thumbnail_mime_type,
            intake_source: row.intake_source,
            imported_at: row
                .imported_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
            archived_at: row
                .archived_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
            created_at: epoch_millis_to_datetime(row.created_at_ms)?,
            updated_at: epoch_millis_to_datetime(row.updated_at_ms)?,
        })
    }
}

impl WorkspacePatientSafetyItemRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            client_id: row.try_get("client_id")?,
            category: row.try_get("category")?,
            name: row.try_get("name")?,
            reaction: row.try_get("reaction")?,
            severity: row.try_get("severity")?,
            dose: row.try_get("dose")?,
            route: row.try_get("route")?,
            frequency: row.try_get("frequency")?,
            status: row.try_get("status")?,
            recorded_date: row.try_get("recorded_date")?,
            notes: row.try_get("notes")?,
            archived_at_ms: row.try_get("archived_at_ms")?,
            created_at_ms: row.try_get("created_at_ms")?,
            updated_at_ms: row.try_get("updated_at_ms")?,
        })
    }
}

impl TryFrom<WorkspacePatientSafetyItemRow> for WorkspacePatientSafetyItem {
    type Error = anyhow::Error;

    fn try_from(row: WorkspacePatientSafetyItemRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            client_id: row.client_id,
            category: row.category,
            name: row.name,
            reaction: row.reaction,
            severity: row.severity,
            dose: row.dose,
            route: row.route,
            frequency: row.frequency,
            status: row.status,
            recorded_date: row.recorded_date,
            notes: row.notes,
            archived_at: row
                .archived_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
            created_at: epoch_millis_to_datetime(row.created_at_ms)?,
            updated_at: epoch_millis_to_datetime(row.updated_at_ms)?,
        })
    }
}

pub(crate) struct WorkspaceTaskRow {
    pub id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub document_id: Option<String>,
    pub title: String,
    pub details: String,
    pub kind: String,
    pub status: String,
    pub priority: String,
    pub due_date: Option<String>,
    pub assigned_to: Option<String>,
    pub completed_at_ms: Option<i64>,
    pub archived_at_ms: Option<i64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl WorkspaceTaskRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            client_id: row.try_get("client_id")?,
            encounter_id: row.try_get("encounter_id")?,
            note_id: row.try_get("note_id")?,
            document_id: row.try_get("document_id")?,
            title: row.try_get("title")?,
            details: row.try_get("details")?,
            kind: row.try_get("kind")?,
            status: row.try_get("status")?,
            priority: row.try_get("priority")?,
            due_date: row.try_get("due_date")?,
            assigned_to: row.try_get("assigned_to")?,
            completed_at_ms: row.try_get("completed_at_ms")?,
            archived_at_ms: row.try_get("archived_at_ms")?,
            created_at_ms: row.try_get("created_at_ms")?,
            updated_at_ms: row.try_get("updated_at_ms")?,
        })
    }
}

impl TryFrom<WorkspaceTaskRow> for WorkspaceTask {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceTaskRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            client_id: row.client_id,
            encounter_id: row.encounter_id,
            note_id: row.note_id,
            document_id: row.document_id,
            title: row.title,
            details: row.details,
            kind: row.kind,
            status: WorkspaceTaskStatus::try_from(row.status.as_str())?,
            priority: WorkspaceTaskPriority::try_from(row.priority.as_str())?,
            due_date: row.due_date,
            assigned_to: row.assigned_to,
            completed_at: row
                .completed_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
            archived_at: row
                .archived_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
            created_at: epoch_millis_to_datetime(row.created_at_ms)?,
            updated_at: epoch_millis_to_datetime(row.updated_at_ms)?,
        })
    }
}

pub(crate) struct WorkspaceEncounterRow {
    pub id: String,
    pub client_id: String,
    pub kind: String,
    pub title: String,
    pub status: String,
    pub started_at_ms: Option<i64>,
    pub ended_at_ms: Option<i64>,
    pub archived_at_ms: Option<i64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl WorkspaceEncounterRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            client_id: row.try_get("client_id")?,
            kind: row.try_get("kind")?,
            title: row.try_get("title")?,
            status: row.try_get("status")?,
            started_at_ms: row.try_get("started_at_ms")?,
            ended_at_ms: row.try_get("ended_at_ms")?,
            archived_at_ms: row.try_get("archived_at_ms")?,
            created_at_ms: row.try_get("created_at_ms")?,
            updated_at_ms: row.try_get("updated_at_ms")?,
        })
    }
}

impl TryFrom<WorkspaceEncounterRow> for WorkspaceEncounter {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceEncounterRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            client_id: row.client_id,
            kind: row.kind,
            title: row.title,
            status: row.status,
            started_at: row
                .started_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
            ended_at: row.ended_at_ms.map(epoch_millis_to_datetime).transpose()?,
            archived_at: row
                .archived_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
            created_at: epoch_millis_to_datetime(row.created_at_ms)?,
            updated_at: epoch_millis_to_datetime(row.updated_at_ms)?,
        })
    }
}

pub(crate) struct WorkspaceNoteRow {
    pub id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub title: String,
    pub kind: String,
    pub body: String,
    pub status: String,
    pub current_revision: i64,
    pub archived_at_ms: Option<i64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl WorkspaceNoteRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            client_id: row.try_get("client_id")?,
            encounter_id: row.try_get("encounter_id")?,
            title: row.try_get("title")?,
            kind: row.try_get("kind")?,
            body: row.try_get("body")?,
            status: row.try_get("status")?,
            current_revision: row.try_get("current_revision")?,
            archived_at_ms: row.try_get("archived_at_ms")?,
            created_at_ms: row.try_get("created_at_ms")?,
            updated_at_ms: row.try_get("updated_at_ms")?,
        })
    }
}

impl TryFrom<WorkspaceNoteRow> for WorkspaceNote {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceNoteRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            client_id: row.client_id,
            encounter_id: row.encounter_id,
            title: row.title,
            kind: row.kind,
            body: row.body,
            status: row.status,
            current_revision: row.current_revision,
            archived_at: row
                .archived_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
            created_at: epoch_millis_to_datetime(row.created_at_ms)?,
            updated_at: epoch_millis_to_datetime(row.updated_at_ms)?,
        })
    }
}

pub(crate) struct WorkspaceNoteSignatureRow {
    pub id: String,
    pub note_id: String,
    pub revision: i64,
    pub signer: String,
    pub body_sha256: String,
    pub signed_at_ms: i64,
}

impl WorkspaceNoteSignatureRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            note_id: row.try_get("note_id")?,
            revision: row.try_get("revision")?,
            signer: row.try_get("signer")?,
            body_sha256: row.try_get("body_sha256")?,
            signed_at_ms: row.try_get("signed_at_ms")?,
        })
    }
}

impl TryFrom<WorkspaceNoteSignatureRow> for WorkspaceNoteSignature {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceNoteSignatureRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            note_id: row.note_id,
            revision: row.revision,
            signer: row.signer,
            body_sha256: row.body_sha256,
            signed_at: epoch_millis_to_datetime(row.signed_at_ms)?,
        })
    }
}

pub(crate) struct WorkspaceNoteAddendumRow {
    pub id: String,
    pub note_id: String,
    pub base_revision: i64,
    pub body: String,
    pub author: String,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub created_at_ms: i64,
}

impl WorkspaceNoteAddendumRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            note_id: row.try_get("note_id")?,
            base_revision: row.try_get("base_revision")?,
            body: row.try_get("body")?,
            author: row.try_get("author")?,
            source_thread_id: row.try_get("source_thread_id")?,
            source_turn_id: row.try_get("source_turn_id")?,
            created_at_ms: row.try_get("created_at_ms")?,
        })
    }
}

impl TryFrom<WorkspaceNoteAddendumRow> for WorkspaceNoteAddendum {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceNoteAddendumRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            note_id: row.note_id,
            base_revision: row.base_revision,
            body: row.body,
            author: row.author,
            source_thread_id: row.source_thread_id,
            source_turn_id: row.source_turn_id,
            created_at: epoch_millis_to_datetime(row.created_at_ms)?,
        })
    }
}

pub(crate) struct WorkspaceNoteProposalRow {
    pub id: String,
    pub note_id: String,
    pub base_revision: i64,
    pub agent_result_id: Option<String>,
    pub proposed_body: String,
    pub summary: String,
    pub status: String,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub created_at_ms: i64,
    pub resolved_at_ms: Option<i64>,
}

impl WorkspaceNoteProposalRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            note_id: row.try_get("note_id")?,
            base_revision: row.try_get("base_revision")?,
            agent_result_id: row.try_get("agent_result_id")?,
            proposed_body: row.try_get("proposed_body")?,
            summary: row.try_get("summary")?,
            status: row.try_get("status")?,
            source_thread_id: row.try_get("source_thread_id")?,
            source_turn_id: row.try_get("source_turn_id")?,
            created_at_ms: row.try_get("created_at_ms")?,
            resolved_at_ms: row.try_get("resolved_at_ms")?,
        })
    }
}

impl TryFrom<WorkspaceNoteProposalRow> for WorkspaceNoteProposal {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceNoteProposalRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            note_id: row.note_id,
            base_revision: row.base_revision,
            agent_result_id: row.agent_result_id,
            proposed_body: row.proposed_body,
            summary: row.summary,
            status: WorkspaceNoteProposalStatus::try_from(row.status.as_str())?,
            source_thread_id: row.source_thread_id,
            source_turn_id: row.source_turn_id,
            created_at: epoch_millis_to_datetime(row.created_at_ms)?,
            resolved_at: row
                .resolved_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
        })
    }
}

pub(crate) struct WorkspaceContextPacketRow {
    pub id: String,
    pub client_id: String,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub source_draft_session_id: Option<String>,
    pub source_draft_checkpoint_id: Option<String>,
    pub source_draft_checkpoint_revision: Option<i64>,
    pub source_draft_checkpoint_sha256: Option<String>,
    pub human_request: String,
    pub selected_artifact_ids_json: String,
    pub selected_derivative_ids_json: String,
    pub selected_clip_ids_json: String,
    pub artifact_summary: String,
    pub derivative_summary: String,
    pub clip_summary: String,
    pub chart_context_summary: String,
    pub context_envelope_json: String,
    pub context_envelope_sha256: String,
    pub clinician_actor: String,
    pub base_note_revision: Option<i64>,
    pub authorized_scope_json: String,
    pub expected_output_kind: String,
    pub status: String,
    pub created_at_ms: i64,
    pub sent_at_ms: i64,
    pub submitted_at_ms: Option<i64>,
    pub canceled_at_ms: Option<i64>,
    pub updated_at_ms: i64,
}

impl WorkspaceContextPacketRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            client_id: row.try_get("client_id")?,
            encounter_id: row.try_get("encounter_id")?,
            note_id: row.try_get("note_id")?,
            source_draft_session_id: row.try_get("source_draft_session_id")?,
            source_draft_checkpoint_id: row.try_get("source_draft_checkpoint_id")?,
            source_draft_checkpoint_revision: row.try_get("source_draft_checkpoint_revision")?,
            source_draft_checkpoint_sha256: row.try_get("source_draft_checkpoint_sha256")?,
            human_request: row.try_get("human_request")?,
            selected_artifact_ids_json: row.try_get("selected_artifact_ids_json")?,
            selected_derivative_ids_json: row.try_get("selected_derivative_ids_json")?,
            selected_clip_ids_json: row.try_get("selected_clip_ids_json")?,
            artifact_summary: row.try_get("artifact_summary")?,
            derivative_summary: row.try_get("derivative_summary")?,
            clip_summary: row.try_get("clip_summary")?,
            chart_context_summary: row.try_get("chart_context_summary")?,
            context_envelope_json: row.try_get("context_envelope_json")?,
            context_envelope_sha256: row.try_get("context_envelope_sha256")?,
            clinician_actor: row.try_get("clinician_actor")?,
            base_note_revision: row.try_get("base_note_revision")?,
            authorized_scope_json: row.try_get("authorized_scope_json")?,
            expected_output_kind: row.try_get("expected_output_kind")?,
            status: row.try_get("status")?,
            created_at_ms: row.try_get("created_at_ms")?,
            sent_at_ms: row.try_get("sent_at_ms")?,
            submitted_at_ms: row.try_get("submitted_at_ms")?,
            canceled_at_ms: row.try_get("canceled_at_ms")?,
            updated_at_ms: row.try_get("updated_at_ms")?,
        })
    }
}

impl TryFrom<WorkspaceContextPacketRow> for WorkspaceContextPacket {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceContextPacketRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            client_id: row.client_id,
            encounter_id: row.encounter_id,
            note_id: row.note_id,
            source_draft_session_id: row.source_draft_session_id,
            source_draft_checkpoint_id: row.source_draft_checkpoint_id,
            source_draft_checkpoint_revision: row.source_draft_checkpoint_revision,
            source_draft_checkpoint_sha256: row.source_draft_checkpoint_sha256,
            human_request: row.human_request,
            selected_artifact_ids_json: row.selected_artifact_ids_json,
            selected_derivative_ids_json: row.selected_derivative_ids_json,
            selected_clip_ids_json: row.selected_clip_ids_json,
            artifact_summary: row.artifact_summary,
            derivative_summary: row.derivative_summary,
            clip_summary: row.clip_summary,
            chart_context_summary: row.chart_context_summary,
            context_envelope_json: row.context_envelope_json,
            context_envelope_sha256: row.context_envelope_sha256,
            clinician_actor: row.clinician_actor,
            base_note_revision: row.base_note_revision,
            authorized_scope_json: row.authorized_scope_json,
            expected_output_kind: row.expected_output_kind,
            status: row.status,
            created_at: epoch_millis_to_datetime(row.created_at_ms)?,
            sent_at: epoch_millis_to_datetime(row.sent_at_ms)?,
            submitted_at: row
                .submitted_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
            canceled_at: row
                .canceled_at_ms
                .map(epoch_millis_to_datetime)
                .transpose()?,
            updated_at: epoch_millis_to_datetime(row.updated_at_ms)?,
        })
    }
}

pub(crate) struct WorkspaceAgentResultRow {
    pub id: String,
    pub packet_id: String,
    pub client_id: String,
    pub note_id: Option<String>,
    pub run_id: Option<String>,
    pub base_note_revision: Option<i64>,
    pub context_envelope_sha256: String,
    pub packet_context_sha256: String,
    pub body: String,
    pub summary: String,
    pub result_kind: String,
    pub structured_changes_json: String,
    pub rationale_summary: String,
    pub status: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl WorkspaceAgentResultRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            packet_id: row.try_get("packet_id")?,
            client_id: row.try_get("client_id")?,
            note_id: row.try_get("note_id")?,
            run_id: row.try_get("run_id")?,
            base_note_revision: row.try_get("base_note_revision")?,
            context_envelope_sha256: row.try_get("context_envelope_sha256")?,
            packet_context_sha256: row.try_get("packet_context_sha256")?,
            body: row.try_get("body")?,
            summary: row.try_get("summary")?,
            result_kind: row.try_get("result_kind")?,
            structured_changes_json: row.try_get("structured_changes_json")?,
            rationale_summary: row.try_get("rationale_summary")?,
            status: row.try_get("status")?,
            created_at_ms: row.try_get("created_at_ms")?,
            updated_at_ms: row.try_get("updated_at_ms")?,
        })
    }
}

impl TryFrom<WorkspaceAgentResultRow> for WorkspaceAgentResult {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceAgentResultRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            packet_id: row.packet_id,
            client_id: row.client_id,
            note_id: row.note_id,
            run_id: row.run_id,
            base_note_revision: row.base_note_revision,
            context_envelope_sha256: row.context_envelope_sha256,
            packet_context_sha256: row.packet_context_sha256,
            body: row.body,
            summary: row.summary,
            result_kind: row.result_kind,
            structured_changes_json: row.structured_changes_json,
            rationale_summary: row.rationale_summary,
            status: row.status,
            created_at: epoch_millis_to_datetime(row.created_at_ms)?,
            updated_at: epoch_millis_to_datetime(row.updated_at_ms)?,
        })
    }
}

pub(crate) struct WorkspaceAuditEventRow {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub action: String,
    pub actor: String,
    pub actor_kind: String,
    pub source: String,
    pub client_id: Option<String>,
    pub encounter_id: Option<String>,
    pub note_id: Option<String>,
    pub document_id: Option<String>,
    pub source_thread_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub success: bool,
    pub summary: String,
    pub metadata_json: Option<String>,
    pub created_at_ms: i64,
}

impl WorkspaceAuditEventRow {
    pub(crate) fn try_from_row(row: &SqliteRow) -> Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            entity_type: row.try_get("entity_type")?,
            entity_id: row.try_get("entity_id")?,
            action: row.try_get("action")?,
            actor: row.try_get("actor")?,
            actor_kind: row.try_get("actor_kind")?,
            source: row.try_get("source")?,
            client_id: row.try_get("client_id")?,
            encounter_id: row.try_get("encounter_id")?,
            note_id: row.try_get("note_id")?,
            document_id: row.try_get("document_id")?,
            source_thread_id: row.try_get("source_thread_id")?,
            source_turn_id: row.try_get("source_turn_id")?,
            success: row.try_get::<i64, _>("success")? != 0,
            summary: row.try_get("summary")?,
            metadata_json: row.try_get("metadata_json")?,
            created_at_ms: row.try_get("created_at_ms")?,
        })
    }
}

impl TryFrom<WorkspaceAuditEventRow> for WorkspaceAuditEvent {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceAuditEventRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            entity_type: row.entity_type,
            entity_id: row.entity_id,
            action: row.action,
            actor: row.actor,
            actor_kind: row.actor_kind,
            source: row.source,
            client_id: row.client_id,
            encounter_id: row.encounter_id,
            note_id: row.note_id,
            document_id: row.document_id,
            source_thread_id: row.source_thread_id,
            source_turn_id: row.source_turn_id,
            success: row.success,
            summary: row.summary,
            metadata_json: row.metadata_json,
            created_at: epoch_millis_to_datetime(row.created_at_ms)?,
        })
    }
}
