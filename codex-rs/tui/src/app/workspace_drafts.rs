use super::*;
use crate::workspace_draft::MEDICAL_WORKSPACE_DRAFT_ACTOR;
use crate::workspace_draft::MedicalWorkspaceWorkingDraftV1;
use crate::workspace_draft::RecoverableMedicalWorkspaceDraft;
use crate::workspace_draft::WorkspaceDraftCheckpointMetadata;
use crate::workspace_draft::WorkspaceDraftCheckpointStart;
use crate::workspace_draft::WorkspaceDraftCheckpointTrigger;
use crate::workspace_draft::WorkspaceDraftCloseDisposition;
use crate::workspace_draft::WorkspaceDraftError;
use crate::workspace_draft::WorkspaceDraftGenerationToken;
use crate::workspace_draft::WorkspaceDraftState;
use codex_app_server_protocol::WorkspaceDraftSessionListParams;
use std::time::Duration;
use std::time::Instant;
use tokio::task::AbortHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceDraftScope {
    client_id: String,
    working_note_id: String,
}

impl WorkspaceDraftScope {
    fn from_draft(draft: &MedicalWorkspaceWorkingDraftV1) -> Self {
        Self {
            client_id: draft.client_id.clone(),
            working_note_id: draft.note.working_note_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WorkspaceDraftTimerRequest {
    token: WorkspaceDraftGenerationToken,
    delay: Duration,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum WorkspaceDraftRecoveryMode {
    #[default]
    ColdStart,
    Navigation,
}

fn select_workspace_draft_recovery(
    current: &MedicalWorkspaceWorkingDraftV1,
    recoveries: Vec<RecoverableMedicalWorkspaceDraft>,
    mode: WorkspaceDraftRecoveryMode,
) -> std::result::Result<Option<RecoverableMedicalWorkspaceDraft>, WorkspaceDraftError> {
    let same_scope = recoveries
        .into_iter()
        .filter(|recovery| {
            recovery.matches_note_scope(
                current.note.note_id.as_deref(),
                current.note.encounter_id.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    let exact = same_scope
        .iter()
        .filter(|recovery| {
            recovery.matches_working_note_scope(
                current.note.note_id.as_deref(),
                current.note.encounter_id.as_deref(),
                &current.note.working_note_id,
            )
        })
        .collect::<Vec<_>>();

    match exact.as_slice() {
        [] => {}
        [recovery] => return Ok(Some((*recovery).clone())),
        _ => {
            return Err(WorkspaceDraftError::InvalidRecovery(format!(
                "{} active local drafts match working note ID {}; none was selected",
                exact.len(),
                current.note.working_note_id
            )));
        }
    }

    if mode == WorkspaceDraftRecoveryMode::Navigation {
        return Ok(None);
    }

    match same_scope.as_slice() {
        [] => Ok(None),
        [recovery] => Ok(Some(recovery.clone())),
        _ => Err(WorkspaceDraftError::InvalidRecovery(format!(
            "{} active local drafts match this note and encounter but not working note ID {}; none was selected",
            same_scope.len(),
            current.note.working_note_id
        ))),
    }
}

#[derive(Debug, Default)]
pub(super) struct WorkspaceDraftRuntime {
    enabled: bool,
    recovery_discovery_complete: bool,
    recovery_mode: WorkspaceDraftRecoveryMode,
    state: WorkspaceDraftState,
    scope: Option<WorkspaceDraftScope>,
    observed: Option<MedicalWorkspaceWorkingDraftV1>,
    scheduled_token: Option<WorkspaceDraftGenerationToken>,
    autosave_timer_abort: Option<AbortHandle>,
}

impl WorkspaceDraftRuntime {
    fn attach_baseline(&mut self, draft: Option<MedicalWorkspaceWorkingDraftV1>) {
        self.cancel_autosave_timer();
        self.scope = draft.as_ref().map(WorkspaceDraftScope::from_draft);
        if let Some(scope) = self.scope.as_ref() {
            self.state.reset_for_client(scope.client_id.clone());
        } else {
            self.state.reset_for_unsaved_patient();
        }
        self.observed = draft;
        self.scheduled_token = None;
    }

    fn observe(
        &mut self,
        draft: Option<MedicalWorkspaceWorkingDraftV1>,
    ) -> Option<WorkspaceDraftTimerRequest> {
        self.observe_inner(draft, None)
    }

    #[cfg(test)]
    fn observe_at(
        &mut self,
        draft: Option<MedicalWorkspaceWorkingDraftV1>,
        now: Instant,
    ) -> Option<WorkspaceDraftTimerRequest> {
        self.observe_inner(draft, Some(now))
    }

    fn observe_inner(
        &mut self,
        draft: Option<MedicalWorkspaceWorkingDraftV1>,
        now: Option<Instant>,
    ) -> Option<WorkspaceDraftTimerRequest> {
        if !self.enabled {
            return None;
        }
        let next_scope = draft.as_ref().map(WorkspaceDraftScope::from_draft);
        if next_scope != self.scope {
            self.cancel_autosave_timer();
            self.scope = next_scope.clone();
            if let Some(scope) = next_scope.as_ref() {
                self.state.reset_for_client(scope.client_id.clone());
            } else {
                self.state.reset_for_unsaved_patient();
            }
            self.observed = None;
            self.scheduled_token = None;
            self.recovery_discovery_complete = next_scope.is_none();
        }
        if draft == self.observed {
            return None;
        }
        self.observed = draft;
        self.scope.as_ref()?;
        let schedule = match now {
            Some(now) => self.state.mark_changed_at(now),
            None => self.state.mark_changed(),
        };
        self.scheduled_token = Some(schedule.token);
        Some(WorkspaceDraftTimerRequest {
            token: schedule.token,
            delay: schedule.delay,
        })
    }

    fn adopt_recovered_scope(&mut self, draft: &MedicalWorkspaceWorkingDraftV1) {
        self.scope = Some(WorkspaceDraftScope::from_draft(draft));
    }

    fn schedule_autosave_timer(
        &mut self,
        app_event_tx: AppEventSender,
        request: WorkspaceDraftTimerRequest,
    ) {
        self.cancel_autosave_timer();
        let task = tokio::spawn(async move {
            tokio::time::sleep(request.delay).await;
            app_event_tx.send(AppEvent::WorkspaceDraftAutosaveTick {
                token: request.token,
            });
        });
        self.autosave_timer_abort = Some(task.abort_handle());
    }

    fn cancel_autosave_timer(&mut self) {
        if let Some(abort) = self.autosave_timer_abort.take() {
            abort.abort();
        }
    }

    fn take_fired_autosave_timer(&mut self, token: WorkspaceDraftGenerationToken) -> bool {
        if self.scheduled_token != Some(token) {
            return false;
        }
        self.autosave_timer_abort = None;
        true
    }

    fn request_focus_checkpoint(
        &self,
        app_event_tx: &AppEventSender,
        previous_focus: WorkspaceFocus,
        current_focus: WorkspaceFocus,
    ) {
        if current_focus == previous_focus {
            return;
        }
        let Some(token) = self.scheduled_token else {
            return;
        };
        app_event_tx.send(AppEvent::WorkspaceDraftFocusCheckpoint { token });
    }
}

impl App {
    pub(super) fn workspace_draft_recovery_needs_retry(&self) -> bool {
        self.workspace_draft_runtime.enabled
            && !self.workspace_draft_runtime.recovery_discovery_complete
    }

    pub(super) fn workspace_draft_recovery_pending(&self) -> bool {
        self.workspace_draft_runtime
            .state
            .pending_recovery()
            .is_some()
    }

    pub(super) async fn initialize_workspace_draft_recovery(
        &mut self,
        app_server: &mut AppServerSession,
    ) {
        self.initialize_workspace_draft_recovery_with_mode(
            app_server,
            WorkspaceDraftRecoveryMode::ColdStart,
        )
        .await;
    }

    pub(super) async fn ensure_workspace_draft_runtime_initialized(
        &mut self,
        app_server: &mut AppServerSession,
    ) {
        if self.workspace_draft_runtime.enabled
            && self.workspace_draft_runtime.recovery_discovery_complete
        {
            return;
        }
        self.initialize_workspace_draft_recovery(app_server).await;
    }

    pub(super) async fn initialize_workspace_draft_recovery_after_navigation(
        &mut self,
        app_server: &mut AppServerSession,
    ) {
        self.initialize_workspace_draft_recovery_with_mode(
            app_server,
            WorkspaceDraftRecoveryMode::Navigation,
        )
        .await;
    }

    pub(super) async fn retry_workspace_draft_recovery(
        &mut self,
        app_server: &mut AppServerSession,
    ) {
        let mode = self.workspace_draft_runtime.recovery_mode;
        self.initialize_workspace_draft_recovery_with_mode(app_server, mode)
            .await;
    }

    async fn initialize_workspace_draft_recovery_with_mode(
        &mut self,
        app_server: &mut AppServerSession,
        mode: WorkspaceDraftRecoveryMode,
    ) {
        self.workspace_draft_runtime.recovery_mode = mode;
        self.workspace_draft_runtime.enabled = !app_server.uses_remote_workspace();
        self.workspace_draft_runtime.recovery_discovery_complete = false;
        let current = match self.current_medical_working_draft() {
            Ok(current) => current,
            Err(error) => {
                self.set_workspace_draft_message(Some(format!(
                    "Local recovery unavailable: {error}"
                )));
                return;
            }
        };
        self.workspace_draft_runtime
            .attach_baseline(current.clone());
        self.set_workspace_draft_recovery_available(false);

        let Some(current) = current else {
            self.workspace_draft_runtime.recovery_discovery_complete = true;
            self.set_workspace_draft_message(Some(
                "Local recovery begins after the new patient is saved.".to_string(),
            ));
            return;
        };
        if app_server.uses_remote_workspace() {
            self.set_workspace_draft_message(Some(
                "Local recovery checkpoints are disabled for a remote workspace store.".to_string(),
            ));
            return;
        }

        let mut cursor = None;
        let mut recoveries = Vec::new();
        loop {
            let response = match app_server
                .workspace_draft_session_list(WorkspaceDraftSessionListParams {
                    client_id: current.client_id.clone(),
                    include_closed: false,
                    cursor,
                    limit: Some(200),
                })
                .await
            {
                Ok(response) => response,
                Err(error) => {
                    self.set_workspace_draft_message(Some(format!(
                        "Local recovery check failed; new checkpoints are paused until the workspace is reopened and discovery succeeds: {error}"
                    )));
                    return;
                }
            };
            for session in response.data {
                let recovery = match RecoverableMedicalWorkspaceDraft::try_from(session) {
                    Ok(recovery) => recovery,
                    Err(error) => {
                        self.set_workspace_draft_message(Some(format!(
                            "Local recovery data could not be verified; new checkpoints are paused until discovery succeeds: {error}"
                        )));
                        return;
                    }
                };
                recoveries.push(recovery);
            }
            let Some(next_cursor) = response.next_cursor else {
                break;
            };
            cursor = Some(next_cursor);
        }
        let recovery = match select_workspace_draft_recovery(&current, recoveries, mode) {
            Ok(recovery) => recovery,
            Err(error) => {
                self.set_workspace_draft_message(Some(format!(
                    "Local recovery is ambiguous; no draft was selected and new checkpoints are paused: {error}"
                )));
                if let Some(dashboard) = self.workspace_dashboard.as_mut() {
                    dashboard.set_status(
                        "Recovery paused: multiple active local drafts match this note; none was applied.",
                    );
                }
                return;
            }
        };
        let Some(recovery) = recovery else {
            self.workspace_draft_runtime.recovery_discovery_complete = true;
            self.set_workspace_draft_message(Some(
                "Local draft recovery ready; Ctrl-S remains the canonical chart save.".to_string(),
            ));
            return;
        };
        if mode == WorkspaceDraftRecoveryMode::ColdStart
            && recovery.draft.note.working_note_id != current.note.working_note_id
        {
            let Some(dashboard) = self.workspace_dashboard.as_mut() else {
                self.set_workspace_draft_message(Some(
                    "Local recovery could not bind its unique working-note identity because the workspace dashboard is closed."
                        .to_string(),
                ));
                return;
            };
            if let Err(error) = dashboard.bind_cold_start_recovery_identity(&recovery.draft) {
                dashboard.set_draft_persistence_message(Some(format!(
                    "Local recovery could not bind its unique working-note identity safely; no draft was selected and new checkpoints are paused: {error}"
                )));
                dashboard.set_status(
                    "Recovery paused: the unique local draft identity could not be verified; nothing was applied.",
                );
                return;
            }
            let rebound = match self.current_medical_working_draft() {
                Ok(rebound) => rebound,
                Err(error) => {
                    self.set_workspace_draft_message(Some(format!(
                        "Local recovery could not confirm its rebound working-note identity; no draft was selected and new checkpoints are paused: {error}"
                    )));
                    return;
                }
            };
            self.workspace_draft_runtime.attach_baseline(rebound);
        }
        if let Err(error) = self.workspace_draft_runtime.state.offer_recovery(recovery) {
            self.set_workspace_draft_message(Some(format!(
                "Local recovery could not be offered safely: {error}"
            )));
            return;
        }
        self.workspace_draft_runtime.recovery_discovery_complete = true;
        self.set_workspace_draft_recovery_available(true);
        if let Some(dashboard) = self.workspace_dashboard.as_mut() {
            dashboard.set_status(
                "Recoverable local draft found. Use Ctrl-P and choose Restore local draft or Discard local draft.",
            );
        }
    }

    pub(super) fn observe_workspace_draft(&mut self) {
        let current = match self.current_medical_working_draft() {
            Ok(current) => current,
            Err(error) => {
                self.set_workspace_draft_message(Some(format!(
                    "Local recovery unavailable: {error}"
                )));
                return;
            }
        };
        if let Some(request) = self.workspace_draft_runtime.observe(current) {
            self.set_workspace_draft_recovery_available(false);
            self.set_workspace_draft_message(Some(
                "Local recovery checkpoint pending; Ctrl-S still saves the canonical chart."
                    .to_string(),
            ));
            self.workspace_draft_runtime
                .schedule_autosave_timer(self.app_event_tx.clone(), request);
        }
    }

    pub(super) fn request_workspace_draft_focus_checkpoint(&self, previous_focus: WorkspaceFocus) {
        let Some(current_focus) = self
            .workspace_dashboard
            .as_ref()
            .map(WorkspaceDashboard::focus)
        else {
            return;
        };
        self.workspace_draft_runtime.request_focus_checkpoint(
            &self.app_event_tx,
            previous_focus,
            current_focus,
        );
    }

    pub(super) async fn handle_workspace_draft_autosave_tick(
        &mut self,
        app_server: &mut AppServerSession,
        token: WorkspaceDraftGenerationToken,
    ) -> bool {
        if !self.workspace_draft_runtime.enabled
            || !self.workspace_draft_runtime.recovery_discovery_complete
            || !self
                .workspace_draft_runtime
                .take_fired_autosave_timer(token)
        {
            return false;
        }
        if !self.workspace_draft_runtime.state.autosave_is_due(token) {
            if let Some(remaining) = self.workspace_draft_runtime.state.autosave_remaining(token) {
                self.workspace_draft_runtime.schedule_autosave_timer(
                    self.app_event_tx.clone(),
                    WorkspaceDraftTimerRequest {
                        token,
                        delay: remaining.max(Duration::from_millis(1)),
                    },
                );
            }
            return false;
        }
        if let Err(error) = self
            .persist_workspace_draft_checkpoint(
                app_server,
                token,
                WorkspaceDraftCheckpointTrigger::IdleTyping,
            )
            .await
        {
            self.set_workspace_draft_message(Some(format!(
                "Local recovery checkpoint failed; working state remains in memory. Automatic retry is paused until the next edit, focus change, or explicit save: {error}"
            )));
        }
        true
    }

    pub(super) async fn handle_workspace_draft_focus_checkpoint(
        &mut self,
        app_server: &mut AppServerSession,
        token: WorkspaceDraftGenerationToken,
    ) {
        if !self.workspace_draft_runtime.enabled
            || !self.workspace_draft_runtime.recovery_discovery_complete
            || self.workspace_draft_runtime.scheduled_token != Some(token)
        {
            return;
        }
        if let Err(error) = self
            .persist_workspace_draft_checkpoint(
                app_server,
                token,
                WorkspaceDraftCheckpointTrigger::FocusChange,
            )
            .await
        {
            self.set_workspace_draft_message(Some(format!(
                "Local recovery checkpoint failed after focus changed; working state remains in memory: {error}"
            )));
        }
    }

    pub(super) async fn flush_workspace_draft(
        &mut self,
        app_server: &mut AppServerSession,
        trigger: WorkspaceDraftCheckpointTrigger,
    ) -> Result<()> {
        if !self.workspace_draft_runtime.enabled {
            self.set_workspace_draft_message(Some(
                "Local recovery checkpoints are disabled for a remote workspace store.".to_string(),
            ));
            return Ok(());
        }
        if !self.workspace_draft_runtime.recovery_discovery_complete {
            return Err(color_eyre::eyre::eyre!(
                "local recovery discovery must succeed before creating a checkpoint; reopen the workspace to retry"
            ));
        }
        let current = self.current_medical_working_draft()?;
        let Some(current) = current else {
            self.set_workspace_draft_message(Some(
                "Working state is memory-only until the patient is saved.".to_string(),
            ));
            return Ok(());
        };
        if self.workspace_draft_runtime.scope.as_ref()
            != Some(&WorkspaceDraftScope::from_draft(&current))
        {
            self.workspace_draft_runtime.recovery_discovery_complete = false;
            return Err(color_eyre::eyre::eyre!(
                "local recovery discovery must run for the current patient and note before creating a checkpoint; reopen the workspace to retry"
            ));
        }
        if self.workspace_draft_runtime.observed.as_ref() != Some(&current) {
            let _ = self.workspace_draft_runtime.observe(Some(current));
        }
        let token = self.workspace_draft_runtime.state.current_token();
        self.persist_workspace_draft_checkpoint(app_server, token, trigger)
            .await
    }

    pub(super) async fn restore_workspace_draft_recovery(&mut self) -> Result<()> {
        let recovered = self
            .workspace_draft_runtime
            .state
            .pending_recovery()
            .ok_or_else(|| color_eyre::eyre::eyre!("no recoverable local draft is available"))?
            .draft
            .clone();
        let dashboard = self
            .workspace_dashboard
            .as_mut()
            .ok_or_else(|| color_eyre::eyre::eyre!("workspace dashboard is not open"))?;
        dashboard.apply_recovered_medical_working_draft(recovered.clone())?;
        let adopted = self.workspace_draft_runtime.state.adopt_recovery()?;
        if adopted != recovered {
            return Err(color_eyre::eyre::eyre!(
                "recovered local draft changed during explicit restore"
            ));
        }
        self.workspace_draft_runtime.adopt_recovered_scope(&adopted);
        self.workspace_draft_runtime.observed = Some(adopted);
        self.workspace_draft_runtime.scheduled_token = None;
        self.workspace_draft_runtime.cancel_autosave_timer();
        let checkpoint = self
            .workspace_draft_runtime
            .state
            .confirmed_checkpoint()
            .cloned();
        if let Some(dashboard) = self.workspace_dashboard.as_mut() {
            dashboard.set_source_checkpoint(checkpoint);
        }
        self.set_workspace_draft_recovery_available(false);
        Ok(())
    }

    pub(super) async fn discard_workspace_draft_recovery(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> Result<()> {
        let recovery = self
            .workspace_draft_runtime
            .state
            .pending_recovery()
            .cloned()
            .ok_or_else(|| color_eyre::eyre::eyre!("no recoverable local draft is available"))?;
        let response = app_server
            .workspace_draft_session_close(recovery.discard_params(
                MEDICAL_WORKSPACE_DRAFT_ACTOR,
                "clinician explicitly discarded local working draft",
            )?)
            .await?;
        self.workspace_draft_runtime
            .state
            .confirm_recovery_discarded(&response.session)?;
        self.set_workspace_draft_recovery_available(false);
        self.set_workspace_draft_message(Some(
            "Local recovery draft discarded; canonical chart was unchanged.".to_string(),
        ));
        if let Some(dashboard) = self.workspace_dashboard.as_mut() {
            dashboard.set_status("Discarded local recovery draft; canonical chart was unchanged.");
        }
        Ok(())
    }

    pub(super) async fn discard_current_workspace_draft_session(
        &mut self,
        app_server: &mut AppServerSession,
    ) -> Result<()> {
        if self
            .workspace_draft_runtime
            .state
            .pending_recovery()
            .is_some()
        {
            return self.discard_workspace_draft_recovery(app_server).await;
        }
        if self
            .workspace_draft_runtime
            .state
            .confirmed_checkpoint()
            .is_none()
        {
            return Ok(());
        }
        let params = self.workspace_draft_runtime.state.exact_close_params(
            WorkspaceDraftCloseDisposition::Discarded,
            MEDICAL_WORKSPACE_DRAFT_ACTOR,
            "clinician explicitly discarded workspace working state",
        )?;
        let response = app_server.workspace_draft_session_close(params).await?;
        self.workspace_draft_runtime
            .state
            .confirm_closed(&response.session, WorkspaceDraftCloseDisposition::Discarded)?;
        Ok(())
    }

    async fn persist_workspace_draft_checkpoint(
        &mut self,
        app_server: &mut AppServerSession,
        token: WorkspaceDraftGenerationToken,
        trigger: WorkspaceDraftCheckpointTrigger,
    ) -> Result<()> {
        let draft = self.current_medical_working_draft()?.ok_or_else(|| {
            color_eyre::eyre::eyre!("durable recovery is unavailable until the patient is saved")
        })?;
        let request = match self.workspace_draft_runtime.state.begin_checkpoint(
            token,
            &draft,
            trigger,
            MEDICAL_WORKSPACE_DRAFT_ACTOR,
        )? {
            WorkspaceDraftCheckpointStart::AlreadyCurrent => {
                self.workspace_draft_runtime.scheduled_token = None;
                self.workspace_draft_runtime.cancel_autosave_timer();
                return Ok(());
            }
            WorkspaceDraftCheckpointStart::Request(request) => request,
        };
        let response = match app_server.workspace_draft_checkpoint_create(request).await {
            Ok(response) => response,
            Err(error) => {
                let _ = self
                    .workspace_draft_runtime
                    .state
                    .fail_checkpoint(token, error.to_string());
                self.workspace_draft_runtime.scheduled_token = Some(token);
                return Err(error);
            }
        };
        let metadata = self
            .workspace_draft_runtime
            .state
            .complete_checkpoint(token, &response)?;
        if let Some(dashboard) = self.workspace_dashboard.as_mut() {
            dashboard.set_source_checkpoint(Some(metadata.clone()));
        }
        self.workspace_draft_runtime.scheduled_token = None;
        self.workspace_draft_runtime.cancel_autosave_timer();
        self.set_workspace_draft_message(Some(format!(
            "Local recovery checkpoint r{} saved; canonical chart unchanged.",
            metadata.revision
        )));
        if let Err(error) = self
            .outdate_workspace_plan_for_checkpoint(app_server, &metadata)
            .await
        {
            tracing::warn!(%error, "failed to mark prior workspace plan revision outdated");
        }
        Ok(())
    }

    pub(super) fn confirmed_workspace_draft_checkpoint(
        &self,
    ) -> Option<WorkspaceDraftCheckpointMetadata> {
        self.workspace_draft_runtime
            .state
            .confirmed_checkpoint()
            .cloned()
    }

    fn current_medical_working_draft(&self) -> Result<Option<MedicalWorkspaceWorkingDraftV1>> {
        self.workspace_dashboard
            .as_ref()
            .map(WorkspaceDashboard::medical_working_draft)
            .transpose()
            .map(Option::flatten)
            .map_err(Into::into)
    }

    fn set_workspace_draft_recovery_available(&mut self, available: bool) {
        if let Some(dashboard) = self.workspace_dashboard.as_mut() {
            dashboard.set_draft_recovery_available(available);
        }
    }

    fn set_workspace_draft_message(&mut self, message: Option<String>) {
        if let Some(dashboard) = self.workspace_dashboard.as_mut() {
            dashboard.set_draft_persistence_message(message);
        }
    }
}

#[cfg(test)]
#[path = "workspace_drafts_tests.rs"]
mod tests;
