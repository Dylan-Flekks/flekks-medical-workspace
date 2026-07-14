use super::*;
use crate::workspace_draft::MedicalWorkspaceWorkingDraftInput;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use pretty_assertions::assert_eq;
use ratatui::layout::Rect;
use tokio::sync::mpsc;

fn working_draft(body: &str) -> MedicalWorkspaceWorkingDraftV1 {
    MedicalWorkspaceWorkingDraftV1::new(MedicalWorkspaceWorkingDraftInput {
        client_id: "client-1".to_string(),
        note_id: Some("note-1".to_string()),
        working_note_id: "note-1".to_string(),
        encounter_id: Some("encounter-1".to_string()),
        base_note_revision: Some(3),
        note_title: "Daily note".to_string(),
        note_body: body.to_string(),
        agent_request_body: String::new(),
        selected_file_ids: Vec::new(),
        selected_reviewed_text_ids: Vec::new(),
        selected_clip_ids: Vec::new(),
    })
    .expect("valid working draft")
}

#[test]
fn draw_and_resize_take_the_synchronous_render_only_path() {
    assert!(workspace_dashboard_render_event(&TuiEvent::Draw));
    assert!(workspace_dashboard_render_event(&TuiEvent::Resize));
    assert!(!workspace_dashboard_render_event(&TuiEvent::Key(
        KeyEvent::from(KeyCode::Tab),
    )));
}

#[test]
fn major_focus_transition_queues_the_current_draft_checkpoint() {
    let baseline = working_draft("baseline");
    let mut runtime = WorkspaceDraftRuntime {
        enabled: true,
        recovery_discovery_complete: true,
        ..WorkspaceDraftRuntime::default()
    };
    runtime.attach_baseline(Some(baseline));
    let request = runtime
        .observe_at(Some(working_draft("changed")), Instant::now())
        .expect("changed draft should schedule persistence");
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sender = AppEventSender::new(tx);

    runtime.request_focus_checkpoint(&sender, WorkspaceFocus::NoteBody, WorkspaceFocus::Workflow);

    match rx.try_recv().expect("focus change checkpoint event") {
        AppEvent::WorkspaceDraftFocusCheckpoint { token } => {
            assert_eq!(token, request.token);
        }
        event => panic!("unexpected app event: {event:?}"),
    }

    runtime.request_focus_checkpoint(&sender, WorkspaceFocus::Workflow, WorkspaceFocus::Workflow);
    assert!(rx.try_recv().is_err());
}

#[test]
fn rapid_typing_restarts_debounce_without_losing_editor_focus_or_cursor() {
    let start = Instant::now();
    let mut runtime = WorkspaceDraftRuntime {
        enabled: true,
        recovery_discovery_complete: true,
        ..WorkspaceDraftRuntime::default()
    };
    runtime.attach_baseline(Some(working_draft("")));

    let first = runtime
        .observe_at(Some(working_draft("a")), start)
        .expect("first edit schedules persistence");
    let second_at = start + Duration::from_millis(100);
    let second = runtime
        .observe_at(Some(working_draft("ab")), second_at)
        .expect("second edit reschedules persistence");
    let latest_at = start + Duration::from_millis(200);
    let latest = runtime
        .observe_at(Some(working_draft("abc")), latest_at)
        .expect("latest edit reschedules persistence");

    assert_ne!(first.token, second.token);
    assert_ne!(second.token, latest.token);
    assert!(!runtime.state.autosave_is_due_at(
        first.token,
        start + WORKSPACE_DRAFT_AUTOSAVE_DELAY + Duration::from_millis(1),
    ));
    assert!(!runtime.state.autosave_is_due_at(
        latest.token,
        latest_at + WORKSPACE_DRAFT_AUTOSAVE_DELAY - Duration::from_millis(1),
    ));
    assert!(
        runtime
            .state
            .autosave_is_due_at(latest.token, latest_at + WORKSPACE_DRAFT_AUTOSAVE_DELAY)
    );

    let viewport = Rect::new(0, 0, 160, 45);
    let mut dashboard = WorkspaceDashboard::new(WorkspaceProfile::Medical);
    for _ in 0..8 {
        if dashboard.focus() == WorkspaceFocus::NoteBody {
            break;
        }
        assert_eq!(
            dashboard.handle_key_event_for_viewport(KeyEvent::from(KeyCode::Tab), Some(viewport),),
            WorkspaceDashboardAction::Consumed,
        );
    }
    assert_eq!(dashboard.focus(), WorkspaceFocus::NoteBody);
    let cursor_before = dashboard
        .cursor_pos(viewport)
        .expect("note editor should expose a cursor");

    for character in "abcdef".chars() {
        assert_eq!(
            dashboard.handle_key_event_for_viewport(
                KeyEvent::from(KeyCode::Char(character)),
                Some(viewport),
            ),
            WorkspaceDashboardAction::Consumed,
        );
    }

    let cursor_after = dashboard
        .cursor_pos(viewport)
        .expect("note editor cursor should remain visible");
    assert_eq!(dashboard.focus(), WorkspaceFocus::NoteBody);
    assert_eq!(cursor_after, (cursor_before.0 + 6, cursor_before.1));
}
