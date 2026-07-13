use super::coverage_render::billing_readiness_short_label;
use super::*;

pub(super) fn patient_admin_editor_field_line(
    label: &str,
    value: &str,
    active: bool,
    placeholder: &str,
) -> Line<'static> {
    let label_style = if active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    let value_span = if value.trim().is_empty() {
        Span::styled(
            placeholder.to_string(),
            Style::default().fg(Color::DarkGray),
        )
    } else {
        Span::raw(value.to_string())
    };
    Line::from(vec![
        Span::styled(
            format!("{}{label}: ", if active { "> " } else { "  " }),
            label_style,
        ),
        value_span,
    ])
}

impl WorkspaceDashboard {
    pub(super) fn cursor_pos_in_patient_admin_inner(
        &self,
        inner: Rect,
        mode: PatientAdminEditMode,
    ) -> Option<(u16, u16)> {
        if inner.width == 0 || inner.height == 0 {
            return None;
        }
        let lines = self.patient_admin_editor_lines(mode);
        let (selected_prefix, label_len, value) = match mode {
            PatientAdminEditMode::Contact => (
                format!("> {}:", self.patient_admin_field.label()),
                self.patient_admin_field.label().chars().count() + 4,
                patient_admin_metadata_for_draft(&self.draft_client)
                    .value(self.patient_admin_field)
                    .to_string(),
            ),
            PatientAdminEditMode::Coverage if self.card_verification_draft.is_some() => {
                if matches!(
                    self.card_verification_field,
                    CardVerificationField::SourceDocument | CardVerificationField::ComparedSubject
                ) {
                    return None;
                }
                (
                    format!("> {}:", self.card_verification_field.label()),
                    self.card_verification_field.label().chars().count() + 4,
                    self.card_verification_field_value(self.card_verification_field),
                )
            }
            PatientAdminEditMode::Coverage => {
                if self.coverage_field.is_toggle() {
                    return None;
                }
                (
                    format!("> {}:", self.coverage_field.label()),
                    self.coverage_field.label().chars().count() + 4,
                    self.coverage_draft
                        .text(self.coverage_field)
                        .unwrap_or_default()
                        .to_string(),
                )
            }
        };
        let row = lines
            .iter()
            .position(|line| line_plain_text(line).starts_with(&selected_prefix))?;
        let visual_row =
            workflow_visual_row_for_line(&lines, row.min(u16::MAX as usize) as u16, inner.width);
        let scroll = visual_row.saturating_add(1).saturating_sub(inner.height);
        cursor_at_wrapped_line_text_end(inner, &lines, row, scroll as usize, label_len, &value)
    }

    pub(super) fn patient_admin_editor_lines(
        &self,
        mode: PatientAdminEditMode,
    ) -> Vec<Line<'static>> {
        if mode == PatientAdminEditMode::Coverage {
            return if self.card_verification_draft.is_some() {
                self.card_verification_editor_lines()
            } else {
                self.coverage_editor_lines()
            };
        }
        let admin = patient_admin_metadata_for_draft(&self.draft_client);
        let mut lines = vec![
            Line::from(format!(
                "Patient: {}",
                nonempty_or(&self.draft_client.display_name, "unsaved patient")
            )),
            Line::from(format!(
                "DOB: {}; Patient ID / MRN: {}",
                nonempty_or(&self.draft_client.date_of_birth, "blank"),
                nonempty_or(&self.draft_client.external_id, "blank")
            )),
            Line::from(Span::styled(
                mode.title(),
                Style::default().add_modifier(Modifier::BOLD),
            )),
        ];
        lines.push(Line::from(format!(
            "Legal/card name: {}",
            admin.legal_name_summary()
        )));
        lines.push(Line::from(format!("Contact: {}", admin.contact_summary())));
        lines.push(Line::from(format!("Address: {}", admin.address_summary())));
        lines.push(Line::from(Span::styled(
            "Enter legal identity exactly as printed; card comparison is recorded separately.",
            Style::default().fg(Color::DarkGray),
        )));
        for field in mode.fields() {
            let heading = match field {
                PatientAdminField::DisplayName => Some("Identity"),
                PatientAdminField::PrimaryPhone => Some("Contact"),
                PatientAdminField::AddressLine1 => Some("Home / Mailing Address"),
                PatientAdminField::EmergencyContactName => Some("Emergency Contact"),
                _ => None,
            };
            if let Some(heading) = heading {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    heading,
                    Style::default()
                        .fg(Color::Gray)
                        .add_modifier(Modifier::BOLD),
                )));
            }
            let active =
                self.focus == WorkspaceFocus::Demographics && self.patient_admin_field == *field;
            lines.push(patient_admin_editor_field_line(
                field.label(),
                admin.value(*field),
                active,
                field.placeholder(),
            ));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Local fake demographics metadata only; Ctrl-S saves the patient.",
            Style::default().fg(Color::DarkGray),
        )));
        lines
    }

    pub(super) fn render_demographics(&self, area: Rect, buf: &mut Buffer) {
        if self.profile == WorkspaceProfile::Medical
            && let Some(mode) = self.patient_admin_edit_mode
        {
            let block = pane_block(mode.title(), self.focus == WorkspaceFocus::Demographics);
            let inner = block.inner(area);
            Clear.render(area, buf);
            block.render(area, buf);
            if inner.width == 0 || inner.height == 0 {
                return;
            }
            let lines = self.patient_admin_editor_lines(mode);
            let selected_prefix = match mode {
                PatientAdminEditMode::Contact => {
                    format!("> {}:", self.patient_admin_field.label())
                }
                PatientAdminEditMode::Coverage if self.card_verification_draft.is_some() => {
                    format!("> {}:", self.card_verification_field.label())
                }
                PatientAdminEditMode::Coverage => {
                    format!("> {}:", self.coverage_field.label())
                }
            };
            let selected_row = lines
                .iter()
                .position(|line| line_plain_text(line).starts_with(&selected_prefix))
                .unwrap_or(0);
            let selected_visual_row = workflow_visual_row_for_line(
                &lines,
                selected_row.min(u16::MAX as usize) as u16,
                inner.width,
            );
            let scroll = selected_visual_row
                .saturating_add(1)
                .saturating_sub(inner.height);
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((scroll, 0))
                .render(inner, buf);
            return;
        }
        if self.profile == WorkspaceProfile::Medical {
            let admin = patient_admin_metadata_for_draft(&self.draft_client);
            let preferred = nonempty_or(&self.draft_client.preferred_name, "not set");
            let lines = vec![
                Line::from(format!(
                    "Identity: {} · preferred {}",
                    nonempty_or(&self.draft_client.display_name, "missing"),
                    preferred
                )),
                Line::from(format!("Legal/card: {}", admin.legal_name_summary())),
                Line::from(format!(
                    "DOB: {} · admin sex: {} · MRN: {}",
                    nonempty_or(&self.draft_client.date_of_birth, "missing"),
                    nonempty_or(&self.draft_client.administrative_sex, "missing"),
                    nonempty_or(&self.draft_client.external_id, "missing")
                )),
                Line::from(format!("Contact: {}", admin.contact_summary())),
                Line::from(format!("Address: {}", admin.address_summary())),
                Line::from(format!("Emergency: {}", admin.emergency_summary())),
                Line::from(format!("Coverage: {}", self.coverage_slots_summary())),
                Line::from(Span::styled(
                    "Enter edits all identity/contact fields · Ctrl-P opens actions",
                    Style::default().fg(Color::DarkGray),
                )),
            ];
            render_pane(
                self.profile.demographics_title(),
                area,
                self.focus == WorkspaceFocus::Demographics,
                Paragraph::new(lines).wrap(Wrap { trim: false }),
                buf,
            );
            return;
        }
        let lines = DemographicsField::ALL
            .iter()
            .map(|field| {
                let active =
                    self.focus == WorkspaceFocus::Demographics && *field == self.demographics_field;
                field_line(
                    field.label(self.profile),
                    self.draft_client.value(*field),
                    active,
                )
            })
            .collect::<Vec<_>>();
        render_pane(
            self.profile.demographics_title(),
            area,
            self.focus == WorkspaceFocus::Demographics,
            Paragraph::new(lines).wrap(Wrap { trim: false }),
            buf,
        );
    }

    pub(super) fn coverage_slots_summary(&self) -> String {
        CoveragePriority::ALL
            .into_iter()
            .map(|priority| {
                let short_priority = match priority {
                    CoveragePriority::Primary => "P",
                    CoveragePriority::Secondary => "S",
                    CoveragePriority::Tertiary => "T",
                };
                if priority == self.coverage_draft.priority
                    && (self.coverage_draft.id.is_some()
                        || coverage_draft_has_input(&self.coverage_draft))
                {
                    return format!(
                        "{short_priority} {} [{}]",
                        compact_preview(
                            &nonempty_or(&self.coverage_draft.payer_name, "payer missing"),
                            18
                        ),
                        billing_readiness_short_label(self.coverage_draft.billing_readiness)
                    );
                }
                let Some(coverage) = self
                    .coverages
                    .iter()
                    .find(|coverage| coverage.priority == priority.number())
                else {
                    return format!("{short_priority} not entered");
                };
                format!(
                    "{short_priority} {} [{}]",
                    compact_preview(
                        coverage.payer_name.as_deref().unwrap_or("payer missing"),
                        18
                    ),
                    billing_readiness_short_label(coverage.billing_readiness)
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    }
}
