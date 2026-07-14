use codex_app_server_protocol::WorkspaceDraftCheckpoint;
use codex_app_server_protocol::WorkspaceDraftSession;
use codex_app_server_protocol::WorkspaceDraftSessionCloseParams;
use codex_app_server_protocol::WorkspaceDraftSessionCloseStatus;
use codex_app_server_protocol::WorkspaceDraftSessionStatus;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use sha2::Digest;
use sha2::Sha256;
use std::time::Duration;
use thiserror::Error;

pub(crate) const MEDICAL_WORKSPACE_DRAFT_SCHEMA_VERSION: i64 = 1;
pub(crate) const MEDICAL_WORKSPACE_DRAFT_KIND: &str = "medicalWorkspaceWorkingDraft";
pub(crate) const MEDICAL_WORKSPACE_DRAFT_ACTOR: &str = "medical workspace TUI";
pub(crate) const WORKSPACE_DRAFT_AUTOSAVE_DELAY: Duration = Duration::from_millis(750);
const MAX_WORKSPACE_DRAFT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum MedicalWorkspaceDraftKindV1 {
    #[serde(rename = "medicalWorkspaceWorkingDraft")]
    MedicalWorkspaceWorkingDraft,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MedicalWorkspaceNoteDraftV1 {
    pub(crate) note_id: Option<String>,
    pub(crate) working_note_id: String,
    pub(crate) encounter_id: Option<String>,
    pub(crate) base_revision: Option<i64>,
    pub(crate) title: String,
    pub(crate) body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MedicalWorkspaceWorkingDraftV1 {
    schema_version: i64,
    kind: MedicalWorkspaceDraftKindV1,
    pub(crate) client_id: String,
    pub(crate) note: MedicalWorkspaceNoteDraftV1,
    pub(crate) agent_request_body: String,
    pub(crate) selected_file_ids: Vec<String>,
    pub(crate) selected_reviewed_text_ids: Vec<String>,
    pub(crate) selected_clip_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MedicalWorkspaceWorkingDraftInput {
    pub(crate) client_id: String,
    pub(crate) note_id: Option<String>,
    pub(crate) working_note_id: String,
    pub(crate) encounter_id: Option<String>,
    pub(crate) base_note_revision: Option<i64>,
    pub(crate) note_title: String,
    pub(crate) note_body: String,
    pub(crate) agent_request_body: String,
    pub(crate) selected_file_ids: Vec<String>,
    pub(crate) selected_reviewed_text_ids: Vec<String>,
    pub(crate) selected_clip_ids: Vec<String>,
}

impl MedicalWorkspaceWorkingDraftV1 {
    pub(crate) fn new(
        input: MedicalWorkspaceWorkingDraftInput,
    ) -> Result<Self, WorkspaceDraftError> {
        let draft = Self {
            schema_version: MEDICAL_WORKSPACE_DRAFT_SCHEMA_VERSION,
            kind: MedicalWorkspaceDraftKindV1::MedicalWorkspaceWorkingDraft,
            client_id: input.client_id.trim().to_string(),
            note: MedicalWorkspaceNoteDraftV1 {
                note_id: normalized_optional_id(input.note_id),
                working_note_id: input.working_note_id.trim().to_string(),
                encounter_id: normalized_optional_id(input.encounter_id),
                base_revision: input.base_note_revision,
                title: input.note_title,
                body: input.note_body,
            },
            agent_request_body: input.agent_request_body,
            selected_file_ids: normalized_ids(input.selected_file_ids),
            selected_reviewed_text_ids: normalized_ids(input.selected_reviewed_text_ids),
            selected_clip_ids: normalized_ids(input.selected_clip_ids),
        };
        draft.validate()?;
        Ok(draft)
    }

    pub(crate) fn decode(value: Value) -> Result<Self, WorkspaceDraftError> {
        let schema_version = value
            .get("schemaVersion")
            .and_then(Value::as_i64)
            .ok_or(WorkspaceDraftError::MissingSchemaVersion)?;
        if schema_version != MEDICAL_WORKSPACE_DRAFT_SCHEMA_VERSION {
            return Err(WorkspaceDraftError::UnsupportedSchemaVersion(
                schema_version,
            ));
        }
        let kind = value
            .get("kind")
            .and_then(Value::as_str)
            .ok_or(WorkspaceDraftError::MissingKind)?;
        if kind != MEDICAL_WORKSPACE_DRAFT_KIND {
            return Err(WorkspaceDraftError::UnsupportedKind(kind.to_string()));
        }
        let draft: Self = serde_json::from_value(value)?;
        draft.validate()?;
        Ok(draft)
    }

    pub(crate) fn encode(&self) -> Result<Value, WorkspaceDraftError> {
        self.validate()?;
        let encoded = serde_json::to_vec(self)?;
        if encoded.len() > MAX_WORKSPACE_DRAFT_BYTES {
            return Err(WorkspaceDraftError::DraftTooLarge {
                bytes: encoded.len(),
                limit: MAX_WORKSPACE_DRAFT_BYTES,
            });
        }
        Ok(serde_json::from_slice(&encoded)?)
    }

    #[cfg(test)]
    pub(crate) fn content_sha256(&self) -> Result<String, WorkspaceDraftError> {
        normalized_draft_sha256(&self.encode()?)
    }

    fn validate(&self) -> Result<(), WorkspaceDraftError> {
        if self.schema_version != MEDICAL_WORKSPACE_DRAFT_SCHEMA_VERSION {
            return Err(WorkspaceDraftError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        if self.client_id.trim().is_empty() {
            return Err(WorkspaceDraftError::InvalidDraft(
                "client ID must not be empty".to_string(),
            ));
        }
        if self.note.note_id.is_some() != self.note.base_revision.is_some() {
            return Err(WorkspaceDraftError::InvalidDraft(
                "saved note ID and base revision must be present together".to_string(),
            ));
        }
        if self.note.working_note_id.is_empty() || self.note.working_note_id.len() > 128 {
            return Err(WorkspaceDraftError::InvalidDraft(
                "working note ID must contain 1 to 128 bytes".to_string(),
            ));
        }
        if self.note.base_revision.is_some_and(|revision| revision < 0) {
            return Err(WorkspaceDraftError::InvalidDraft(
                "base note revision must not be negative".to_string(),
            ));
        }
        for (label, ids) in [
            ("selected file", &self.selected_file_ids),
            ("selected reviewed-text", &self.selected_reviewed_text_ids),
            ("selected clip", &self.selected_clip_ids),
        ] {
            if normalized_ids(ids.clone()) != *ids {
                return Err(WorkspaceDraftError::InvalidDraft(format!(
                    "{label} IDs must be non-empty, sorted, and unique"
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkspaceDraftCheckpointTrigger {
    IdleTyping,
    FocusChange,
    ExplicitSave,
    PatientNavigation,
    NoteNavigation,
    WorkspaceClose,
    PacketPreview,
    AgentHandoff,
}

impl WorkspaceDraftCheckpointTrigger {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::IdleTyping => "idle_typing",
            Self::FocusChange => "focus_change",
            Self::ExplicitSave => "explicit_save",
            Self::PatientNavigation => "patient_navigation",
            Self::NoteNavigation => "note_navigation",
            Self::WorkspaceClose => "workspace_close",
            Self::PacketPreview => "packet_preview",
            Self::AgentHandoff => "agent_handoff",
        }
    }

    pub(super) fn requires_exact_checkpoint(self) -> bool {
        matches!(self, Self::PacketPreview | Self::AgentHandoff)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkspaceDraftCloseDisposition {
    Closed,
    Discarded,
}

impl WorkspaceDraftCloseDisposition {
    fn protocol_status(self) -> WorkspaceDraftSessionCloseStatus {
        match self {
            Self::Closed => WorkspaceDraftSessionCloseStatus::Closed,
            Self::Discarded => WorkspaceDraftSessionCloseStatus::Discarded,
        }
    }

    fn session_status(self) -> WorkspaceDraftSessionStatus {
        match self {
            Self::Closed => WorkspaceDraftSessionStatus::Closed,
            Self::Discarded => WorkspaceDraftSessionStatus::Discarded,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceDraftCheckpointMetadata {
    pub(crate) checkpoint_id: String,
    pub(crate) session_id: String,
    pub(crate) client_id: String,
    pub(crate) encounter_id: Option<String>,
    pub(crate) note_id: Option<String>,
    pub(crate) base_note_revision: Option<i64>,
    pub(crate) revision: i64,
    pub(crate) content_sha256: String,
    pub(crate) trigger: String,
    pub(crate) created_at: i64,
}

impl WorkspaceDraftCheckpointMetadata {
    pub(super) fn from_checkpoint(
        checkpoint: &WorkspaceDraftCheckpoint,
    ) -> Result<(Self, MedicalWorkspaceWorkingDraftV1), WorkspaceDraftError> {
        if checkpoint.schema_version != MEDICAL_WORKSPACE_DRAFT_SCHEMA_VERSION {
            return Err(WorkspaceDraftError::UnsupportedSchemaVersion(
                checkpoint.schema_version,
            ));
        }
        for (label, value) in [
            ("checkpoint", checkpoint.id.as_str()),
            ("session", checkpoint.session_id.as_str()),
            ("client", checkpoint.client_id.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(WorkspaceDraftError::InvalidCheckpoint(format!(
                    "{label} ID must not be empty"
                )));
            }
        }
        if checkpoint.revision < 1 {
            return Err(WorkspaceDraftError::InvalidCheckpoint(
                "revision must be positive".to_string(),
            ));
        }
        if checkpoint.content_sha256.len() != 64
            || !checkpoint
                .content_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(WorkspaceDraftError::InvalidCheckpoint(
                "content hash must be a 64-character SHA-256 value".to_string(),
            ));
        }
        let draft = MedicalWorkspaceWorkingDraftV1::decode(checkpoint.draft.clone())?;
        let expected_content_sha256 = normalized_draft_sha256(&checkpoint.draft)?;
        if checkpoint.content_sha256 != expected_content_sha256 {
            return Err(WorkspaceDraftError::InvalidCheckpoint(
                "content hash does not match the normalized typed working draft".to_string(),
            ));
        }
        if draft.client_id != checkpoint.client_id
            || draft.note.encounter_id != checkpoint.encounter_id
            || draft.note.note_id != checkpoint.note_id
            || draft.note.base_revision != checkpoint.base_note_revision
        {
            return Err(WorkspaceDraftError::InvalidCheckpoint(
                "checkpoint metadata does not match its typed working draft".to_string(),
            ));
        }
        Ok((
            Self {
                checkpoint_id: checkpoint.id.clone(),
                session_id: checkpoint.session_id.clone(),
                client_id: checkpoint.client_id.clone(),
                encounter_id: checkpoint.encounter_id.clone(),
                note_id: checkpoint.note_id.clone(),
                base_note_revision: checkpoint.base_note_revision,
                revision: checkpoint.revision,
                content_sha256: checkpoint.content_sha256.clone(),
                trigger: checkpoint.trigger.clone(),
                created_at: checkpoint.created_at,
            },
            draft,
        ))
    }

    pub(crate) fn close_params(
        &self,
        disposition: WorkspaceDraftCloseDisposition,
        actor: &str,
        reason: &str,
    ) -> Result<WorkspaceDraftSessionCloseParams, WorkspaceDraftError> {
        let actor = required_text("close actor", actor)?;
        let reason = required_text("close reason", reason)?;
        Ok(WorkspaceDraftSessionCloseParams {
            session_id: self.session_id.clone(),
            client_id: self.client_id.clone(),
            status: disposition.protocol_status(),
            expected_current_checkpoint_id: Some(self.checkpoint_id.clone()),
            expected_current_checkpoint_revision: Some(self.revision),
            expected_current_checkpoint_sha256: Some(self.content_sha256.clone()),
            actor,
            reason,
        })
    }

    pub(crate) fn verify_terminal_session(
        &self,
        session: &WorkspaceDraftSession,
        disposition: WorkspaceDraftCloseDisposition,
    ) -> Result<(), WorkspaceDraftError> {
        if session.status != disposition.session_status()
            || session.id != self.session_id
            || session.client_id != self.client_id
            || session.current_revision != self.revision
            || session.current_checkpoint.id != self.checkpoint_id
            || session.current_checkpoint.session_id != self.session_id
            || session.current_checkpoint.client_id != self.client_id
            || session.current_checkpoint.encounter_id != self.encounter_id
            || session.current_checkpoint.note_id != self.note_id
            || session.current_checkpoint.base_note_revision != self.base_note_revision
            || session.current_checkpoint.schema_version != MEDICAL_WORKSPACE_DRAFT_SCHEMA_VERSION
            || session.current_checkpoint.revision != self.revision
            || session.current_checkpoint.content_sha256 != self.content_sha256
        {
            return Err(WorkspaceDraftError::InvalidCheckpoint(
                "terminal session did not confirm the exact checkpoint identity".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecoverableMedicalWorkspaceDraft {
    pub(crate) draft: MedicalWorkspaceWorkingDraftV1,
    pub(crate) checkpoint: WorkspaceDraftCheckpointMetadata,
    pub(crate) session_updated_at: i64,
}

impl TryFrom<WorkspaceDraftSession> for RecoverableMedicalWorkspaceDraft {
    type Error = WorkspaceDraftError;

    fn try_from(session: WorkspaceDraftSession) -> Result<Self, Self::Error> {
        if session.status != WorkspaceDraftSessionStatus::Active || session.closed_at.is_some() {
            return Err(WorkspaceDraftError::InvalidRecovery(
                "only an active draft session can be recovered".to_string(),
            ));
        }
        let (checkpoint, draft) =
            WorkspaceDraftCheckpointMetadata::from_checkpoint(&session.current_checkpoint)?;
        if session.id != checkpoint.session_id
            || session.client_id != checkpoint.client_id
            || session.current_revision != checkpoint.revision
        {
            return Err(WorkspaceDraftError::InvalidRecovery(
                "session and current checkpoint identities are inconsistent".to_string(),
            ));
        }
        Ok(Self {
            draft,
            checkpoint,
            session_updated_at: session.updated_at,
        })
    }
}

impl RecoverableMedicalWorkspaceDraft {
    pub(crate) fn matches_note_scope(
        &self,
        note_id: Option<&str>,
        encounter_id: Option<&str>,
    ) -> bool {
        self.checkpoint.note_id.as_deref() == note_id
            && self.checkpoint.encounter_id.as_deref() == encounter_id
    }

    pub(crate) fn matches_working_note_scope(
        &self,
        note_id: Option<&str>,
        encounter_id: Option<&str>,
        working_note_id: &str,
    ) -> bool {
        self.matches_note_scope(note_id, encounter_id)
            && self.draft.note.working_note_id == working_note_id
    }

    pub(crate) fn discard_params(
        &self,
        actor: &str,
        reason: &str,
    ) -> Result<WorkspaceDraftSessionCloseParams, WorkspaceDraftError> {
        self.checkpoint
            .close_params(WorkspaceDraftCloseDisposition::Discarded, actor, reason)
    }
}

#[derive(Debug, Error)]
pub(crate) enum WorkspaceDraftError {
    #[error("workspace working draft is missing schemaVersion")]
    MissingSchemaVersion,
    #[error("unsupported workspace working draft schemaVersion {0}")]
    UnsupportedSchemaVersion(i64),
    #[error("workspace working draft is missing kind")]
    MissingKind,
    #[error("unsupported workspace working draft kind {0}")]
    UnsupportedKind(String),
    #[error("invalid workspace working draft: {0}")]
    InvalidDraft(String),
    #[error("workspace working draft is {bytes} bytes; limit is {limit} bytes")]
    DraftTooLarge { bytes: usize, limit: usize },
    #[error("invalid workspace draft checkpoint: {0}")]
    InvalidCheckpoint(String),
    #[error("invalid workspace draft recovery: {0}")]
    InvalidRecovery(String),
    #[error("workspace draft generation is stale")]
    StaleGeneration,
    #[error("a workspace draft checkpoint is already in flight")]
    CheckpointInFlight,
    #[error("no workspace draft checkpoint is in flight")]
    NoCheckpointInFlight,
    #[error("durable workspace draft recovery is unavailable until the patient is saved")]
    DurableRecoveryUnavailable,
    #[error("no workspace draft recovery is available")]
    NoRecoveryAvailable,
    #[error("no confirmed workspace draft checkpoint is available")]
    NoConfirmedCheckpoint,
    #[error("workspace draft session cannot close with uncheckpointed changes")]
    UncheckpointedClose,
    #[error("workspace draft JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
}

fn normalized_optional_id(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn normalized_ids(ids: Vec<String>) -> Vec<String> {
    let mut ids = ids
        .into_iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn normalized_draft_sha256(value: &Value) -> Result<String, WorkspaceDraftError> {
    let normalized = serde_json::to_vec(value)?;
    Ok(format!("{:x}", Sha256::digest(normalized)))
}

pub(super) fn required_text(label: &str, value: &str) -> Result<String, WorkspaceDraftError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(WorkspaceDraftError::InvalidDraft(format!(
            "{label} must not be empty"
        )));
    }
    Ok(value.to_string())
}
