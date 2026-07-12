use super::*;

impl WorkspaceDashboard {
    pub(super) fn open_patient_admin_editor(
        &mut self,
        mode: PatientAdminEditMode,
        field: PatientAdminField,
    ) {
        self.patient_search_query = None;
        self.patient_search_selection_index = 0;
        self.patient_search_return_focus = None;
        self.patient_search_return_section = None;
        self.action_overlay_visible = false;
        self.command_input = None;
        self.patient_admin_edit_mode = Some(mode);
        if mode == PatientAdminEditMode::Coverage {
            self.coverage_field = CoverageField::PayerName;
        } else {
            self.patient_admin_field = if field.mode() == mode {
                field
            } else {
                mode.first_field()
            };
        }
        self.focus = WorkspaceFocus::Demographics;
        self.status = match mode {
            PatientAdminEditMode::Contact => {
                "Editing patient demographics and emergency contact fields. Ctrl-S saves."
                    .to_string()
            }
            PatientAdminEditMode::Coverage => {
                "Editing coverage fields. Member ID stays under coverage; Ctrl-S saves.".to_string()
            }
        };
    }

    pub(super) fn handle_demographics_key_event(
        &mut self,
        key_event: KeyEvent,
    ) -> WorkspaceDashboardAction {
        if self.profile == WorkspaceProfile::Medical
            && let Some(mode) = self.patient_admin_edit_mode
        {
            return self.handle_patient_admin_key_event(mode, key_event);
        }
        if self.profile == WorkspaceProfile::Medical {
            if key_event.code == KeyCode::Enter {
                self.open_patient_admin_editor(
                    PatientAdminEditMode::Contact,
                    PatientAdminField::DisplayName,
                );
            } else {
                self.status =
                    "Patient demographics summary. Press Enter to edit identity, contact, address, and emergency fields."
                        .to_string();
            }
            return WorkspaceDashboardAction::Consumed;
        }
        match key_event.code {
            KeyCode::Up => self.demographics_field = self.demographics_field.previous(),
            KeyCode::Down | KeyCode::Enter => {
                self.demographics_field = self.demographics_field.next()
            }
            KeyCode::Backspace => {
                self.draft_client.value_mut(self.demographics_field).pop();
                self.mark_dirty();
            }
            KeyCode::Char(c) => {
                self.draft_client.value_mut(self.demographics_field).push(c);
                self.mark_dirty();
            }
            _ if text_cursor_navigation_key(&key_event) => {
                self.status =
                    "Text fields are end-anchored: type appends, Backspace deletes, Up/Down changes field."
                        .to_string();
            }
            _ => {}
        }
        WorkspaceDashboardAction::Consumed
    }

    fn handle_patient_admin_key_event(
        &mut self,
        mode: PatientAdminEditMode,
        key_event: KeyEvent,
    ) -> WorkspaceDashboardAction {
        if mode == PatientAdminEditMode::Coverage {
            return self.handle_coverage_key_event(key_event);
        }
        match key_event.code {
            KeyCode::Up => self.patient_admin_field = self.patient_admin_field.previous_in(mode),
            KeyCode::Down | KeyCode::Enter => {
                self.patient_admin_field = self.patient_admin_field.next_in(mode)
            }
            KeyCode::Backspace => {
                self.draft_client
                    .admin_value_mut(self.patient_admin_field)
                    .pop();
                self.mark_dirty();
            }
            KeyCode::Char(c) => {
                self.draft_client
                    .admin_value_mut(self.patient_admin_field)
                    .push(c);
                self.mark_dirty();
            }
            _ if text_cursor_navigation_key(&key_event) => {
                self.status =
                    "Text fields are end-anchored: type appends, Backspace deletes, Up/Down changes field."
                        .to_string();
            }
            _ => {}
        }
        WorkspaceDashboardAction::Consumed
    }

    fn handle_coverage_key_event(&mut self, key_event: KeyEvent) -> WorkspaceDashboardAction {
        if self.card_verification_draft.is_some() {
            return self.handle_card_verification_key_event(key_event);
        }
        match key_event.code {
            KeyCode::Left | KeyCode::Right if self.dirty => {
                self.status = "Save the current coverage before switching priority.".to_string();
            }
            KeyCode::Left => {
                return WorkspaceDashboardAction::SelectCoveragePriority(
                    self.coverage_draft.priority.previous_wrapping().number(),
                );
            }
            KeyCode::Right => {
                return WorkspaceDashboardAction::SelectCoveragePriority(
                    self.coverage_draft.priority.next_wrapping().number(),
                );
            }
            KeyCode::Up => self.coverage_field = self.coverage_field.previous_wrapping(),
            KeyCode::Down | KeyCode::Enter => {
                self.coverage_field = self.coverage_field.next_wrapping()
            }
            KeyCode::Char(' ') if self.coverage_field.is_toggle() => {
                self.coverage_draft
                    .toggle_subscriber_address_same_as_patient();
                self.mark_dirty();
            }
            KeyCode::Backspace => {
                if let Some(value) = self.coverage_draft.text_mut(self.coverage_field) {
                    value.pop();
                    self.mark_dirty();
                }
            }
            KeyCode::Char(c) => {
                if let Some(value) = self.coverage_draft.text_mut(self.coverage_field) {
                    value.push(c);
                    self.mark_dirty();
                }
            }
            _ if text_cursor_navigation_key(&key_event) => {
                self.status =
                    "Coverage fields are end-anchored; Up/Down changes field and Left/Right changes priority."
                        .to_string();
            }
            _ => {}
        }
        WorkspaceDashboardAction::Consumed
    }

    fn handle_card_verification_key_event(
        &mut self,
        key_event: KeyEvent,
    ) -> WorkspaceDashboardAction {
        match key_event.code {
            KeyCode::Up => self.card_verification_field = self.card_verification_field.previous(),
            KeyCode::Down | KeyCode::Enter => {
                self.card_verification_field = self.card_verification_field.next()
            }
            KeyCode::Char(' ')
                if self.card_verification_field == CardVerificationField::ComparedSubject =>
            {
                if let Some(draft) = self.card_verification_draft.as_mut() {
                    draft.toggle_compared_subject();
                }
            }
            KeyCode::Char(' ')
                if self.card_verification_field == CardVerificationField::SourceDocument =>
            {
                self.cycle_card_verification_source_document();
            }
            KeyCode::Backspace => {
                if let Some(value) = self.card_verification_value_mut() {
                    value.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(value) = self.card_verification_value_mut() {
                    value.push(c);
                }
            }
            _ if text_cursor_navigation_key(&key_event) => {
                self.status =
                    "Card fields are human-entered and end-anchored; Up/Down changes field."
                        .to_string();
            }
            _ => {}
        }
        WorkspaceDashboardAction::Consumed
    }

    fn card_verification_value_mut(&mut self) -> Option<&mut String> {
        let draft = self.card_verification_draft.as_mut()?;
        match self.card_verification_field {
            CardVerificationField::SourceDocument => None,
            CardVerificationField::ComparedSubject => None,
            CardVerificationField::PrintedFirstName => Some(&mut draft.observed_first_name),
            CardVerificationField::PrintedMiddleName => Some(&mut draft.observed_middle_name),
            CardVerificationField::PrintedLastName => Some(&mut draft.observed_last_name),
            CardVerificationField::PrintedSuffix => Some(&mut draft.observed_suffix),
            CardVerificationField::PrintedMemberId => Some(&mut draft.observed_member_id),
            CardVerificationField::ConfirmedBy => Some(&mut draft.actor),
        }
    }

    fn cycle_card_verification_source_document(&mut self) {
        let eligible = self
            .documents
            .iter()
            .filter(|document| coverage_card_document_is_eligible(document))
            .map(|document| {
                (
                    document.id.clone(),
                    document.version.clone(),
                    document.title.clone(),
                )
            })
            .collect::<Vec<_>>();
        if eligible.is_empty() {
            self.status =
                "Add a present, hashed local insurance-card reference before recording comparison."
                    .to_string();
            return;
        }
        let Some(draft) = self.card_verification_draft.as_mut() else {
            return;
        };
        let current = eligible
            .iter()
            .position(|(document_id, _, _)| document_id == &draft.source_document_id);
        let next = current.map_or(0, |index| (index + 1) % eligible.len());
        let (document_id, document_version, document_title) = &eligible[next];
        draft.source_document_id.clone_from(document_id);
        draft
            .expected_document_version
            .clone_from(document_version);
        self.status = format!(
            "Selected source card document: {}.",
            compact_preview(document_title, 56)
        );
    }
}
