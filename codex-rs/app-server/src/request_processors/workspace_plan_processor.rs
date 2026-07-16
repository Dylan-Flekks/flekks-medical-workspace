use super::*;
use codex_app_server_protocol::WorkspacePlanActiveRun;
use codex_app_server_protocol::WorkspacePlanGuideRun;
use codex_app_server_protocol::WorkspacePlanGuideRunFinishParams;
use codex_app_server_protocol::WorkspacePlanGuideRunFinishResponse;
use codex_app_server_protocol::WorkspacePlanGuideRunOutcome;
use codex_app_server_protocol::WorkspacePlanGuideRunStartParams;
use codex_app_server_protocol::WorkspacePlanGuideRunStartResponse;
use codex_app_server_protocol::WorkspacePlanGuideRunStatus;
use codex_app_server_protocol::WorkspacePlanMessage;
use codex_app_server_protocol::WorkspacePlanMessageAppendParams;
use codex_app_server_protocol::WorkspacePlanMessageAppendResponse;
use codex_app_server_protocol::WorkspacePlanMessageRole;
use codex_app_server_protocol::WorkspacePlanProposal;
use codex_app_server_protocol::WorkspacePlanProposalPayload;
use codex_app_server_protocol::WorkspacePlanProposalResolution;
use codex_app_server_protocol::WorkspacePlanProposalResolveParams;
use codex_app_server_protocol::WorkspacePlanProposalResolveResponse;
use codex_app_server_protocol::WorkspacePlanProposalStatus;
use codex_app_server_protocol::WorkspacePlanRecoveryGetParams;
use codex_app_server_protocol::WorkspacePlanRecoveryGetResponse;
use codex_app_server_protocol::WorkspacePlanRecoveryState;
use codex_app_server_protocol::WorkspacePlanRevision;
use codex_app_server_protocol::WorkspacePlanRevisionOutdateParams;
use codex_app_server_protocol::WorkspacePlanRevisionOutdateResponse;
use codex_app_server_protocol::WorkspacePlanRevisionStatus;
use codex_app_server_protocol::WorkspacePlanRevisionSubmitParams;
use codex_app_server_protocol::WorkspacePlanRevisionSubmitResponse;
use codex_app_server_protocol::WorkspacePlanSession;
use codex_app_server_protocol::WorkspacePlanSessionBindThreadParams;
use codex_app_server_protocol::WorkspacePlanSessionBindThreadResponse;
use codex_app_server_protocol::WorkspacePlanSessionGetParams;
use codex_app_server_protocol::WorkspacePlanSessionGetResponse;
use codex_app_server_protocol::WorkspacePlanSessionOpenParams;
use codex_app_server_protocol::WorkspacePlanSessionOpenResponse;
use codex_app_server_protocol::WorkspacePlanSessionStatus;
use codex_app_server_protocol::WorkspacePlanSnapshotGetParams;
use codex_app_server_protocol::WorkspacePlanSnapshotGetResponse;
use codex_app_server_protocol::WorkspacePlanSubmissionReceipt;
use codex_app_server_protocol::WorkspacePlanTurnCompletionReceipt;

impl WorkspaceRequestProcessor {
    pub(crate) async fn plan_session_open(
        &self,
        params: WorkspacePlanSessionOpenParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let client_id = required_plan_text("clientId", params.client_id)?;
        let session = self
            .state_db()?
            .workspace()
            .open_plan_session(codex_state::WorkspacePlanSessionOpen {
                client_id,
                created_by: "local clinician".to_string(),
            })
            .await
            .map_err(plan_operation_error)?;
        Ok(Some(
            WorkspacePlanSessionOpenResponse {
                session: api_plan_session(session),
            }
            .into(),
        ))
    }

    pub(crate) async fn plan_session_get(
        &self,
        params: WorkspacePlanSessionGetParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let client_id = required_plan_text("clientId", params.client_id)?;
        let state_db = self.state_db()?;
        let store = state_db.workspace();
        let session = match empty_to_none(params.session_id) {
            Some(session_id) => store.get_plan_session(&session_id, &client_id).await,
            None => store.get_active_plan_session(&client_id).await,
        }
        .map_err(plan_operation_error)?;
        if session
            .as_ref()
            .is_some_and(|session| session.client_id != client_id)
        {
            return Err(invalid_request(
                "workspace plan session does not belong to clientId",
            ));
        }
        Ok(Some(
            WorkspacePlanSessionGetResponse {
                session: session.map(api_plan_session),
            }
            .into(),
        ))
    }

    pub(crate) async fn plan_session_bind_thread(
        &self,
        params: WorkspacePlanSessionBindThreadParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let session = self
            .state_db()?
            .workspace()
            .bind_plan_session_thread(codex_state::WorkspacePlanSessionThreadBind {
                session_id: required_plan_text("sessionId", params.session_id)?,
                client_id: required_plan_text("clientId", params.client_id)?,
                expected_thread_id: empty_to_none(params.expected_thread_id),
                source_thread_id: required_plan_text("sourceThreadId", params.source_thread_id)?,
                actor: "workspace plan harness".to_string(),
            })
            .await
            .map_err(plan_operation_error)?;
        Ok(Some(
            WorkspacePlanSessionBindThreadResponse {
                session: api_plan_session(session),
            }
            .into(),
        ))
    }

    pub(crate) async fn plan_snapshot_get(
        &self,
        params: WorkspacePlanSnapshotGetParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let client_id = required_plan_text("clientId", params.client_id)?;
        let state_db = self.state_db()?;
        let store = state_db.workspace();
        let session = match empty_to_none(params.plan_session_id) {
            Some(session_id) => store.get_plan_session(&session_id, &client_id).await,
            None => store.get_active_plan_session(&client_id).await,
        }
        .map_err(plan_operation_error)?;
        let Some(session) = session else {
            return Ok(Some(
                WorkspacePlanSnapshotGetResponse {
                    session: None,
                    messages: Vec::new(),
                    revisions: Vec::new(),
                    submission_receipts: Vec::new(),
                    proposals: Vec::new(),
                }
                .into(),
            ));
        };
        if session.client_id != client_id {
            return Err(invalid_request(
                "workspace plan session does not belong to clientId",
            ));
        }
        let session_id = session.id.clone();
        let messages = store
            .list_plan_messages(codex_state::WorkspacePlanMessageFilter {
                plan_session_id: session_id.clone(),
                client_id: client_id.clone(),
                after_sequence: params.after_message_sequence,
                limit: Some(params.message_limit.unwrap_or(200).clamp(1, 500)),
            })
            .await
            .map_err(plan_operation_error)?
            .into_iter()
            .map(api_plan_message)
            .collect();
        let revision_models = store
            .list_plan_revisions(codex_state::WorkspacePlanRevisionFilter {
                plan_session_id: session_id.clone(),
                client_id: client_id.clone(),
                before_revision: None,
                limit: Some(params.revision_limit.unwrap_or(50).clamp(1, 100)),
            })
            .await
            .map_err(plan_operation_error)?;
        let submitted_revision_ids = revision_models
            .iter()
            .filter(|revision| {
                revision.status == codex_state::WorkspacePlanRevisionStatus::Submitted
            })
            .map(|revision| revision.id.clone())
            .collect::<Vec<_>>();
        let submission_receipts = store
            .list_plan_submission_receipts(&session_id, &client_id, &submitted_revision_ids)
            .await
            .map_err(plan_operation_error)?
            .into_iter()
            .map(api_plan_submission_receipt)
            .collect();
        let revisions = revision_models
            .into_iter()
            .map(api_plan_revision)
            .collect::<Result<Vec<_>, _>>()?;
        let proposals = store
            .list_plan_proposals(codex_state::WorkspacePlanProposalFilter {
                plan_session_id: session_id,
                client_id,
                status: None,
                limit: Some(params.proposal_limit.unwrap_or(100).clamp(1, 200)),
            })
            .await
            .map_err(plan_operation_error)?
            .into_iter()
            .map(api_plan_proposal)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(
            WorkspacePlanSnapshotGetResponse {
                session: Some(api_plan_session(session)),
                messages,
                revisions,
                submission_receipts,
                proposals,
            }
            .into(),
        ))
    }

    pub(crate) async fn plan_recovery_get(
        &self,
        params: WorkspacePlanRecoveryGetParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let recovery = self
            .state_db()?
            .workspace()
            .reconcile_plan_session(codex_state::WorkspacePlanRecoveryRequest {
                plan_session_id: required_plan_text("planSessionId", params.plan_session_id)?,
                client_id: required_plan_text("clientId", params.client_id)?,
            })
            .await
            .map_err(plan_operation_error)?;
        Ok(Some(
            WorkspacePlanRecoveryGetResponse {
                recovery: api_plan_recovery(recovery)?,
            }
            .into(),
        ))
    }

    pub(crate) async fn plan_guide_run_start(
        &self,
        params: WorkspacePlanGuideRunStartParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let run = self
            .state_db()?
            .workspace()
            .start_guide_run(codex_state::WorkspaceGuideRunStart {
                client_id: required_plan_text("clientId", params.client_id)?,
                session_id: required_plan_text("draftSessionId", params.draft_session_id)?,
                source_checkpoint_id: required_plan_text(
                    "sourceCheckpointId",
                    params.source_checkpoint_id,
                )?,
                source_checkpoint_revision: params.source_checkpoint_revision,
                source_checkpoint_sha256: required_plan_text(
                    "sourceCheckpointSha256",
                    params.source_checkpoint_sha256,
                )?,
                request_json: required_plan_text("requestJson", params.request_json)?,
                idempotency_key: required_plan_text("idempotencyKey", params.idempotency_key)?,
                trigger: required_plan_text("trigger", params.trigger)?,
                actor: "local clinician".to_string(),
                provider: required_plan_text("provider", params.provider)?,
                model: required_plan_text("model", params.model)?,
                model_tool_mode: codex_state::WorkspaceGuideModelToolMode::WorkspacePlanningOnly,
            })
            .await
            .map_err(|error| {
                invalid_request(format!("failed to start workspace plan turn: {error}"))
            })?;
        Ok(Some(
            WorkspacePlanGuideRunStartResponse {
                run: api_plan_guide_run(run),
            }
            .into(),
        ))
    }

    pub(crate) async fn plan_guide_run_finish(
        &self,
        params: WorkspacePlanGuideRunFinishParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let outcome = match params.outcome {
            WorkspacePlanGuideRunOutcome::Failed { error_summary } => {
                codex_state::WorkspaceGuideRunOutcome::Failed { error_summary }
            }
            WorkspacePlanGuideRunOutcome::Canceled { reason } => {
                codex_state::WorkspaceGuideRunOutcome::Canceled { reason }
            }
        };
        let run = self
            .state_db()?
            .workspace()
            .finish_guide_run(codex_state::WorkspaceGuideRunFinish {
                run_id: params.run_id,
                client_id: params.client_id,
                session_id: params.draft_session_id,
                source_checkpoint_id: params.source_checkpoint_id,
                source_checkpoint_revision: params.source_checkpoint_revision,
                source_checkpoint_sha256: params.source_checkpoint_sha256,
                request_envelope_sha256: params.request_envelope_sha256,
                source_thread_id: empty_to_none(params.source_thread_id),
                source_turn_id: empty_to_none(params.source_turn_id),
                outcome,
                actor: "workspace plan harness".to_string(),
            })
            .await
            .map_err(|error| {
                invalid_request(format!("failed to finish workspace plan turn: {error}"))
            })?;
        Ok(Some(
            WorkspacePlanGuideRunFinishResponse {
                run: api_plan_guide_run(run),
            }
            .into(),
        ))
    }

    pub(crate) async fn plan_message_append(
        &self,
        params: WorkspacePlanMessageAppendParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let message = self
            .state_db()?
            .workspace()
            .append_plan_message(codex_state::WorkspacePlanMessageAppend {
                plan_session_id: params.plan_session_id,
                client_id: params.client_id,
                guide_run_id: params.guide_run_id,
                role: codex_state::WorkspacePlanMessageRole::Human,
                content: params.content,
                idempotency_key: params.idempotency_key,
                source_thread_id: None,
                source_turn_id: None,
            })
            .await
            .map_err(plan_operation_error)?;
        Ok(Some(
            WorkspacePlanMessageAppendResponse {
                message: api_plan_message(message),
            }
            .into(),
        ))
    }

    pub(crate) async fn plan_revision_outdate(
        &self,
        params: WorkspacePlanRevisionOutdateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let revision = self
            .state_db()?
            .workspace()
            .outdate_plan_revision(codex_state::WorkspacePlanRevisionOutdate {
                revision_id: params.revision_id,
                plan_session_id: params.plan_session_id,
                client_id: params.client_id,
                content_sha256: params.content_sha256,
                actor: "workspace plan harness".to_string(),
                reason: params.reason,
            })
            .await
            .map_err(plan_operation_error)?;
        Ok(Some(
            WorkspacePlanRevisionOutdateResponse {
                revision: api_plan_revision(revision)?,
            }
            .into(),
        ))
    }

    pub(crate) async fn plan_revision_submit(
        &self,
        params: WorkspacePlanRevisionSubmitParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let revision = self
            .state_db()?
            .workspace()
            .submit_plan_revision(codex_state::WorkspacePlanRevisionSubmit {
                revision_id: params.revision_id,
                plan_session_id: params.plan_session_id,
                client_id: params.client_id,
                packet_id: params.packet_id,
                agent_run_id: params.agent_run_id,
                source_checkpoint_id: params.source_checkpoint_id,
                source_checkpoint_revision: params.source_checkpoint_revision,
                source_checkpoint_sha256: params.source_checkpoint_sha256,
                content_sha256: params.content_sha256,
                actor: "local clinician".to_string(),
            })
            .await
            .map_err(plan_operation_error)?;
        Ok(Some(
            WorkspacePlanRevisionSubmitResponse {
                revision: api_plan_revision(revision)?,
            }
            .into(),
        ))
    }

    pub(crate) async fn plan_proposal_resolve(
        &self,
        params: WorkspacePlanProposalResolveParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let resolution = match params.resolution {
            WorkspacePlanProposalResolution::Accept => {
                codex_state::WorkspacePlanProposalResolution::Accept
            }
            WorkspacePlanProposalResolution::Decline => {
                codex_state::WorkspacePlanProposalResolution::Decline
            }
        };
        let proposal = self
            .state_db()?
            .workspace()
            .resolve_plan_proposal(codex_state::WorkspacePlanProposalResolve {
                proposal_id: params.proposal_id,
                plan_session_id: params.plan_session_id,
                client_id: params.client_id,
                resolution,
                actor: "local clinician".to_string(),
            })
            .await
            .map_err(plan_operation_error)?;
        Ok(Some(
            WorkspacePlanProposalResolveResponse {
                proposal: api_plan_proposal(proposal)?,
            }
            .into(),
        ))
    }
}

fn required_plan_text(field: &str, value: String) -> Result<String, JSONRPCErrorError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(invalid_request(format!(
            "workspace plan {field} must not be empty"
        )));
    }
    Ok(value.to_string())
}

fn plan_operation_error(error: codex_state::WorkspacePlanError) -> JSONRPCErrorError {
    invalid_request(format!("failed to update workspace plan: {error}"))
}

fn api_plan_session(value: codex_state::WorkspacePlanSession) -> WorkspacePlanSession {
    WorkspacePlanSession {
        id: value.id,
        client_id: value.client_id,
        source_thread_id: value.source_thread_id,
        status: match value.status {
            codex_state::WorkspacePlanSessionStatus::Active => WorkspacePlanSessionStatus::Active,
            codex_state::WorkspacePlanSessionStatus::Closed => WorkspacePlanSessionStatus::Closed,
        },
        latest_revision: value.latest_revision,
        created_by: value.created_by,
        created_at: value.created_at.timestamp_millis(),
        updated_at: value.updated_at.timestamp_millis(),
        closed_at: value.closed_at.map(|value| value.timestamp_millis()),
        replayed: value.replayed,
    }
}

fn api_plan_message(value: codex_state::WorkspacePlanMessage) -> WorkspacePlanMessage {
    WorkspacePlanMessage {
        id: value.id,
        plan_session_id: value.plan_session_id,
        client_id: value.client_id,
        guide_run_id: value.guide_run_id,
        sequence: value.sequence,
        role: match value.role {
            codex_state::WorkspacePlanMessageRole::Human => WorkspacePlanMessageRole::Human,
            codex_state::WorkspacePlanMessageRole::Assistant => WorkspacePlanMessageRole::Assistant,
            codex_state::WorkspacePlanMessageRole::Question => WorkspacePlanMessageRole::Question,
            codex_state::WorkspacePlanMessageRole::Answer => WorkspacePlanMessageRole::Answer,
            codex_state::WorkspacePlanMessageRole::Error => WorkspacePlanMessageRole::Error,
            codex_state::WorkspacePlanMessageRole::SystemStatus => {
                WorkspacePlanMessageRole::SystemStatus
            }
        },
        content: value.content,
        content_sha256: value.content_sha256,
        idempotency_key: value.idempotency_key,
        source_checkpoint_id: value.source_checkpoint_id,
        source_checkpoint_revision: value.source_checkpoint_revision,
        source_checkpoint_sha256: value.source_checkpoint_sha256,
        encounter_id: value.encounter_id,
        note_id: value.note_id,
        source_thread_id: value.source_thread_id,
        source_turn_id: value.source_turn_id,
        created_at: value.created_at.timestamp_millis(),
        replayed: value.replayed,
    }
}

fn api_plan_revision(
    value: codex_state::WorkspacePlanRevision,
) -> Result<WorkspacePlanRevision, JSONRPCErrorError> {
    Ok(WorkspacePlanRevision {
        id: value.id,
        plan_session_id: value.plan_session_id,
        client_id: value.client_id,
        guide_run_id: value.guide_run_id,
        revision: value.revision,
        plan_markdown: value.plan_markdown,
        decisions_json: value.decisions_json,
        open_questions_json: value.open_questions_json,
        content_sha256: value.content_sha256,
        evidence_manifest_json: value.evidence_manifest_json,
        evidence_manifest_sha256: value.evidence_manifest_sha256,
        evidence_read_count: value.evidence_read_count,
        idempotency_key: value.idempotency_key,
        status: match value.status {
            codex_state::WorkspacePlanRevisionStatus::Current => {
                WorkspacePlanRevisionStatus::Current
            }
            codex_state::WorkspacePlanRevisionStatus::Outdated => {
                WorkspacePlanRevisionStatus::Outdated
            }
            codex_state::WorkspacePlanRevisionStatus::Submitted => {
                WorkspacePlanRevisionStatus::Submitted
            }
        },
        source_checkpoint_id: value.source_checkpoint_id,
        source_checkpoint_revision: value.source_checkpoint_revision,
        source_checkpoint_sha256: value.source_checkpoint_sha256,
        encounter_id: value.encounter_id,
        note_id: value.note_id,
        source_thread_id: value.source_thread_id,
        source_turn_id: value.source_turn_id,
        created_at: value.created_at.timestamp_millis(),
        submitted_at: value.submitted_at.map(|value| value.timestamp_millis()),
        replayed: value.replayed,
    })
}

fn api_plan_submission_receipt(
    value: codex_state::WorkspacePlanSubmissionReceipt,
) -> WorkspacePlanSubmissionReceipt {
    WorkspacePlanSubmissionReceipt {
        plan_revision_id: value.plan_revision_id,
        packet_id: value.packet_id,
        agent_run_id: value.agent_run_id,
        plan_session_id: value.plan_session_id,
        client_id: value.client_id,
        plan_content_sha256: value.plan_content_sha256,
        evidence_manifest_sha256: value.evidence_manifest_sha256,
        submitted_by: value.submitted_by,
        submitted_at: value.submitted_at.timestamp_millis(),
    }
}

fn api_plan_recovery(
    value: codex_state::WorkspacePlanRecoveryState,
) -> Result<WorkspacePlanRecoveryState, JSONRPCErrorError> {
    Ok(WorkspacePlanRecoveryState {
        session: api_plan_session(value.session),
        active_runs: value
            .active_runs
            .into_iter()
            .map(api_plan_active_run)
            .collect(),
        pending_questions: value
            .pending_questions
            .into_iter()
            .map(api_plan_message)
            .collect(),
        current_revision: value.current_revision.map(api_plan_revision).transpose()?,
        last_completion: value.last_completion.map(api_plan_completion_receipt),
    })
}

fn api_plan_active_run(value: codex_state::WorkspacePlanActiveRun) -> WorkspacePlanActiveRun {
    WorkspacePlanActiveRun {
        run: api_plan_guide_run(value.run),
        plan_session_id: value.plan_session_id,
        source_thread_id: value.source_thread_id,
        source_turn_id: value.source_turn_id,
        provider: value.provider,
        model: value.model,
        prompt_sha256: value.prompt_sha256,
        context_read_count: value.context_read_count,
        claimed_at: value.claimed_at.timestamp_millis(),
    }
}

fn api_plan_completion_receipt(
    value: codex_state::WorkspacePlanTurnCompletionReceipt,
) -> WorkspacePlanTurnCompletionReceipt {
    WorkspacePlanTurnCompletionReceipt {
        guide_run_id: value.guide_run_id,
        plan_session_id: value.plan_session_id,
        client_id: value.client_id,
        idempotency_key: value.idempotency_key,
        assistant_message_id: value.assistant_message_id,
        plan_revision_id: value.plan_revision_id,
        completion_input_sha256: value.completion_input_sha256,
        evidence_manifest_sha256: value.evidence_manifest_sha256,
        evidence_read_count: value.evidence_read_count,
        terminal_envelope_sha256: value.terminal_envelope_sha256,
        source_checkpoint_id: value.source_checkpoint_id,
        source_checkpoint_revision: value.source_checkpoint_revision,
        source_checkpoint_sha256: value.source_checkpoint_sha256,
        source_thread_id: value.source_thread_id,
        source_turn_id: value.source_turn_id,
        provider: value.provider,
        model: value.model,
        prompt_sha256: value.prompt_sha256,
        completed_at: value.completed_at.timestamp_millis(),
        replayed: value.replayed,
    }
}

fn api_plan_proposal(
    value: codex_state::WorkspacePlanProposal,
) -> Result<WorkspacePlanProposal, JSONRPCErrorError> {
    let payload = match value.payload {
        codex_state::WorkspacePlanProposalPayload::NoteRevision {
            note_id,
            base_revision,
            proposed_body,
        } => WorkspacePlanProposalPayload::NoteRevision {
            note_id,
            base_revision,
            proposed_body,
        },
        codex_state::WorkspacePlanProposalPayload::NoteAddendum {
            note_id,
            base_revision,
            body,
        } => WorkspacePlanProposalPayload::NoteAddendum {
            note_id,
            base_revision,
            body,
        },
        codex_state::WorkspacePlanProposalPayload::TaskDraft {
            title,
            details,
            task_kind,
            priority,
            due_date,
            assigned_to,
        } => WorkspacePlanProposalPayload::TaskDraft {
            title,
            details,
            task_kind,
            priority: match priority {
                codex_state::WorkspaceTaskPriority::Low => {
                    codex_app_server_protocol::WorkspaceTaskPriority::Low
                }
                codex_state::WorkspaceTaskPriority::Normal => {
                    codex_app_server_protocol::WorkspaceTaskPriority::Normal
                }
                codex_state::WorkspaceTaskPriority::High => {
                    codex_app_server_protocol::WorkspaceTaskPriority::High
                }
                codex_state::WorkspaceTaskPriority::Urgent => {
                    codex_app_server_protocol::WorkspaceTaskPriority::Urgent
                }
            },
            due_date,
            assigned_to,
        },
    };
    Ok(WorkspacePlanProposal {
        id: value.id,
        plan_session_id: value.plan_session_id,
        plan_revision_id: value.plan_revision_id,
        client_id: value.client_id,
        guide_run_id: value.guide_run_id,
        payload,
        payload_sha256: value.payload_sha256,
        summary: value.summary,
        rationale: value.rationale,
        idempotency_key: value.idempotency_key,
        status: match value.status {
            codex_state::WorkspacePlanProposalStatus::Pending => {
                WorkspacePlanProposalStatus::Pending
            }
            codex_state::WorkspacePlanProposalStatus::Accepted => {
                WorkspacePlanProposalStatus::Accepted
            }
            codex_state::WorkspacePlanProposalStatus::Declined => {
                WorkspacePlanProposalStatus::Declined
            }
            codex_state::WorkspacePlanProposalStatus::Outdated => {
                WorkspacePlanProposalStatus::Outdated
            }
        },
        source_checkpoint_id: value.source_checkpoint_id,
        source_checkpoint_revision: value.source_checkpoint_revision,
        source_checkpoint_sha256: value.source_checkpoint_sha256,
        source_thread_id: value.source_thread_id,
        source_turn_id: value.source_turn_id,
        created_at: value.created_at.timestamp_millis(),
        resolved_at: value.resolved_at.map(|value| value.timestamp_millis()),
        resolved_by: value.resolved_by,
        replayed: value.replayed,
    })
}

fn api_plan_guide_run(value: codex_state::WorkspaceGuideRun) -> WorkspacePlanGuideRun {
    WorkspacePlanGuideRun {
        id: value.id,
        client_id: value.client_id,
        draft_session_id: value.session_id,
        source_checkpoint_id: value.source_checkpoint_id,
        source_checkpoint_revision: value.source_checkpoint_revision,
        source_checkpoint_sha256: value.source_checkpoint_sha256,
        encounter_id: value.encounter_id,
        note_id: value.note_id,
        request_envelope_sha256: value.request_envelope_sha256,
        idempotency_key: value.idempotency_key,
        trigger: value.trigger,
        provider: value.provider,
        model: value.model,
        status: match value.status {
            codex_state::WorkspaceGuideRunStatus::Running => WorkspacePlanGuideRunStatus::Running,
            codex_state::WorkspaceGuideRunStatus::Completed => {
                WorkspacePlanGuideRunStatus::Completed
            }
            codex_state::WorkspaceGuideRunStatus::Failed => WorkspacePlanGuideRunStatus::Failed,
            codex_state::WorkspaceGuideRunStatus::Canceled => WorkspacePlanGuideRunStatus::Canceled,
        },
        source_thread_id: value.source_thread_id,
        source_turn_id: value.source_turn_id,
        created_at: value.created_at.timestamp_millis(),
        updated_at: value.updated_at.timestamp_millis(),
        terminal_at: value.terminal_at.map(|value| value.timestamp_millis()),
        is_stale: value.is_stale,
        replayed: value.replayed,
    }
}
