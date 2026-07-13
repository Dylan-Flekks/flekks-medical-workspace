use super::*;
use codex_protocol::config_types::ModelToolMode;
use pretty_assertions::assert_eq;

fn request_with_tool_mode(model_tool_mode: Option<ModelToolMode>) -> TurnStartRequest<'static> {
    TurnStartRequest {
        thread_id: ThreadId::new(),
        items: vec![UserInput::Text {
            text: "synthetic context packet".to_string(),
            text_elements: Vec::new(),
        }],
        cwd: PathBuf::from("/tmp/medical-context-test"),
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        permissions_override: TurnPermissionsOverride::Preserve,
        workspace_roots: &[],
        model: "test-model".to_string(),
        effort: None,
        summary: None,
        service_tier: None,
        collaboration_mode: None,
        personality: None,
        output_schema: None,
        model_tool_mode,
    }
}

#[test]
fn workspace_context_only_mode_reaches_turn_start_params() {
    let params = request_with_tool_mode(Some(ModelToolMode::WorkspaceContextOnly)).into_params();

    assert_eq!(
        params.model_tool_mode,
        Some(ModelToolMode::WorkspaceContextOnly)
    );
}
