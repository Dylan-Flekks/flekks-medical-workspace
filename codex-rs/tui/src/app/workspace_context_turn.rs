//! Tool-isolation guard for medical context handoffs.
//!
//! A pending medical capture owns the next user turn. The guard runs at the shared app-server
//! submission boundary so direct composer input, queued input, and targeted thread submissions
//! all receive the same one-turn tool restriction.

use super::*;
use codex_protocol::config_types::ModelToolMode;

pub(super) const WORKSPACE_CONTEXT_ACTIVE_TURN_MESSAGE: &str = "Medical context handoff was held because another turn is active. Retry it after the current turn finishes.";
pub(super) const WORKSPACE_CONTEXT_UNAUDITED_INPUT_MESSAGE: &str = "Medical context handoff was held: remove inline attachments, skills, and mentions, then select audited files in the context packet.";
pub(super) const WORKSPACE_CONTEXT_BINDING_MISMATCH_MESSAGE: &str = "Medical context handoff was held because its generated prompt changed. Return to the medical workspace and prepare the handoff again.";
pub(super) const WORKSPACE_CONTEXT_NO_ACTIVE_THREAD_MESSAGE: &str = "Medical context handoff needs an active Codex thread. Open or resume a thread, then submit the packet again.";
pub(super) const WORKSPACE_CONTEXT_COMPOSER_NOT_EMPTY_MESSAGE: &str = "Medical context handoff was not prepared because the Codex composer already contains unrelated text. Send or clear that draft, then submit the packet again.";
pub(super) const WORKSPACE_CONTEXT_STRUCTURED_OUTPUT_MESSAGE: &str = "Medical context handoff was held because structured output is enabled. Clear the output schema, then submit the packet again.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum WorkspaceContextTurnRoute {
    Start,
    Steer(String),
    HoldRestricted,
}

pub(super) fn workspace_context_turn_route(
    model_tool_mode: Option<ModelToolMode>,
    active_turn_id: Option<String>,
) -> WorkspaceContextTurnRoute {
    match active_turn_id {
        Some(_) if model_tool_mode == Some(ModelToolMode::WorkspaceContextOnly) => {
            WorkspaceContextTurnRoute::HoldRestricted
        }
        Some(turn_id) => WorkspaceContextTurnRoute::Steer(turn_id),
        None => WorkspaceContextTurnRoute::Start,
    }
}

impl App {
    pub(super) fn prepare_workspace_context_turn_submission(
        &mut self,
        thread_id: ThreadId,
        mut op: AppCommand,
    ) -> Option<AppCommand> {
        let Some(pending_capture) = self.pending_workspace_agent_capture.as_ref() else {
            return Some(op);
        };
        if !matches!(
            &op,
            AppCommand::UserTurn { .. }
                | AppCommand::RunUserShellCommand { .. }
                | AppCommand::Review { .. }
                | AppCommand::Compact
        ) {
            return Some(op);
        }
        if !pending_capture.thread_is_allowed(&thread_id.to_string()) {
            self.preserve_workspace_context_rejected_op(&op);
            self.chat_widget
                .add_error_message(WORKSPACE_CONTEXT_BINDING_MISMATCH_MESSAGE.to_string());
            return None;
        }
        let AppCommand::UserTurn {
            items,
            model,
            final_output_json_schema,
            ..
        } = &op
        else {
            match op {
                AppCommand::RunUserShellCommand { .. } => {
                    self.preserve_workspace_context_rejected_op(&op);
                    self.chat_widget
                        .add_error_message(WORKSPACE_CONTEXT_BINDING_MISMATCH_MESSAGE.to_string());
                    return None;
                }
                AppCommand::Review { .. } | AppCommand::Compact => {
                    self.chat_widget
                        .add_error_message(WORKSPACE_CONTEXT_BINDING_MISMATCH_MESSAGE.to_string());
                    return None;
                }
                _ => return Some(op),
            }
        };
        if final_output_json_schema.is_some() {
            self.chat_widget.preserve_rejected_user_turn(items);
            self.chat_widget
                .add_error_message(WORKSPACE_CONTEXT_STRUCTURED_OUTPUT_MESSAGE.to_string());
            return None;
        }
        if items
            .iter()
            .any(|item| !matches!(item, codex_app_server_protocol::UserInput::Text { .. }))
        {
            self.chat_widget.preserve_rejected_user_turn(items);
            self.chat_widget
                .add_error_message(WORKSPACE_CONTEXT_UNAUDITED_INPUT_MESSAGE.to_string());
            return None;
        }
        if !pending_capture.submission_matches(items) {
            self.chat_widget.preserve_rejected_user_turn(items);
            self.chat_widget
                .add_error_message(WORKSPACE_CONTEXT_BINDING_MISMATCH_MESSAGE.to_string());
            return None;
        }
        if !pending_capture.model_matches(model) {
            self.chat_widget.preserve_rejected_user_turn(items);
            self.chat_widget
                .add_error_message(WORKSPACE_CONTEXT_BINDING_MISMATCH_MESSAGE.to_string());
            return None;
        }
        op.set_user_turn_model_tool_mode(ModelToolMode::WorkspaceContextOnly);
        Some(op)
    }

    fn preserve_workspace_context_rejected_op(&mut self, op: &AppCommand) {
        match op {
            AppCommand::UserTurn { items, .. } => {
                self.chat_widget.preserve_rejected_user_turn(items);
            }
            AppCommand::RunUserShellCommand { command } => {
                self.chat_widget.preserve_rejected_user_turn(&[
                    codex_app_server_protocol::UserInput::Text {
                        text: format!("!{command}"),
                        text_elements: Vec::new(),
                    },
                ]);
            }
            _ => {}
        }
    }

    pub(super) fn hold_workspace_context_turn(
        &mut self,
        items: &[codex_app_server_protocol::UserInput],
    ) {
        self.chat_widget.preserve_rejected_user_turn(items);
        self.chat_widget
            .add_error_message(WORKSPACE_CONTEXT_ACTIVE_TURN_MESSAGE.to_string());
    }

    pub(super) fn suppress_workspace_context_message_history(&self, _thread_id: ThreadId) -> bool {
        self.pending_workspace_agent_capture.is_some()
    }
}

#[cfg(test)]
#[path = "workspace_context_turn_tests.rs"]
mod tests;
