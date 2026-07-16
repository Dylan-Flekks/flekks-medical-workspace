use super::*;
use crate::key_hint;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;

#[test]
fn prompt_submit_takes_precedence_over_the_default_enter_newline() {
    let editor = WorkspaceEditor::with_policy(WorkspaceEditorPolicy::Prompt);
    editor.reset("first line");

    assert_eq!(
        editor.handle_input("first line", KeyEvent::from(KeyCode::Enter)),
        WorkspaceEditorInput::Submit("first line".to_string())
    );
    assert_eq!(
        editor.handle_input(
            "first line",
            KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT),
        ),
        WorkspaceEditorInput::Editing("first line\n".to_string())
    );
}

#[test]
fn prompt_uses_live_configured_submit_and_newline_bindings() {
    let editor = WorkspaceEditor::with_policy(WorkspaceEditorPolicy::Prompt);
    let mut keymap = RuntimeKeymap::defaults();
    keymap.composer.submit = vec![key_hint::ctrl(KeyCode::Enter)];
    keymap.editor.insert_newline = vec![key_hint::alt(KeyCode::Char('n'))];
    editor.set_keymap_bindings(&keymap);
    editor.reset("draft");

    assert_eq!(
        editor.handle_input("draft", KeyEvent::from(KeyCode::Enter)),
        WorkspaceEditorInput::Editing("draft".to_string())
    );
    assert_eq!(
        editor.handle_input(
            "draft",
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::ALT),
        ),
        WorkspaceEditorInput::Editing("draft\n".to_string())
    );
    assert_eq!(
        editor.handle_input(
            "draft\n",
            KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        ),
        WorkspaceEditorInput::Submit("draft\n".to_string())
    );
}

#[test]
fn document_enter_inserts_a_newline_inside_unicode_text() {
    let editor = WorkspaceEditor::with_policy(WorkspaceEditorPolicy::Document);
    editor.reset("A😀B");
    let text = editor
        .handle_input("A😀B", KeyEvent::from(KeyCode::Left))
        .into_text();

    assert_eq!(
        editor.handle_input(&text, KeyEvent::from(KeyCode::Enter)),
        WorkspaceEditorInput::Editing("A😀\nB".to_string())
    );
}

#[test]
fn reset_retains_live_keymap_while_clearing_scope_state() {
    let editor = WorkspaceEditor::with_policy(WorkspaceEditorPolicy::Document);
    let mut keymap = RuntimeKeymap::defaults();
    keymap.editor.move_left = vec![key_hint::alt(KeyCode::Char('h'))];
    editor.set_keymap_bindings(&keymap);
    editor.reset("patient A");

    let text = editor
        .handle_input(
            "patient A",
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT),
        )
        .into_text();
    let text = editor
        .handle_input(&text, KeyEvent::from(KeyCode::Char('!')))
        .into_text();
    assert_eq!(text, "patient !A");

    editor.reset("patient B");
    let text = editor
        .handle_input(
            "patient B",
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT),
        )
        .into_text();
    assert_eq!(
        editor.handle_input(&text, KeyEvent::from(KeyCode::Char('!'))),
        WorkspaceEditorInput::Editing("patient !B".to_string())
    );
}

#[test]
fn clone_retains_live_keymap_without_copying_kill_yank_history() {
    let editor = WorkspaceEditor::with_policy(WorkspaceEditorPolicy::Document);
    let mut keymap = RuntimeKeymap::defaults();
    keymap.editor.move_left = vec![key_hint::alt(KeyCode::Char('h'))];
    editor.set_keymap_bindings(&keymap);
    editor.reset("patient A private text");
    let text = editor
        .handle_input("patient A private text", KeyEvent::from(KeyCode::Home))
        .into_text();
    let _ = editor.handle_input(
        &text,
        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
    );

    let cloned = editor;
    cloned.reset("patient B");
    let text = cloned
        .handle_input(
            "patient B",
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL),
        )
        .into_text();
    let text = cloned
        .handle_input(&text, KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT))
        .into_text();

    assert_eq!(
        cloned.handle_input(&text, KeyEvent::from(KeyCode::Char('!'))),
        WorkspaceEditorInput::Editing("patient !B".to_string())
    );
}

#[test]
fn explicit_line_navigation_preserves_the_preferred_column() {
    let editor = WorkspaceEditor::new();
    editor.reset("abc\ndefgh\nxy");

    let text = editor.input("abc\ndefgh\nxy", KeyEvent::from(KeyCode::Up));
    let text = editor.input(&text, KeyEvent::from(KeyCode::Char('X')));

    assert_eq!(text, "abc\ndeXfgh\nxy");
}

#[test]
fn wrapped_line_navigation_uses_the_rendered_width() {
    let editor = WorkspaceEditor::new();
    editor.reset("abcdefgh");
    let area = Rect::new(0, 0, 4, 2);
    let mut buf = Buffer::empty(area);
    editor.render("abcdefgh", area, &mut buf);

    let text = editor.input("abcdefgh", KeyEvent::from(KeyCode::Up));
    let text = editor.input(&text, KeyEvent::from(KeyCode::Char('X')));

    assert_eq!(text, "abcdXefgh");
}

#[test]
fn unicode_movement_and_cursor_aware_paste_keep_graphemes_intact() {
    let editor = WorkspaceEditor::new();
    editor.reset("A😀B");

    let text = editor.input("A😀B", KeyEvent::from(KeyCode::Left));
    let text = editor.paste(&text, "中\n文");

    assert_eq!(text, "A😀中\n文B");
}

#[test]
fn home_end_delete_and_backspace_edit_the_active_line() {
    let editor = WorkspaceEditor::new();
    editor.reset("alpha\nbeta");

    let text = editor.input("alpha\nbeta", KeyEvent::from(KeyCode::Home));
    let text = editor.input(&text, KeyEvent::from(KeyCode::Delete));
    let text = editor.input(&text, KeyEvent::from(KeyCode::End));
    let text = editor.input(&text, KeyEvent::from(KeyCode::Backspace));

    assert_eq!(text, "alpha\net");
}

#[test]
fn scrolling_keeps_the_cursor_inside_the_editor_viewport() {
    let editor = WorkspaceEditor::new();
    editor.reset("one\ntwo\nthree");
    let area = Rect::new(5, 7, 8, 2);
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 12));

    editor.render("one\ntwo\nthree", area, &mut buf);

    assert_eq!(editor.cursor_pos("one\ntwo\nthree", area), Some((10, 8)));
}

#[test]
fn reset_clears_kill_yank_history_between_patient_scopes() {
    let editor = WorkspaceEditor::new();
    editor.reset("patient A private text");
    let text = editor.input("patient A private text", KeyEvent::from(KeyCode::Home));
    let _ = editor.input(
        &text,
        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
    );

    editor.reset("patient B text");
    let text = editor.input(
        "patient B text",
        KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL),
    );

    assert_eq!(text, "patient B text");
}
