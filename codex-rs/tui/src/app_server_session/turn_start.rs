//! Construction of app-server `turn/start` requests.

use std::path::PathBuf;

use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use codex_config::types::ApprovalsReviewer;
use codex_protocol::ThreadId;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModelToolMode;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ReasoningEffort;
use codex_utils_absolute_path::AbsolutePathBuf;

use super::TurnPermissionsOverride;
use super::turn_permissions_overrides;

pub(super) struct TurnStartRequest<'a> {
    pub(super) thread_id: ThreadId,
    pub(super) items: Vec<UserInput>,
    pub(super) cwd: PathBuf,
    pub(super) approval_policy: AskForApproval,
    pub(super) approvals_reviewer: ApprovalsReviewer,
    pub(super) permissions_override: TurnPermissionsOverride,
    pub(super) workspace_roots: &'a [AbsolutePathBuf],
    pub(super) model: String,
    pub(super) effort: Option<ReasoningEffort>,
    pub(super) summary: Option<ReasoningSummary>,
    pub(super) service_tier: Option<Option<String>>,
    pub(super) collaboration_mode: Option<CollaborationMode>,
    pub(super) personality: Option<Personality>,
    pub(super) output_schema: Option<serde_json::Value>,
    pub(super) model_tool_mode: Option<ModelToolMode>,
}

impl TurnStartRequest<'_> {
    pub(super) fn into_params(self) -> TurnStartParams {
        let (sandbox_policy, permissions) =
            turn_permissions_overrides(self.permissions_override, self.cwd.as_path());
        TurnStartParams {
            thread_id: self.thread_id.to_string(),
            client_user_message_id: None,
            input: self.items,
            responsesapi_client_metadata: None,
            additional_context: None,
            environments: None,
            cwd: Some(self.cwd),
            runtime_workspace_roots: Some(self.workspace_roots.to_vec()),
            approval_policy: Some(self.approval_policy),
            approvals_reviewer: Some(self.approvals_reviewer.into()),
            sandbox_policy,
            permissions,
            model: Some(self.model),
            service_tier: self.service_tier,
            effort: self.effort,
            summary: self.summary,
            personality: self.personality,
            output_schema: self.output_schema,
            model_tool_mode: self.model_tool_mode,
            collaboration_mode: self.collaboration_mode,
            multi_agent_mode: None,
        }
    }
}

#[cfg(test)]
#[path = "turn_start_tests.rs"]
mod tests;
