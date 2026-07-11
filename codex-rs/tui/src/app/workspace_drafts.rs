use super::*;
use crate::workspace_dashboard::DashboardCheckpointOutcome;
use crate::workspace_draft::WorkspaceDraftCheckpointTrigger;

impl App {
    pub(super) fn discard_workspace_dashboard_if_checkpoint_safe(&mut self) -> bool {
        let can_clear = self
            .workspace_dashboard
            .as_ref()
            .is_none_or(WorkspaceDashboard::can_clear_dashboard_checkpoint_safely);
        if can_clear {
            self.workspace_dashboard = None;
            return true;
        }
        if let Some(dashboard) = self.workspace_dashboard.as_mut() {
            dashboard.set_checkpoint_scope_change_blocked_status("closing the workspace");
        }
        self.chat_widget.add_error_message(
            "Workspace remains open until its owned draft checkpoint task finishes.".to_string(),
        );
        false
    }

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
        if let Err(error) = result {
            self.chat_widget.add_error_message(format!(
                "Local draft checkpoint continuation failed and will retry: {error}"
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
        if self
            .workspace_close_checkpoint_permits_hide(app_server)
            .await
        {
            self.hide_workspace_dashboard_and_leave_alt_screen(tui);
        }
    }

    pub(super) async fn workspace_close_checkpoint_permits_hide(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> bool {
        match self
            .checkpoint_workspace_draft(app_server, WorkspaceDraftCheckpointTrigger::Close)
            .await
        {
            Ok(outcome) if outcome.permits_close() => true,
            Ok(_) => {
                self.chat_widget.add_error_message(
                    "Workspace remains open because its local draft is not durably checkpointed."
                        .to_string(),
                );
                false
            }
            Err(error) => {
                self.chat_widget.add_error_message(format!(
                    "Workspace remains open because its draft checkpoint failed: {error}"
                ));
                false
            }
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
                let is_medical = self
                    .workspace_dashboard
                    .as_ref()
                    .is_some_and(|dashboard| dashboard.profile() == WorkspaceProfile::Medical);
                match self.send_workspace_context_to_agent(app_server).await {
                    Err(error) => self
                        .chat_widget
                        .add_error_message(format!("Failed to send workspace context: {error}")),
                    Ok(sent) => {
                        if sent && is_medical {
                            match self
                                .checkpoint_workspace_draft(
                                    app_server,
                                    WorkspaceDraftCheckpointTrigger::HandoffCleared,
                                )
                                .await
                            {
                                Ok(DashboardCheckpointOutcome::Saved)
                                | Ok(DashboardCheckpointOutcome::AlreadyCurrent) => {
                                    if let Some(dashboard) = self.workspace_dashboard.as_mut() {
                                        dashboard.set_status(
                                            "Medical Agent Plan sent; cleared local request checkpoint saved and the draft session remains open.",
                                        );
                                    }
                                }
                                Ok(DashboardCheckpointOutcome::Pending) => {
                                    if let Some(dashboard) = self.workspace_dashboard.as_mut() {
                                        dashboard.set_status(
                                            "Medical Agent Plan sent; cleared local request checkpoint is still saving and the draft session remains open.",
                                        );
                                    }
                                }
                                Ok(_) => {
                                    if let Some(dashboard) = self.workspace_dashboard.as_mut() {
                                        dashboard.set_status(
                                            "Medical Agent Plan sent; cleared local request was not checkpointed and the draft session remains open.",
                                        );
                                    }
                                    self.chat_widget.add_error_message(
                                        "Medical Agent Plan was sent, but its cleared request checkpoint is unavailable and will retry."
                                            .to_string(),
                                    );
                                }
                                Err(error) => {
                                    if let Some(dashboard) = self.workspace_dashboard.as_mut() {
                                        dashboard.set_status(
                                            "Medical Agent Plan sent; cleared local request checkpoint failed and will retry while the draft session remains open.",
                                        );
                                    }
                                    self.chat_widget.add_error_message(format!(
                                        "Medical Agent Plan was sent, but its cleared request checkpoint failed and will retry: {error}"
                                    ));
                                }
                            }
                        }
                        if was_visible && !self.workspace_dashboard_active() {
                            let _ = tui.leave_alt_screen();
                        }
                    }
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
        let checkpoint_outcome = match self
            .checkpoint_workspace_draft(app_server, WorkspaceDraftCheckpointTrigger::ExplicitSave)
            .await
        {
            Ok(outcome) if outcome.permits_canonical_save() => outcome,
            Ok(_) => {
                self.chat_widget.add_error_message(
                    "Canonical save blocked because the local draft checkpoint is unavailable or still pending."
                        .to_string(),
                );
                return;
            }
            Err(error) => {
                self.chat_widget.add_error_message(format!(
                    "Canonical save blocked because the local draft checkpoint failed: {error}"
                ));
                return;
            }
        };
        let pre_save_generation = self
            .workspace_dashboard
            .as_ref()
            .map(WorkspaceDashboard::draft_checkpoint_generation)
            .unwrap_or_default();
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
        if checkpoint_outcome == DashboardCheckpointOutcome::CanonicalOnly
            && let Some(dashboard) = self.workspace_dashboard.as_mut()
        {
            dashboard.acknowledge_canonical_only_save_through(pre_save_generation);
        }

        let retain_for_context = self
            .workspace_dashboard
            .as_ref()
            .is_some_and(WorkspaceDashboard::has_unsent_checkpoint_context);
        let post_save_checkpoint_needed = checkpoint_outcome
            == DashboardCheckpointOutcome::CanonicalBootstrap
            || retain_for_context
            || self
                .workspace_dashboard
                .as_ref()
                .is_some_and(WorkspaceDashboard::has_uncheckpointed_draft_edits);
        if post_save_checkpoint_needed {
            if !retain_for_context && let Some(dashboard) = self.workspace_dashboard.as_mut() {
                dashboard.mark_canonical_save_pending_close();
            }
            let post_save_outcome = self
                .checkpoint_workspace_draft(
                    app_server,
                    WorkspaceDraftCheckpointTrigger::PostCanonicalSave,
                )
                .await;
            if retain_for_context {
                match post_save_outcome {
                    Ok(outcome) => {
                        if let Some(dashboard) = self.workspace_dashboard.as_mut() {
                            dashboard.set_post_canonical_context_status(outcome);
                        }
                    }
                    Err(error) => {
                        if let Some(dashboard) = self.workspace_dashboard.as_mut() {
                            dashboard.set_status(
                                "Canonical chart saved; agent context checkpoint failed and will retry while the draft session remains open.",
                            );
                        }
                        self.chat_widget.add_error_message(format!(
                            "Canonical chart saved, but its agent context checkpoint failed; the draft session remains open and will retry safely: {error}"
                        ));
                    }
                }
                return;
            }
            match post_save_outcome {
                Ok(outcome) if outcome.permits_close() => {}
                Ok(_) => {
                    self.chat_widget.add_error_message(
                        "Canonical chart saved, but its local draft checkpoint is still pending; the draft session remains open and will continue in the background."
                            .to_string(),
                    );
                    return;
                }
                Err(error) => {
                    self.chat_widget.add_error_message(format!(
                        "Canonical chart saved, but its local draft checkpoint failed; the draft session remains open and will retry safely: {error}"
                    ));
                    return;
                }
            }
        }
        if let Some(dashboard) = self.workspace_dashboard.as_mut() {
            dashboard.arm_canonical_close_if_confirmed();
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
