use super::*;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;

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
    let text = editor.input(
        "patient A private text",
        KeyEvent::from(KeyCode::Home),
    );
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
