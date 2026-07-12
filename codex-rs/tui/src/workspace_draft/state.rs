use super::model::MedicalWorkspaceWorkingDraftV1;
use super::model::RecoverableMedicalWorkspaceDraft;
use super::model::WORKSPACE_DRAFT_AUTOSAVE_DELAY;
use super::model::WorkspaceDraftCheckpointMetadata;
use super::model::WorkspaceDraftCheckpointTrigger;
use super::model::WorkspaceDraftCloseDisposition;
use super::model::WorkspaceDraftError;
use super::model::required_text;
use codex_app_server_protocol::WorkspaceDraftCheckpointCreateParams;
use codex_app_server_protocol::WorkspaceDraftCheckpointCreateResponse;
use codex_app_server_protocol::WorkspaceDraftSession;
use codex_app_server_protocol::WorkspaceDraftSessionCloseParams;
use std::time::Duration;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct WorkspaceDraftGenerationToken {
    scope_generation: u64,
    edit_generation: u64,
}

impl WorkspaceDraftGenerationToken {
    #[cfg(test)]
    pub(crate) fn scope_generation(self) -> u64 {
        self.scope_generation
    }

    #[cfg(test)]
    pub(crate) fn edit_generation(self) -> u64 {
        self.edit_generation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WorkspaceDraftAutosaveSchedule {
    pub(crate) token: WorkspaceDraftGenerationToken,
    pub(crate) delay: Duration,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkspaceDraftPersistenceStatus {
    Unavailable,
    Idle,
    Pending(WorkspaceDraftGenerationToken),
    Saving(WorkspaceDraftGenerationToken),
    Saved(WorkspaceDraftCheckpointMetadata),
    Failed {
        token: WorkspaceDraftGenerationToken,
        message: String,
    },
    RecoveryAvailable(WorkspaceDraftCheckpointMetadata),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum WorkspaceDraftCheckpointStart {
    AlreadyCurrent,
    Request(WorkspaceDraftCheckpointCreateParams),
}

#[derive(Debug, Clone)]
struct InFlightCheckpoint {
    token: WorkspaceDraftGenerationToken,
    draft: MedicalWorkspaceWorkingDraftV1,
}

#[derive(Debug, Clone)]
#[derive(Default)]
pub(crate) struct WorkspaceDraftState {
    client_id: Option<String>,
    scope_generation: u64,
    edit_generation: u64,
    checkpointed_generation: u64,
    debounce_deadline: Option<Instant>,
    in_flight: Option<InFlightCheckpoint>,
    confirmed_checkpoint: Option<WorkspaceDraftCheckpointMetadata>,
    pending_recovery: Option<RecoverableMedicalWorkspaceDraft>,
    last_failure: Option<(WorkspaceDraftGenerationToken, String)>,
}


impl WorkspaceDraftState {
    pub(crate) fn reset_for_client(&mut self, client_id: impl Into<String>) {
        self.reset_scope(Some(client_id.into()));
    }

    pub(crate) fn reset_for_unsaved_patient(&mut self) {
        self.reset_scope(None);
    }

    pub(crate) fn mark_changed(&mut self) -> WorkspaceDraftAutosaveSchedule {
        self.mark_changed_at(Instant::now())
    }

    pub(super) fn mark_changed_at(&mut self, now: Instant) -> WorkspaceDraftAutosaveSchedule {
        self.edit_generation = self.edit_generation.wrapping_add(1);
        self.debounce_deadline = Some(now + WORKSPACE_DRAFT_AUTOSAVE_DELAY);
        self.last_failure = None;
        WorkspaceDraftAutosaveSchedule {
            token: self.current_token(),
            delay: WORKSPACE_DRAFT_AUTOSAVE_DELAY,
        }
    }

    pub(crate) fn current_token(&self) -> WorkspaceDraftGenerationToken {
        WorkspaceDraftGenerationToken {
            scope_generation: self.scope_generation,
            edit_generation: self.edit_generation,
        }
    }

    pub(crate) fn autosave_is_due(&self, token: WorkspaceDraftGenerationToken) -> bool {
        self.autosave_is_due_at(token, Instant::now())
    }

    pub(crate) fn autosave_remaining(
        &self,
        token: WorkspaceDraftGenerationToken,
    ) -> Option<Duration> {
        self.autosave_remaining_at(token, Instant::now())
    }

    pub(super) fn autosave_remaining_at(
        &self,
        token: WorkspaceDraftGenerationToken,
        now: Instant,
    ) -> Option<Duration> {
        if token != self.current_token()
            || !self.has_uncheckpointed_changes()
            || self.in_flight.is_some()
        {
            return None;
        }
        self.debounce_deadline
            .map(|deadline| deadline.saturating_duration_since(now))
    }

    pub(super) fn autosave_is_due_at(
        &self,
        token: WorkspaceDraftGenerationToken,
        now: Instant,
    ) -> bool {
        token == self.current_token()
            && self.has_uncheckpointed_changes()
            && self.in_flight.is_none()
            && self
                .debounce_deadline
                .is_some_and(|deadline| deadline <= now)
    }

    pub(crate) fn begin_checkpoint(
        &mut self,
        token: WorkspaceDraftGenerationToken,
        draft: &MedicalWorkspaceWorkingDraftV1,
        trigger: WorkspaceDraftCheckpointTrigger,
        actor: &str,
    ) -> Result<WorkspaceDraftCheckpointStart, WorkspaceDraftError> {
        if token != self.current_token() {
            return Err(WorkspaceDraftError::StaleGeneration);
        }
        if self.in_flight.is_some() {
            return Err(WorkspaceDraftError::CheckpointInFlight);
        }
        if self.pending_recovery.is_some() {
            return Err(WorkspaceDraftError::InvalidRecovery(
                "resolve the offered recovery before creating a new checkpoint".to_string(),
            ));
        }
        let client_id = self
            .client_id
            .as_deref()
            .ok_or(WorkspaceDraftError::DurableRecoveryUnavailable)?;
        if draft.client_id != client_id {
            return Err(WorkspaceDraftError::InvalidDraft(
                "working draft belongs to a different patient scope".to_string(),
            ));
        }
        if !self.has_uncheckpointed_changes()
            && (self.confirmed_checkpoint.is_some() || !trigger.requires_exact_checkpoint())
        {
            return Ok(WorkspaceDraftCheckpointStart::AlreadyCurrent);
        }
        let actor = required_text("checkpoint actor", actor)?;
        let confirmed = self.confirmed_checkpoint.as_ref();
        let params = WorkspaceDraftCheckpointCreateParams {
            session_id: confirmed.map(|checkpoint| checkpoint.session_id.clone()),
            client_id: draft.client_id.clone(),
            expected_current_checkpoint_id: confirmed
                .map(|checkpoint| checkpoint.checkpoint_id.clone()),
            expected_current_checkpoint_revision: confirmed
                .map(|checkpoint| checkpoint.revision),
            expected_current_checkpoint_sha256: confirmed
                .map(|checkpoint| checkpoint.content_sha256.clone()),
            encounter_id: draft.note.encounter_id.clone(),
            note_id: draft.note.note_id.clone(),
            base_note_revision: draft.note.base_revision,
            draft: draft.encode()?,
            trigger: trigger.as_str().to_string(),
            actor,
        };
        self.in_flight = Some(InFlightCheckpoint {
            token,
            draft: draft.clone(),
        });
        self.debounce_deadline = None;
        self.last_failure = None;
        Ok(WorkspaceDraftCheckpointStart::Request(params))
    }

    pub(crate) fn complete_checkpoint(
        &mut self,
        token: WorkspaceDraftGenerationToken,
        response: &WorkspaceDraftCheckpointCreateResponse,
    ) -> Result<WorkspaceDraftCheckpointMetadata, WorkspaceDraftError> {
        if token.scope_generation != self.scope_generation {
            return Err(WorkspaceDraftError::StaleGeneration);
        }
        let in_flight = self
            .in_flight
            .take()
            .ok_or(WorkspaceDraftError::NoCheckpointInFlight)?;
        if in_flight.token != token {
            self.in_flight = Some(in_flight);
            return Err(WorkspaceDraftError::StaleGeneration);
        }
        let (metadata, persisted_draft) =
            match WorkspaceDraftCheckpointMetadata::from_checkpoint(&response.checkpoint) {
                Ok(validated) => validated,
                Err(error) => {
                    self.record_completion_failure(token, &error);
                    return Err(error);
                }
            };
        if persisted_draft != in_flight.draft {
            let error = WorkspaceDraftError::InvalidCheckpoint(
                "checkpoint response did not contain the requested working draft".to_string(),
            );
            self.record_completion_failure(token, &error);
            return Err(error);
        }
        if self
            .confirmed_checkpoint
            .as_ref()
            .is_some_and(|checkpoint| checkpoint.session_id != metadata.session_id)
        {
            let error = WorkspaceDraftError::InvalidCheckpoint(
                "checkpoint response changed the active draft session".to_string(),
            );
            self.record_completion_failure(token, &error);
            return Err(error);
        }
        self.confirmed_checkpoint = Some(metadata.clone());
        self.checkpointed_generation = token.edit_generation;
        self.last_failure = None;
        if self.has_uncheckpointed_changes() {
            if self.debounce_deadline.is_none() {
                self.debounce_deadline = Some(Instant::now() + WORKSPACE_DRAFT_AUTOSAVE_DELAY);
            }
        } else {
            self.debounce_deadline = None;
        }
        Ok(metadata)
    }

    pub(crate) fn fail_checkpoint(
        &mut self,
        token: WorkspaceDraftGenerationToken,
        message: impl Into<String>,
    ) -> Result<(), WorkspaceDraftError> {
        self.fail_checkpoint_at(token, message.into(), Instant::now())
    }

    pub(super) fn fail_checkpoint_at(
        &mut self,
        token: WorkspaceDraftGenerationToken,
        message: String,
        now: Instant,
    ) -> Result<(), WorkspaceDraftError> {
        if token.scope_generation != self.scope_generation {
            return Err(WorkspaceDraftError::StaleGeneration);
        }
        let in_flight = self
            .in_flight
            .take()
            .ok_or(WorkspaceDraftError::NoCheckpointInFlight)?;
        if in_flight.token != token {
            self.in_flight = Some(in_flight);
            return Err(WorkspaceDraftError::StaleGeneration);
        }
        self.last_failure = Some((token, message));
        if self.has_uncheckpointed_changes() {
            self.debounce_deadline = Some(now + WORKSPACE_DRAFT_AUTOSAVE_DELAY);
        }
        Ok(())
    }

    pub(crate) fn offer_recovery(
        &mut self,
        recovery: RecoverableMedicalWorkspaceDraft,
    ) -> Result<(), WorkspaceDraftError> {
        if self.in_flight.is_some()
            || self.has_uncheckpointed_changes()
            || self.confirmed_checkpoint.is_some()
            || self.pending_recovery.is_some()
        {
            return Err(WorkspaceDraftError::InvalidRecovery(
                "cannot offer recovery over owned or changed draft state".to_string(),
            ));
        }
        if self.client_id.as_deref() != Some(recovery.checkpoint.client_id.as_str()) {
            return Err(WorkspaceDraftError::InvalidRecovery(
                "recovery belongs to a different patient scope".to_string(),
            ));
        }
        self.pending_recovery = Some(recovery);
        Ok(())
    }

    pub(crate) fn pending_recovery(&self) -> Option<&RecoverableMedicalWorkspaceDraft> {
        self.pending_recovery.as_ref()
    }

    pub(crate) fn adopt_recovery(
        &mut self,
    ) -> Result<MedicalWorkspaceWorkingDraftV1, WorkspaceDraftError> {
        let recovery = self
            .pending_recovery
            .take()
            .ok_or(WorkspaceDraftError::NoRecoveryAvailable)?;
        self.confirmed_checkpoint = Some(recovery.checkpoint);
        self.checkpointed_generation = self.edit_generation;
        self.debounce_deadline = None;
        self.last_failure = None;
        Ok(recovery.draft)
    }

    pub(crate) fn confirm_recovery_discarded(
        &mut self,
        session: &WorkspaceDraftSession,
    ) -> Result<(), WorkspaceDraftError> {
        let recovery = self
            .pending_recovery
            .as_ref()
            .ok_or(WorkspaceDraftError::NoRecoveryAvailable)?;
        recovery
            .checkpoint
            .verify_terminal_session(session, WorkspaceDraftCloseDisposition::Discarded)?;
        self.pending_recovery = None;
        Ok(())
    }

    pub(crate) fn exact_close_params(
        &self,
        disposition: WorkspaceDraftCloseDisposition,
        actor: &str,
        reason: &str,
    ) -> Result<WorkspaceDraftSessionCloseParams, WorkspaceDraftError> {
        if self.in_flight.is_some() || self.has_uncheckpointed_changes() {
            return Err(WorkspaceDraftError::UncheckpointedClose);
        }
        self.confirmed_checkpoint
            .as_ref()
            .ok_or(WorkspaceDraftError::NoConfirmedCheckpoint)?
            .close_params(disposition, actor, reason)
    }

    pub(crate) fn confirm_closed(
        &mut self,
        session: &WorkspaceDraftSession,
        disposition: WorkspaceDraftCloseDisposition,
    ) -> Result<(), WorkspaceDraftError> {
        let checkpoint = self
            .confirmed_checkpoint
            .as_ref()
            .ok_or(WorkspaceDraftError::NoConfirmedCheckpoint)?;
        checkpoint.verify_terminal_session(session, disposition)?;
        self.confirmed_checkpoint = None;
        self.pending_recovery = None;
        self.last_failure = None;
        Ok(())
    }

    pub(crate) fn confirmed_checkpoint(&self) -> Option<&WorkspaceDraftCheckpointMetadata> {
        self.confirmed_checkpoint.as_ref()
    }

    pub(crate) fn has_uncheckpointed_changes(&self) -> bool {
        self.edit_generation != self.checkpointed_generation
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn persistence_status(&self) -> WorkspaceDraftPersistenceStatus {
        if self.client_id.is_none() {
            return WorkspaceDraftPersistenceStatus::Unavailable;
        }
        if let Some(recovery) = self.pending_recovery.as_ref() {
            return WorkspaceDraftPersistenceStatus::RecoveryAvailable(recovery.checkpoint.clone());
        }
        if let Some(in_flight) = self.in_flight.as_ref() {
            return WorkspaceDraftPersistenceStatus::Saving(in_flight.token);
        }
        if let Some((token, message)) = self.last_failure.as_ref() {
            return WorkspaceDraftPersistenceStatus::Failed {
                token: *token,
                message: message.clone(),
            };
        }
        if self.has_uncheckpointed_changes() {
            return WorkspaceDraftPersistenceStatus::Pending(self.current_token());
        }
        self.confirmed_checkpoint
            .clone()
            .map(WorkspaceDraftPersistenceStatus::Saved)
            .unwrap_or(WorkspaceDraftPersistenceStatus::Idle)
    }

    fn record_completion_failure(
        &mut self,
        token: WorkspaceDraftGenerationToken,
        error: &WorkspaceDraftError,
    ) {
        self.last_failure = Some((token, error.to_string()));
        if self.has_uncheckpointed_changes() {
            self.debounce_deadline = Some(Instant::now() + WORKSPACE_DRAFT_AUTOSAVE_DELAY);
        }
    }

    fn reset_scope(&mut self, client_id: Option<String>) {
        self.client_id = client_id.and_then(|client_id| {
            let client_id = client_id.trim();
            (!client_id.is_empty()).then(|| client_id.to_string())
        });
        self.scope_generation = self.scope_generation.wrapping_add(1);
        self.edit_generation = 0;
        self.checkpointed_generation = 0;
        self.debounce_deadline = None;
        self.in_flight = None;
        self.confirmed_checkpoint = None;
        self.pending_recovery = None;
        self.last_failure = None;
    }
}
