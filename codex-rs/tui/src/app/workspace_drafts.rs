use super::*;
use crate::workspace_dashboard::DashboardCheckpointOutcome;
use crate::workspace_draft::WorkspaceDraftCheckpointTrigger;

impl App {
    pub(super) async fn checkpoint_workspace_draft(
        &mut self,
        app_server: &mut AppServerSession,
        trigger: WorkspaceDraftCheckpointTrigger,
    ) -> Result<DashboardCheckpointOutcome> {
        let Some(dashboard) = self.workspace_dashboard.as_mut() else {
            return Ok(DashboardCheckpointOutcome::AlreadyCurrent);
        };
        dashboard.checkpoint_draft(app_server, trigger).await
    }

    pub(super) async fn checkpoint_idle_workspace_draft(
        &mut self,
        app_server: &mut AppServerSession,
    ) {
        let result = if let Some(dashboard) = self.workspace_dashboard.as_mut() {
            dashboard.checkpoint_idle_draft_if_due(app_server).await
        } else {
            Ok(())
        };
        if let Err(error) = result
            && let Some(dashboard) = self.workspace_dashboard.as_mut()
        {
            dashboard.set_status(format!(
                    "Local draft checkpoint failed; canonical chart unchanged. Retrying after idle: {error}"
                ));
        }
    }

    pub(super) async fn discard_workspace_recovery(&mut self, app_server: &mut AppServerSession) {
        let result = if let Some(dashboard) = self.workspace_dashboard.as_mut() {
            dashboard.discard_pending_draft(app_server).await
        } else {
            Ok(())
        };
        if let Err(error) = result {
            self.chat_widget.add_error_message(format!(
                "Failed to discard workspace draft checkpoint: {error}"
            ));
        }
    }

    pub(super) async fn restore_workspace_recovery(&mut self, app_server: &mut AppServerSession) {
        let result = if let Some(dashboard) = self.workspace_dashboard.as_mut() {
            dashboard.restore_pending_draft(app_server).await
        } else {
            Ok(())
        };
        if let Err(error) = result {
            self.chat_widget.add_error_message(format!(
                "Failed to restore workspace draft checkpoint: {error}"
            ));
        }
    }

    pub(super) fn schedule_workspace_draft_checkpoint(&self, tui: &tui::Tui) {
        if let Some(delay) = self
            .workspace_dashboard
            .as_ref()
            .and_then(WorkspaceDashboard::draft_checkpoint_pending_delay)
        {
            tui.frame_requester().schedule_frame_in(delay);
        }
    }

    pub(super) async fn close_workspace_with_checkpoint(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
    ) {
        match self
            .checkpoint_workspace_draft(app_server, WorkspaceDraftCheckpointTrigger::Close)
            .await
        {
            Ok(_) => self.hide_workspace_dashboard_and_leave_alt_screen(tui),
            Err(error) => self.chat_widget.add_error_message(format!(
                "Workspace remains open because its draft checkpoint failed: {error}"
            )),
        }
    }

    pub(super) async fn send_workspace_context_after_checkpoint(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
    ) {
        match self
            .checkpoint_workspace_draft(app_server, WorkspaceDraftCheckpointTrigger::Handoff)
            .await
        {
            Ok(outcome) if outcome.permits_handoff() => {
                let was_visible = self.workspace_dashboard_active();
                if let Err(error) = self.send_workspace_context_to_agent(app_server).await {
                    self.chat_widget
                        .add_error_message(format!("Failed to send workspace context: {error}"));
                } else if was_visible && !self.workspace_dashboard_active() {
                    let _ = tui.leave_alt_screen();
                }
            }
            Ok(_) => {}
            Err(error) => self.chat_widget.add_error_message(format!(
                "Agent handoff blocked because the local draft checkpoint failed: {error}"
            )),
        }
    }

    pub(super) async fn save_workspace_with_checkpoint(
        &mut self,
        app_server: &mut AppServerSession,
    ) {
        if let Err(error) = self
            .checkpoint_workspace_draft(app_server, WorkspaceDraftCheckpointTrigger::ExplicitSave)
            .await
        {
            self.chat_widget.add_error_message(format!(
                "Local draft checkpoint failed before canonical save: {error}"
            ));
        }
        let result = if let Some(dashboard) = self.workspace_dashboard.as_mut() {
            dashboard.save(app_server).await
        } else {
            Ok(())
        };
        if let Err(error) = result {
            self.chat_widget
                .add_error_message(format!("Failed to save workspace: {error}"));
            return;
        }
        if !self
            .workspace_dashboard
            .as_ref()
            .is_some_and(WorkspaceDashboard::canonical_save_completed)
        {
            return;
        }
        let close_result = if let Some(dashboard) = self.workspace_dashboard.as_mut() {
            dashboard.close_draft_after_canonical_save(app_server).await
        } else {
            Ok(())
        };
        if let Err(error) = close_result {
            self.chat_widget.add_error_message(format!(
                "Canonical chart saved, but the draft checkpoint session could not close: {error}"
            ));
        }
    }

    pub(super) async fn checkpoint_workspace_focus_change(
        &mut self,
        app_server: &mut AppServerSession,
    ) {
        let requested = self
            .workspace_dashboard
            .as_mut()
            .is_some_and(WorkspaceDashboard::take_focus_checkpoint_request);
        if requested
            && let Err(error) = self
                .checkpoint_workspace_draft(
                    app_server,
                    WorkspaceDraftCheckpointTrigger::FocusChange,
                )
                .await
        {
            self.chat_widget
                .add_error_message(format!("Failed to checkpoint workspace draft: {error}"));
        }
    }
}
