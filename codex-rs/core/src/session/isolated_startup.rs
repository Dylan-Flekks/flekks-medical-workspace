//! Minimal session construction for one-shot isolated model requests.
//!
//! This path deliberately does not initialize persistence, MCP servers, hooks, plugins, skills,
//! AGENTS instructions, shells, execution environments, managed proxies, lifecycle contributors,
//! rollout traces, startup prewarming, external clocks, or analytics delivery.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use anyhow::Context;
use anyhow::ensure;
use arc_swap::ArcSwap;
use arc_swap::ArcSwapOption;
use async_channel::Sender;
use codex_analytics::AnalyticsEventsClient;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_connectors::ConnectorSnapshot;
use codex_core_plugins::PluginsManager;
use codex_exec_server::EnvironmentManager;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionDataInit;
use codex_hooks::Hooks;
use codex_login::AuthManager;
use codex_mcp::McpConfig;
use codex_mcp::McpConnectionManager;
use codex_mcp::McpResourceClient;
use codex_mcp::McpRuntimeContext;
use codex_mcp::ResolvedMcpCatalog;
use codex_models_manager::manager::SharedModelsManager;
use codex_otel::SessionTelemetry;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::SessionConfiguredEvent;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_rollout_trace::ThreadTraceContext;
use codex_thread_store::ThreadStore;
use rmcp::model::ElicitationCapability;
use tokio::sync::Mutex;
use tokio::sync::Semaphore;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::SkillsService;
use crate::agent::AgentControl;
use crate::agents_md_manager::AgentsMdManager;
use crate::client::ModelClient;
use crate::config::Config;
use crate::config::Constrained;
use crate::current_time::SleepFuture;
use crate::current_time::TimeFuture;
use crate::current_time::TimeProvider;
use crate::environment_selection::ThreadEnvironments;
use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::exec_policy::ExecPolicyManager;
use crate::guardian::GuardianReviewSessionManager;
use crate::mcp::McpManager;
use crate::realtime_conversation::RealtimeConversationManager;
use crate::shell;
use crate::shell_snapshot::ShellSnapshot;
use crate::state::AutoCompactWindowIds;
use crate::state::SessionServices;
use crate::state::SessionState;
use crate::tools::network_approval::NetworkApprovalService;
use crate::tools::sandboxing::ApprovalStore;
use crate::unified_exec::UnifiedExecProcessManager;

use super::INITIAL_SUBMIT_ID;
use super::McpRuntimeSnapshot;
use super::input_queue::InputQueue;
use super::session::Session;
use super::session::SessionConfiguration;

pub(super) struct IsolatedSessionInit {
    pub(super) session_configuration: SessionConfiguration,
    pub(super) config: Arc<Config>,
    pub(super) installation_id: String,
    pub(super) auth_manager: Arc<AuthManager>,
    pub(super) models_manager: SharedModelsManager,
    pub(super) exec_policy: Arc<ExecPolicyManager>,
    pub(super) tx_event: Sender<Event>,
    pub(super) agent_status: watch::Sender<AgentStatus>,
    pub(super) skills_service: Arc<SkillsService>,
    pub(super) plugins_manager: Arc<PluginsManager>,
    pub(super) mcp_manager: Arc<McpManager>,
    pub(super) code_mode_session_provider: Arc<dyn codex_code_mode::CodeModeSessionProvider>,
    pub(super) thread_extension_init: ExtensionDataInit,
    pub(super) agent_control: AgentControl,
    pub(super) thread_store: Arc<dyn ThreadStore>,
}

pub(super) async fn new(mut input: IsolatedSessionInit) -> anyhow::Result<Arc<Session>> {
    validate_input(&input)?;

    let thread_id = ThreadId::default();
    let session_id = SessionId::from(thread_id);
    input.session_configuration.session_source = SessionSource::Unknown;
    input.session_configuration.history_mode = ThreadHistoryMode::Legacy;
    input.session_configuration.metrics_service_name = None;
    input.session_configuration.app_server_client_name = None;
    input.session_configuration.app_server_client_version = None;
    input.session_configuration.originator = "isolated".to_string();
    input.session_configuration.service_tier = None;

    let model = input
        .session_configuration
        .collaboration_mode
        .model()
        .to_string();
    let initial_auto_compact_window_ids = AutoCompactWindowIds::new_initial();
    let agent_control = input
        .agent_control
        .with_session_id(session_id, /*max_threads*/ 1);
    let multi_agent_version = OnceLock::from(MultiAgentVersion::Disabled);
    let mcp_thread_init = input.thread_extension_init.clone();
    let thread_extension_data =
        ExtensionData::new_with_init(thread_id.to_string(), input.thread_extension_init);
    let session_extension_data = ExtensionData::new(session_id.to_string());

    let environment_manager = Arc::new(EnvironmentManager::without_environments());
    let inert_shell = shell::Shell {
        shell_type: shell::ShellType::Sh,
        shell_path: PathBuf::new(),
    };
    let turn_environments = Arc::new(ThreadEnvironments::new(
        Arc::clone(&environment_manager),
        inert_shell.clone(),
        ShellSnapshot::disabled(),
        TurnEnvironmentSnapshot::default(),
        /*non_blocking_snapshots*/ false,
    ));

    let isolated_approval_policy = Constrained::allow_any(AskForApproval::Never);
    let isolated_permission_profile = PermissionProfile::Disabled;
    let mcp_connection_manager = Arc::new(
        McpConnectionManager::new_uninitialized_with_permission_profile(
            &isolated_approval_policy,
            &isolated_permission_profile,
            /*prefix_mcp_tool_names*/ false,
        ),
    );
    let mcp_runtime_context = McpRuntimeContext::new(environment_manager, PathBuf::new());
    let mcp_runtime = Arc::new(McpRuntimeSnapshot::new(
        Arc::new(McpConfig {
            chatgpt_base_url: String::new(),
            apps_mcp_product_sku: None,
            codex_home: PathBuf::new(),
            mcp_oauth_credentials_store_mode: OAuthCredentialsStoreMode::default(),
            auth_keyring_backend_kind: AuthKeyringBackendKind::default(),
            mcp_oauth_callback_port: None,
            mcp_oauth_callback_url: None,
            skill_mcp_dependency_install_enabled: false,
            approval_policy: isolated_approval_policy,
            codex_linux_sandbox_exe: None,
            use_legacy_landlock: false,
            apps_enabled: false,
            prefix_mcp_tool_names: false,
            client_elicitation_capability: ElicitationCapability::default(),
            mcp_server_catalog: ResolvedMcpCatalog::default(),
            connector_snapshot: ConnectorSnapshot::default(),
        }),
        /*plugins_available*/ false,
        Arc::clone(&mcp_connection_manager),
        mcp_runtime_context,
        Vec::new(),
    ));
    let mcp_connection_manager_swap = Arc::new(ArcSwap::from(Arc::clone(&mcp_connection_manager)));
    session_extension_data.insert(McpResourceClient::new(Arc::clone(
        &mcp_connection_manager_swap,
    )));

    let session_telemetry = SessionTelemetry::new(
        thread_id,
        &model,
        &model,
        /*account_id*/ None,
        /*account_email*/ None,
        /*auth_mode*/ None,
        "isolated".to_string(),
        /*log_user_prompts*/ false,
        "isolated".to_string(),
        SessionSource::Unknown,
    );
    let time_provider: Arc<dyn TimeProvider> = Arc::new(IsolatedTimeProvider);
    let state = SessionState::new_with_auto_compact_window_ids(
        input.session_configuration.clone(),
        initial_auto_compact_window_ids,
    );
    let services = SessionServices {
        mcp_connection_manager: mcp_connection_manager_swap,
        mcp_runtime: ArcSwapOption::from(Some(mcp_runtime)),
        mcp_projection_lock: Mutex::new(()),
        mcp_startup_cancellation_token: Mutex::new(CancellationToken::new()),
        unified_exec_manager: UnifiedExecProcessManager::default(),
        elicitations: crate::elicitation::ElicitationService::new(),
        shell_zsh_path: None,
        main_execve_wrapper_exe: None,
        analytics_events_client: AnalyticsEventsClient::disabled(),
        hooks: ArcSwap::from_pointee(Hooks::default()),
        rollout_thread_trace: ThreadTraceContext::disabled(),
        user_shell: Arc::new(inert_shell),
        show_raw_agent_reasoning: false,
        exec_policy: input.exec_policy,
        auth_manager: Arc::clone(&input.auth_manager),
        models_manager: Arc::clone(&input.models_manager),
        session_telemetry,
        tool_approvals: Mutex::new(ApprovalStore::default()),
        guardian_rejections: Mutex::new(HashMap::new()),
        guardian_rejection_circuit_breaker: Mutex::new(Default::default()),
        runtime_handle: tokio::runtime::Handle::current(),
        skills_service: input.skills_service,
        agents_md_manager: Arc::new(AgentsMdManager::new(None)),
        plugins_manager: input.plugins_manager,
        mcp_manager: input.mcp_manager,
        extensions: codex_extension_api::empty_extension_registry(),
        session_extension_data,
        thread_extension_data,
        selected_capability_roots: Vec::new(),
        mcp_thread_init,
        supports_openai_form_elicitation: AtomicBool::new(false),
        agent_control,
        network_proxy: ArcSwapOption::empty(),
        network_proxy_audit_metadata: Default::default(),
        managed_network_requirements_configured: false,
        network_approval: Arc::new(NetworkApprovalService::default()),
        state_db: None,
        live_thread: None,
        thread_store: input.thread_store,
        attestation_provider: None,
        time_provider,
        model_client: ModelClient::new_isolated(
            input.auth_manager,
            thread_id,
            input.session_configuration.provider.clone(),
            input.config.http_client_factory(),
        )?,
        code_mode_service: crate::tools::code_mode::CodeModeService::new(
            input.code_mode_session_provider,
        ),
        tool_search_handler_cache: Default::default(),
        turn_environments,
    };
    let sess = Arc::new(Session {
        thread_id,
        installation_id: String::new(),
        tx_event: input.tx_event,
        agent_status: input.agent_status,
        state: Mutex::new(state),
        managed_network_proxy_refresh_lock: Semaphore::new(/*permits*/ 1),
        features: input.config.features.clone(),
        multi_agent_version,
        pending_mcp_server_refresh_config: Mutex::new(None),
        conversation: Arc::new(RealtimeConversationManager::new()),
        active_turn: Mutex::new(None),
        input_queue: InputQueue::new(),
        guardian_review_session: GuardianReviewSessionManager::default(),
        services,
        next_internal_sub_id: AtomicU64::new(0),
    });

    sess.send_event_raw(Event {
        id: INITIAL_SUBMIT_ID.to_owned(),
        msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
            session_id,
            thread_id,
            forked_from_id: None,
            parent_thread_id: None,
            thread_source: input.session_configuration.thread_source.clone(),
            thread_name: None,
            model,
            model_provider_id: input.config.model_provider_id.clone(),
            service_tier: None,
            approval_policy: input.session_configuration.approval_policy.value(),
            approvals_reviewer: input.session_configuration.approvals_reviewer,
            permission_profile: input.session_configuration.permission_profile(),
            active_permission_profile: input.session_configuration.active_permission_profile(),
            cwd: input.session_configuration.cwd().clone(),
            reasoning_effort: input
                .session_configuration
                .collaboration_mode
                .reasoning_effort(),
            initial_messages: None,
            network_proxy: None,
            rollout_path: None,
        }),
    })
    .await;

    Ok(sess)
}

fn validate_input(input: &IsolatedSessionInit) -> anyhow::Result<()> {
    ensure!(
        input.session_configuration.model_tool_mode.is_isolated(),
        "isolated startup requires isolated model mode"
    );
    ensure!(input.config.ephemeral, "isolated startup must be ephemeral");
    ensure!(
        input.session_configuration.dynamic_tools.is_empty(),
        "isolated startup cannot install dynamic tools"
    );
    ensure!(
        input
            .session_configuration
            .environment_selections()
            .is_empty(),
        "isolated startup cannot attach execution environments"
    );
    ensure!(
        input.session_configuration.workspace_roots.is_empty(),
        "isolated startup cannot attach workspace roots"
    );
    ensure!(
        input.session_configuration.forked_from_thread_id.is_none()
            && input.session_configuration.parent_thread_id.is_none(),
        "isolated startup cannot inherit another thread"
    );
    ensure!(
        input.session_configuration.user_shell_override.is_none(),
        "isolated startup cannot inherit a shell"
    );
    ensure!(
        !input.installation_id.trim().is_empty(),
        "thread manager installation identity must be initialized"
    );
    input
        .config
        .model
        .as_deref()
        .context("isolated startup requires an explicit model")?;
    Ok(())
}

struct IsolatedTimeProvider;

impl TimeProvider for IsolatedTimeProvider {
    fn current_time(&self, _thread_id: ThreadId) -> TimeFuture<'_> {
        Box::pin(async { anyhow::bail!("current time is unavailable in isolated model mode") })
    }

    fn sleep(&self, _thread_id: ThreadId, _duration: Duration) -> SleepFuture<'_> {
        Box::pin(async { anyhow::bail!("timers are unavailable in isolated model mode") })
    }
}
