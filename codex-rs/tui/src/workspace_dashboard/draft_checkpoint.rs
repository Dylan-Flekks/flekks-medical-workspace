use super::*;
use crate::workspace_draft::WorkspaceDraftCheckpointInput;
use crate::workspace_draft::WorkspaceDraftCheckpointOutcome;
use crate::workspace_draft::WorkspaceDraftCheckpointTrigger;
use codex_app_server_protocol::WorkspaceDraftSession;
use ratatui::style::Stylize;
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
    pub(crate) fn has_pending_draft_recovery_for_tests(&self) -> bool {
        self.draft_coordinator.pending_recovery().is_some()
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

    async fn detect_draft_recovery(&mut self, app_server: &mut AppServerSession) -> Result<()> {
        if self.profile != WorkspaceProfile::Medical {
            self.draft_coordinator.clear();
            return Ok(());
        }
        let Some(client_id) = self.draft_client.id.clone() else {
            self.draft_coordinator.clear();
            self.status =
                "Save this new patient before local draft checkpointing is available; canonical chart unchanged."
                    .to_string();
            return Ok(());
        };
        self.draft_coordinator
            .detect_recovery(app_server, &client_id)
            .await?;
        if self.draft_coordinator.pending_recovery().is_some() {
            self.status =
                "Local draft checkpoint found. Restore or discard it explicitly; canonical chart unchanged."
                    .to_string();
        }
        Ok(())
    }

    pub(crate) async fn refresh_draft_recovery(&mut self, app_server: &mut AppServerSession) {
        if let Err(error) = self.detect_draft_recovery(app_server).await {
            self.status =
                format!("Workspace opened, but local draft recovery is unavailable: {error}");
        }
    }

    pub(super) fn draft_recovery_key_action(
        &mut self,
        key_event: KeyEvent,
    ) -> Option<WorkspaceDashboardAction> {
        self.draft_coordinator.pending_recovery()?;
        Some(match key_event.code {
            KeyCode::Char(c) if c.eq_ignore_ascii_case(&'r') => {
                WorkspaceDashboardAction::RestoreDraftCheckpoint
            }
            KeyCode::Char(c) if c.eq_ignore_ascii_case(&'d') => {
                WorkspaceDashboardAction::DiscardDraftCheckpoint
            }
            _ => {
                self.status =
                    "Choose R to restore or D to discard the local draft checkpoint; canonical chart unchanged."
                        .to_string();
                WorkspaceDashboardAction::Consumed
            }
        })
    }

    pub(super) fn block_interaction_for_draft_recovery(&mut self, status: &str) -> bool {
        if self.draft_coordinator.pending_recovery().is_none() {
            return false;
        }
        self.status = status.to_string();
        true
    }

    pub(crate) async fn restore_pending_draft(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> Result<()> {
        let Some(recovery) = self.draft_coordinator.pending_recovery().cloned() else {
            return Ok(());
        };
        let snapshot = match self.validate_recovery_snapshot(&recovery) {
            Ok(snapshot) => snapshot,
            Err(status) => {
                self.status = status;
                return Ok(());
            }
        };
        let restored_revision = recovery.current_checkpoint.revision;
        self.apply_recovery_snapshot(snapshot);
        self.draft_coordinator.accept_recovery();
        self.reload_packet_history(app_server).await?;
        self.load_active_note_details(app_server).await?;
        self.status = format!(
            "Restored local draft checkpoint r{restored_revision}; canonical chart unchanged."
        );
        Ok(())
    }

    pub(crate) async fn discard_pending_draft(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> Result<()> {
        if self.draft_coordinator.discard_recovery(app_server).await? {
            self.status =
                "Discarded local draft checkpoint; canonical chart unchanged.".to_string();
        }
        Ok(())
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

    pub(super) fn render_draft_recovery_overlay(&self, area: Rect, buf: &mut Buffer) {
        let Some(recovery) = self.draft_coordinator.pending_recovery() else {
            return;
        };
        let width = area.width.saturating_sub(4).min(76);
        let height = area.height.min(11);
        let overlay = Rect::new(
            area.x + area.width.saturating_sub(width) / 2,
            area.y + area.height.saturating_sub(height) / 2,
            width,
            height,
        );
        let checkpoint = &recovery.current_checkpoint;
        let note_scope = checkpoint
            .note_id
            .as_deref()
            .map(|_| "saved note")
            .unwrap_or("new note");
        let mut lines: Vec<Line<'static>> = vec![
            "A local workspace draft was not closed.".into(),
            format!(
                "Checkpoint r{} · {} · {}",
                checkpoint.revision, note_scope, checkpoint.trigger
            )
            .into(),
            "".into(),
            "R  Restore this exact local draft".into(),
            "D  Discard this checkpoint session".into(),
            "".into(),
            "Canonical chart data is unchanged until Ctrl-S saves."
                .dim()
                .into(),
            "Restore is blocked if the patient or note baseline changed."
                .dim()
                .into(),
        ];
        if self.status.starts_with("Restore blocked") {
            lines.push(self.status.clone().cyan().into());
        }
        Clear.render(overlay, buf);
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" Restore local draft? ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: true })
            .render(overlay, buf);
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

    fn validate_recovery_snapshot(
        &self,
        recovery: &WorkspaceDraftSession,
    ) -> std::result::Result<WorkspaceDraftSnapshotV1, String> {
        let checkpoint = &recovery.current_checkpoint;
        let snapshot: WorkspaceDraftSnapshotV1 = serde_json::from_value(checkpoint.draft.clone())
            .map_err(|error| {
            format!("Restore blocked: checkpoint schema is invalid ({error}).")
        })?;
        if snapshot.schema_version != DRAFT_SCHEMA_VERSION {
            return Err("Restore blocked: checkpoint schema version is unsupported.".to_string());
        }
        let Some(canonical_client) = self.clients.get(self.client_index) else {
            return Err("Restore blocked: saved patient is no longer loaded.".to_string());
        };
        if snapshot.client.id.as_deref() != Some(canonical_client.id.as_str())
            || checkpoint.client_id != canonical_client.id
            || snapshot.base_client_version != canonical_client.version
        {
            return Err(
                "Restore blocked: canonical patient data changed; discard or reload before editing."
                    .to_string(),
            );
        }
        if snapshot.note.id != checkpoint.note_id
            || snapshot.note.encounter_id != checkpoint.encounter_id
        {
            return Err("Restore blocked: checkpoint note scope is inconsistent.".to_string());
        }
        match snapshot.note.id.as_deref() {
            Some(note_id) => {
                let Some(canonical_note) = self.notes.iter().find(|note| note.id == note_id) else {
                    return Err(
                        "Restore blocked: canonical note is no longer available.".to_string()
                    );
                };
                if checkpoint.base_note_revision != Some(canonical_note.current_revision)
                    || snapshot.note.current_revision != canonical_note.current_revision
                    || snapshot.note.status != canonical_note.status
                {
                    return Err(
                        "Restore blocked: canonical note revision changed; no draft was merged."
                            .to_string(),
                    );
                }
            }
            None if checkpoint.base_note_revision.is_some() => {
                return Err("Restore blocked: new-note checkpoint has a note revision.".to_string());
            }
            None => {}
        }
        if let Some(encounter_id) = snapshot.note.encounter_id.as_deref()
            && !self
                .encounters
                .iter()
                .any(|encounter| encounter.id == encounter_id)
        {
            return Err(
                "Restore blocked: checkpoint encounter is no longer available.".to_string(),
            );
        }
        Ok(snapshot)
    }

    fn apply_recovery_snapshot(&mut self, snapshot: WorkspaceDraftSnapshotV1) {
        self.draft_client = snapshot.client;
        self.draft_note = snapshot.note;
        self.note_index = self
            .draft_note
            .id
            .as_deref()
            .and_then(|id| self.notes.iter().position(|note| note.id == id))
            .unwrap_or(self.notes.len());
        self.focus = snapshot.focus.workspace_focus();
        self.select_encounter_for_active_note();
        self.pending_chart_changeset = None;
        self.next_chart_save_purpose = ChartChangesetPurpose::General;
        self.addendum_draft.clear();
        self.dirty = true;
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

    fn workspace_focus(self) -> WorkspaceFocus {
        match self {
            Self::Demographics => WorkspaceFocus::Demographics,
            Self::NoteTitle => WorkspaceFocus::NoteTitle,
            Self::NoteBody => WorkspaceFocus::NoteBody,
            Self::Workflow => WorkspaceFocus::Workflow,
        }
    }
}

#[cfg(test)]
#[path = "draft_checkpoint_tests.rs"]
mod tests;
