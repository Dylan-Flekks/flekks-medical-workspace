use super::*;
use codex_protocol::models::PermissionProfile;

impl ThreadRequestProcessor {
    pub(super) async fn isolated_thread_start(
        &self,
        request_id: ConnectionRequestId,
        params: ThreadStartParams,
    ) -> Result<(), JSONRPCErrorError> {
        validate_isolated_thread_start(&params)?;

        let mut config = self.config.as_ref().clone();
        let Some(model) = params.model.clone() else {
            return Err(invalid_request(
                "isolated model mode requires an explicit non-empty model",
            ));
        };
        config.model = Some(model);
        if let Some(model_provider_id) = params.model_provider.as_ref() {
            let model_provider = config
                .model_providers
                .get(model_provider_id)
                .cloned()
                .ok_or_else(|| {
                    invalid_request(format!(
                        "unknown model provider `{model_provider_id}` for isolated model mode"
                    ))
                })?;
            config.model_provider_id = model_provider_id.clone();
            config.model_provider = model_provider;
        }
        config.ephemeral = true;
        config.service_tier = None;
        config.workspace_roots.clear();
        config.workspace_roots_explicit = true;
        config.permissions.set_workspace_roots(Vec::new());
        config
            .permissions
            .set_permission_profile(PermissionProfile::read_only())
            .map_err(|err| {
                invalid_request(format!(
                    "isolated model mode could not select read-only permissions: {err}"
                ))
            })?;

        let listener_task_context = self.listener_task_context();
        let outgoing = Arc::clone(&listener_task_context.outgoing);
        let error_request_id = request_id.clone();
        let thread_source = params.thread_source.map(Into::into);
        let thread_start_task = async move {
            if let Err(error) = Self::isolated_thread_start_task(
                listener_task_context,
                request_id,
                config,
                thread_source,
            )
            .await
            {
                outgoing.send_error(error_request_id, error).await;
            }
        };
        // Do not attach the incoming request span. Isolated creation must not
        // inherit an app-server trace carrier or ambient request context.
        self.background_tasks.spawn(thread_start_task);
        Ok(())
    }

    async fn isolated_thread_start_task(
        listener_task_context: ListenerTaskContext,
        request_id: ConnectionRequestId,
        config: Config,
        thread_source: Option<codex_protocol::protocol::ThreadSource>,
    ) -> Result<(), JSONRPCErrorError> {
        let mut thread_extension_init = ExtensionDataInit::new();
        thread_extension_init.insert(ModelToolMode::Isolated);

        let new_thread = listener_task_context
            .thread_manager
            .start_isolated_thread_unpublished(StartThreadOptions {
                config,
                allow_provider_model_fallback: false,
                initial_history: InitialHistory::New,
                history_mode: None,
                session_source: None,
                thread_source,
                dynamic_tools: Vec::new(),
                metrics_service_name: None,
                parent_trace: None,
                environments: Vec::new(),
                thread_extension_init,
                supports_openai_form_elicitation: false,
            })
            .await
            .map_err(|err| match err {
                CodexErr::InvalidRequest(message) => invalid_request(message),
                CodexErr::UnsupportedOperation(message) => method_not_found(message),
                err => internal_error(format!("error creating isolated thread: {err}")),
            })?;
        let thread_id = new_thread.thread_id;
        let thread = Arc::clone(&new_thread.thread);
        let session_configured = new_thread.session_configured.clone();

        let published = match listener_task_context
            .thread_state_manager
            .register_and_publish_isolated_thread(
                listener_task_context.thread_manager.as_ref(),
                &new_thread,
                request_id.connection_id,
            )
            .await
        {
            Ok(published) => published,
            Err(err) => {
                let _ = super::super::thread_lifecycle::wait_for_thread_shutdown(&thread).await;
                return Err(match err {
                    CodexErr::InvalidRequest(message) => invalid_request(message),
                    CodexErr::UnsupportedOperation(message) => method_not_found(message),
                    err => internal_error(format!("error publishing isolated thread: {err}")),
                });
            }
        };
        if !published {
            let _ = super::super::thread_lifecycle::wait_for_thread_shutdown(&thread).await;
            return Err(invalid_request(
                "isolated thread owner connection closed during creation",
            ));
        }

        if !listener_task_context
            .thread_watch_manager
            .route_status_notifications_to_connection(
                &thread_id.to_string(),
                request_id.connection_id,
            )
            .await
        {
            teardown_failed_isolated_thread_start(
                &listener_task_context,
                thread_id,
                &thread,
                /*listener_attached*/ false,
            )
            .await;
            return Err(internal_error(
                "isolated thread status route did not match its owner",
            ));
        }

        let config_snapshot = thread.config_snapshot().await;
        let mut api_thread = build_thread_from_snapshot(
            thread_id,
            session_configured.session_id.to_string(),
            &config_snapshot,
            session_configured.rollout_path.clone(),
        );
        if !listener_task_context
            .thread_watch_manager
            .upsert_isolated_thread_silently(api_thread.clone(), request_id.connection_id)
            .await
        {
            teardown_failed_isolated_thread_start(
                &listener_task_context,
                thread_id,
                &thread,
                /*listener_attached*/ false,
            )
            .await;
            return Err(internal_error(
                "isolated thread status route closed during creation",
            ));
        }

        match super::super::thread_lifecycle::ensure_isolated_conversation_listener(
            listener_task_context.clone(),
            thread_id,
            request_id.connection_id,
            &config_snapshot,
        )
        .await
        {
            Ok(EnsureConversationListenerResult::Attached) => {}
            Ok(EnsureConversationListenerResult::ConnectionClosed) => {
                teardown_failed_isolated_thread_start(
                    &listener_task_context,
                    thread_id,
                    &thread,
                    /*listener_attached*/ false,
                )
                .await;
                return Err(invalid_request(
                    "isolated thread owner connection closed during creation",
                ));
            }
            Err(error) => {
                teardown_failed_isolated_thread_start(
                    &listener_task_context,
                    thread_id,
                    &thread,
                    /*listener_attached*/ false,
                )
                .await;
                return Err(error);
            }
        }

        if !listener_task_context
            .thread_state_manager
            .isolated_thread_has_live_owner(thread_id, request_id.connection_id)
            .await
        {
            teardown_failed_isolated_thread_start(
                &listener_task_context,
                thread_id,
                &thread,
                /*listener_attached*/ true,
            )
            .await;
            return Err(invalid_request(
                "isolated thread owner connection closed during creation",
            ));
        }
        api_thread.status = resolve_thread_status(
            listener_task_context
                .thread_watch_manager
                .loaded_status_for_thread(&api_thread.id)
                .await,
            /*has_in_progress_turn*/ false,
        );

        let response = ThreadStartResponse {
            thread: api_thread.clone(),
            model: config_snapshot.model.clone(),
            model_provider: config_snapshot.model_provider_id.clone(),
            service_tier: config_snapshot.service_tier.clone(),
            cwd: config_snapshot.cwd().clone(),
            runtime_workspace_roots: config_snapshot.workspace_roots.clone(),
            instruction_sources: Vec::new(),
            approval_policy: config_snapshot.approval_policy.into(),
            approvals_reviewer: config_snapshot.approvals_reviewer.into(),
            sandbox: thread_response_sandbox_policy(
                &config_snapshot.permission_profile,
                config_snapshot.cwd().as_path(),
            ),
            active_permission_profile: thread_response_active_permission_profile(
                config_snapshot.active_permission_profile.clone(),
            ),
            reasoning_effort: config_snapshot.reasoning_effort.clone(),
            model_tool_mode: config_snapshot.model_tool_mode,
            multi_agent_mode: MultiAgentMode::ExplicitRequestOnly,
        };
        let notification = thread_started_notification(api_thread);
        let connection_id = request_id.connection_id;
        listener_task_context
            .outgoing
            .send_response_without_analytics(request_id, response)
            .await;
        listener_task_context
            .outgoing
            .send_server_notification_to_connections(
                &[connection_id],
                ServerNotification::ThreadStarted(notification),
            )
            .await;
        Ok(())
    }
}

async fn teardown_failed_isolated_thread_start(
    listener_task_context: &ListenerTaskContext,
    thread_id: ThreadId,
    thread: &Arc<CodexThread>,
    listener_attached: bool,
) {
    let removed_thread = listener_task_context
        .thread_manager
        .remove_thread(&thread_id)
        .await;
    listener_task_context
        .outgoing
        .cancel_requests_for_thread(thread_id, /*error*/ None)
        .await;
    listener_task_context
        .thread_state_manager
        .remove_thread_state(thread_id)
        .await;
    listener_task_context
        .thread_watch_manager
        .remove_thread(&thread_id.to_string())
        .await;
    if !listener_attached {
        listener_task_context
            .thread_watch_manager
            .note_status_listener_closed(&thread_id.to_string())
            .await;
    }
    listener_task_context
        .pending_thread_unloads
        .lock()
        .await
        .remove(&thread_id);
    let thread = removed_thread.unwrap_or_else(|| Arc::clone(thread));
    let _ = super::super::thread_lifecycle::wait_for_thread_shutdown(&thread).await;
}

fn validate_isolated_thread_start(params: &ThreadStartParams) -> Result<(), JSONRPCErrorError> {
    if params
        .model
        .as_deref()
        .is_none_or(|model| model.trim().is_empty())
    {
        return Err(invalid_request(
            "isolated model mode requires an explicit non-empty model",
        ));
    }
    if params
        .model_provider
        .as_deref()
        .is_some_and(|provider| provider.trim().is_empty())
    {
        return Err(invalid_request(
            "isolated model mode does not accept an empty model provider",
        ));
    }
    if params.allow_provider_model_fallback {
        return Err(invalid_request(
            "isolated model mode requires provider model fallback to remain disabled",
        ));
    }
    if params.ephemeral != Some(true) {
        return Err(invalid_request(
            "isolated model mode requires `ephemeral: true`",
        ));
    }
    if !matches!(
        params.session_start_source,
        None | Some(codex_app_server_protocol::ThreadStartSource::Startup)
    ) {
        return Err(invalid_request(
            "isolated model mode requires fresh startup history",
        ));
    }
    if !matches!(
        params.thread_source.as_ref(),
        None | Some(codex_app_server_protocol::ThreadSource::User)
    ) {
        return Err(invalid_request(
            "isolated model mode accepts only user-originated threads",
        ));
    }

    let unsupported = [
        ("serviceTier", params.service_tier.is_some()),
        ("cwd", params.cwd.is_some()),
        (
            "runtimeWorkspaceRoots",
            params
                .runtime_workspace_roots
                .as_ref()
                .is_some_and(|roots| !roots.is_empty()),
        ),
        ("approvalPolicy", params.approval_policy.is_some()),
        ("approvalsReviewer", params.approvals_reviewer.is_some()),
        ("sandbox", params.sandbox.is_some()),
        ("permissions", params.permissions.is_some()),
        ("config", params.config.is_some()),
        ("serviceName", params.service_name.is_some()),
        ("baseInstructions", params.base_instructions.is_some()),
        (
            "developerInstructions",
            params.developer_instructions.is_some(),
        ),
        ("personality", params.personality.is_some()),
        ("multiAgentMode", params.multi_agent_mode.is_some()),
        ("historyMode", params.history_mode.is_some()),
        (
            "environments",
            params
                .environments
                .as_ref()
                .is_some_and(|environments| !environments.is_empty()),
        ),
        (
            "dynamicTools",
            params
                .dynamic_tools
                .as_ref()
                .is_some_and(|tools| !tools.is_empty()),
        ),
        (
            "selectedCapabilityRoots",
            params
                .selected_capability_roots
                .as_ref()
                .is_some_and(|roots| !roots.is_empty()),
        ),
        (
            "mockExperimentalField",
            params.mock_experimental_field.is_some(),
        ),
        ("experimentalRawEvents", params.experimental_raw_events),
    ];
    if let Some((field, _)) = unsupported.into_iter().find(|(_, present)| *present) {
        return Err(invalid_request(format!(
            "isolated model mode does not accept `{field}`"
        )));
    }

    Ok(())
}
