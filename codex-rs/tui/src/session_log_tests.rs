use super::*;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::UserInput;
use codex_protocol::ThreadId;

fn restricted_user_turn(marker: &str) -> AppCommand {
    let mut op = AppCommand::user_turn(
        vec![UserInput::Text {
            text: marker.to_string(),
            text_elements: Vec::new(),
        }],
        PathBuf::from("/tmp/medical-context-test"),
        AskForApproval::Never,
        /*active_permission_profile*/ None,
        "test-model".to_string(),
        /*effort*/ None,
        /*summary*/ None,
        /*service_tier*/ None,
        /*final_output_json_schema*/ None,
        /*collaboration_mode*/ None,
        /*personality*/ None,
    );
    assert!(op.set_user_turn_model_tool_mode(ModelToolMode::WorkspaceContextOnly));
    op
}

#[test]
fn workspace_context_turn_session_log_payload_is_redacted() {
    let op = restricted_user_turn("synthetic clinical marker that must not be logged");

    let payload = outbound_op_payload(&op);

    assert_eq!(
        payload,
        json!({
            "variant": "UserTurn",
            "model_tool_mode": "workspaceContextOnly",
            "input_item_count": 1,
            "payload_redacted": true,
        })
    );
    assert!(!payload.to_string().contains("synthetic clinical marker"));
}

#[test]
fn brace_style_app_event_variant_names_never_include_payloads() {
    let marker = "synthetic medical handoff marker that must not be logged";
    let thread_id = ThreadId::new();
    let history_event = AppEvent::AppendMessageHistoryEntry {
        thread_id,
        text: marker.to_string(),
    };
    let submit_event = AppEvent::SubmitThreadOp {
        thread_id,
        op: restricted_user_turn(marker),
    };

    let history_variant = app_event_variant_name(&history_event);
    let submit_variant = app_event_variant_name(&submit_event);

    assert_eq!(history_variant, "AppendMessageHistoryEntry");
    assert_eq!(submit_variant, "SubmitThreadOp");
    assert!(!history_variant.contains(marker));
    assert!(!submit_variant.contains(marker));
}
