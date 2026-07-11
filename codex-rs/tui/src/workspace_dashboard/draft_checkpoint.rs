use super::*;
use crate::workspace_draft::CHECKPOINT_BOUNDARY_WAIT;
use crate::workspace_draft::WorkspaceDraftCheckpointInput;
use crate::workspace_draft::WorkspaceDraftCheckpointOutcome;
use crate::workspace_draft::WorkspaceDraftCheckpointTrigger;
use serde::Deserialize;
use serde::Serialize;
use std::time::Duration;
use std::time::Instant;

const DRAFT_SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy)]
struct CheckpointWaitBudget {
    started_at: Instant,
    total: Duration,
}

impl CheckpointWaitBudget {
    fn new(total: Duration) -> Self {
        Self::new_at(total, Instant::now())
    }

    fn new_at(total: Duration, started_at: Instant) -> Self {
        Self { started_at, total }
    }

    fn remaining(self) -> Duration {
        self.remaining_at(Instant::now())
    }

    fn remaining_at(self, now: Instant) -> Duration {
        self.total
            .saturating_sub(now.saturating_duration_since(self.started_at))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DashboardCheckpointOutcome {
    Saved,
    AlreadyCurrent,
    Pending,
    Unavailable,
    CanonicalBootstrap,
    CanonicalOnly,
}

impl DashboardCheckpointOutcome {
    pub(crate) fn permits_handoff(self) -> bool {
        matches!(self, Self::Saved | Self::AlreadyCurrent)
    }

    pub(crate) fn permits_close(self) -> bool {
        matches!(self, Self::Saved | Self::AlreadyCurrent)
    }

    pub(crate) fn permits_canonical_save(self) -> bool {
        matches!(
            self,
            Self::Saved | Self::AlreadyCurrent | Self::CanonicalBootstrap | Self::CanonicalOnly
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum DraftFocusV1 {
    Demographics,
    NoteTitle,
    NoteBody,
    Workflow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceDraftSnapshotV1 {
    schema_version: i64,
    base_client_version: String,
    client: ClientDraft,
    note: NoteDraft,
    focus: DraftFocusV1,
}

impl WorkspaceDashboard {
    pub(crate) fn canonical_save_completed(&self) -> bool {
        !self.dirty && self.pending_chart_changeset.is_none()
    }

    pub(crate) fn draft_checkpoint_pending_delay(&self) -> Option<Duration> {
        self.draft_coordinator.pending_delay()
    }

    pub(crate) fn take_focus_checkpoint_request(&mut self) -> bool {
        self.draft_coordinator.take_focus_checkpoint_request()
    }

    pub(crate) fn mark_canonical_save_pending_close(&mut self) {
        self.draft_coordinator.mark_canonical_save_pending_close();
        self.status =
            "Canonical chart saved; local draft checkpoint confirmation is pending; draft session remains open."
                .to_string();
    }

    pub(crate) fn arm_canonical_close_if_confirmed(&mut self) -> bool {
        if !self.draft_coordinator.has_confirmed_session() {
            return false;
        }
        self.mark_canonical_save_pending_close();
        true
    }

    pub(crate) fn first_session_checkpoint_retry_is_blocked(&self) -> bool {
        self.draft_coordinator.first_session_retry_is_blocked()
    }

    pub(crate) fn draft_checkpoint_blocks_scope_change(&self) -> bool {
        self.draft_coordinator.scope_change_is_blocked()
    }

    pub(crate) fn can_clear_dashboard_checkpoint_safely(&self) -> bool {
        self.draft_coordinator.can_clear_dashboard()
    }

    pub(crate) fn set_checkpoint_scope_change_blocked_status(&mut self, target: &str) {
        self.status = if self.draft_coordinator.first_session_retry_is_blocked() {
            format!(
                "Local draft checkpoint outcome is unknown; safe recovery is required before {target}."
            )
        } else if self.draft_coordinator.canonical_save_pending_close() {
            format!(
                "Canonical chart saved; wait for the local draft session to close before {target}."
            )
        } else if self.draft_coordinator.has_in_flight_checkpoint() {
            format!("Wait for the local draft checkpoint before {target}.")
        } else {
            format!("Save before {target}; close the current draft session first.")
        };
    }

    #[cfg(test)]
    pub(crate) fn draft_checkpoint_status_for_tests(&self) -> &str {
        &self.status
    }

    pub(super) fn draft_checkpoint_status_requires_attention(&self) -> bool {
        self.status
            .starts_with("Local draft checkpoint is still saving")
            || self.status.starts_with("Local draft checkpoint failed")
            || self
                .status
                .starts_with("Local draft checkpoint outcome is unknown")
            || self
                .status
                .starts_with("Local draft checkpoints are unavailable")
            || self.status.starts_with("Local checkpoints currently")
            || self.status.starts_with("Save this new patient")
            || self.status.contains("draft session remains open")
    }

    pub(crate) async fn checkpoint_idle_draft_if_due(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> Result<()> {
        if self.draft_coordinator.has_in_flight_checkpoint()
            || self.draft_coordinator.idle_checkpoint_is_due()
        {
            let outcome = self
                .checkpoint_draft(app_server, WorkspaceDraftCheckpointTrigger::IdleTyping)
                .await?;
            if outcome == DashboardCheckpointOutcome::Pending {
                return Ok(());
            }
        }
        if self.draft_coordinator.canonical_save_pending_close()
            && !self.draft_coordinator.first_session_retry_is_blocked()
            && !self.draft_coordinator.has_in_flight_checkpoint()
            && !self.draft_coordinator.has_uncheckpointed_edits()
        {
            self.close_draft_after_canonical_save(app_server).await?;
        }
        Ok(())
    }

    pub(crate) async fn checkpoint_draft(
        &mut self,
        app_server: &mut AppServerSession,
        trigger: WorkspaceDraftCheckpointTrigger,
    ) -> Result<DashboardCheckpointOutcome> {
        if self.profile != WorkspaceProfile::Medical {
            return Ok(DashboardCheckpointOutcome::AlreadyCurrent);
        }

        let canonical_bootstrap = trigger == WorkspaceDraftCheckpointTrigger::ExplicitSave
            && self.draft_client.id.is_none();
        let checkpoint_needed = canonical_bootstrap
            || self.draft_coordinator.has_in_flight_checkpoint()
            || self.draft_coordinator.should_checkpoint(trigger)
            || self.has_unsaved_unsupported_checkpoint_editor();
        if !checkpoint_needed {
            return Ok(DashboardCheckpointOutcome::AlreadyCurrent);
        }
        if !self.draft_coordinator.has_in_flight_checkpoint()
            && trigger == WorkspaceDraftCheckpointTrigger::ExplicitSave
            && self.has_unsaved_unsupported_chart_editor()
            && !self.has_unsaved_agent_or_addendum_editor()
            && !self.has_checkpointable_patient_or_note_changes()
        {
            self.draft_coordinator.pause_debounce();
            self.status =
                "This file, safety, or job draft will save canonically without a local draft checkpoint."
                    .to_string();
            return Ok(DashboardCheckpointOutcome::CanonicalOnly);
        }
        if app_server.uses_remote_workspace() {
            self.draft_coordinator.pause_debounce();
            self.status = if self.draft_coordinator.canonical_save_pending_close() {
                "Canonical chart saved; local draft checkpoints are unavailable through a remote app-server; no snapshot was sent and the draft session remains open."
                    .to_string()
            } else {
                "Local draft checkpoints are unavailable through a remote app-server; no workspace snapshot was sent."
                    .to_string()
            };
            color_eyre::eyre::bail!(
                "medical workspace draft checkpoints require a local app-server session"
            );
        }

        let wait = if trigger == WorkspaceDraftCheckpointTrigger::IdleTyping {
            Duration::ZERO
        } else {
            CHECKPOINT_BOUNDARY_WAIT
        };
        let wait_budget = CheckpointWaitBudget::new(wait);
        let mut completed_checkpoint = false;
        if self.draft_coordinator.has_in_flight_checkpoint() {
            match self
                .draft_coordinator
                .poll_in_flight_checkpoint(wait_budget.remaining())
                .await
            {
                Err(error) => {
                    self.set_checkpoint_failure_status(&error);
                    return Err(error);
                }
                Ok(WorkspaceDraftCheckpointOutcome::Saved { revision }) => {
                    self.set_checkpoint_saved_status(revision);
                    completed_checkpoint = true;
                }
                Ok(WorkspaceDraftCheckpointOutcome::Pending) => {
                    self.set_checkpoint_pending_status();
                    return Ok(DashboardCheckpointOutcome::Pending);
                }
                Ok(WorkspaceDraftCheckpointOutcome::AlreadyCurrent) => {}
            }
        }

        if self.has_unsaved_unsupported_checkpoint_editor() {
            self.draft_coordinator.pause_debounce();
            self.status =
                "Local checkpoints currently cover patient and note fields only; save or clear the open file, safety, job, addendum, or agent draft."
                    .to_string();
            return Ok(DashboardCheckpointOutcome::Unavailable);
        }
        if self.draft_client.id.is_none() {
            self.draft_coordinator.pause_debounce();
            self.status =
                "Save this new patient before local draft checkpointing is available; canonical chart unchanged."
                    .to_string();
            return Ok(
                if trigger == WorkspaceDraftCheckpointTrigger::ExplicitSave {
                    DashboardCheckpointOutcome::CanonicalBootstrap
                } else {
                    DashboardCheckpointOutcome::Unavailable
                },
            );
        }
        if completed_checkpoint && !self.draft_coordinator.has_uncheckpointed_edits() {
            return Ok(DashboardCheckpointOutcome::Saved);
        }
        if !self.draft_coordinator.should_checkpoint(trigger) {
            return Ok(DashboardCheckpointOutcome::AlreadyCurrent);
        }

        let input = match self.draft_checkpoint_input() {
            Ok(input) => input,
            Err(status) => {
                self.draft_coordinator.pause_debounce();
                self.status = status;
                return Ok(DashboardCheckpointOutcome::Unavailable);
            }
        };
        let result = self
            .draft_coordinator
            .checkpoint(app_server, input, trigger, wait_budget.remaining())
            .await;
        match result {
            Err(error) => {
                self.set_checkpoint_failure_status(&error);
                Err(error)
            }
            Ok(WorkspaceDraftCheckpointOutcome::Saved { revision }) => {
                self.set_checkpoint_saved_status(revision);
                Ok(DashboardCheckpointOutcome::Saved)
            }
            Ok(WorkspaceDraftCheckpointOutcome::AlreadyCurrent) => {
                Ok(DashboardCheckpointOutcome::AlreadyCurrent)
            }
            Ok(WorkspaceDraftCheckpointOutcome::Pending) => {
                self.set_checkpoint_pending_status();
                Ok(DashboardCheckpointOutcome::Pending)
            }
        }
    }

    pub(crate) async fn close_draft_after_canonical_save(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> Result<()> {
        match self
            .draft_coordinator
            .close_after_canonical_save(app_server)
            .await
        {
            Ok(true) => {
                self.status =
                    "Canonical chart saved; local draft checkpoint session closed.".to_string();
                Ok(())
            }
            Ok(false) if self.draft_coordinator.canonical_save_pending_close() => {
                let error = color_eyre::eyre::eyre!(
                    "local draft checkpoint session was not confirmed; session remains open"
                );
                self.set_checkpoint_close_failure_status(&error);
                Err(error)
            }
            Ok(false) => Ok(()),
            Err(error) => {
                self.set_checkpoint_close_failure_status(&error);
                Err(error)
            }
        }
    }

    pub(crate) fn acknowledge_canonical_only_save(&mut self) {
        self.draft_coordinator.acknowledge_canonical_only_save();
    }

    fn set_checkpoint_saved_status(&mut self, revision: i64) {
        self.status = if self.draft_coordinator.canonical_save_pending_close() {
            format!(
                "Canonical chart saved; local draft checkpoint r{revision} confirmed; draft session remains open until close completes."
            )
        } else {
            format!("Local draft checkpoint r{revision} saved; canonical chart unchanged.")
        };
    }

    fn set_checkpoint_pending_status(&mut self) {
        self.status = if self.draft_coordinator.canonical_save_pending_close() {
            "Canonical chart saved; local draft checkpoint is still saving; draft session remains open."
                .to_string()
        } else {
            "Local draft checkpoint is still saving; canonical chart unchanged.".to_string()
        };
    }

    fn set_checkpoint_failure_status(&mut self, error: &color_eyre::Report) {
        if self.draft_coordinator.first_session_retry_is_blocked() {
            self.status = if self.draft_coordinator.canonical_save_pending_close() {
                format!(
                    "Canonical chart saved; local draft checkpoint outcome is unknown; automatic retry is blocked to avoid a duplicate session, and the draft session remains open: {error}"
                )
            } else {
                format!(
                    "Local draft checkpoint outcome is unknown; automatic retry is blocked to avoid a duplicate active session: {error}"
                )
            };
            return;
        }
        self.status = if self.draft_coordinator.canonical_save_pending_close() {
            format!(
                "Canonical chart saved; local draft checkpoint failed; draft session remains open and will retry: {error}"
            )
        } else {
            format!(
                "Local draft checkpoint failed; canonical chart unchanged. Retry after idle: {error}"
            )
        };
    }

    fn set_checkpoint_close_failure_status(&mut self, error: &color_eyre::Report) {
        self.status = format!(
            "Canonical chart saved; local draft checkpoint is confirmed, but the draft session remains open and will retry closing: {error}"
        );
    }

    fn draft_checkpoint_input(&self) -> std::result::Result<WorkspaceDraftCheckpointInput, String> {
        let Some(client_id) = self.draft_client.id.clone() else {
            return Err(
                "Save this new patient before local draft checkpointing is available; canonical chart unchanged."
                    .to_string(),
            );
        };
        let canonical_client = self
            .clients
            .iter()
            .find(|client| client.id == client_id)
            .ok_or_else(|| {
                "Reload the saved patient before checkpointing this draft.".to_string()
            })?;
        let snapshot = WorkspaceDraftSnapshotV1 {
            schema_version: DRAFT_SCHEMA_VERSION,
            base_client_version: canonical_client.version.clone(),
            client: self.draft_client.clone(),
            note: self.draft_note.clone(),
            focus: DraftFocusV1::from_dashboard(self),
        };
        let draft = serde_json::to_value(snapshot)
            .map_err(|error| format!("Could not encode local draft checkpoint: {error}"))?;
        Ok(WorkspaceDraftCheckpointInput {
            client_id,
            encounter_id: self.draft_note.encounter_id.clone(),
            note_id: self.draft_note.id.clone(),
            base_note_revision: self
                .draft_note
                .id
                .as_ref()
                .map(|_| self.draft_note.current_revision),
            draft,
        })
    }

    pub(super) fn has_unsaved_unsupported_checkpoint_editor(&self) -> bool {
        self.has_unsaved_unsupported_chart_editor() || self.has_unsaved_agent_or_addendum_editor()
    }

    pub(super) fn has_unsaved_unsupported_chart_editor(&self) -> bool {
        self.dirty
            && (self.draft_document.is_active()
                || self.draft_safety.is_active()
                || self.derivative_draft.is_active()
                || self.clip_draft.is_active()
                || self.draft_task.is_active())
    }

    fn has_unsaved_agent_or_addendum_editor(&self) -> bool {
        self.addendum_draft.should_save()
            || self.agent_request.is_active() && self.agent_request.has_text()
            || self.agent_result.is_active() && self.agent_result.has_text()
    }

    pub(super) fn has_checkpointable_patient_or_note_changes(&self) -> bool {
        let client_changed = self.draft_client.id.as_deref().is_none_or(|client_id| {
            self.clients
                .iter()
                .find(|client| client.id == client_id)
                .is_none_or(|client| {
                    self.draft_client.upsert_params()
                        != ClientDraft::from_client(client).upsert_params()
                })
        });
        let note_changed = match self.draft_note.id.as_deref() {
            Some(note_id) => self
                .notes
                .iter()
                .find(|note| note.id == note_id)
                .is_none_or(|note| {
                    note.title != self.effective_note_title()
                        || note.body != self.draft_note.body
                        || note.status != self.draft_note.status_label()
                        || note.encounter_id != self.draft_note.encounter_id
                }),
            None => self.draft_note.should_save(),
        };
        client_changed || note_changed
    }
}

impl DraftFocusV1 {
    fn from_dashboard(dashboard: &WorkspaceDashboard) -> Self {
        match dashboard.focus {
            WorkspaceFocus::Demographics => Self::Demographics,
            WorkspaceFocus::NoteTitle => Self::NoteTitle,
            WorkspaceFocus::NoteBody => Self::NoteBody,
            WorkspaceFocus::Clients
            | WorkspaceFocus::Notes
            | WorkspaceFocus::Workflow
            | WorkspaceFocus::Agent
            | WorkspaceFocus::PatientFiles => Self::Workflow,
        }
    }
}

#[cfg(test)]
#[path = "draft_checkpoint_tests.rs"]
mod tests;
