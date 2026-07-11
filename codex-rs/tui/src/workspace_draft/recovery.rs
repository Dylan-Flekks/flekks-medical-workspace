use super::*;
use codex_app_server_protocol::WorkspaceDraftSession;
use codex_app_server_protocol::WorkspaceDraftSessionStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveredContextSubmission {
    Empty,
    Submitted,
    Unsubmitted,
}

impl WorkspaceDraftCoordinator {
    pub(crate) async fn close_after_canonical_save(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> Result<bool> {
        if self.in_flight.is_some() || self.has_uncheckpointed_edits() {
            color_eyre::eyre::bail!(
                "local draft checkpoint is not confirmed; draft session remains open"
            );
        }
        let Some(params) = self.canonical_close_params()? else {
            return Ok(false);
        };
        let expected_checkpoint_id = params
            .expected_current_checkpoint_id
            .clone()
            .ok_or_else(|| color_eyre::eyre::eyre!("exact draft close checkpoint ID is missing"))?;
        let expected_checkpoint_revision = params
            .expected_current_checkpoint_revision
            .ok_or_else(|| color_eyre::eyre::eyre!("exact draft close revision is missing"))?;
        let expected_checkpoint_sha256 = params
            .expected_current_checkpoint_sha256
            .clone()
            .ok_or_else(|| color_eyre::eyre::eyre!("exact draft close hash is missing"))?;
        let expected_session_id = params.session_id.clone();
        let expected_client_id = params.client_id.clone();
        let response = app_server.workspace_draft_session_close(params).await?;
        let session = response.session;
        if session.status != WorkspaceDraftSessionStatus::Closed
            || session.id != expected_session_id
            || session.client_id != expected_client_id
            || session.current_revision != expected_checkpoint_revision
            || session.current_checkpoint.session_id != expected_session_id
            || session.current_checkpoint.client_id != expected_client_id
            || session.current_checkpoint.id != expected_checkpoint_id
            || session.current_checkpoint.revision != expected_checkpoint_revision
            || session.current_checkpoint.content_sha256 != expected_checkpoint_sha256
        {
            color_eyre::eyre::bail!(
                "local draft close response did not confirm the exact checkpoint identity"
            );
        }
        self.session_id = None;
        self.session_creation_key = None;
        self.last_confirmed_checkpoint = None;
        self.canonical_save_pending_close = false;
        self.saved_generation = self.edit_generation;
        self.saved_context_generation = self.context_generation;
        self.submitted_context_generation = self.context_generation;
        self.debounce_deadline = None;
        Ok(true)
    }

    pub(crate) fn owned_session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub(crate) fn adopt_recovered_session(
        &mut self,
        session: &WorkspaceDraftSession,
        context_submission: RecoveredContextSubmission,
    ) -> Result<()> {
        if self.in_flight.is_some()
            || self.session_id.is_some()
            || self.session_creation_key.is_some()
            || self.last_confirmed_checkpoint.is_some()
            || self.canonical_save_pending_close
            || self.has_uncheckpointed_edits()
        {
            color_eyre::eyre::bail!(
                "cannot adopt a recovered draft while another local draft session is owned"
            );
        }
        if session.status != WorkspaceDraftSessionStatus::Active
            || session.current_checkpoint.session_id != session.id
            || session.current_checkpoint.client_id != session.client_id
            || session.current_checkpoint.revision != session.current_revision
        {
            color_eyre::eyre::bail!("recovered draft session identity is inconsistent");
        }

        let generation = 1;
        let context_generation = u64::from(context_submission != RecoveredContextSubmission::Empty);
        self.active_client_id = Some(session.client_id.clone());
        self.session_id = Some(session.id.clone());
        self.session_creation_key = None;
        self.last_confirmed_checkpoint = Some(session.current_checkpoint.clone());
        self.edit_generation = generation;
        self.saved_generation = generation;
        self.context_generation = context_generation;
        self.saved_context_generation = context_generation;
        self.submitted_context_generation = match context_submission {
            RecoveredContextSubmission::Empty | RecoveredContextSubmission::Unsubmitted => 0,
            RecoveredContextSubmission::Submitted => context_generation,
        };
        self.debounce_deadline = None;
        self.focus_checkpoint_requested = false;
        self.in_flight = None;
        self.canonical_save_pending_close = false;
        Ok(())
    }
}
