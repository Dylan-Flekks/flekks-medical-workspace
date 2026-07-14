use super::*;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use crate::history_cell;
use crate::history_cell::HistoryCell;
use codex_app_server_protocol::ByteRange;
use codex_app_server_protocol::TextElement;
use codex_app_server_protocol::Turn as AppServerTurn;
use codex_app_server_protocol::TurnItemsView;
use codex_app_server_protocol::TurnStartedNotification;
use codex_app_server_protocol::UserInput;
use codex_app_server_protocol::WorkspaceAgentRun;
use codex_app_server_protocol::WorkspaceContextPacket;
use codex_protocol::config_types::ModelToolMode;
use pretty_assertions::assert_eq;

fn user_turn(text: &str) -> AppCommand {
    AppCommand::user_turn(
        vec![UserInput::Text {
            text: text.to_string(),
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
    )
}

fn medical_handoff_text(run_id: &str) -> String {
    format!(
        "Medical workspace context packet selected.\n- packet_id: packet-1\n- context_envelope_sha256: packet-hash\n- run_id: {run_id}"
    )
}

fn user_turn_tool_mode(op: &AppCommand) -> Option<ModelToolMode> {
    let AppCommand::UserTurn {
        model_tool_mode, ..
    } = op
    else {
        return None;
    };
    *model_tool_mode
}

fn pending_capture(thread_id: ThreadId) -> PendingWorkspaceAgentCapture {
    let packet = WorkspaceContextPacket {
        id: "packet-1".to_string(),
        client_id: "patient-1".to_string(),
        encounter_id: Some("encounter-1".to_string()),
        note_id: Some("note-1".to_string()),
        human_request: "Draft a synthetic note proposal.".to_string(),
        selected_artifact_ids_json: "[]".to_string(),
        selected_derivative_ids_json: "[]".to_string(),
        selected_clip_ids_json: "[]".to_string(),
        artifact_summary: "0 selected artifacts".to_string(),
        derivative_summary: "0 selected derivatives".to_string(),
        clip_summary: "0 selected clips".to_string(),
        chart_context_summary: "synthetic patient context".to_string(),
        context_envelope_json: "{}".to_string(),
        context_envelope_sha256: "packet-hash".to_string(),
        clinician_actor: "local synthetic clinician".to_string(),
        base_note_revision: Some(1),
        authorized_scope_json: "{}".to_string(),
        expected_output_kind: "note_proposal".to_string(),
        workspace_profile: "medical".to_string(),
        plan_schema_version: 1,
        source_checkpoint_id: None,
        source_checkpoint_sha256: None,
        readiness_json: r#"{"version":1,"warnings":[],"acknowledgements":[],"legacy":true}"#
            .to_string(),
        status: "sent".to_string(),
        created_at: 1,
        sent_at: 1,
        submitted_at: Some(1),
        canceled_at: None,
        updated_at: 1,
    };
    let run = WorkspaceAgentRun {
        id: "run-1".to_string(),
        packet_id: packet.id.clone(),
        client_id: packet.client_id.clone(),
        note_id: packet.note_id.clone(),
        base_note_revision: packet.base_note_revision,
        context_envelope_sha256: packet.context_envelope_sha256.clone(),
        run_kind: "note_proposal".to_string(),
        idempotency_key: "test-run".to_string(),
        provider: Some("test-provider".to_string()),
        model: Some("test-model".to_string()),
        source_thread_id: Some(thread_id.to_string()),
        source_turn_id: None,
        status: "running".to_string(),
        error_summary: None,
        started_at: 1,
        completed_at: None,
        created_at: 1,
        updated_at: 1,
    };
    PendingWorkspaceAgentCapture::try_new(&packet, &run, medical_handoff_text("run-1"))
        .expect("synthetic capture has concrete provenance")
}

fn thread_session(thread_id: ThreadId) -> ThreadSessionState {
    ThreadSessionState {
        thread_id,
        forked_from_id: None,
        fork_parent_title: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        permission_profile: PermissionProfile::read_only(),
        active_permission_profile: None,
        cwd: PathBuf::from("/tmp/medical-context-test").abs(),
        runtime_workspace_roots: Vec::new(),
        instruction_source_paths: Vec::new(),
        reasoning_effort: None,
        collaboration_mode: None,
        personality: None,
        message_history: None,
        network_proxy: None,
        rollout_path: None,
    }
}

#[tokio::test]
async fn pending_medical_capture_forces_workspace_context_only_mode() {
    let mut app = test_support::make_test_app().await;
    let thread_id = ThreadId::new();
    app.pending_workspace_agent_capture = Some(pending_capture(thread_id));

    let op = app
        .prepare_workspace_context_turn_submission(
            thread_id,
            user_turn(&medical_handoff_text("run-1")),
        )
        .expect("text-only medical handoff should be submitted");

    assert_eq!(
        user_turn_tool_mode(&op),
        Some(ModelToolMode::WorkspaceContextOnly)
    );
}

#[tokio::test]
async fn generic_submission_keeps_default_tool_mode() {
    let mut app = test_support::make_test_app().await;
    let thread_id = ThreadId::new();

    let op = app
        .prepare_workspace_context_turn_submission(thread_id, user_turn("ordinary prompt"))
        .expect("generic user turn should be submitted");

    assert_eq!(user_turn_tool_mode(&op), None);
}

#[tokio::test]
async fn pending_capture_leaves_non_user_turn_untouched() {
    let mut app = test_support::make_test_app().await;
    let thread_id = ThreadId::new();
    app.pending_workspace_agent_capture = Some(pending_capture(thread_id));
    let op = AppCommand::interrupt();

    let prepared = app
        .prepare_workspace_context_turn_submission(thread_id, op.clone())
        .expect("non-user operation should remain routable");

    assert_eq!(prepared, op);
}

#[test]
fn active_restricted_route_never_selects_steer() {
    assert_eq!(
        workspace_context_turn_route(
            Some(ModelToolMode::WorkspaceContextOnly),
            Some("active-turn".to_string()),
        ),
        WorkspaceContextTurnRoute::HoldRestricted
    );
}

#[test]
fn active_generic_route_still_selects_steer() {
    assert_eq!(
        workspace_context_turn_route(None, Some("active-turn".to_string())),
        WorkspaceContextTurnRoute::Steer("active-turn".to_string())
    );
}

#[test]
fn active_turn_medical_handoff_error_snapshot() {
    let cell = history_cell::new_error_event(WORKSPACE_CONTEXT_ACTIVE_TURN_MESSAGE.to_string());
    let rendered = cell
        .display_lines(/*width*/ 80)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!("active_turn_medical_handoff_error", rendered);
}

#[test]
fn unaudited_medical_handoff_input_error_snapshot() {
    let cell = history_cell::new_error_event(WORKSPACE_CONTEXT_UNAUDITED_INPUT_MESSAGE.to_string());
    let rendered = cell
        .display_lines(/*width*/ 80)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!("unaudited_medical_handoff_input_error", rendered);
}

#[test]
fn changed_medical_handoff_binding_error_snapshot() {
    let cell =
        history_cell::new_error_event(WORKSPACE_CONTEXT_BINDING_MISMATCH_MESSAGE.to_string());
    let rendered = cell
        .display_lines(/*width*/ 80)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!("changed_medical_handoff_binding_error", rendered);
}

#[test]
fn missing_active_medical_thread_error_snapshot() {
    let cell =
        history_cell::new_error_event(WORKSPACE_CONTEXT_NO_ACTIVE_THREAD_MESSAGE.to_string());
    let rendered = cell
        .display_lines(/*width*/ 80)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!("missing_active_medical_thread_error", rendered);
}

#[test]
fn structured_output_medical_handoff_error_snapshot() {
    let cell =
        history_cell::new_error_event(WORKSPACE_CONTEXT_STRUCTURED_OUTPUT_MESSAGE.to_string());
    let rendered = cell
        .display_lines(/*width*/ 80)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!("structured_output_medical_handoff_error", rendered);
}

#[tokio::test]
async fn pending_capture_holds_an_edited_cross_run_binding() {
    let mut app = test_support::make_test_app().await;
    let thread_id = ThreadId::new();
    app.pending_workspace_agent_capture = Some(pending_capture(thread_id));

    let prepared = app.prepare_workspace_context_turn_submission(
        thread_id,
        user_turn(&medical_handoff_text("different-authorized-run")),
    );

    assert_eq!(prepared, None);
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        medical_handoff_text("different-authorized-run")
    );
    assert!(app.pending_workspace_agent_capture.is_some());
}

#[tokio::test]
async fn pending_capture_holds_appended_text_to_generated_prompt() {
    let mut app = test_support::make_test_app().await;
    let thread_id = ThreadId::new();
    app.pending_workspace_agent_capture = Some(pending_capture(thread_id));
    let edited_prompt = format!("{}\nappended text", medical_handoff_text("run-1"));

    let prepared =
        app.prepare_workspace_context_turn_submission(thread_id, user_turn(&edited_prompt));

    assert_eq!(prepared, None);
    assert_eq!(app.chat_widget.composer_text_with_pending(), edited_prompt);
    assert!(app.pending_workspace_agent_capture.is_some());
}

#[tokio::test]
async fn pending_capture_holds_exact_text_with_inline_elements() {
    let mut app = test_support::make_test_app().await;
    let thread_id = ThreadId::new();
    app.pending_workspace_agent_capture = Some(pending_capture(thread_id));
    let prompt = medical_handoff_text("run-1");
    let mut op = user_turn(&prompt);
    let AppCommand::UserTurn { items, .. } = &mut op else {
        panic!("expected user turn");
    };
    let UserInput::Text { text_elements, .. } = &mut items[0] else {
        panic!("expected text input");
    };
    text_elements.push(TextElement::new(
        ByteRange { start: 0, end: 4 },
        Some("<mention>".to_string()),
    ));

    let prepared = app.prepare_workspace_context_turn_submission(thread_id, op);

    assert_eq!(prepared, None);
    assert_eq!(app.chat_widget.composer_text_with_pending(), prompt);
    assert!(app.pending_workspace_agent_capture.is_some());
}

#[tokio::test]
async fn pending_capture_holds_wrong_thread_submission() {
    let mut app = test_support::make_test_app().await;
    let target_thread_id = ThreadId::new();
    let wrong_thread_id = ThreadId::new();
    app.pending_workspace_agent_capture = Some(pending_capture(target_thread_id));
    let prompt = medical_handoff_text("run-1");

    let prepared =
        app.prepare_workspace_context_turn_submission(wrong_thread_id, user_turn(&prompt));

    assert_eq!(prepared, None);
    assert_eq!(app.chat_widget.composer_text_with_pending(), prompt);
    assert!(app.pending_workspace_agent_capture.is_some());
}

#[tokio::test]
async fn pending_capture_holds_wrong_model_submission() {
    let mut app = test_support::make_test_app().await;
    let thread_id = ThreadId::new();
    app.pending_workspace_agent_capture = Some(pending_capture(thread_id));
    let prompt = medical_handoff_text("run-1");
    let mut op = user_turn(&prompt);
    let AppCommand::UserTurn { model, .. } = &mut op else {
        panic!("expected user turn");
    };
    *model = "different-model".to_string();

    let prepared = app.prepare_workspace_context_turn_submission(thread_id, op);

    assert_eq!(prepared, None);
    assert_eq!(app.chat_widget.composer_text_with_pending(), prompt);
    assert!(app.pending_workspace_agent_capture.is_some());
}

#[tokio::test]
async fn pending_capture_holds_structured_output_submission() {
    let mut app = test_support::make_test_app().await;
    let thread_id = ThreadId::new();
    app.pending_workspace_agent_capture = Some(pending_capture(thread_id));
    let prompt = medical_handoff_text("run-1");
    let mut op = user_turn(&prompt);
    let AppCommand::UserTurn {
        final_output_json_schema,
        ..
    } = &mut op
    else {
        panic!("expected user turn");
    };
    *final_output_json_schema = Some(serde_json::json!({"type": "object"}));

    let prepared = app.prepare_workspace_context_turn_submission(thread_id, op);

    assert_eq!(prepared, None);
    assert_eq!(app.chat_widget.composer_text_with_pending(), prompt);
    assert!(app.pending_workspace_agent_capture.is_some());
}

#[tokio::test]
async fn pending_capture_holds_shell_command_escape() {
    let mut app = test_support::make_test_app().await;
    let thread_id = ThreadId::new();
    app.pending_workspace_agent_capture = Some(pending_capture(thread_id));

    let prepared = app.prepare_workspace_context_turn_submission(
        thread_id,
        AppCommand::run_user_shell_command("echo synthetic".to_string()),
    );

    assert_eq!(prepared, None);
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "!echo synthetic"
    );
    assert!(app.pending_workspace_agent_capture.is_some());
}

#[tokio::test]
async fn pending_capture_suppresses_handoff_history_across_thread_races() {
    let mut app = test_support::make_test_app().await;
    let target_thread_id = ThreadId::new();
    let other_thread_id = ThreadId::new();
    app.pending_workspace_agent_capture = Some(pending_capture(target_thread_id));

    assert!(app.suppress_workspace_context_message_history(target_thread_id));
    assert!(app.suppress_workspace_context_message_history(other_thread_id));
    app.pending_workspace_agent_capture = None;
    assert!(!app.suppress_workspace_context_message_history(target_thread_id));
}

#[tokio::test]
async fn pending_capture_holds_and_restores_inline_attachment() {
    let mut app = test_support::make_test_app().await;
    let (mut chat_widget, _app_event_tx, _app_event_rx, mut op_rx) =
        make_chatwidget_manual_with_sender().await;
    let thread_id = ThreadId::new();
    let image_path = PathBuf::from("/tmp/synthetic-card.png");
    chat_widget.handle_thread_session(thread_session(thread_id));
    app.chat_widget = chat_widget;
    app.pending_workspace_agent_capture = Some(pending_capture(thread_id));
    let handoff_text = format!("{} [Image #1]", medical_handoff_text("run-1"));
    let mut op = user_turn(&handoff_text);
    let AppCommand::UserTurn { items, .. } = &mut op else {
        panic!("expected user turn");
    };
    items.push(UserInput::LocalImage {
        path: image_path.clone(),
        detail: None,
    });

    let prepared = app.prepare_workspace_context_turn_submission(thread_id, op);

    assert_eq!(prepared, None);
    assert_eq!(app.chat_widget.composer_text_with_pending(), handoff_text);
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let restored = loop {
        match op_rx.try_recv() {
            Ok(op @ AppCommand::UserTurn { .. }) => break op,
            Ok(_) => continue,
            Err(err) => panic!("expected restored attachment submission: {err}"),
        }
    };
    let AppCommand::UserTurn { items, .. } = restored else {
        panic!("expected restored user turn");
    };
    assert!(
        items
            .iter()
            .any(|item| matches!(item, UserInput::LocalImage { path, .. } if path == &image_path))
    );
}

#[tokio::test]
async fn active_turn_holds_restricted_handoff_in_rejected_steer_queue() {
    let mut app = test_support::make_test_app().await;
    let (mut chat_widget, _app_event_tx, _app_event_rx, mut op_rx) =
        make_chatwidget_manual_with_sender().await;
    let thread_id = ThreadId::new();
    let turn_id = "active-turn";
    chat_widget.handle_thread_session(thread_session(thread_id));
    chat_widget.handle_server_notification(
        ServerNotification::TurnStarted(TurnStartedNotification {
            thread_id: thread_id.to_string(),
            turn: AppServerTurn {
                id: turn_id.to_string(),
                items: Vec::new(),
                items_view: TurnItemsView::Full,
                status: TurnStatus::InProgress,
                error: None,
                started_at: Some(1),
                completed_at: None,
                duration_ms: None,
            },
        }),
        /*replay_kind*/ None,
    );
    chat_widget.apply_external_edit(medical_handoff_text("run-1"));
    chat_widget.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let submitted_op = loop {
        match op_rx.try_recv() {
            Ok(op @ AppCommand::UserTurn { .. }) => break op,
            Ok(_) => continue,
            Err(err) => panic!("expected running-turn submission: {err}"),
        }
    };
    app.chat_widget = chat_widget;
    app.pending_workspace_agent_capture = Some(pending_capture(thread_id));
    let channel = ThreadEventChannel::new(THREAD_EVENT_CHANNEL_CAPACITY);
    channel.store.lock().await.active_turn_id = Some(turn_id.to_string());
    app.thread_event_channels.insert(thread_id, channel);

    let prepared = app
        .prepare_workspace_context_turn_submission(thread_id, submitted_op)
        .expect("text-only handoff should reach active-turn routing");
    let AppCommand::UserTurn { items, .. } = &prepared else {
        panic!("expected user turn");
    };
    assert_eq!(
        app.active_turn_id_for_thread(thread_id).await,
        Some(turn_id.to_string())
    );
    app.hold_workspace_context_turn(items);

    assert_eq!(
        user_turn_tool_mode(&prepared),
        Some(ModelToolMode::WorkspaceContextOnly)
    );
    assert_eq!(
        app.chat_widget.queued_user_message_texts(),
        vec![medical_handoff_text("run-1")]
    );
}
