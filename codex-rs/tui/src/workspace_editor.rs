//! Small adapter that lets full-screen workspace fields share the mature composer editor.
//!
//! Workspace drafts remain the persistence-facing source of truth. This adapter owns only
//! transient cursor, wrapping, and scroll state, synchronizing from a draft before every edit or
//! render and returning the edited text to the caller afterward.

use crate::bottom_pane::TextArea;
use crate::bottom_pane::TextAreaState;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::RuntimeKeymap;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::StatefulWidgetRef;
use std::cell::RefCell;

/// Input behavior for one workspace text field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkspaceEditorPolicy {
    /// Composer behavior: submit bindings win over overlapping newline bindings.
    Prompt,
    /// Text-editor behavior: newline bindings always edit the document.
    Document,
}

/// Semantic result of dispatching a key through a [`WorkspaceEditor`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkspaceEditorInput {
    /// The field remains active. The text may be unchanged when the key only moved the cursor.
    Editing(String),
    /// The prompt should be submitted with the returned text.
    Submit(String),
}

impl WorkspaceEditorInput {
    pub(crate) fn into_text(self) -> String {
        match self {
            Self::Editing(text) | Self::Submit(text) => text,
        }
    }
}

#[derive(Debug)]
pub(crate) struct WorkspaceEditor {
    textarea: RefCell<TextArea>,
    state: RefCell<TextAreaState>,
    keymap: RefCell<RuntimeKeymap>,
    policy: WorkspaceEditorPolicy,
}

impl WorkspaceEditor {
    pub(crate) fn new() -> Self {
        Self::with_policy(WorkspaceEditorPolicy::Document)
    }

    pub(crate) fn with_policy(policy: WorkspaceEditorPolicy) -> Self {
        let keymap = RuntimeKeymap::defaults();
        let mut textarea = TextArea::new();
        textarea.set_keymap_bindings(&keymap);
        Self {
            textarea: RefCell::new(textarea),
            state: RefCell::new(TextAreaState::default()),
            keymap: RefCell::new(keymap),
            policy,
        }
    }

    /// Apply a resolved runtime keymap without disturbing the active draft or cursor.
    pub(crate) fn set_keymap_bindings(&self, keymap: &RuntimeKeymap) {
        self.textarea.borrow_mut().set_keymap_bindings(keymap);
        *self.keymap.borrow_mut() = keymap.clone();
    }

    /// Replace the loaded draft and place the cursor at its end.
    pub(crate) fn reset(&self, text: &str) {
        // A fresh editor is intentional here. `TextArea::set_text_clearing_elements` preserves
        // its kill/yank buffer, which must never cross a patient, note, or request scope reset.
        let mut textarea = TextArea::new();
        textarea.set_keymap_bindings(&self.keymap.borrow());
        textarea.set_text_clearing_elements(text);
        textarea.set_cursor(text.len());
        *self.textarea.borrow_mut() = textarea;
        *self.state.borrow_mut() = TextAreaState::default();
    }

    /// Apply a conventional text-editing key and return the resulting draft text.
    #[cfg(test)]
    pub(crate) fn input(&self, current_text: &str, event: KeyEvent) -> String {
        self.sync(current_text);
        self.textarea.borrow_mut().input(event);
        self.text()
    }

    /// Dispatch input according to this field's policy and return its semantic result.
    pub(crate) fn handle_input(&self, current_text: &str, event: KeyEvent) -> WorkspaceEditorInput {
        self.sync(current_text);

        let keymap = self.keymap.borrow();
        match self.policy {
            WorkspaceEditorPolicy::Prompt if keymap.composer.submit.is_pressed(event) => {
                return WorkspaceEditorInput::Submit(self.text());
            }
            WorkspaceEditorPolicy::Prompt | WorkspaceEditorPolicy::Document => {}
        }
        drop(keymap);

        self.textarea.borrow_mut().input(event);
        WorkspaceEditorInput::Editing(self.text())
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
        let cloned = Self::with_policy(self.policy);
        cloned.set_keymap_bindings(&self.keymap.borrow());
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
