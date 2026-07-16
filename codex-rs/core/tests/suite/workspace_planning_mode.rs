#![allow(clippy::unwrap_used)]

use super::model_tool_mode::advertised_tool_names;
use super::model_tool_mode::user_turn;
use super::workspace_planning_mode_support::PLAN_INSTRUCTIONS_MARKER;
use super::workspace_planning_mode_support::configure_dedicated_plan_thread;
use super::workspace_planning_mode_support::planning_fixture;
use super::workspace_planning_mode_support::planning_prompt;
use super::workspace_planning_mode_support::planning_turn;
use codex_protocol::config_types::ModelToolMode;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::request_user_input::RequestUserInputResponse;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;

const HOST_CONTEXT_MARKER: &str = "unrelated-host-context-must-not-enter-planning-turn";

fn plan_artifact_message(
    visible_message: &str,
    plan_markdown: &str,
    decisions: &[&str],
    open_questions: &[&str],
) -> String {
    let artifact = json!({
        "planMarkdown": plan_markdown,
        "decisions": decisions,
        "openQuestions": open_questions,
    });
    format!(
        "{visible_message}\n\n<workspace_plan_artifact>\n{artifact}\n</workspace_plan_artifact>"
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn planning_mode_has_exact_tools_keeps_its_history_and_restores_disabled()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.developer_instructions = Some(HOST_CONTEXT_MARKER.to_string());
    });
    let test = builder.build_with_auto_env(&server).await?;
    configure_dedicated_plan_thread(&test).await?;
    let fixture = planning_fixture(&test).await?;
    let first_prompt = planning_prompt(&test, &fixture, "first planning question").await?;
    let first_run_id = first_prompt
        .lines()
        .find_map(|line| line.strip_prefix("- run_id: "))
        .expect("planning prompt contains run id")
        .to_string();
    let second_prompt = planning_prompt(&test, &fixture, "second planning question").await?;
    let read_call_id = "planning-context-read";
    let mut commentary = ev_assistant_message(
        "msg-commentary",
        "I will inspect the patient-scoped visit history before answering.",
    );
    commentary["item"]["phase"] = json!("commentary");
    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                commentary,
                ev_function_call(
                    read_call_id,
                    "workspace_context_read",
                    &json!({
                        "run_id": first_run_id,
                        "category": "visit_history",
                        "limit": 3,
                    })
                    .to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "first-plan-answer-marker"),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-2", "second plan answer"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;

    test.codex
        .submit(planning_turn(&test, &first_prompt))
        .await?;
    let turn_complete = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnComplete(turn_complete) => Some(turn_complete.clone()),
        _ => None,
    })
    .await;
    assert!(
        turn_complete.error.is_none(),
        "an ordinary planning answer must complete successfully"
    );
    let ordinary_completion = test
        .codex
        .state_db()
        .expect("planning test has state")
        .workspace()
        .get_plan_turn_completion(&first_run_id, &fixture.plan_session_id, &fixture.client_id)
        .await?
        .expect("ordinary planning answer must complete atomically");
    assert_eq!(
        ordinary_completion.assistant_message.content,
        "first-plan-answer-marker"
    );
    assert!(ordinary_completion.revision.is_none());
    test.codex
        .submit(planning_turn(&test, &second_prompt))
        .await?;
    let turn_complete = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnComplete(turn_complete) => Some(turn_complete.clone()),
        _ => None,
    })
    .await;
    assert!(
        turn_complete.error.is_none(),
        "the second ordinary planning answer must complete successfully"
    );
    assert_eq!(
        test.codex.config_snapshot().await.model_tool_mode,
        ModelToolMode::Disabled
    );
    test.codex
        .submit(user_turn("ordinary disabled turn", None))
        .await?;
    let error = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::Error(error) => Some(error.clone()),
        _ => None,
    })
    .await;
    assert_eq!(
        error.message,
        "thread is bound to an active workspace medical planning session; only verified workspacePlanningOnly turns are allowed"
    );
    assert_eq!(error.codex_error_info, Some(CodexErrorInfo::BadRequest));

    let requests = mock.requests();
    assert_eq!(requests.len(), 3);
    for request in &requests {
        assert_eq!(
            advertised_tool_names(&request.body_json()),
            vec!["workspace_context_read"]
        );
    }
    assert_eq!(
        requests[0].body_json()["tools"][0]["parameters"]["properties"]["category"]["enum"],
        json!([
            "visit_history",
            "progress_notes",
            "patient_chart",
            "selected_context"
        ])
    );
    let first_input = serde_json::to_string(&requests[0].body_json()["input"])?;
    assert!(first_input.contains(PLAN_INSTRUCTIONS_MARKER));
    assert!(!first_input.contains(HOST_CONTEXT_MARKER));
    let (read_output, read_success) = requests[1]
        .function_call_output_content_and_success(read_call_id)
        .expect("planning context read should return a tool output");
    assert_eq!(read_success, None);
    let read_output: Value =
        serde_json::from_str(&read_output.expect("planning context output text"))?;
    assert_eq!(read_output["client_id"], fixture.client_id);
    assert_eq!(read_output["run_id"], first_run_id);
    let second_input = serde_json::to_string(&requests[2].body_json()["input"])?;
    assert!(second_input.contains("first-plan-answer-marker"));
    assert!(second_input.contains("first planning question"));
    assert!(test.codex.workspace_context_only_memory_tainted());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn planning_mode_commits_marked_artifact_after_required_evidence_reads() -> anyhow::Result<()>
{
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build_with_auto_env(&server).await?;
    configure_dedicated_plan_thread(&test).await?;
    let fixture = planning_fixture(&test).await?;
    let prompt = planning_prompt(&test, &fixture, "Build an evidence-backed plan.").await?;
    let run_id = prompt
        .lines()
        .find_map(|line| line.strip_prefix("- run_id: "))
        .expect("planning prompt contains run id")
        .to_string();
    let visible_message = "The evidence-backed plan is ready for clinician review.";
    let plan_markdown = "1. Reconcile the referral with the current chart.\n2. Reassess gait before changing the long-range goal.";
    let decisions = [
        "Keep the referral context visible in the daily plan.",
        "Reassess gait before changing the long-range goal.",
    ];
    let raw_message = plan_artifact_message(visible_message, plan_markdown, &decisions, &[]);
    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-chart"),
                ev_function_call(
                    "read-chart",
                    "workspace_context_read",
                    &json!({
                        "run_id": run_id.clone(),
                        "category": "patient_chart",
                        "limit": 20,
                    })
                    .to_string(),
                ),
                ev_completed("resp-chart"),
            ]),
            sse(vec![
                ev_response_created("resp-selected"),
                ev_function_call(
                    "read-selected",
                    "workspace_context_read",
                    &json!({
                        "run_id": run_id.clone(),
                        "category": "selected_context",
                        "limit": 20,
                    })
                    .to_string(),
                ),
                ev_completed("resp-selected"),
            ]),
            sse(vec![
                ev_response_created("resp-final"),
                responses::ev_message_item_added("msg-final", ""),
                responses::ev_output_text_delta(&raw_message),
                ev_assistant_message("msg-final", &raw_message),
                ev_completed("resp-final"),
            ]),
            sse(vec![
                ev_response_created("resp-follow-up"),
                ev_assistant_message("msg-follow-up", "Follow-up guidance after publication."),
                ev_completed("resp-follow-up"),
            ]),
        ],
    )
    .await;

    test.codex.submit(planning_turn(&test, &prompt)).await?;
    let mut assistant_deltas = Vec::new();
    let mut visible_assistant_messages = Vec::new();
    let completed = loop {
        match wait_for_event(&test.codex, |_| true).await {
            EventMsg::AgentMessageContentDelta(event) => assistant_deltas.push(event.delta),
            EventMsg::AgentMessage(event) => visible_assistant_messages.push(event.message),
            EventMsg::ItemCompleted(event) => {
                if let TurnItem::AgentMessage(message) = event.item {
                    visible_assistant_messages.extend(message.content.into_iter().map(|content| {
                        match content {
                            AgentMessageContent::Text { text } => text,
                        }
                    }));
                }
            }
            EventMsg::TurnComplete(event) => break event,
            _ => {}
        }
    };
    assert!(
        assistant_deltas.is_empty(),
        "uncommitted planning output must not stream to the client"
    );
    assert_eq!(
        completed.last_agent_message.as_deref(),
        Some(visible_message)
    );
    assert!(
        visible_assistant_messages
            .iter()
            .any(|message| message == visible_message),
        "the committed visible assistant message must reach the client"
    );
    for message in &visible_assistant_messages {
        assert!(!message.contains("workspace_plan_artifact"));
        assert!(!message.contains("planMarkdown"));
        assert!(!message.contains(plan_markdown));
    }

    let completion = test
        .codex
        .state_db()
        .expect("planning test has state")
        .workspace()
        .get_plan_turn_completion(&run_id, &fixture.plan_session_id, &fixture.client_id)
        .await?
        .expect("marked planning response must complete atomically");
    assert_eq!(completion.assistant_message.content, visible_message);
    let revision = completion
        .revision
        .expect("marked artifact should publish a revision");
    assert_eq!(revision.plan_markdown, plan_markdown);
    assert_eq!(revision.decisions_json, serde_json::to_string(&decisions)?);
    assert_eq!(revision.open_questions_json, "[]");
    assert_eq!(
        completion
            .evidence_manifest
            .iter()
            .map(|entry| entry.category.as_str())
            .collect::<Vec<_>>(),
        vec!["patient_chart", "selected_context"]
    );
    let follow_up_prompt =
        planning_prompt(&test, &fixture, "Give ordinary guidance after publication.").await?;
    test.codex
        .submit(planning_turn(&test, &follow_up_prompt))
        .await?;
    let follow_up_complete = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnComplete(turn_complete) => Some(turn_complete.clone()),
        _ => None,
    })
    .await;
    assert!(follow_up_complete.error.is_none());

    let requests = mock.requests();
    assert_eq!(requests.len(), 4);
    let follow_up_input = serde_json::to_string(&requests[3].body_json()["input"])?;
    assert!(!follow_up_input.contains("workspace_plan_artifact"));
    assert!(!follow_up_input.contains("planMarkdown"));
    assert!(!follow_up_input.contains(plan_markdown));
    for decision in decisions {
        assert!(!follow_up_input.contains(decision));
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn planning_mode_refuses_artifact_without_required_evidence() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build_with_auto_env(&server).await?;
    configure_dedicated_plan_thread(&test).await?;
    let fixture = planning_fixture(&test).await?;
    let prompt = planning_prompt(&test, &fixture, "Try to publish too early.").await?;
    let run_id = prompt
        .lines()
        .find_map(|line| line.strip_prefix("- run_id: "))
        .expect("planning prompt contains run id")
        .to_string();
    responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-chart"),
                ev_function_call(
                    "read-chart-only",
                    "workspace_context_read",
                    &json!({
                        "run_id": run_id.clone(),
                        "category": "patient_chart",
                        "limit": 20,
                    })
                    .to_string(),
                ),
                ev_completed("resp-chart"),
            ]),
            sse(vec![
                ev_response_created("resp-final"),
                ev_assistant_message(
                    "msg-final",
                    &plan_artifact_message(
                        "This should not be visible as complete.",
                        "Incomplete evidence plan.",
                        &["Do not publish without selected context."],
                        &[],
                    ),
                ),
                ev_completed("resp-final"),
            ]),
        ],
    )
    .await;

    test.codex.submit(planning_turn(&test, &prompt)).await?;
    let error = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::Error(error) => Some(error.clone()),
        _ => None,
    })
    .await;
    assert_eq!(
        error.message,
        "workspace medical planning response could not be committed"
    );
    let turn_complete = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnComplete(turn_complete) => Some(turn_complete.clone()),
        _ => None,
    })
    .await;
    assert!(
        turn_complete.error.is_some(),
        "an atomic planning completion rejection must fail the turn"
    );
    assert!(
        test.codex
            .state_db()
            .expect("planning test has state")
            .workspace()
            .get_plan_turn_completion(&run_id, &fixture.plan_session_id, &fixture.client_id)
            .await?
            .is_none(),
        "failed artifact must not leave a partial terminal completion"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn planning_mode_rejects_open_questions_before_any_artifact_becomes_visible()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build_with_auto_env(&server).await?;
    configure_dedicated_plan_thread(&test).await?;
    let fixture = planning_fixture(&test).await?;
    let prompt =
        planning_prompt(&test, &fixture, "Publish despite an unresolved question.").await?;
    let run_id = prompt
        .lines()
        .find_map(|line| line.strip_prefix("- run_id: "))
        .expect("planning prompt contains run id")
        .to_string();
    let raw_message = plan_artifact_message(
        "This invalid artifact must not become visible.",
        "Plan that is not decision-complete.",
        &["Wait for referral confirmation."],
        &["Did the referring clinician confirm the restriction?"],
    );
    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-open-question"),
                responses::ev_message_item_added("msg-open-question", ""),
                responses::ev_output_text_delta(&raw_message),
                ev_assistant_message("msg-open-question", &raw_message),
                ev_completed("resp-open-question"),
            ]),
            sse(vec![
                ev_response_created("resp-after-rejection"),
                ev_assistant_message(
                    "msg-after-rejection",
                    "Safe guidance after rejected artifact.",
                ),
                ev_completed("resp-after-rejection"),
            ]),
        ],
    )
    .await;

    test.codex.submit(planning_turn(&test, &prompt)).await?;
    let mut error_message = None;
    let mut assistant_output_seen = false;
    let turn_complete = loop {
        match wait_for_event(&test.codex, |_| true).await {
            EventMsg::Error(error) => error_message = Some(error.message),
            EventMsg::AgentMessageContentDelta(_) | EventMsg::AgentMessage(_) => {
                assistant_output_seen = true;
            }
            EventMsg::ItemCompleted(event) if matches!(event.item, TurnItem::AgentMessage(_)) => {
                assistant_output_seen = true;
            }
            EventMsg::TurnComplete(event) => break event,
            _ => {}
        }
    };
    assert_eq!(
        error_message.as_deref(),
        Some("workspace plan artifact cannot be published while openQuestions is non-empty")
    );
    assert!(turn_complete.error.is_some());
    assert!(
        !assistant_output_seen,
        "a rejected artifact must not emit assistant output"
    );
    assert!(
        test.codex
            .state_db()
            .expect("planning test has state")
            .workspace()
            .get_plan_turn_completion(&run_id, &fixture.plan_session_id, &fixture.client_id)
            .await?
            .is_none(),
        "a rejected artifact must not create a terminal completion"
    );

    let follow_up_marker = "safe-follow-up-after-rejected-artifact";
    let follow_up_prompt = planning_prompt(&test, &fixture, follow_up_marker).await?;
    test.codex
        .submit(planning_turn(&test, &follow_up_prompt))
        .await?;
    let follow_up_complete = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnComplete(turn_complete) => Some(turn_complete.clone()),
        _ => None,
    })
    .await;
    assert!(follow_up_complete.error.is_none());

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    let follow_up_input = serde_json::to_string(&requests[1].body_json()["input"])?;
    assert!(follow_up_input.contains(follow_up_marker));
    assert!(
        !follow_up_input.contains("Publish despite an unresolved question."),
        "a failed planning prompt must not become future model context"
    );
    assert!(
        !follow_up_input.contains("This invalid artifact must not become visible."),
        "a rejected assistant artifact must not become future model context"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn planning_mode_rejects_request_user_input_as_nonrecoverable() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                "planning-question-call",
                "request_user_input",
                &json!({
                    "questions": [{
                        "id": "goal_scope",
                        "header": "Goal",
                        "question": "Which goal should anchor this note?",
                        "options": [{
                            "label": "Long-term (Recommended)",
                            "description": "Use the multi-month goal."
                        }, {
                            "label": "Daily",
                            "description": "Use only today's goal."
                        }]
                    }]
                })
                .to_string(),
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let mut builder = test_codex();
    let test = builder.build_with_auto_env(&server).await?;
    configure_dedicated_plan_thread(&test).await?;
    let fixture = planning_fixture(&test).await?;
    test.codex
        .submit(planning_turn(
            &test,
            &planning_prompt(&test, &fixture, "Help me choose the goal scope.").await?,
        ))
        .await?;

    let error = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::Error(error) => Some(error.clone()),
        _ => None,
    })
    .await;
    assert_eq!(
        error.message,
        "model returned a disallowed output item while model tool mode is workspacePlanningOnly; only assistant messages, reasoning, and the non-namespaced workspace_context_read function tool are allowed"
    );
    assert_eq!(error.codex_error_info, Some(CodexErrorInfo::Other));
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = mock.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        advertised_tool_names(&requests[0].body_json()),
        vec!["workspace_context_read"]
    );
    test.codex
        .submit(Op::UserInputAnswer {
            id: "stale-planning-question".to_string(),
            response: RequestUserInputResponse {
                answers: HashMap::new(),
            },
        })
        .await?;
    let error = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::Error(error) => Some(error.clone()),
        _ => None,
    })
    .await;
    assert_eq!(
        error.message,
        "thread is bound to an active workspace medical planning session; only verified workspacePlanningOnly turns are allowed"
    );
    assert_eq!(error.codex_error_info, Some(CodexErrorInfo::BadRequest));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn planning_mode_rejects_disallowed_model_tools() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call("call-1", "update_plan", &json!({"plan": []}).to_string()),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let mut builder = test_codex();
    let test = builder.build_with_auto_env(&server).await?;
    configure_dedicated_plan_thread(&test).await?;
    let fixture = planning_fixture(&test).await?;
    test.codex
        .submit(planning_turn(
            &test,
            &planning_prompt(&test, &fixture, "Try an unavailable tool.").await?,
        ))
        .await?;

    let error = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::Error(error) => Some(error.clone()),
        _ => None,
    })
    .await;
    assert_eq!(
        error.message,
        "model returned a disallowed output item while model tool mode is workspacePlanningOnly; only assistant messages, reasoning, and the non-namespaced workspace_context_read function tool are allowed"
    );
    assert_eq!(error.codex_error_info, Some(CodexErrorInfo::Other));
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn planning_mode_is_turn_only_and_requires_plan_collaboration() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build_with_auto_env(&server).await?;

    test.codex
        .submit(Op::ThreadSettings {
            thread_settings: ThreadSettingsOverrides {
                model_tool_mode: Some(ModelToolMode::WorkspacePlanningOnly),
                ..Default::default()
            },
        })
        .await?;
    let error = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::Error(error) => Some(error.clone()),
        _ => None,
    })
    .await;
    assert_eq!(
        error.message,
        "workspacePlanningOnly modelToolMode is valid only as a user-turn override"
    );
    assert_eq!(error.codex_error_info, Some(CodexErrorInfo::BadRequest));

    test.codex
        .submit(user_turn(
            "Patient planning request.\n- run_id: run-1",
            Some(ModelToolMode::WorkspacePlanningOnly),
        ))
        .await?;
    let error = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::Error(error) => Some(error.clone()),
        _ => None,
    })
    .await;
    assert_eq!(
        error.message,
        "workspacePlanningOnly turns require Plan collaboration mode"
    );
    assert_eq!(error.codex_error_info, Some(CodexErrorInfo::BadRequest));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn planning_mode_does_not_restore_on_resume_but_history_survives_reentry()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "pre-resume-plan-marker"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "reentered plan response"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let mut builder = test_codex();
    let initial = builder.build_with_auto_env(&server).await?;
    configure_dedicated_plan_thread(&initial).await?;
    let fixture = planning_fixture(&initial).await?;
    initial
        .codex
        .submit(planning_turn(
            &initial,
            &planning_prompt(&initial, &fixture, "plan before resume").await?,
        ))
        .await?;
    wait_for_event(&initial.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("persisted test thread has a rollout path");

    let mut resume_builder = test_codex();
    let resumed = resume_builder
        .resume(&server, initial.home.clone(), rollout_path.clone())
        .await?;
    assert_eq!(
        resumed.codex.config_snapshot().await.model_tool_mode,
        ModelToolMode::Disabled
    );
    assert!(resumed.codex.workspace_context_only_memory_tainted());
    resumed
        .codex
        .submit(planning_turn(
            &resumed,
            &planning_prompt(&resumed, &fixture, "plan after resume").await?,
        ))
        .await?;
    wait_for_event(&resumed.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let mut second_resume_builder = test_codex();
    let resumed_again = second_resume_builder
        .resume(&server, initial.home.clone(), rollout_path)
        .await?;
    resumed_again
        .codex
        .submit(user_turn("ordinary turn after second resume", None))
        .await?;
    let error = wait_for_event_match(&resumed_again.codex, |event| match event {
        EventMsg::Error(error) => Some(error.clone()),
        _ => None,
    })
    .await;
    assert_eq!(
        error.message,
        "thread is bound to an active workspace medical planning session; only verified workspacePlanningOnly turns are allowed"
    );
    assert!(resumed_again.codex.workspace_context_only_memory_tainted());

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        advertised_tool_names(&requests[1].body_json()),
        vec!["workspace_context_read"]
    );
    let reentered_input = serde_json::to_string(&requests[1].body_json()["input"])?;
    assert!(reentered_input.contains("pre-resume-plan-marker"));
    assert!(reentered_input.contains("plan before resume"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn planning_binding_filters_preexisting_history_and_rejects_direct_injection()
-> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-ordinary"),
                ev_assistant_message("msg-ordinary", "ordinary-assistant-secret-marker"),
                ev_completed("resp-ordinary"),
            ]),
            sse(vec![
                ev_response_created("resp-plan"),
                ev_assistant_message("msg-plan", "filtered planning response"),
                ev_completed("resp-plan"),
            ]),
        ],
    )
    .await;
    let mut builder = test_codex();
    let test = builder.build_with_auto_env(&server).await?;
    test.codex
        .submit(user_turn("ordinary-user-secret-marker", None))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    configure_dedicated_plan_thread(&test).await?;
    let fixture = planning_fixture(&test).await?;
    let injection_error = test
        .codex
        .inject_response_items(vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "injected-secret-marker".to_string(),
            }],
            phase: None,
            internal_chat_message_metadata_passthrough: None,
        }])
        .await
        .expect_err("bound planning thread must reject direct transcript injection");
    assert_eq!(
        injection_error.to_string(),
        "cannot inject response items into a thread bound to an active workspace medical planning session"
    );
    let state_db = test
        .codex
        .state_db()
        .expect("planning test has SQLite state");
    assert_eq!(
        state_db
            .get_thread_memory_mode(test.session_configured.thread_id)
            .await?
            .as_deref(),
        Some("polluted")
    );

    test.codex
        .submit(planning_turn(
            &test,
            &planning_prompt(&test, &fixture, "safe planning prompt marker").await?,
        ))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    let planning_input = serde_json::to_string(&requests[1].body_json()["input"])?;
    assert!(planning_input.contains("safe planning prompt marker"));
    assert!(!planning_input.contains("ordinary-user-secret-marker"));
    assert!(!planning_input.contains("ordinary-assistant-secret-marker"));
    assert!(!planning_input.contains("injected-secret-marker"));

    Ok(())
}
