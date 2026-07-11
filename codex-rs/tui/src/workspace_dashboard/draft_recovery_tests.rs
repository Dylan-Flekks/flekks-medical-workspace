use super::super::draft_snapshot::DraftFocusV1;
use super::super::draft_snapshot::WorkspaceDraftSnapshotV2;
use super::*;
use crate::tui::MouseScrollDirection;
use crate::tui::MouseScrollEvent;
use pretty_assertions::assert_eq;
use sha2::Digest as _;

fn test_session(session_id: &str, checkpoint_revision: i64) -> WorkspaceDraftSession {
    let client_id = "patient-recovery";
    let encounter_id = "encounter-recovery";
    let note_id = "note-recovery";
    let client = ClientDraft {
        id: Some(client_id.to_string()),
        display_name: "Jordan Recovery".to_string(),
        ..ClientDraft::default()
    };
    let note = NoteDraft {
        id: Some(note_id.to_string()),
        encounter_id: Some(encounter_id.to_string()),
        title: "Daily mobility note".to_string(),
        body: "Patient reports improved tolerance for stairs.".to_string(),
        status: "draft".to_string(),
        current_revision: 3,
    };
    let draft = serde_json::to_value(WorkspaceDraftSnapshotV2 {
        schema_version: 2,
        base_client_version: "patient-version-7".to_string(),
        client,
        note,
        focus: DraftFocusV1::NoteBody,
        active_encounter_id: Some(encounter_id.to_string()),
        agent_request_body: "Generate a similar daily note template.".to_string(),
        context_submitted: false,
        selected_artifact_ids: vec!["artifact-a".to_string()],
        selected_derivative_ids: Vec::new(),
        selected_clip_ids: Vec::new(),
    })
    .expect("serialize recovery fixture");
    let content_sha256 = draft_sha256(&draft);
    let checkpoint = codex_app_server_protocol::WorkspaceDraftCheckpoint {
        id: format!("checkpoint-{session_id}-{checkpoint_revision}"),
        session_id: session_id.to_string(),
        client_id: client_id.to_string(),
        encounter_id: Some(encounter_id.to_string()),
        note_id: Some(note_id.to_string()),
        base_note_revision: Some(3),
        schema_version: 2,
        revision: checkpoint_revision,
        draft,
        content_sha256,
        trigger: "focus_change".to_string(),
        actor: "test clinician".to_string(),
        created_at: 1_710_000_000,
    };
    WorkspaceDraftSession {
        id: session_id.to_string(),
        client_id: client_id.to_string(),
        status: WorkspaceDraftSessionStatus::Active,
        current_revision: checkpoint_revision,
        current_checkpoint: checkpoint,
        created_by: "test clinician".to_string(),
        created_at: 1_710_000_000,
        updated_at: 1_710_003_600,
        closed_at: None,
    }
}

fn draft_sha256(draft: &serde_json::Value) -> String {
    let encoded = serde_json::to_string(draft).expect("serialize draft for hashing");
    format!("{:x}", sha2::Sha256::digest(encoded.as_bytes()))
}

fn rehash(session: &mut WorkspaceDraftSession) {
    session.current_checkpoint.content_sha256 = draft_sha256(&session.current_checkpoint.draft);
}

fn recovery_dashboard() -> WorkspaceDashboard {
    let mut dashboard = WorkspaceDashboard::new(WorkspaceProfile::Medical);
    dashboard.focus = WorkspaceFocus::NoteBody;
    dashboard.draft_note.body = "Current canonical editor text".to_string();
    dashboard.draft_recovery.items = vec![
        DraftRecoveryItem {
            session: test_session("session-alpha", /*checkpoint_revision*/ 7),
        },
        DraftRecoveryItem {
            session: test_session("session-beta", /*checkpoint_revision*/ 8),
        },
    ];
    dashboard
}

fn render_dashboard(dashboard: &WorkspaceDashboard, width: u16, height: u16) -> String {
    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
    let mut buffer = Buffer::empty(area);
    dashboard.render(area, &mut buffer);
    (0..height)
        .map(|row| {
            let mut line = String::new();
            for column in 0..width {
                let symbol = buffer[(column, row)].symbol();
                if symbol.is_empty() {
                    line.push(' ');
                } else {
                    line.push_str(symbol);
                }
            }
            line.trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn recovery_modal_snapshots_cover_wide_compact_and_tiny_layouts() {
    let dashboard = recovery_dashboard();

    let wide = render_dashboard(&dashboard, /*width*/ 120, /*height*/ 32);
    assert!(wide.contains("Draft Recovery"));
    assert!(wide.contains("1 other unfinished draft(s) target this same patient and note"));
    assert!(wide.contains("Mode: Draft recovery"));
    assert!(wide.contains("R restore"));
    assert!(wide.contains("D discard"));
    assert!(wide.contains("N/P"));
    assert!(wide.contains("Esc"));
    assert!(!wide.contains("Ctrl-P"));
    assert!(!wide.contains("Tab/"));
    insta::assert_snapshot!("medical_draft_recovery_modal_120x32", wide);

    insta::assert_snapshot!(
        "medical_draft_recovery_modal_80x20",
        render_dashboard(&dashboard, /*width*/ 80, /*height*/ 20)
    );
    insta::assert_snapshot!(
        "medical_draft_recovery_modal_40x12",
        render_dashboard(&dashboard, /*width*/ 40, /*height*/ 12)
    );
}

#[test]
fn recovery_modal_owns_input_and_actions_keep_the_selected_session_id() {
    let mut dashboard = recovery_dashboard();
    let body_before = dashboard.draft_note.body.clone();
    let focus_before = dashboard.focus;

    assert_eq!(
        dashboard.handle_key_event(KeyEvent::from(KeyCode::Char('x'))),
        WorkspaceDashboardAction::Consumed
    );
    assert_eq!(dashboard.draft_note.body, body_before);
    assert_eq!(dashboard.focus, focus_before);

    assert_eq!(
        dashboard.handle_paste("must not enter the note".to_string()),
        WorkspaceDashboardAction::Consumed
    );
    assert_eq!(dashboard.draft_note.body, body_before);

    assert_eq!(
        dashboard.handle_mouse_scroll(
            MouseScrollEvent {
                direction: MouseScrollDirection::Down,
                column: 60,
                row: 12,
            },
            Some(Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 120, /*height*/ 32,
            )),
        ),
        WorkspaceDashboardAction::Consumed
    );
    assert_eq!(dashboard.focus, focus_before);
    assert_eq!(dashboard.draft_recovery.index, 0);
    for area in [
        Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 120, /*height*/ 32,
        ),
        Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 20,
        ),
        Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 12,
        ),
    ] {
        assert_eq!(dashboard.cursor_pos(area), None);
    }

    let restore_alpha = dashboard.handle_key_event(KeyEvent::from(KeyCode::Char('r')));
    assert_eq!(
        restore_alpha,
        WorkspaceDashboardAction::RestoreRecoveryDraft {
            session_id: "session-alpha".to_string(),
        }
    );
    assert_eq!(
        dashboard.handle_key_event(KeyEvent::from(KeyCode::Char('n'))),
        WorkspaceDashboardAction::Consumed
    );
    assert_eq!(dashboard.draft_recovery.index, 1);
    assert_eq!(
        restore_alpha,
        WorkspaceDashboardAction::RestoreRecoveryDraft {
            session_id: "session-alpha".to_string(),
        },
        "a queued action must retain the session selected when the key was pressed"
    );
    assert_eq!(
        dashboard.handle_key_event(KeyEvent::from(KeyCode::Char('D'))),
        WorkspaceDashboardAction::DiscardRecoveryDraft {
            session_id: "session-beta".to_string(),
        }
    );

    dashboard.handle_key_event(KeyEvent::from(KeyCode::Char('n')));
    assert_eq!(dashboard.draft_recovery.index, 0);
    dashboard.handle_key_event(KeyEvent::from(KeyCode::Char('p')));
    assert_eq!(dashboard.draft_recovery.index, 1);
}

#[test]
fn recovery_destructive_actions_ignore_modified_and_repeated_keys() {
    let mut dashboard = recovery_dashboard();
    let queue_before = dashboard.draft_recovery.clone();

    assert_eq!(
        dashboard.handle_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL,)),
        WorkspaceDashboardAction::Consumed
    );
    assert_eq!(dashboard.draft_recovery, queue_before);

    assert_eq!(
        dashboard.handle_key_event(KeyEvent::new_with_kind(
            KeyCode::Char('d'),
            KeyModifiers::NONE,
            KeyEventKind::Repeat,
        )),
        WorkspaceDashboardAction::Consumed
    );
    assert_eq!(dashboard.draft_recovery, queue_before);
}

#[test]
fn recovery_queue_visibility_defers_for_an_owned_session_and_resumes_after_close() {
    let mut dashboard = recovery_dashboard();
    assert!(dashboard.recovery_modal_visible());

    dashboard.finish_recovery_adoption("session-alpha");
    assert_eq!(
        dashboard
            .draft_recovery
            .items
            .iter()
            .map(|item| item.session.id.as_str())
            .collect::<Vec<_>>(),
        vec!["session-beta"]
    );
    assert!(dashboard.draft_recovery.deferred_for_owned_session);
    assert!(!dashboard.recovery_modal_visible());

    dashboard.draft_recovery.dismissed = true;
    assert!(dashboard.resume_recovery_after_owned_close());
    assert!(!dashboard.draft_recovery.deferred_for_owned_session);
    assert!(!dashboard.draft_recovery.dismissed);
    assert!(dashboard.recovery_modal_visible());

    dashboard.finish_recovery_adoption("session-beta");
    assert!(dashboard.draft_recovery.items.is_empty());
    assert!(!dashboard.resume_recovery_after_owned_close());
    assert!(!dashboard.recovery_modal_visible());

    let mut unavailable = recovery_dashboard();
    unavailable.draft_recovery.available = false;
    assert!(!unavailable.recovery_modal_visible());
}

#[test]
fn recovery_session_envelope_requires_exact_hash_schema_scope_and_revision() {
    let valid = test_session("session-valid", /*checkpoint_revision*/ 4);
    validate_recovery_session_envelope(&valid, /*require_active*/ true)
        .expect("well-formed active recovery session");

    let mut terminal = valid.clone();
    terminal.status = WorkspaceDraftSessionStatus::Closed;
    validate_recovery_session_envelope(&terminal, /*require_active*/ false)
        .expect("patient-scoped reconciliation may validate a terminal session");
    let error = validate_recovery_session_envelope(&terminal, /*require_active*/ true)
        .expect_err("global discovery must reject terminal sessions");
    assert!(error.to_string().contains("non-active"));

    let mut wrong_revision = valid.clone();
    wrong_revision.current_revision += 1;
    let error = validate_recovery_session_envelope(&wrong_revision, /*require_active*/ true)
        .expect_err("session revision must equal checkpoint revision");
    assert!(error.to_string().contains("session/checkpoint identity"));

    let mut wrong_hash = valid.clone();
    wrong_hash.current_checkpoint.content_sha256 = "0".repeat(64);
    let error = validate_recovery_session_envelope(&wrong_hash, /*require_active*/ true)
        .expect_err("tampered draft bytes must be rejected");
    assert!(error.to_string().contains("content hash"));

    let mut wrong_schema = valid.clone();
    wrong_schema.current_checkpoint.schema_version = 1;
    let error = validate_recovery_session_envelope(&wrong_schema, /*require_active*/ true)
        .expect_err("outer and inner schema versions must match");
    assert!(error.to_string().contains("schema identity"));

    let mut wrong_scope = valid.clone();
    wrong_scope.current_checkpoint.draft["client"]["id"] =
        serde_json::Value::String("another-patient".to_string());
    rehash(&mut wrong_scope);
    let error = validate_recovery_session_envelope(&wrong_scope, /*require_active*/ true)
        .expect_err("outer and inner patient identities must match");
    assert!(error.to_string().contains("outer and inner scope"));

    let mut unknown_field = valid;
    unknown_field.current_checkpoint.draft["unexpectedRecoveryField"] =
        serde_json::Value::Bool(true);
    rehash(&mut unknown_field);
    let error = validate_recovery_session_envelope(&unknown_field, /*require_active*/ true)
        .expect_err("strict recovery codec must reject unknown fields");
    assert!(error.to_string().contains("unknown field"));
}

#[test]
fn recovery_session_unchanged_comparison_includes_checkpoint_identity() {
    let expected = test_session("session-current", /*checkpoint_revision*/ 4);
    let refreshed = test_session("session-current", /*checkpoint_revision*/ 5);

    let error = validate_recovery_session_unchanged(&expected, &refreshed)
        .expect_err("a newer checkpoint must force another recovery decision");

    assert!(
        error
            .to_string()
            .contains("changed while recovery data was loading")
    );
}
