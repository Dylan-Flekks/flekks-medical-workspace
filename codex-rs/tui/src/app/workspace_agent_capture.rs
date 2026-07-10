//! Binds a submitted medical context packet to the next matching model turn.
//!
//! Captured output is persisted as review-pending Agent Work. This never applies
//! a proposal or mutates the canonical chart.

use super::*;
use codex_app_server_protocol::UserInput;
use codex_app_server_protocol::WorkspaceAgentResultCreateParams;
use codex_app_server_protocol::WorkspaceAgentRun;
use codex_app_server_protocol::WorkspaceAgentRunStatusUpdateParams;
use codex_app_server_protocol::WorkspaceContextPacket;
use codex_protocol::models::MessagePhase;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingWorkspaceAgentCapture {
    packet_id: String,
    run_id: String,
    client_id: String,
    note_id: Option<String>,
    context_envelope_sha256: String,
    expected_thread_id: Option<String>,
    bound_thread_id: Option<String>,
    bound_turn_id: Option<String>,
    last_final_answer: Option<String>,
    last_legacy_message: Option<String>,
}

impl PendingWorkspaceAgentCapture {
    pub(super) fn new(packet: &WorkspaceContextPacket, run: &WorkspaceAgentRun) -> Self {
        Self {
            packet_id: packet.id.clone(),
            run_id: run.id.clone(),
            client_id: packet.client_id.clone(),
            note_id: packet.note_id.clone(),
            context_envelope_sha256: packet.context_envelope_sha256.clone(),
            expected_thread_id: run.source_thread_id.clone(),
            bound_thread_id: None,
            bound_turn_id: None,
            last_final_answer: None,
            last_legacy_message: None,
        }
    }

    fn thread_is_allowed(&self, thread_id: &str) -> bool {
        self.expected_thread_id
            .as_deref()
            .is_none_or(|expected| expected == thread_id)
    }

    fn turn_is_bound(&self, thread_id: &str, turn_id: &str) -> bool {
        self.bound_thread_id.as_deref() == Some(thread_id)
            && self.bound_turn_id.as_deref() == Some(turn_id)
    }

    pub(super) fn run_id(&self) -> &str {
        &self.run_id
    }

    fn reviewable_body(&self) -> Option<String> {
        self.last_final_answer
            .as_deref()
            .or(self.last_legacy_message.as_deref())
            .map(str::trim)
            .filter(|body| !body.is_empty())
            .map(ToString::to_string)
    }

    fn observe_item(&mut self, thread_id: &str, turn_id: &str, item: &ThreadItem) {
        if !self.thread_is_allowed(thread_id) {
            return;
        }
        if let ThreadItem::UserMessage { content, .. } = item
            && user_content_matches_packet(content, &self.packet_id, &self.context_envelope_sha256)
        {
            self.bound_thread_id = Some(thread_id.to_string());
            self.bound_turn_id = Some(turn_id.to_string());
            return;
        }
        if let ThreadItem::AgentMessage { text, phase, .. } = item
            && self.turn_is_bound(thread_id, turn_id)
            && !text.trim().is_empty()
        {
            match phase {
                Some(MessagePhase::FinalAnswer) => {
                    self.last_final_answer = Some(text.trim().to_string());
                }
                None => {
                    self.last_legacy_message = Some(text.trim().to_string());
                }
                Some(MessagePhase::Commentary) => {}
            }
        }
    }

    fn is_mismatched_submitted_user_message(&self, thread_id: &str, item: &ThreadItem) -> bool {
        self.thread_is_allowed(thread_id)
            && matches!(
                item,
                ThreadItem::UserMessage { content, .. }
                    if !user_content_matches_packet(
                        content,
                        &self.packet_id,
                        &self.context_envelope_sha256,
                    )
            )
    }
}

#[derive(Debug)]
enum WorkspaceAgentCaptureOutcome {
    Completed {
        pending: PendingWorkspaceAgentCapture,
        body: String,
    },
    Failed {
        pending: PendingWorkspaceAgentCapture,
        status: &'static str,
        error_summary: String,
    },
}

impl App {
    pub(super) async fn handle_workspace_agent_capture_event(
        &mut self,
        app_server: &mut AppServerSession,
        event: &ThreadBufferedEvent,
    ) {
        let Some(outcome) = self.workspace_agent_capture_outcome(event) else {
            return;
        };
        match outcome {
            WorkspaceAgentCaptureOutcome::Completed { pending, body } => {
                let response = app_server
                    .workspace_agent_result_create(WorkspaceAgentResultCreateParams {
                        packet_id: pending.packet_id.clone(),
                        run_id: Some(pending.run_id.clone()),
                        source_thread_id: pending.bound_thread_id.clone(),
                        source_turn_id: pending.bound_turn_id.clone(),
                        body,
                        summary: None,
                        client_id: Some(pending.client_id.clone()),
                        note_id: pending.note_id.clone(),
                        context_envelope_sha256: Some(pending.context_envelope_sha256.clone()),
                        result_kind: Some("note_proposal".to_string()),
                        structured_changes_json: None,
                        rationale_summary: None,
                    })
                    .await;
                match response {
                    Ok(response) => {
                        if let Some(dashboard) = self.workspace_dashboard.as_mut()
                            && let Err(err) = dashboard
                                .refresh_after_agent_capture(app_server, response.result.id.clone())
                                .await
                        {
                            tracing::warn!(
                                error = %err,
                                "saved medical agent result but failed to refresh Agent Work"
                            );
                        }
                        self.chat_widget.add_info_message(
                            "Medical agent response saved as review-pending Agent Work."
                                .to_string(),
                            Some("Reopen /workspacemedical to compare or reject it.".to_string()),
                        );
                    }
                    Err(err) => {
                        self.pending_workspace_agent_capture = Some(pending);
                        self.chat_widget.add_error_message(format!(
                            "Failed to save completed medical agent response: {err}. Use :agent result save as a fallback."
                        ));
                    }
                }
            }
            WorkspaceAgentCaptureOutcome::Failed {
                pending,
                status,
                error_summary,
            } => {
                if let Err(err) = app_server
                    .workspace_agent_run_status_update(WorkspaceAgentRunStatusUpdateParams {
                        run_id: pending.run_id,
                        status: status.to_string(),
                        error_summary: Some(error_summary.clone()),
                    })
                    .await
                {
                    tracing::warn!(error = %err, "failed to close medical agent run");
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
                        error_summary:
                            "the submitted user message did not contain the prepared packet id and hash"
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
                        error_summary:
                            "the submitted user message did not contain the prepared packet id and hash"
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
                    TurnStatus::Completed => match pending.reviewable_body() {
                        Some(body) if !body.trim().is_empty() => {
                            Some(WorkspaceAgentCaptureOutcome::Completed { pending, body })
                        }
                        _ => Some(WorkspaceAgentCaptureOutcome::Failed {
                            pending,
                            status: "failed",
                            error_summary: "turn completed without a final agent message"
                                .to_string(),
                        }),
                    },
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

fn user_content_matches_packet(content: &[UserInput], packet_id: &str, packet_hash: &str) -> bool {
    let mut has_packet_id = false;
    let mut has_packet_hash = false;
    for input in content {
        if let UserInput::Text { text, .. } = input {
            has_packet_id |= text.contains(packet_id);
            has_packet_hash |= text.contains(packet_hash);
        }
    }
    has_packet_id && has_packet_hash
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
            context_envelope_sha256: "hash-1".to_string(),
            expected_thread_id: Some("thread-1".to_string()),
            bound_thread_id: Some("thread-1".to_string()),
            bound_turn_id: Some("turn-1".to_string()),
            last_final_answer: None,
            last_legacy_message: None,
        }
    }

    fn agent_message(text: &str, phase: Option<MessagePhase>) -> ThreadItem {
        ThreadItem::AgentMessage {
            id: format!("message-{text}"),
            text: text.to_string(),
            phase,
            memory_citation: None,
        }
    }

    #[test]
    fn final_answer_wins_over_commentary_and_legacy_output() {
        let mut pending = pending_capture();
        pending.observe_item(
            "thread-1",
            "turn-1",
            &agent_message("legacy fallback", None),
        );
        pending.observe_item(
            "thread-1",
            "turn-1",
            &agent_message("checking the chart", Some(MessagePhase::Commentary)),
        );
        pending.observe_item(
            "thread-1",
            "turn-1",
            &agent_message("reviewable proposal", Some(MessagePhase::FinalAnswer)),
        );
        pending.observe_item(
            "thread-1",
            "turn-1",
            &agent_message("later commentary", Some(MessagePhase::Commentary)),
        );

        assert_eq!(
            pending.reviewable_body().as_deref(),
            Some("reviewable proposal")
        );
    }

    #[test]
    fn commentary_only_turn_has_no_reviewable_body() {
        let mut pending = pending_capture();
        pending.observe_item(
            "thread-1",
            "turn-1",
            &agent_message("still working", Some(MessagePhase::Commentary)),
        );

        assert_eq!(pending.reviewable_body(), None);
    }
}
