//! Local draft checkpoint coordination for the structured workspace.
//!
//! The coordinator tracks UI edit generations and owns the active app-server
//! draft session. It never writes canonical chart data; callers provide a
//! schema-versioned JSON snapshot when a checkpoint is due.

use crate::app_server_session::AppServerSession;
use codex_app_server_protocol::WorkspaceDraftCheckpoint;
use codex_app_server_protocol::WorkspaceDraftCheckpointCreateParams;
use codex_app_server_protocol::WorkspaceDraftSessionCloseParams;
use codex_app_server_protocol::WorkspaceDraftSessionCloseStatus;
use color_eyre::eyre::Result;
use serde_json::Value;
use std::time::Duration;
use std::time::Instant;

const CHECKPOINT_IDLE_DELAY: Duration = Duration::from_millis(900);
const CHECKPOINT_RETRY_DELAY: Duration = Duration::from_secs(5);
const CHECKPOINT_ACTOR: &str = "medical workspace TUI";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkspaceDraftCheckpointTrigger {
    IdleTyping,
    FocusChange,
    ExplicitSave,
    Close,
    Handoff,
}

impl WorkspaceDraftCheckpointTrigger {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::IdleTyping => "idle_typing",
            Self::FocusChange => "focus_change",
            Self::ExplicitSave => "explicit_save",
            Self::Close => "workspace_close",
            Self::Handoff => "agent_handoff",
        }
    }

    pub(crate) fn forces_checkpoint(self) -> bool {
        matches!(self, Self::Handoff)
    }
}

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
    Saved(WorkspaceDraftCheckpoint),
    AlreadyCurrent,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct WorkspaceDraftCoordinator {
    active_client_id: Option<String>,
    session_id: Option<String>,
    edit_generation: u64,
    saved_generation: u64,
    debounce_deadline: Option<Instant>,
    focus_checkpoint_requested: bool,
}

impl WorkspaceDraftCoordinator {
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn note_edit(&mut self) {
        self.note_edit_at(Instant::now());
    }

    pub(crate) fn request_focus_checkpoint(&mut self) {
        self.focus_checkpoint_requested = true;
    }

    pub(crate) fn take_focus_checkpoint_request(&mut self) -> bool {
        std::mem::take(&mut self.focus_checkpoint_requested)
    }

    fn note_edit_at(&mut self, now: Instant) {
        self.edit_generation = self.edit_generation.wrapping_add(1);
        self.debounce_deadline = Some(now + CHECKPOINT_IDLE_DELAY);
    }

    pub(crate) fn pending_delay(&self) -> Option<Duration> {
        self.pending_delay_at(Instant::now())
    }

    fn pending_delay_at(&self, now: Instant) -> Option<Duration> {
        self.debounce_deadline
            .map(|deadline| deadline.saturating_duration_since(now))
    }

    pub(crate) fn idle_checkpoint_is_due(&self) -> bool {
        self.idle_checkpoint_is_due_at(Instant::now())
    }

    fn idle_checkpoint_is_due_at(&self, now: Instant) -> bool {
        self.debounce_deadline
            .is_some_and(|deadline| deadline <= now)
            && self.has_uncheckpointed_edits()
    }

    pub(crate) fn should_checkpoint(&self, trigger: WorkspaceDraftCheckpointTrigger) -> bool {
        trigger.forces_checkpoint() || self.has_uncheckpointed_edits()
    }

    pub(crate) fn pause_debounce(&mut self) {
        self.debounce_deadline = None;
    }

    pub(crate) async fn checkpoint(
        &mut self,
        app_server: &mut AppServerSession,
        input: WorkspaceDraftCheckpointInput,
        trigger: WorkspaceDraftCheckpointTrigger,
    ) -> Result<WorkspaceDraftCheckpointOutcome> {
        if !self.should_checkpoint(trigger) {
            return Ok(WorkspaceDraftCheckpointOutcome::AlreadyCurrent);
        }
        if self.active_client_id.as_deref() != Some(input.client_id.as_str()) {
            self.reset_for_client(&input.client_id);
        }
        let result = app_server
            .workspace_draft_checkpoint_create(WorkspaceDraftCheckpointCreateParams {
                session_id: self.session_id.clone(),
                client_id: input.client_id.clone(),
                encounter_id: input.encounter_id,
                note_id: input.note_id,
                base_note_revision: input.base_note_revision,
                draft: input.draft,
                trigger: trigger.as_str().to_string(),
                actor: CHECKPOINT_ACTOR.to_string(),
            })
            .await;
        match result {
            Ok(response) => {
                self.active_client_id = Some(input.client_id);
                self.session_id = Some(response.checkpoint.session_id.clone());
                self.saved_generation = self.edit_generation;
                self.debounce_deadline = None;
                Ok(WorkspaceDraftCheckpointOutcome::Saved(response.checkpoint))
            }
            Err(error) => {
                self.debounce_deadline = Some(Instant::now() + CHECKPOINT_RETRY_DELAY);
                Err(error)
            }
        }
    }

    pub(crate) async fn close_after_canonical_save(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> Result<bool> {
        let (Some(session_id), Some(client_id)) =
            (self.session_id.clone(), self.active_client_id.clone())
        else {
            return Ok(false);
        };
        app_server
            .workspace_draft_session_close(WorkspaceDraftSessionCloseParams {
                session_id,
                client_id,
                status: WorkspaceDraftSessionCloseStatus::Closed,
                actor: CHECKPOINT_ACTOR.to_string(),
                reason: "canonical chart save confirmed".to_string(),
            })
            .await?;
        self.session_id = None;
        self.saved_generation = self.edit_generation;
        self.debounce_deadline = None;
        Ok(true)
    }

    fn has_uncheckpointed_edits(&self) -> bool {
        self.edit_generation != self.saved_generation
    }

    fn reset_for_client(&mut self, client_id: &str) {
        self.active_client_id = Some(client_id.to_string());
        self.session_id = None;
        self.edit_generation = 0;
        self.saved_generation = 0;
        self.debounce_deadline = None;
        self.focus_checkpoint_requested = false;
    }
}

#[cfg(test)]
#[path = "workspace_draft_tests.rs"]
mod tests;
