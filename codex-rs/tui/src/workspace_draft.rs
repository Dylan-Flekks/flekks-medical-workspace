//! Local draft checkpoint coordination for the structured workspace.
//!
//! The coordinator tracks UI edit generations and owns the active app-server
//! draft session. It never writes canonical chart data; callers provide a
//! schema-versioned JSON snapshot when a checkpoint is due.

mod trigger;

pub(crate) use trigger::WorkspaceDraftCheckpointTrigger;

use crate::app_server_session::AppServerSession;
use codex_app_server_protocol::WorkspaceDraftCheckpoint;
use codex_app_server_protocol::WorkspaceDraftCheckpointCreateParams;
use codex_app_server_protocol::WorkspaceDraftCheckpointCreateResponse;
use codex_app_server_protocol::WorkspaceDraftSessionCloseParams;
use codex_app_server_protocol::WorkspaceDraftSessionCloseStatus;
use color_eyre::eyre::Result;
use serde_json::Value;
use std::time::Duration;
use std::time::Instant;
use uuid::Uuid;

const CHECKPOINT_IDLE_DELAY: Duration = Duration::from_millis(900);
const CHECKPOINT_RETRY_DELAY: Duration = Duration::from_secs(5);
const CHECKPOINT_POLL_DELAY: Duration = Duration::from_millis(100);
const CHECKPOINT_CLOSE_RETRY_DELAY: Duration = Duration::from_secs(1);
pub(crate) const CHECKPOINT_BOUNDARY_WAIT: Duration = Duration::from_secs(1);
const CHECKPOINT_ACTOR: &str = "medical workspace TUI";

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceDraftCheckpointInput {
    pub(crate) client_id: String,
    pub(crate) encounter_id: Option<String>,
    pub(crate) note_id: Option<String>,
    pub(crate) base_note_revision: Option<i64>,
    pub(crate) draft: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum WorkspaceDraftCheckpointOutcome {
    Saved { revision: i64 },
    AlreadyCurrent,
    Pending,
}

#[derive(Debug)]
struct WorkspaceDraftCheckpointInFlight {
    client_id: String,
    generation: u64,
    context_generation: u64,
    task: tokio::task::JoinHandle<Result<WorkspaceDraftCheckpointCreateResponse>>,
}

#[derive(Debug, Default)]
pub(crate) struct WorkspaceDraftCoordinator {
    active_client_id: Option<String>,
    session_id: Option<String>,
    session_creation_key: Option<String>,
    last_confirmed_checkpoint: Option<WorkspaceDraftCheckpoint>,
    edit_generation: u64,
    saved_generation: u64,
    context_generation: u64,
    saved_context_generation: u64,
    submitted_context_generation: u64,
    debounce_deadline: Option<Instant>,
    focus_checkpoint_requested: bool,
    in_flight: Option<WorkspaceDraftCheckpointInFlight>,
    canonical_save_pending_close: bool,
}

impl Clone for WorkspaceDraftCoordinator {
    fn clone(&self) -> Self {
        let debounce_deadline = if self.in_flight.is_some()
            && (self.has_uncheckpointed_edits() || self.session_creation_key.is_some())
        {
            Some(Instant::now())
        } else {
            self.debounce_deadline
        };
        Self {
            active_client_id: self.active_client_id.clone(),
            session_id: self.session_id.clone(),
            session_creation_key: self.session_creation_key.clone(),
            last_confirmed_checkpoint: self.last_confirmed_checkpoint.clone(),
            edit_generation: self.edit_generation,
            saved_generation: self.saved_generation,
            context_generation: self.context_generation,
            saved_context_generation: self.saved_context_generation,
            submitted_context_generation: self.submitted_context_generation,
            debounce_deadline,
            focus_checkpoint_requested: self.focus_checkpoint_requested,
            in_flight: None,
            canonical_save_pending_close: self.canonical_save_pending_close,
        }
    }
}

impl WorkspaceDraftCoordinator {
    pub(crate) fn try_clear(&mut self) -> bool {
        if !self.can_clear_dashboard() {
            return false;
        }
        *self = Self::default();
        true
    }

    pub(crate) fn note_edit(&mut self) {
        self.note_edit_at(Instant::now());
    }

    pub(crate) fn context_edit(&mut self) {
        self.note_edit_at(Instant::now());
        self.context_generation = self.context_generation.wrapping_add(1);
    }

    pub(crate) fn request_focus_checkpoint(&mut self) {
        self.focus_checkpoint_requested = true;
    }

    pub(crate) fn take_focus_checkpoint_request(&mut self) -> bool {
        std::mem::take(&mut self.focus_checkpoint_requested)
    }

    fn note_edit_at(&mut self, now: Instant) {
        // A newer clinician edit is not covered by the canonical save that armed
        // the close continuation. Keep the draft session open until that newer
        // generation is explicitly saved canonically.
        self.canonical_save_pending_close = false;
        self.edit_generation = self.edit_generation.wrapping_add(1);
        self.debounce_deadline = Some(now + CHECKPOINT_IDLE_DELAY);
    }

    pub(crate) fn pending_delay(&self) -> Option<Duration> {
        self.pending_delay_at(Instant::now())
    }

    fn pending_delay_at(&self, now: Instant) -> Option<Duration> {
        if self.in_flight.is_some() {
            return Some(CHECKPOINT_POLL_DELAY);
        }
        if self.has_uncheckpointed_edits() || self.session_creation_key.is_some() {
            return self
                .debounce_deadline
                .map(|deadline| deadline.saturating_duration_since(now));
        }
        self.canonical_save_pending_close
            .then_some(CHECKPOINT_CLOSE_RETRY_DELAY)
    }

    pub(crate) fn idle_checkpoint_is_due(&self) -> bool {
        self.idle_checkpoint_is_due_at(Instant::now())
    }

    fn idle_checkpoint_is_due_at(&self, now: Instant) -> bool {
        self.in_flight.is_none()
            && self
                .debounce_deadline
                .is_some_and(|deadline| deadline <= now)
            && (self.has_uncheckpointed_edits() || self.session_creation_key.is_some())
    }

    pub(crate) fn has_in_flight_checkpoint(&self) -> bool {
        self.in_flight.is_some()
    }

    pub(crate) fn canonical_save_pending_close(&self) -> bool {
        self.canonical_save_pending_close
    }

    pub(crate) fn has_confirmed_session(&self) -> bool {
        self.session_id.is_some() && self.last_confirmed_checkpoint.is_some()
    }

    pub(crate) fn has_unresolved_session_creation_retry(&self) -> bool {
        self.session_id.is_none() && self.session_creation_key.is_some()
    }

    pub(crate) fn mark_canonical_save_pending_close(&mut self) {
        self.canonical_save_pending_close = true;
    }

    pub(crate) fn scope_change_is_blocked(&self) -> bool {
        self.in_flight.is_some()
            || self.canonical_save_pending_close
            || self.session_creation_key.is_some()
            || self.has_uncheckpointed_edits()
            || self.session_id.is_some()
    }

    pub(crate) fn can_clear_dashboard(&self) -> bool {
        self.in_flight.is_none()
            && !self.canonical_save_pending_close
            && self.session_creation_key.is_none()
    }

    pub(crate) fn prepare_client_scope(&mut self, client_id: &str) -> bool {
        if self.active_client_id.as_deref() == Some(client_id) {
            return true;
        }
        if self.scope_change_is_blocked() {
            return false;
        }
        self.reset_for_client(client_id);
        true
    }

    pub(crate) fn should_checkpoint(&self, trigger: WorkspaceDraftCheckpointTrigger) -> bool {
        trigger.forces_checkpoint()
            || self.has_uncheckpointed_edits()
            || self.session_creation_key.is_some()
            || self.canonical_save_pending_close && !self.has_confirmed_session()
    }

    pub(crate) fn pause_debounce(&mut self) {
        self.debounce_deadline = None;
    }

    pub(crate) fn generation(&self) -> u64 {
        self.edit_generation
    }

    pub(crate) fn acknowledge_canonical_only_save_through(&mut self, generation: u64) {
        debug_assert!(self.in_flight.is_none());
        self.saved_generation = generation;
        if self.has_uncheckpointed_edits() {
            self.debounce_deadline = Some(Instant::now() + CHECKPOINT_IDLE_DELAY);
        } else {
            self.debounce_deadline = None;
        }
    }

    pub(crate) async fn checkpoint(
        &mut self,
        app_server: &mut AppServerSession,
        input: WorkspaceDraftCheckpointInput,
        trigger: WorkspaceDraftCheckpointTrigger,
        wait: Duration,
    ) -> Result<WorkspaceDraftCheckpointOutcome> {
        debug_assert!(self.in_flight.is_none());
        if !self.should_checkpoint(trigger) {
            return Ok(WorkspaceDraftCheckpointOutcome::AlreadyCurrent);
        }
        self.bind_client_for_checkpoint(&input.client_id)?;
        let generation = self.edit_generation;
        let context_generation = self.context_generation;
        let client_id = input.client_id.clone();
        let (session_id, session_creation_key) = self.checkpoint_session_identity();
        let task = app_server.spawn_workspace_draft_checkpoint_create(
            WorkspaceDraftCheckpointCreateParams {
                session_id,
                session_creation_key,
                client_id: input.client_id,
                encounter_id: input.encounter_id,
                note_id: input.note_id,
                base_note_revision: input.base_note_revision,
                draft: input.draft,
                trigger: trigger.as_str().to_string(),
                actor: CHECKPOINT_ACTOR.to_string(),
            },
        );
        self.in_flight = Some(WorkspaceDraftCheckpointInFlight {
            client_id,
            generation,
            context_generation,
            task,
        });
        self.debounce_deadline = None;
        self.poll_in_flight_checkpoint(wait).await
    }

    pub(crate) async fn poll_in_flight_checkpoint(
        &mut self,
        wait: Duration,
    ) -> Result<WorkspaceDraftCheckpointOutcome> {
        let Some(mut in_flight) = self.in_flight.take() else {
            return Ok(WorkspaceDraftCheckpointOutcome::AlreadyCurrent);
        };
        let result = match tokio::time::timeout(wait, &mut in_flight.task).await {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => {
                self.record_checkpoint_failure();
                return Err(color_eyre::eyre::eyre!(
                    "workspace draft checkpoint task failed to complete: {error}"
                ));
            }
            Err(_) => {
                self.in_flight = Some(in_flight);
                return Ok(WorkspaceDraftCheckpointOutcome::Pending);
            }
        };
        match result {
            Ok(response) => {
                if response.checkpoint.client_id != in_flight.client_id {
                    self.record_checkpoint_failure();
                    color_eyre::eyre::bail!(
                        "workspace draft checkpoint response changed patient scope"
                    );
                }
                self.active_client_id = Some(in_flight.client_id);
                self.session_id = Some(response.checkpoint.session_id.clone());
                self.session_creation_key = None;
                self.last_confirmed_checkpoint = Some(response.checkpoint.clone());
                self.saved_generation = in_flight.generation;
                self.saved_context_generation = in_flight.context_generation;
                if !self.has_uncheckpointed_edits() {
                    self.debounce_deadline = None;
                } else if self.debounce_deadline.is_none() {
                    self.debounce_deadline = Some(Instant::now() + CHECKPOINT_IDLE_DELAY);
                }
                Ok(WorkspaceDraftCheckpointOutcome::Saved {
                    revision: response.checkpoint.revision,
                })
            }
            Err(error) => {
                self.record_checkpoint_failure();
                Err(error)
            }
        }
    }

    pub(crate) async fn close_after_canonical_save(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> Result<bool> {
        if self.in_flight.is_some() || self.has_uncheckpointed_edits() {
            color_eyre::eyre::bail!(
                "local draft checkpoint is not confirmed; draft session remains open"
            );
        }
        let Some(params) = self.canonical_close_params()? else {
            return Ok(false);
        };
        app_server.workspace_draft_session_close(params).await?;
        self.session_id = None;
        self.session_creation_key = None;
        self.last_confirmed_checkpoint = None;
        self.canonical_save_pending_close = false;
        self.saved_generation = self.edit_generation;
        self.saved_context_generation = self.context_generation;
        self.submitted_context_generation = self.context_generation;
        self.debounce_deadline = None;
        Ok(true)
    }

    pub(crate) fn has_uncheckpointed_edits(&self) -> bool {
        self.edit_generation != self.saved_generation
    }

    pub(crate) fn has_uncheckpointed_context_edits(&self) -> bool {
        self.context_generation != self.saved_context_generation
    }

    pub(crate) fn has_unsubmitted_context_edits(&self) -> bool {
        self.context_generation != self.submitted_context_generation
    }

    pub(crate) fn mark_context_submitted(&mut self) {
        self.submitted_context_generation = self.context_generation;
    }

    #[cfg(test)]
    pub(crate) fn active_client_id_for_tests(&self) -> Option<&str> {
        self.active_client_id.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn session_id_for_tests(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn session_creation_key_for_tests(&self) -> Option<&str> {
        self.session_creation_key.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn set_session_creation_key_for_tests(&mut self, key: &str) {
        self.session_creation_key = Some(key.to_string());
    }

    #[cfg(test)]
    pub(crate) fn corrupt_confirmed_checkpoint_sha_for_tests(&mut self) {
        if let Some(checkpoint) = self.last_confirmed_checkpoint.as_mut() {
            checkpoint.content_sha256 = "f".repeat(64);
        }
    }

    fn bind_client_for_checkpoint(&mut self, client_id: &str) -> Result<()> {
        match self.active_client_id.as_deref() {
            None => {
                // The first durable patient identifier can arrive after edits were captured
                // (the canonical bootstrap save assigns it). Bind that identifier without
                // resetting the unsaved generation we are about to checkpoint.
                self.active_client_id = Some(client_id.to_string());
                self.session_id = None;
                Ok(())
            }
            Some(active_client_id) if active_client_id == client_id => Ok(()),
            Some(_) if self.scope_change_is_blocked() => {
                color_eyre::eyre::bail!(
                    "cannot move an active draft checkpoint session to another patient"
                )
            }
            Some(_) => {
                self.reset_for_client(client_id);
                Ok(())
            }
        }
    }

    fn checkpoint_session_identity(&mut self) -> (Option<String>, Option<String>) {
        match self.session_id.as_ref() {
            Some(session_id) => (Some(session_id.clone()), None),
            None => {
                let session_creation_key = self
                    .session_creation_key
                    .get_or_insert_with(|| Uuid::new_v4().to_string())
                    .clone();
                (None, Some(session_creation_key))
            }
        }
    }

    fn canonical_close_params(&self) -> Result<Option<WorkspaceDraftSessionCloseParams>> {
        let state = (
            self.session_id.as_deref(),
            self.active_client_id.as_deref(),
            self.last_confirmed_checkpoint.as_ref(),
        );
        let (session_id, client_id, checkpoint) = match state {
            (None, _, None) => return Ok(None),
            (Some(session_id), Some(client_id), Some(checkpoint)) => {
                (session_id, client_id, checkpoint)
            }
            _ => {
                color_eyre::eyre::bail!(
                    "local draft checkpoint identity is incomplete; exact session close is blocked"
                )
            }
        };
        if checkpoint.session_id != session_id || checkpoint.client_id != client_id {
            color_eyre::eyre::bail!(
                "local draft checkpoint identity changed scope; exact session close is blocked"
            );
        }
        Ok(Some(WorkspaceDraftSessionCloseParams {
            session_id: session_id.to_string(),
            client_id: client_id.to_string(),
            status: WorkspaceDraftSessionCloseStatus::Closed,
            expected_current_checkpoint_id: Some(checkpoint.id.clone()),
            expected_current_checkpoint_revision: Some(checkpoint.revision),
            expected_current_checkpoint_sha256: Some(checkpoint.content_sha256.clone()),
            actor: CHECKPOINT_ACTOR.to_string(),
            reason: "canonical chart save confirmed".to_string(),
        }))
    }

    fn record_checkpoint_failure(&mut self) {
        self.debounce_deadline = Some(Instant::now() + CHECKPOINT_RETRY_DELAY);
    }

    fn reset_for_client(&mut self, client_id: &str) {
        self.active_client_id = Some(client_id.to_string());
        self.session_id = None;
        self.session_creation_key = None;
        self.last_confirmed_checkpoint = None;
        self.edit_generation = 0;
        self.saved_generation = 0;
        self.context_generation = 0;
        self.saved_context_generation = 0;
        self.submitted_context_generation = 0;
        self.debounce_deadline = None;
        self.focus_checkpoint_requested = false;
        self.in_flight = None;
        self.canonical_save_pending_close = false;
    }
}

#[cfg(test)]
#[path = "workspace_draft_tests.rs"]
mod tests;
