//! Small adapter that lets full-screen workspace fields share the mature composer editor.
//!
//! Workspace drafts remain the persistence-facing source of truth. This adapter owns only
//! transient cursor, wrapping, and scroll state, synchronizing from a draft before every edit or
//! render and returning the edited text to the caller afterward.

use crate::bottom_pane::TextArea;
use crate::bottom_pane::TextAreaState;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::StatefulWidgetRef;
use std::cell::RefCell;

#[derive(Debug)]
pub(crate) struct WorkspaceEditor {
    textarea: RefCell<TextArea>,
    state: RefCell<TextAreaState>,
}

impl WorkspaceEditor {
    pub(crate) fn new() -> Self {
        Self {
            textarea: RefCell::new(TextArea::new()),
            state: RefCell::new(TextAreaState::default()),
        }
    }

    /// Replace the loaded draft and place the cursor at its end.
    pub(crate) fn reset(&self, text: &str) {
        // A fresh editor is intentional here. `TextArea::set_text_clearing_elements` preserves
        // its kill/yank buffer, which must never cross a patient, note, or request scope reset.
        let mut textarea = TextArea::new();
        textarea.set_text_clearing_elements(text);
        textarea.set_cursor(text.len());
        *self.textarea.borrow_mut() = textarea;
        *self.state.borrow_mut() = TextAreaState::default();
    }

    /// Apply a conventional text-editing key and return the resulting draft text.
    pub(crate) fn input(&self, current_text: &str, event: KeyEvent) -> String {
        self.sync(current_text);
        self.textarea.borrow_mut().input(event);
        self.text()
    }

    /// Insert bracketed-paste text at the active cursor and return the resulting draft text.
    pub(crate) fn paste(&self, current_text: &str, pasted: &str) -> String {
        self.sync(current_text);
        self.textarea.borrow_mut().insert_str(pasted);
        self.text()
    }

    pub(crate) fn render(&self, current_text: &str, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.sync(current_text);
        let textarea = self.textarea.borrow();
        let mut state = self.state.borrow_mut();
        StatefulWidgetRef::render_ref(&&*textarea, area, buf, &mut state);
    }

    pub(crate) fn cursor_pos(&self, current_text: &str, area: Rect) -> Option<(u16, u16)> {
        if area.width == 0 || area.height == 0 {
            return None;
        }
        self.sync(current_text);
        self.textarea
            .borrow()
            .cursor_pos_with_state(area, *self.state.borrow())
    }

    fn sync(&self, current_text: &str) {
        let already_synced = self.textarea.borrow().text() == current_text;
        if !already_synced {
            self.reset(current_text);
        }
    }

    fn text(&self) -> String {
        self.textarea.borrow().text().to_string()
    }
}

impl Clone for WorkspaceEditor {
    fn clone(&self) -> Self {
        let cloned = Self::new();
        let textarea = self.textarea.borrow();
        cloned.reset(textarea.text());
        cloned.textarea.borrow_mut().set_cursor(textarea.cursor());
        *cloned.state.borrow_mut() = *self.state.borrow();
        cloned
    }
}

impl Default for WorkspaceEditor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "workspace_editor_tests.rs"]
mod tests;
