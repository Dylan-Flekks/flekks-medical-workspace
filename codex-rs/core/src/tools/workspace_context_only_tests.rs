use super::*;

fn user_message(text: &str) -> UserInput {
    UserInput::Text {
        text: text.to_string(),
        text_elements: Vec::new(),
    }
}

#[test]
fn model_tool_mode_workspace_context_only_requires_a_run_id_line() {
    let error = parse_run_id(&[user_message("Read the submitted packet.")])
        .expect_err("missing run id should fail closed");
    let CodexErr::InvalidRequest(message) = error else {
        panic!("expected invalid request");
    };
    assert_eq!(
        message,
        "workspaceContextOnly requires exactly one non-empty `- run_id: ...` line in the submitted text input"
    );
}

#[test]
fn model_tool_mode_workspace_context_only_rejects_duplicate_run_id_lines() {
    let error = parse_run_id(&[user_message(
        "- run_id: current-run\n- run_id: previous-run",
    )])
    .expect_err("ambiguous run ids should fail closed");
    let CodexErr::InvalidRequest(message) = error else {
        panic!("expected invalid request");
    };
    assert_eq!(
        message,
        "workspaceContextOnly requires exactly one non-empty `- run_id: ...` line in the submitted text input"
    );
}

#[test]
fn workspace_context_only_redacts_generic_tool_log_payloads() {
    let payload = ToolPayload::Function {
        arguments: r#"{"run_id":"clinical-capability-marker","category":"visit_history"}"#
            .to_string(),
    };

    let redacted = tool_log_payload(ModelToolMode::WorkspaceContextOnly, &payload);

    assert_eq!(redacted, r#"{"payload_redacted":true}"#);
    assert!(!redacted.contains("clinical-capability-marker"));
    assert_eq!(
        tool_log_payload(ModelToolMode::Default, &payload),
        payload.log_payload()
    );
}

#[test]
fn workspace_context_only_redacts_tool_errors_only_for_observability() {
    let marker = "run-and-patient-marker-must-not-be-logged";
    let original = FunctionCallError::RespondToModel(marker.to_string());

    let (logged, preserved) = tool_error_for_logging(ModelToolMode::WorkspaceContextOnly, original);

    assert!(!logged.to_string().contains(marker));
    assert_eq!(
        preserved.expect("restricted errors retain the model-visible original"),
        FunctionCallError::RespondToModel(marker.to_string())
    );

    let normal = FunctionCallError::RespondToModel(marker.to_string());
    let (logged, preserved) = tool_error_for_logging(ModelToolMode::Default, normal);
    assert!(logged.to_string().contains(marker));
    assert!(preserved.is_none());
}

#[test]
fn workspace_context_only_rejects_all_compaction_output_variants() {
    let items = [
        ResponseItem::Compaction {
            id: None,
            encrypted_content: "encrypted-marker".to_string(),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::ContextCompaction {
            id: None,
            encrypted_content: Some("encrypted-marker".to_string()),
            internal_chat_message_metadata_passthrough: None,
        },
        ResponseItem::CompactionTrigger {},
    ];

    for item in items {
        assert!(
            validate_model_output(ModelToolMode::WorkspaceContextOnly, &item).is_err(),
            "restricted output unexpectedly accepted {item:?}"
        );
    }
}

#[test]
fn workspace_context_only_accepts_only_plain_reader_function_calls() {
    let function_call = |name: &str, namespace: Option<&str>| ResponseItem::FunctionCall {
        id: None,
        name: name.to_string(),
        namespace: namespace.map(str::to_string),
        arguments: r#"{"run_id":"run-1","category":"visit_history"}"#.to_string(),
        call_id: "call-1".to_string(),
        internal_chat_message_metadata_passthrough: None,
    };

    assert!(
        validate_model_output(
            ModelToolMode::WorkspaceContextOnly,
            &function_call(WORKSPACE_CONTEXT_READ_TOOL_NAME, None),
        )
        .is_ok()
    );
    assert!(
        validate_model_output(
            ModelToolMode::WorkspaceContextOnly,
            &function_call(WORKSPACE_CONTEXT_READ_TOOL_NAME, Some("extension")),
        )
        .is_err()
    );
    assert!(
        validate_model_output(
            ModelToolMode::WorkspaceContextOnly,
            &function_call("update_plan", None),
        )
        .is_err()
    );
}

#[test]
fn workspace_context_only_rejects_model_authored_non_assistant_messages() {
    let message = |role: &str| ResponseItem::Message {
        id: None,
        role: role.to_string(),
        content: Vec::new(),
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };

    assert!(
        validate_model_output(ModelToolMode::WorkspaceContextOnly, &message("assistant")).is_ok()
    );
    assert!(validate_model_output(ModelToolMode::WorkspaceContextOnly, &message("user")).is_err());
    assert!(
        validate_model_output(ModelToolMode::WorkspaceContextOnly, &message("developer")).is_err()
    );
    assert!(
        validate_model_output(ModelToolMode::WorkspaceContextOnly, &message("system")).is_err()
    );
    let collaboration_message = ResponseItem::AgentMessage {
        id: None,
        author: "/root".to_string(),
        recipient: "/root/worker".to_string(),
        content: Vec::new(),
        internal_chat_message_metadata_passthrough: None,
    };
    assert!(
        validate_model_output(ModelToolMode::WorkspaceContextOnly, &collaboration_message).is_err()
    );
}
