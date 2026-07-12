use super::*;
use crate::error_code::method_not_found;
use crate::thread_state::IsolatedTurnClaimResult;

const MAX_ISOLATED_INPUT_BYTES: usize = 32 * 1024;

impl TurnRequestProcessor {
    pub(crate) async fn connection_is_isolated(&self, connection_id: ConnectionId) -> bool {
        self.thread_state_manager
            .connection_is_isolated(connection_id)
            .await
    }

    pub(crate) async fn thread_is_isolated(&self, thread_id: &str) -> bool {
        let Ok(thread_id) = ThreadId::from_string(thread_id) else {
            return false;
        };
        let Ok(thread) = self.thread_manager.get_thread(thread_id).await else {
            return false;
        };
        thread.config_snapshot().await.model_tool_mode.is_isolated()
    }

    pub(super) async fn isolated_turn_start(
        &self,
        request_id: ConnectionRequestId,
        params: TurnStartParams,
        thread_id: ThreadId,
        thread: Arc<CodexThread>,
    ) -> Result<TurnStartResponse, JSONRPCErrorError> {
        validate_isolated_turn_start(&params)?;

        match self
            .thread_state_manager
            .claim_isolated_thread_turn(thread_id, request_id.connection_id)
            .await
        {
            IsolatedTurnClaimResult::Claimed => {}
            IsolatedTurnClaimResult::AlreadyClaimed => {
                return Err(invalid_request(
                    "isolated model mode permits exactly one turn",
                ));
            }
            IsolatedTurnClaimResult::NotFound | IsolatedTurnClaimResult::NotOwner => {
                return Err(invalid_request(format!("thread not found: {thread_id}")));
            }
        }

        let TurnStartParams {
            input,
            output_schema,
            ..
        } = params;
        let turn_op = Op::UserInput {
            items: input.into_iter().map(V2UserInput::into_core).collect(),
            final_output_json_schema: output_schema,
            responsesapi_client_metadata: None,
            additional_context: BTreeMap::new(),
            thread_settings: Default::default(),
        };
        let turn_id = match thread.submit_with_trace(turn_op, /*trace*/ None).await {
            Ok(turn_id) => turn_id,
            Err(err) => {
                self.thread_state_manager
                    .release_isolated_thread_turn_claim(thread_id, request_id.connection_id)
                    .await;
                return Err(match err {
                    CodexErr::InvalidRequest(message) => invalid_request(message),
                    CodexErr::UnsupportedOperation(message) => method_not_found(message),
                    err => internal_error(format!("failed to start isolated turn: {err}")),
                });
            }
        };

        // Keep the request-to-turn correlation local to app-server bookkeeping.
        // No trace context is requested from the client or forwarded to core.
        self.outgoing
            .record_request_turn_id(&request_id, &turn_id)
            .await;
        Ok(TurnStartResponse {
            turn: Turn {
                id: turn_id,
                items: Vec::new(),
                items_view: TurnItemsView::NotLoaded,
                error: None,
                status: TurnStatus::InProgress,
                started_at: None,
                completed_at: None,
                duration_ms: None,
            },
        })
    }
}

fn validate_isolated_turn_start(params: &TurnStartParams) -> Result<(), JSONRPCErrorError> {
    let [
        V2UserInput::Text {
            text,
            text_elements,
        },
    ] = params.input.as_slice()
    else {
        return Err(invalid_request(
            "isolated model mode requires exactly one plain text input",
        ));
    };
    if text.trim().is_empty() {
        return Err(invalid_request(
            "isolated model mode input must not be empty",
        ));
    }
    if !text_elements.is_empty() {
        return Err(invalid_request(
            "isolated model mode does not accept text elements",
        ));
    }
    if text.len() > MAX_ISOLATED_INPUT_BYTES {
        return Err(invalid_request(format!(
            "isolated model mode input exceeds the {MAX_ISOLATED_INPUT_BYTES}-byte limit"
        )));
    }
    if params.output_schema.is_none() {
        return Err(invalid_request(
            "isolated model mode requires a strict object output schema",
        ));
    }

    let unsupported = [
        (
            "clientUserMessageId",
            params.client_user_message_id.is_some(),
        ),
        (
            "responsesapiClientMetadata",
            params.responsesapi_client_metadata.is_some(),
        ),
        ("additionalContext", params.additional_context.is_some()),
        ("environments", params.environments.is_some()),
        ("cwd", params.cwd.is_some()),
        (
            "runtimeWorkspaceRoots",
            params.runtime_workspace_roots.is_some(),
        ),
        ("approvalPolicy", params.approval_policy.is_some()),
        ("approvalsReviewer", params.approvals_reviewer.is_some()),
        ("sandboxPolicy", params.sandbox_policy.is_some()),
        ("permissions", params.permissions.is_some()),
        ("model", params.model.is_some()),
        ("serviceTier", params.service_tier.is_some()),
        ("effort", params.effort.is_some()),
        ("summary", params.summary.is_some()),
        ("personality", params.personality.is_some()),
        ("modelToolMode", params.model_tool_mode.is_some()),
        ("collaborationMode", params.collaboration_mode.is_some()),
        ("multiAgentMode", params.multi_agent_mode.is_some()),
    ];
    if let Some((field, _)) = unsupported.into_iter().find(|(_, present)| *present) {
        return Err(invalid_request(format!(
            "isolated model mode does not accept `{field}`"
        )));
    }

    Ok(())
}
