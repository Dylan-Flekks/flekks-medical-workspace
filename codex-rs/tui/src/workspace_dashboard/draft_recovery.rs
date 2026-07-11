use super::*;
use codex_app_server_protocol::WorkspaceDraftSession;
use codex_app_server_protocol::WorkspaceDraftSessionCloseParams;
use codex_app_server_protocol::WorkspaceDraftSessionCloseStatus;
use codex_app_server_protocol::WorkspaceDraftSessionListParams;
use codex_app_server_protocol::WorkspaceDraftSessionStatus;

mod render;
mod storage;
mod validation;

use storage::RecoveryListScope;
use storage::list_recovery_sessions;
use validation::stage_recovered_dashboard;
use validation::validate_recovery_session_envelope;
use validation::validate_recovery_session_unchanged;

const RECOVERY_ACTOR: &str = "medical workspace TUI";

#[derive(Debug, Clone, PartialEq)]
pub(super) struct DraftRecoveryItem {
    pub(super) session: WorkspaceDraftSession,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct DraftRecoveryState {
    pub(super) items: Vec<DraftRecoveryItem>,
    pub(super) index: usize,
    pub(super) dismissed: bool,
    pub(super) deferred_for_owned_session: bool,
    pub(super) available: bool,
}

impl Default for DraftRecoveryState {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            index: 0,
            dismissed: false,
            deferred_for_owned_session: false,
            available: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DraftRecoveryIdentity {
    session_id: String,
    client_id: String,
    checkpoint_id: String,
    checkpoint_revision: i64,
    checkpoint_sha256: String,
}

impl DraftRecoveryIdentity {
    fn from_session(session: &WorkspaceDraftSession) -> Self {
        Self {
            session_id: session.id.clone(),
            client_id: session.client_id.clone(),
            checkpoint_id: session.current_checkpoint.id.clone(),
            checkpoint_revision: session.current_checkpoint.revision,
            checkpoint_sha256: session.current_checkpoint.content_sha256.clone(),
        }
    }
}

enum RecoveryRefreshOutcome {
    Current(Box<WorkspaceDraftSession>),
    Replaced,
    Removed,
}

impl WorkspaceDashboard {
    pub(crate) async fn discover_draft_recovery(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> Result<()> {
        if self.profile != WorkspaceProfile::Medical {
            return Ok(());
        }
        if app_server.uses_remote_workspace() {
            self.draft_recovery.available = false;
            self.status =
                "Local draft recovery is unavailable through a remote app-server; saved chart data and queued recovery choices are unchanged."
                    .to_string();
            return Ok(());
        }

        let sessions = list_recovery_sessions(app_server, RecoveryListScope::AllActive).await?;
        let owned_session_id = self.draft_coordinator.owned_session_id();
        let items = sessions
            .into_iter()
            .filter(|session| Some(session.id.as_str()) != owned_session_id)
            .map(|session| DraftRecoveryItem { session })
            .collect::<Vec<_>>();
        let deferred_for_owned_session = (owned_session_id.is_some()
            || self.draft_coordinator.scope_change_is_blocked())
            && !items.is_empty();
        self.draft_recovery = DraftRecoveryState {
            items,
            index: 0,
            dismissed: false,
            deferred_for_owned_session,
            available: true,
        };
        if self.draft_recovery.items.is_empty() {
            self.status = "No unfinished local draft sessions found.".to_string();
        } else if deferred_for_owned_session {
            self.status = format!(
                "{} other unfinished local draft session(s) will be reviewed after the current draft closes.",
                self.draft_recovery.items.len()
            );
        } else {
            self.status = format!(
                "{} unfinished local draft session(s) need a restore or discard decision.",
                self.draft_recovery.items.len()
            );
        }
        Ok(())
    }

    pub(crate) async fn prepare_draft_recovery_on_reopen(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> Result<()> {
        if self.profile != WorkspaceProfile::Medical {
            return Ok(());
        }
        if self.draft_coordinator.owned_session_id().is_some()
            || self.draft_coordinator.scope_change_is_blocked()
        {
            self.draft_recovery.deferred_for_owned_session = !self.draft_recovery.items.is_empty();
            if self.draft_recovery.deferred_for_owned_session {
                self.status = format!(
                    "{} other unfinished local draft session(s) will be reviewed after the current draft closes.",
                    self.draft_recovery.items.len()
                );
            }
            return Ok(());
        }
        self.discover_draft_recovery(app_server).await
    }

    pub(super) fn recovery_modal_visible(&self) -> bool {
        self.profile == WorkspaceProfile::Medical
            && self.draft_recovery.available
            && !self.draft_recovery.dismissed
            && !self.draft_recovery.deferred_for_owned_session
            && !self.draft_recovery.items.is_empty()
    }

    pub(super) fn current_recovery_item(&self) -> Option<&DraftRecoveryItem> {
        self.draft_recovery.items.get(self.draft_recovery.index)
    }

    pub(crate) fn set_recovery_action_failed(&mut self, action: &str) {
        self.status = format!(
            "Draft {action} failed; the unfinished draft is retained, canonical chart data is unchanged, and R/D remain available."
        );
    }

    pub(crate) fn set_recovery_discovery_failed(&mut self) {
        self.status =
            "Draft recovery check failed; existing chart data and recovery queue are unchanged. Reopen the workspace to retry."
                .to_string();
    }

    pub(super) fn handle_recovery_key_event(
        &mut self,
        key_event: KeyEvent,
    ) -> WorkspaceDashboardAction {
        if key_event.kind != KeyEventKind::Press
            || !matches!(
                key_event.modifiers,
                KeyModifiers::NONE | KeyModifiers::SHIFT
            )
        {
            self.status =
                "Recovery ignores modified and repeated keys: press R, D, N, P, or Esc once."
                    .to_string();
            return WorkspaceDashboardAction::Consumed;
        }
        match key_event.code {
            KeyCode::Char(character) if character.eq_ignore_ascii_case(&'r') => {
                self.current_recovery_action(true)
            }
            KeyCode::Char(character) if character.eq_ignore_ascii_case(&'d') => {
                self.current_recovery_action(false)
            }
            KeyCode::Char(character) if character.eq_ignore_ascii_case(&'n') => {
                self.move_recovery_selection(1);
                WorkspaceDashboardAction::Consumed
            }
            KeyCode::Char(character) if character.eq_ignore_ascii_case(&'p') => {
                self.move_recovery_selection(-1);
                WorkspaceDashboardAction::Consumed
            }
            KeyCode::Esc | KeyCode::Char('l') | KeyCode::Char('L') | KeyCode::Char(' ') => {
                self.draft_recovery.dismissed = true;
                self.status =
                    "Draft recovery deferred until the workspace is reopened.".to_string();
                WorkspaceDashboardAction::Consumed
            }
            _ => {
                self.status =
                    "Recovery decision required: R restore, D discard, N/P navigate, Esc later."
                        .to_string();
                WorkspaceDashboardAction::Consumed
            }
        }
    }

    pub(crate) async fn discard_current_recovery(
        &mut self,
        app_server: &mut AppServerSession,
        requested_session_id: &str,
    ) -> Result<()> {
        if !self.recovery_action_still_targets(requested_session_id) {
            return Ok(());
        }
        let session = match self.refresh_current_recovery(app_server).await? {
            RecoveryRefreshOutcome::Current(session) => session,
            RecoveryRefreshOutcome::Replaced | RecoveryRefreshOutcome::Removed => return Ok(()),
        };
        let identity = DraftRecoveryIdentity::from_session(&session);
        let result = app_server
            .workspace_draft_session_close(WorkspaceDraftSessionCloseParams {
                session_id: identity.session_id.clone(),
                client_id: identity.client_id.clone(),
                status: WorkspaceDraftSessionCloseStatus::Discarded,
                expected_current_checkpoint_id: Some(identity.checkpoint_id.clone()),
                expected_current_checkpoint_revision: Some(identity.checkpoint_revision),
                expected_current_checkpoint_sha256: Some(identity.checkpoint_sha256.clone()),
                actor: RECOVERY_ACTOR.to_string(),
                reason: "clinician discarded recovered local draft".to_string(),
            })
            .await;
        match result {
            Ok(response)
                if response.session.status == WorkspaceDraftSessionStatus::Discarded
                    && DraftRecoveryIdentity::from_session(&response.session) == identity =>
            {
                self.finish_recovery_removal(&identity.session_id, "Discarded unfinished draft.");
                Ok(())
            }
            Ok(_) => {
                self.reconcile_discard_response(app_server, &identity, None)
                    .await
            }
            Err(error) => {
                self.reconcile_discard_response(app_server, &identity, Some(error))
                    .await
            }
        }
    }

    pub(crate) async fn restore_current_recovery(
        &mut self,
        app_server: &mut AppServerSession,
        requested_session_id: &str,
    ) -> Result<()> {
        if !self.recovery_action_still_targets(requested_session_id) {
            return Ok(());
        }
        let session = match self.refresh_current_recovery(app_server).await? {
            RecoveryRefreshOutcome::Current(session) => session,
            RecoveryRefreshOutcome::Replaced | RecoveryRefreshOutcome::Removed => return Ok(()),
        };
        let staged = stage_recovered_dashboard(self, app_server, &session).await?;
        let refreshed = match self.refresh_current_recovery(app_server).await? {
            RecoveryRefreshOutcome::Current(session) => session,
            RecoveryRefreshOutcome::Replaced | RecoveryRefreshOutcome::Removed => return Ok(()),
        };
        validate_recovery_session_unchanged(&session, &refreshed)?;
        *self = staged;
        Ok(())
    }

    pub(super) fn finish_recovery_adoption(&mut self, session_id: &str) {
        self.remove_recovery_session(session_id);
        self.draft_recovery.deferred_for_owned_session = !self.draft_recovery.items.is_empty();
        self.draft_recovery.dismissed = false;
    }

    pub(super) fn resume_recovery_after_owned_close(&mut self) -> bool {
        if self.draft_recovery.items.is_empty() {
            self.draft_recovery.deferred_for_owned_session = false;
            return false;
        }
        self.draft_recovery.deferred_for_owned_session = false;
        self.draft_recovery.dismissed = false;
        true
    }

    async fn refresh_current_recovery(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> Result<RecoveryRefreshOutcome> {
        let expected = self
            .current_recovery_item()
            .cloned()
            .ok_or_else(|| color_eyre::eyre::eyre!("no queued draft recovery item"))?;
        let expected_identity = DraftRecoveryIdentity::from_session(&expected.session);
        let sessions = list_recovery_sessions(
            app_server,
            RecoveryListScope::ClientIncludingClosed(expected.session.client_id.clone()),
        )
        .await?;
        let Some(session) = sessions
            .into_iter()
            .find(|session| session.id == expected.session.id)
        else {
            self.finish_recovery_removal(
                &expected.session.id,
                "Unfinished draft no longer exists; recovery queue reconciled.",
            );
            return Ok(RecoveryRefreshOutcome::Removed);
        };
        if session.status != WorkspaceDraftSessionStatus::Active {
            self.finish_recovery_removal(
                &session.id,
                "Draft was already closed or discarded; recovery queue reconciled.",
            );
            return Ok(RecoveryRefreshOutcome::Removed);
        }
        if DraftRecoveryIdentity::from_session(&session) != expected_identity {
            self.replace_current_recovery(session);
            self.status =
                "Draft changed after this recovery prompt; review the refreshed revision before deciding."
                    .to_string();
            return Ok(RecoveryRefreshOutcome::Replaced);
        }
        Ok(RecoveryRefreshOutcome::Current(Box::new(session)))
    }

    async fn reconcile_discard_response(
        &mut self,
        app_server: &mut AppServerSession,
        expected: &DraftRecoveryIdentity,
        original_error: Option<color_eyre::Report>,
    ) -> Result<()> {
        let sessions = list_recovery_sessions(
            app_server,
            RecoveryListScope::ClientIncludingClosed(expected.client_id.clone()),
        )
        .await?;
        let Some(session) = sessions
            .into_iter()
            .find(|session| session.id == expected.session_id)
        else {
            self.finish_recovery_removal(
                &expected.session_id,
                "Draft no longer exists; discard outcome reconciled.",
            );
            return Ok(());
        };
        let actual = DraftRecoveryIdentity::from_session(&session);
        if session.status == WorkspaceDraftSessionStatus::Discarded && actual == *expected {
            self.finish_recovery_removal(
                &expected.session_id,
                "Discard confirmed after reconnecting to local storage.",
            );
            return Ok(());
        }
        if session.status != WorkspaceDraftSessionStatus::Active {
            self.finish_recovery_removal(
                &expected.session_id,
                "Draft reached a terminal state elsewhere; recovery queue reconciled.",
            );
            return Ok(());
        }
        if actual != *expected {
            self.replace_current_recovery(session);
            self.status =
                "Discard was not applied because a newer draft revision exists; review it before deciding."
                    .to_string();
            return Ok(());
        }
        Err(original_error.unwrap_or_else(|| {
            color_eyre::eyre::eyre!("discard response did not confirm the exact queued draft")
        }))
    }

    fn move_recovery_selection(&mut self, delta: isize) {
        let len = self.draft_recovery.items.len();
        if len <= 1 {
            return;
        }
        self.draft_recovery.index = if delta.is_negative() {
            (self.draft_recovery.index + len - 1) % len
        } else {
            (self.draft_recovery.index + 1) % len
        };
    }

    fn current_recovery_action(&self, restore: bool) -> WorkspaceDashboardAction {
        let Some(item) = self.current_recovery_item() else {
            return WorkspaceDashboardAction::Consumed;
        };
        if restore {
            WorkspaceDashboardAction::RestoreRecoveryDraft {
                session_id: item.session.id.clone(),
            }
        } else {
            WorkspaceDashboardAction::DiscardRecoveryDraft {
                session_id: item.session.id.clone(),
            }
        }
    }

    fn recovery_action_still_targets(&mut self, requested_session_id: &str) -> bool {
        if self
            .current_recovery_item()
            .is_some_and(|item| item.session.id == requested_session_id)
        {
            return true;
        }
        self.status =
            "Recovery selection changed before the action ran; review the current draft again."
                .to_string();
        false
    }

    fn replace_current_recovery(&mut self, session: WorkspaceDraftSession) {
        if let Some(item) = self.draft_recovery.items.get_mut(self.draft_recovery.index) {
            *item = DraftRecoveryItem { session };
        }
    }

    fn finish_recovery_removal(&mut self, session_id: &str, status: &str) {
        self.remove_recovery_session(session_id);
        self.status = if self.draft_recovery.items.is_empty() {
            status.to_string()
        } else {
            format!("{status} Review the next unfinished draft.")
        };
    }

    fn remove_recovery_session(&mut self, session_id: &str) {
        self.draft_recovery
            .items
            .retain(|item| item.session.id != session_id);
        self.draft_recovery.index = self
            .draft_recovery
            .index
            .min(self.draft_recovery.items.len().saturating_sub(1));
    }
}

#[cfg(test)]
#[path = "draft_recovery_tests.rs"]
mod tests;
