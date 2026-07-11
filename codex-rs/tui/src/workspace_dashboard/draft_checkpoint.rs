use super::*;
use crate::workspace_draft::WorkspaceDraftCheckpointInput;
use crate::workspace_draft::WorkspaceDraftCheckpointOutcome;
use crate::workspace_draft::WorkspaceDraftCheckpointTrigger;
use serde::Deserialize;
use serde::Serialize;

const DRAFT_SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DashboardCheckpointOutcome {
    Saved,
    AlreadyCurrent,
    Unavailable,
}

impl DashboardCheckpointOutcome {
    pub(crate) fn permits_handoff(self) -> bool {
        matches!(self, Self::Saved | Self::AlreadyCurrent)
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

    #[cfg(test)]
    pub(crate) fn draft_checkpoint_status_for_tests(&self) -> &str {
        &self.status
    }

    pub(crate) async fn checkpoint_idle_draft_if_due(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> Result<()> {
        if self.draft_coordinator.idle_checkpoint_is_due() {
            self.checkpoint_draft(app_server, WorkspaceDraftCheckpointTrigger::IdleTyping)
                .await?;
        }
        Ok(())
    }

    pub(crate) async fn checkpoint_draft(
        &mut self,
        app_server: &mut AppServerSession,
        trigger: WorkspaceDraftCheckpointTrigger,
    ) -> Result<DashboardCheckpointOutcome> {
        if self.profile != WorkspaceProfile::Medical
            || !self.draft_coordinator.should_checkpoint(trigger)
        {
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
            .checkpoint(app_server, input, trigger)
            .await;
        match result {
            Err(error) => {
                self.status = format!(
                    "Local draft checkpoint failed; canonical chart unchanged. Retry after idle: {error}"
                );
                Err(error)
            }
            Ok(WorkspaceDraftCheckpointOutcome::Saved(checkpoint)) => {
                self.status = format!(
                    "Local draft checkpoint r{} saved; canonical chart unchanged.",
                    checkpoint.revision
                );
                Ok(DashboardCheckpointOutcome::Saved)
            }
            Ok(WorkspaceDraftCheckpointOutcome::AlreadyCurrent) => {
                Ok(DashboardCheckpointOutcome::AlreadyCurrent)
            }
        }
    }

    pub(crate) async fn close_draft_after_canonical_save(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> Result<()> {
        if self
            .draft_coordinator
            .close_after_canonical_save(app_server)
            .await?
        {
            self.status =
                "Canonical chart saved; local draft checkpoint session closed.".to_string();
        }
        Ok(())
    }

    fn draft_checkpoint_input(&self) -> std::result::Result<WorkspaceDraftCheckpointInput, String> {
        let Some(client_id) = self.draft_client.id.clone() else {
            return Err(
                "Save this new patient before local draft checkpointing is available; canonical chart unchanged."
                    .to_string(),
            );
        };
        if self.has_unsupported_checkpoint_editor() {
            return Err(
                "Local checkpoints currently cover patient and note fields only; save or clear the open file, safety, job, addendum, or agent draft."
                    .to_string(),
            );
        }
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

    fn has_unsupported_checkpoint_editor(&self) -> bool {
        self.draft_document.is_active()
            || self.draft_safety.is_active()
            || self.derivative_draft.is_active()
            || self.clip_draft.is_active()
            || self.draft_task.is_active()
            || self.addendum_draft.active
            || self.agent_request.is_active()
            || self.agent_result.is_active()
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
