use super::*;

fn user_message(text: &str) -> UserInput {
    UserInput::Text {
        text: text.to_string(),
        text_elements: Vec::new(),
    }
}

#[test]
fn model_tool_mode_workspace_context_only_requires_a_run_id_line() {
    let error = parse_run_contract(
        ModelToolMode::WorkspaceContextOnly,
        &[user_message("Read the submitted packet.")],
    )
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
    let error = parse_run_contract(
        ModelToolMode::WorkspaceContextOnly,
        &[user_message(
            "- run_id: current-run\n- run_id: previous-run",
        )],
    )
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
fn workspace_planning_only_parses_the_complete_immutable_run_contract() {
    let contract = parse_run_contract(
        ModelToolMode::WorkspacePlanningOnly,
        &[user_message(
            "Patient planning request.\n- run_id: guide-1\n- plan_session_id: plan-1\n- patient_id: patient-1\n- checkpoint_id: checkpoint-1\n- checkpoint_revision: 7\n- checkpoint_sha256: abcdef",
        )],
    )
    .expect("complete planning contract should parse");
    assert_eq!(
        contract,
        WorkspaceRunContract::Planning {
            guide_run_id: "guide-1".to_string(),
            plan_session_id: "plan-1".to_string(),
            client_id: "patient-1".to_string(),
            source_checkpoint_id: "checkpoint-1".to_string(),
            source_checkpoint_revision: 7,
            source_checkpoint_sha256: "abcdef".to_string(),
        }
    );
}

#[test]
fn workspace_planning_only_rejects_missing_or_ambiguous_contract_fields() {
    let missing = parse_run_contract(
        ModelToolMode::WorkspacePlanningOnly,
        &[user_message(
            "- run_id: guide-1\n- plan_session_id: plan-1\n- patient_id: patient-1\n- checkpoint_id: checkpoint-1\n- checkpoint_revision: 1",
        )],
    )
    .expect_err("missing checkpoint hash should fail closed");
    assert_eq!(
        missing.to_string(),
        "workspacePlanningOnly requires exactly one non-empty `- checkpoint_sha256: ...` line in the submitted text input"
    );

    let ambiguous = parse_run_contract(
        ModelToolMode::WorkspacePlanningOnly,
        &[user_message(
            "- run_id: guide-1\n- plan_session_id: plan-1\n- plan_session_id: plan-2\n- patient_id: patient-1\n- checkpoint_id: checkpoint-1\n- checkpoint_revision: 1\n- checkpoint_sha256: abcdef",
        )],
    )
    .expect_err("duplicate plan session should fail closed");
    assert_eq!(
        ambiguous.to_string(),
        "workspacePlanningOnly requires exactly one non-empty `- plan_session_id: ...` line in the submitted text input"
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
fn workspace_planning_only_accepts_exactly_the_reader_tool() {
    let function_call = |name: &str, namespace: Option<&str>| ResponseItem::FunctionCall {
        id: None,
        name: name.to_string(),
        namespace: namespace.map(str::to_string),
        arguments: "{}".to_string(),
        call_id: "call-1".to_string(),
        internal_chat_message_metadata_passthrough: None,
    };

    assert!(
        validate_model_output(
            ModelToolMode::WorkspacePlanningOnly,
            &function_call(WORKSPACE_CONTEXT_READ_TOOL_NAME, None),
        )
        .is_ok()
    );
    assert!(
        validate_model_output(
            ModelToolMode::WorkspacePlanningOnly,
            &function_call(WORKSPACE_CONTEXT_READ_TOOL_NAME, Some("extension")),
        )
        .is_err()
    );

    for name in [
        "request_user_input",
        "update_plan",
        "exec_command",
        "web_search",
        "spawn_agent",
    ] {
        assert!(
            validate_model_output(
                ModelToolMode::WorkspacePlanningOnly,
                &function_call(name, None),
            )
            .is_err(),
            "planning mode accepted disallowed `{name}`"
        );
    }
}

#[test]
fn workspace_plan_artifact_is_stripped_from_the_visible_message() {
    let parsed = parse_workspace_plan_artifact(
        r#"The plan is ready for review.

<workspace_plan_artifact>
{"planMarkdown":"1. Reassess gait.\n2. Compare the referral.","decisions":["Reassess gait before changing the long-range goal.","Compare the current findings with the referral."],"openQuestions":[]}
</workspace_plan_artifact>"#,
    )
    .expect("one complete artifact should parse");

    assert_eq!(parsed.assistant_message, "The plan is ready for review.");
    let plan = parsed.plan.expect("valid artifact should publish a plan");
    assert_eq!(
        plan.plan_markdown,
        "1. Reassess gait.\n2. Compare the referral."
    );
    assert_eq!(
        plan.decisions_json,
        r#"["Reassess gait before changing the long-range goal.","Compare the current findings with the referral."]"#
    );
    assert_eq!(plan.open_questions_json, "[]");
}

#[test]
fn workspace_plan_artifact_rejects_malformed_duplicate_or_non_publishable_payloads() {
    for message in [
        "Visible only <workspace_plan_artifact>",
        "Visible only </workspace_plan_artifact>",
        "Visible <workspace_plan_artifact></workspace_plan_artifact>",
        r#"Visible <workspace_plan_artifact>{"planMarkdown":"one","decisions":[],"openQuestions":[]}</workspace_plan_artifact> <workspace_plan_artifact>{"planMarkdown":"two","decisions":[],"openQuestions":[]}</workspace_plan_artifact>"#,
        r#"<workspace_plan_artifact>{"planMarkdown":"plan without a visible explanation","decisions":[],"openQuestions":[]}</workspace_plan_artifact>"#,
        "Visible <workspace_plan_artifact>not JSON</workspace_plan_artifact>",
        r#"Visible <workspace_plan_artifact>```json
{"planMarkdown":"fenced","decisions":[],"openQuestions":[]}
```</workspace_plan_artifact>"#,
        r#"Visible <workspace_plan_artifact>{"planMarkdown":"missing questions","decisions":[]}</workspace_plan_artifact>"#,
        r#"Visible <workspace_plan_artifact>{"planMarkdown":"unknown field","decisions":[],"openQuestions":[],"extra":true}</workspace_plan_artifact>"#,
        r#"Visible <workspace_plan_artifact>{"planMarkdown":"duplicate field","planMarkdown":"duplicate field again","decisions":[],"openQuestions":[]}</workspace_plan_artifact>"#,
        r#"Visible <workspace_plan_artifact>[]</workspace_plan_artifact>"#,
        r#"Visible <workspace_plan_artifact>{"planMarkdown":"wrong decision type","decisions":[1],"openQuestions":[]}</workspace_plan_artifact>"#,
        r#"Visible <workspace_plan_artifact>{"planMarkdown":"wrong question type","decisions":[],"openQuestions":"none"}</workspace_plan_artifact>"#,
        r#"Visible <workspace_plan_artifact>{"planMarkdown":"   ","decisions":[],"openQuestions":[]}</workspace_plan_artifact>"#,
        r#"Visible <workspace_plan_artifact>{"planMarkdown":"not decision complete","decisions":[],"openQuestions":["Is more evidence needed?"]}</workspace_plan_artifact>"#,
    ] {
        assert!(
            parse_workspace_plan_artifact(message).is_err(),
            "invalid planning response unexpectedly parsed: {message}"
        );
    }
}

#[test]
fn ordinary_workspace_planning_message_does_not_create_an_artifact() {
    let parsed = parse_workspace_plan_artifact("What changed since the last visit?")
        .expect("ordinary planning question should parse");
    assert_eq!(
        parsed.assistant_message,
        "What changed since the last visit?"
    );
    assert!(parsed.plan.is_none());
}

#[test]
fn workspace_planning_only_uses_the_same_observability_redaction_boundary() {
    let marker = "planning-run-and-patient-marker-must-not-be-logged";
    let payload = ToolPayload::Function {
        arguments: format!(r#"{{"run_id":"{marker}"}}"#),
    };
    assert_eq!(
        tool_log_payload(ModelToolMode::WorkspacePlanningOnly, &payload),
        r#"{"payload_redacted":true}"#
    );

    let original = FunctionCallError::RespondToModel(marker.to_string());
    let (logged, preserved) =
        tool_error_for_logging(ModelToolMode::WorkspacePlanningOnly, original);
    assert!(!logged.to_string().contains(marker));
    assert_eq!(
        preserved.expect("planning errors retain the model-visible original"),
        FunctionCallError::RespondToModel(marker.to_string())
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
