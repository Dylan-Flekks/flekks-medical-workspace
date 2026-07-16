//! Binds a submitted medical context packet to the next matching model turn.
//!
//! Core commits the exact bound model output atomically as review-pending Agent Review before the
//! response can reach the TUI. This module only follows that durable result into the dashboard; it
//! never re-captures or attributes caller-supplied text to the agent.

use super::*;
use codex_app_server_protocol::UserInput;
use codex_app_server_protocol::WorkspaceAgentResultListParams;
use codex_app_server_protocol::WorkspaceAgentRun;
use codex_app_server_protocol::WorkspaceAgentRunListParams;
use codex_app_server_protocol::WorkspaceAgentRunStatusUpdateParams;
use codex_app_server_protocol::WorkspaceContextPacket;
use codex_app_server_protocol::WorkspaceContextPacketListParams;
use codex_app_server_protocol::WorkspacePlanRevision;
use codex_app_server_protocol::WorkspacePlanRevisionStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingWorkspaceAgentCapture {
    packet_id: String,
    run_id: String,
    client_id: String,
    note_id: Option<String>,
    expected_prompt: String,
    model: String,
    expected_thread_id: String,
    handoff_authorized: bool,
    handoff_enqueued: bool,
    bound_thread_id: Option<String>,
    bound_turn_id: Option<String>,
}

impl PendingWorkspaceAgentCapture {
    pub(super) fn try_new(
        packet: &WorkspaceContextPacket,
        run: &WorkspaceAgentRun,
        expected_prompt: String,
    ) -> std::result::Result<Self, &'static str> {
        if packet.workspace_plan_revision_id != run.workspace_plan_revision_id
            || packet.workspace_plan_content_sha256 != run.workspace_plan_content_sha256
            || packet.workspace_plan_evidence_manifest_sha256
                != run.workspace_plan_evidence_manifest_sha256
        {
            return Err("medical agent run plan binding does not match its context packet");
        }
        let expected_thread_id = run
            .source_thread_id
            .clone()
            .ok_or("medical agent run is missing its source thread")?;
        let model = run
            .model
            .clone()
            .ok_or("medical agent run is missing its model")?;
        Ok(Self {
            packet_id: packet.id.clone(),
            run_id: run.id.clone(),
            client_id: packet.client_id.clone(),
            note_id: packet.note_id.clone(),
            expected_prompt,
            model,
            expected_thread_id,
            handoff_authorized: false,
            handoff_enqueued: false,
            bound_thread_id: None,
            bound_turn_id: None,
        })
    }

    pub(super) fn thread_is_allowed(&self, thread_id: &str) -> bool {
        self.expected_thread_id == thread_id
    }

    pub(super) fn authorize_handoff(&mut self) {
        self.handoff_authorized = true;
    }

    pub(super) fn handoff_is_authorized(&self) -> bool {
        self.handoff_authorized
    }

    pub(super) fn mark_handoff_enqueued(&mut self) {
        self.handoff_enqueued = true;
    }

    pub(super) fn handoff_is_enqueued(&self) -> bool {
        self.handoff_enqueued
    }

    pub(super) fn expected_prompt(&self) -> &str {
        &self.expected_prompt
    }

    fn turn_is_bound(&self, thread_id: &str, turn_id: &str) -> bool {
        self.bound_thread_id.as_deref() == Some(thread_id)
            && self.bound_turn_id.as_deref() == Some(turn_id)
    }

    pub(super) fn run_id(&self) -> &str {
        &self.run_id
    }

    pub(super) fn submission_matches(&self, content: &[UserInput]) -> bool {
        matches!(
            content,
            [UserInput::Text {
                text,
                text_elements,
            }] if text == &self.expected_prompt && text_elements.is_empty()
        )
    }

    pub(super) fn model_matches(&self, model: &str) -> bool {
        self.model == model
    }

    fn observe_item(&mut self, thread_id: &str, turn_id: &str, item: &ThreadItem) {
        if !self.thread_is_allowed(thread_id) {
            return;
        }
        if let ThreadItem::UserMessage { content, .. } = item
            && self.submission_matches(content)
        {
            self.bound_thread_id = Some(thread_id.to_string());
            self.bound_turn_id = Some(turn_id.to_string());
        }
    }

    fn is_mismatched_submitted_user_message(&self, thread_id: &str, item: &ThreadItem) -> bool {
        self.thread_is_allowed(thread_id)
            && matches!(
                item,
                ThreadItem::UserMessage { content, .. }
                    if !self.submission_matches(content)
            )
    }
}

#[derive(Debug)]
enum WorkspaceAgentCaptureOutcome {
    Completed(PendingWorkspaceAgentCapture),
    Failed {
        pending: PendingWorkspaceAgentCapture,
        status: &'static str,
        error_summary: String,
    },
}

impl App {
    /// Rebuilds the in-memory capture for the one durable submitted Plan handoff.
    ///
    /// A process may exit after the immutable Plan receipt commits but before the user turn is
    /// enqueued. The receipt intentionally forbids replacement packet/run pairs, so recovery must
    /// reuse this exact unclaimed run and its canonical prompt.
    pub(super) async fn recover_submitted_workspace_agent_capture(
        &mut self,
        app_server: &mut AppServerSession,
        revision: &WorkspacePlanRevision,
        expected_thread_id: &str,
        expected_model: &str,
    ) -> Result<Option<String>> {
        if revision.status != WorkspacePlanRevisionStatus::Submitted {
            return Ok(None);
        }
        if self.pending_workspace_agent_capture.is_some() {
            color_eyre::eyre::bail!(
                "cannot recover a submitted medical handoff while another capture is pending"
            );
        }

        let receipt = self
            .workspace_dashboard
            .as_ref()
            .and_then(|dashboard| dashboard.submission_receipt_for_plan_revision(&revision.id))
            .ok_or_else(|| {
                color_eyre::eyre::eyre!(
                    "submitted medical Plan `{}` is missing its immutable submission receipt",
                    revision.id
                )
            })?;
        if receipt.plan_session_id != revision.plan_session_id
            || receipt.client_id != revision.client_id
            || receipt.plan_content_sha256 != revision.content_sha256
            || receipt.evidence_manifest_sha256 != revision.evidence_manifest_sha256
        {
            color_eyre::eyre::bail!(
                "submitted medical Plan `{}` does not match its immutable submission receipt",
                revision.id
            );
        }

        let runs = app_server
            .workspace_agent_run_list(WorkspaceAgentRunListParams {
                client_id: revision.client_id.clone(),
                note_id: revision.note_id.clone(),
                packet_id: Some(receipt.packet_id.clone()),
                limit: Some(100),
            })
            .await?
            .runs;
        let matching_runs = runs
            .iter()
            .filter(|run| {
                run.id == receipt.agent_run_id
                    && run.packet_id == receipt.packet_id
                    && run.run_kind == "agent"
                    && run.workspace_plan_revision_id.as_deref() == Some(revision.id.as_str())
                    && run.workspace_plan_content_sha256.as_deref()
                        == Some(receipt.plan_content_sha256.as_str())
                    && run.workspace_plan_evidence_manifest_sha256.as_deref()
                        == Some(receipt.evidence_manifest_sha256.as_str())
            })
            .collect::<Vec<_>>();
        let [run] = matching_runs.as_slice() else {
            color_eyre::eyre::bail!(
                "submitted medical Plan `{}` must resolve its exact receipt-bound master-agent run; found {}",
                revision.id,
                matching_runs.len()
            );
        };
        if run.status != "running" {
            color_eyre::eyre::bail!(
                "submitted medical Plan `{}` is bound to a `{}` master-agent run; publish a new Plan revision before retrying",
                revision.id,
                run.status
            );
        }
        if run.source_turn_id.is_some() {
            color_eyre::eyre::bail!(
                "submitted medical Plan `{}` has already been claimed by its master-agent turn",
                revision.id
            );
        }
        if run.source_thread_id.as_deref() != Some(expected_thread_id) {
            color_eyre::eyre::bail!(
                "submitted medical Plan `{}` belongs to a different master-agent thread",
                revision.id
            );
        }
        if run.model.as_deref() != Some(expected_model) {
            color_eyre::eyre::bail!(
                "submitted medical Plan `{}` belongs to model `{}` rather than `{expected_model}`",
                revision.id,
                run.model.as_deref().unwrap_or("unknown")
            );
        }

        let packet = app_server
            .workspace_context_packet_list(WorkspaceContextPacketListParams {
                client_id: revision.client_id.clone(),
                note_id: revision.note_id.clone(),
                limit: Some(100),
            })
            .await?
            .packets
            .into_iter()
            .find(|packet| packet.id == receipt.packet_id)
            .ok_or_else(|| {
                color_eyre::eyre::eyre!(
                    "submitted medical Plan `{}` is missing its exact context packet `{}`",
                    revision.id,
                    receipt.packet_id
                )
            })?;
        if packet.status != "submitted" {
            color_eyre::eyre::bail!(
                "submitted medical Plan `{}` has a context packet in unexpected `{}` state",
                revision.id,
                packet.status
            );
        }
        let prompt = packet_scoped_agent_handoff_prompt_for_run(&packet, Some(run.id.as_str()));
        let mut pending = PendingWorkspaceAgentCapture::try_new(&packet, run, prompt.clone())
            .map_err(|message| color_eyre::eyre::eyre!(message))?;
        if !pending.thread_is_allowed(expected_thread_id) || !pending.model_matches(expected_model)
        {
            color_eyre::eyre::bail!(
                "recovered medical handoff provenance does not match the active thread and model"
            );
        }
        pending.authorize_handoff();
        self.pending_workspace_agent_capture = Some(pending);
        if let Some(dashboard) = self.workspace_dashboard.as_mut() {
            dashboard.mark_agent_run_started((*run).clone());
            dashboard.mark_agent_context_sent(packet);
            dashboard.set_status(
                "Recovered the exact submitted Plan handoff. Ctrl-G is retrying its unclaimed master-agent turn.",
            );
        }
        Ok(Some(prompt))
    }

    pub(super) async fn handle_workspace_agent_capture_event(
        &mut self,
        app_server: &mut AppServerSession,
        event: &ThreadBufferedEvent,
    ) {
        let Some(outcome) = self.workspace_agent_capture_outcome(event) else {
            return;
        };
        match outcome {
            WorkspaceAgentCaptureOutcome::Completed(pending) => {
                let response = app_server
                    .workspace_agent_result_list(WorkspaceAgentResultListParams {
                        client_id: pending.client_id.clone(),
                        note_id: pending.note_id.clone(),
                        packet_id: Some(pending.packet_id.clone()),
                        limit: Some(100),
                    })
                    .await;
                match response {
                    Ok(response) => {
                        let Some(result) = response.results.into_iter().find(|result| {
                            result.run_id.as_deref() == Some(pending.run_id.as_str())
                        }) else {
                            self.chat_widget.add_error_message(
                                "The medical agent turn completed without its required durable result. No response was attributed to the agent."
                                    .to_string(),
                            );
                            return;
                        };
                        if let Some(dashboard) = self.workspace_dashboard.as_mut()
                            && let Err(err) = dashboard
                                .refresh_after_agent_capture(app_server, result.id.clone())
                                .await
                        {
                            tracing::warn!(
                                error = %err,
                                "saved medical agent result but failed to refresh Agent Review"
                            );
                        }
                        self.chat_widget.add_info_message(
                            "Medical agent response saved as review-pending Agent Review."
                                .to_string(),
                            Some("Reopen /workspace-medical to compare or reject it.".to_string()),
                        );
                    }
                    Err(err) => {
                        self.chat_widget.add_error_message(format!(
                            "The medical agent response is durable, but Agent Review could not refresh: {err}. Reopen /workspace-medical to retry the read."
                        ));
                    }
                }
            }
            WorkspaceAgentCaptureOutcome::Failed {
                pending,
                status,
                error_summary,
            } => {
                let run_status_updated = match app_server
                    .workspace_agent_run_status_update(WorkspaceAgentRunStatusUpdateParams {
                        run_id: pending.run_id,
                        status: status.to_string(),
                        error_summary: Some(error_summary.clone()),
                    })
                    .await
                {
                    Ok(_) => true,
                    Err(err) => {
                        tracing::warn!(error = %err, "failed to close medical agent run");
                        false
                    }
                };
                if run_status_updated
                    && let Some(dashboard) = self.workspace_dashboard.as_mut()
                    && let Err(err) = dashboard
                        .refresh_after_agent_run_ended(app_server, status, &error_summary)
                        .await
                {
                    tracing::warn!(
                        error = %err,
                        "closed medical agent run but failed to refresh Context Plan state"
                    );
                }
                self.chat_widget.add_error_message(format!(
                    "Medical agent run did not produce reviewable work: {error_summary}"
                ));
            }
        }
    }

    fn workspace_agent_capture_outcome(
        &mut self,
        event: &ThreadBufferedEvent,
    ) -> Option<WorkspaceAgentCaptureOutcome> {
        let pending = self.pending_workspace_agent_capture.as_mut()?;
        match event {
            ThreadBufferedEvent::Notification(ServerNotification::ItemStarted(notification)) => {
                if pending.is_mismatched_submitted_user_message(
                    &notification.thread_id,
                    &notification.item,
                ) {
                    let pending = self.pending_workspace_agent_capture.take()?;
                    return Some(WorkspaceAgentCaptureOutcome::Failed {
                        pending,
                        status: "canceled",
                        error_summary: "the submitted user message did not exactly match the generated medical handoff prompt"
                            .to_string(),
                    });
                }
                pending.observe_item(
                    &notification.thread_id,
                    &notification.turn_id,
                    &notification.item,
                );
                None
            }
            ThreadBufferedEvent::Notification(ServerNotification::ItemCompleted(notification)) => {
                if pending.is_mismatched_submitted_user_message(
                    &notification.thread_id,
                    &notification.item,
                ) {
                    let pending = self.pending_workspace_agent_capture.take()?;
                    return Some(WorkspaceAgentCaptureOutcome::Failed {
                        pending,
                        status: "canceled",
                        error_summary: "the submitted user message did not exactly match the generated medical handoff prompt"
                            .to_string(),
                    });
                }
                pending.observe_item(
                    &notification.thread_id,
                    &notification.turn_id,
                    &notification.item,
                );
                None
            }
            ThreadBufferedEvent::Notification(ServerNotification::TurnCompleted(notification)) => {
                for item in &notification.turn.items {
                    pending.observe_item(&notification.thread_id, &notification.turn.id, item);
                }
                if !pending.turn_is_bound(&notification.thread_id, &notification.turn.id) {
                    return None;
                }
                let pending = self.pending_workspace_agent_capture.take()?;
                match notification.turn.status {
                    TurnStatus::Completed => Some(WorkspaceAgentCaptureOutcome::Completed(pending)),
                    TurnStatus::Interrupted => Some(WorkspaceAgentCaptureOutcome::Failed {
                        pending,
                        status: "canceled",
                        error_summary: "agent turn was interrupted".to_string(),
                    }),
                    TurnStatus::Failed | TurnStatus::InProgress => {
                        let error_summary = notification
                            .turn
                            .error
                            .as_ref()
                            .map(|error| error.message.trim())
                            .filter(|message| !message.is_empty())
                            .unwrap_or("agent turn failed")
                            .to_string();
                        Some(WorkspaceAgentCaptureOutcome::Failed {
                            pending,
                            status: "failed",
                            error_summary,
                        })
                    }
                }
            }
            ThreadBufferedEvent::Notification(_)
            | ThreadBufferedEvent::Request(_)
            | ThreadBufferedEvent::HistoryEntryResponse(_)
            | ThreadBufferedEvent::FeedbackSubmission(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending_capture() -> PendingWorkspaceAgentCapture {
        PendingWorkspaceAgentCapture {
            packet_id: "packet-1".to_string(),
            run_id: "run-1".to_string(),
            client_id: "client-1".to_string(),
            note_id: Some("note-1".to_string()),
            expected_prompt: "exact generated prompt".to_string(),
            model: "test-model".to_string(),
            expected_thread_id: "thread-1".to_string(),
            handoff_authorized: true,
            handoff_enqueued: true,
            bound_thread_id: Some("thread-1".to_string()),
            bound_turn_id: Some("turn-1".to_string()),
        }
    }

    #[test]
    fn submission_binding_requires_exact_generated_prompt() {
        let pending = pending_capture();
        let exact = UserInput::Text {
            text: "exact generated prompt".to_string(),
            text_elements: Vec::new(),
        };

        assert!(pending.submission_matches(std::slice::from_ref(&exact)));
        assert!(!pending.submission_matches(&[UserInput::Text {
            text: "exact generated prompt\nappended text".to_string(),
            text_elements: Vec::new(),
        }]));
        assert!(!pending.submission_matches(&[
            exact,
            UserInput::Text {
                text: "second text item".to_string(),
                text_elements: Vec::new(),
            },
        ]));
        assert!(!pending.submission_matches(&[UserInput::Text {
            text: "packet-1 hash-1 run-1".to_string(),
            text_elements: Vec::new(),
        }]));
    }
}
