//! Chat widget hooks for side-conversation mode.
//!
//! App-level side-thread lifecycle lives in `app::side`; this module owns the
//! chat-surface pieces that side mode toggles, such as the composer placeholder,
//! footer label, and inline `/side` message submission behavior.

use super::*;

impl ChatWidget {
    pub(crate) fn submit_user_message_as_plain_user_turn(
        &mut self,
        user_message: UserMessage,
    ) -> Option<AppCommand> {
        self.submit_user_message_with_shell_escape_policy(user_message, ShellEscapePolicy::Disallow)
    }

    /// Submit an application-generated, audited prompt without applying any of the normal
    /// composer enrichments.
    ///
    /// Medical workspace handoffs persist the exact prompt that the agent is expected to receive.
    /// Running that prompt through mention, skill, connector, or IDE-context discovery would make
    /// the submitted turn differ from the persisted audit record. Keep this path text-only while
    /// leaving normal user and side-conversation submission behavior unchanged.
    pub(crate) fn can_accept_audited_text_as_plain_user_turn(&self, text: &str) -> bool {
        self.is_session_configured()
            && !text.is_empty()
            && !self
                .effective_collaboration_mode()
                .model()
                .trim()
                .is_empty()
    }

    pub(crate) fn accept_audited_text_as_plain_user_turn(&mut self, text: String) -> bool {
        if !self.can_accept_audited_text_as_plain_user_turn(&text) {
            if self.is_session_configured()
                && !text.is_empty()
                && self
                    .effective_collaboration_mode()
                    .model()
                    .trim()
                    .is_empty()
            {
                self.add_error_message(
                    "Thread model is unavailable. Wait for the thread to finish syncing or choose a model before sending input.".to_string(),
                );
            }
            return false;
        }

        let effective_mode = self.effective_collaboration_mode();
        let render_in_history = !self.turn_lifecycle.agent_turn_running;
        let items = vec![UserInput::Text {
            text: text.clone(),
            text_elements: Vec::new(),
        }];
        let collaboration_mode = if self.collaboration_modes_enabled() {
            self.active_collaboration_mask
                .as_ref()
                .map(|_| effective_mode.clone())
        } else {
            None
        };
        let personality = self
            .config
            .personality
            .filter(|_| self.config.features.enabled(Feature::Personality))
            .filter(|_| self.current_model_supports_personality());
        let service_tier = self.service_tier_update_for_core();
        let active_permission_profile = self.config.permissions.active_permission_profile();
        let op = AppCommand::user_turn(
            items,
            self.config.cwd.to_path_buf(),
            AskForApproval::from(self.config.permissions.approval_policy.value()),
            active_permission_profile,
            effective_mode.model().to_string(),
            effective_mode.reasoning_effort(),
            /*summary*/ None,
            service_tier,
            /*final_output_json_schema*/ None,
            collaboration_mode,
            personality,
        );

        if !self.submit_op(op) {
            return false;
        }
        if render_in_history {
            self.input_queue.user_turn_pending_start = true;
            let user_message = UserMessage::from(text.clone());
            self.record_cancel_edit_candidate(user_message.clone());
            self.on_user_message_display(user_message_display_for_history(
                user_message,
                &UserMessageHistoryRecord::UserMessageText,
            ));
        }
        self.append_message_history_entry(text);
        self.transcript.needs_final_message_separator = false;
        true
    }

    pub(crate) fn agent_turn_is_running(&self) -> bool {
        self.turn_lifecycle.agent_turn_running
    }

    #[cfg(test)]
    pub(crate) fn mark_agent_turn_running_for_tests(&mut self) {
        self.on_task_started();
    }

    pub(crate) fn set_side_conversation_active(&mut self, active: bool) {
        self.active_side_conversation = active;
        let placeholder = if active {
            self.side_placeholder_text.clone()
        } else {
            self.normal_placeholder_text.clone()
        };
        self.bottom_pane.set_placeholder_text(placeholder);
        self.bottom_pane.set_side_conversation_active(active);
    }

    pub(crate) fn side_conversation_active(&self) -> bool {
        self.active_side_conversation
    }

    pub(crate) fn set_side_conversation_context_label(&mut self, label: Option<String>) {
        self.bottom_pane.set_side_conversation_context_label(label);
    }
}
