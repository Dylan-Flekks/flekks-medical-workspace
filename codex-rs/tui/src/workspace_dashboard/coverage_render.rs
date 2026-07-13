use super::patient_admin_render::patient_admin_editor_field_line;
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CardVerificationField {
    SourceDocument,
    ComparedSubject,
    PrintedFirstName,
    PrintedMiddleName,
    PrintedLastName,
    PrintedSuffix,
    PrintedMemberId,
    ConfirmedBy,
}

impl CardVerificationField {
    const ALL: [Self; 8] = [
        Self::SourceDocument,
        Self::ComparedSubject,
        Self::PrintedFirstName,
        Self::PrintedMiddleName,
        Self::PrintedLastName,
        Self::PrintedSuffix,
        Self::PrintedMemberId,
        Self::ConfirmedBy,
    ];

    fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|field| *field == self)
            .unwrap_or(0)
    }

    pub(super) fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub(super) fn previous(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::SourceDocument => "Source card document ID",
            Self::ComparedSubject => "Compare against",
            Self::PrintedFirstName => "Printed first name",
            Self::PrintedMiddleName => "Printed middle name",
            Self::PrintedLastName => "Printed last name",
            Self::PrintedSuffix => "Printed suffix",
            Self::PrintedMemberId => "Printed member ID",
            Self::ConfirmedBy => "Confirmed by",
        }
    }
}

impl WorkspaceDashboard {
    pub(super) fn coverage_header_status(&self) -> String {
        let mut count = self.coverages.len();
        if self.coverage_draft.id.is_none() && coverage_draft_has_input(&self.coverage_draft) {
            count = count.saturating_add(1).min(CoveragePriority::ALL.len());
            return format!("{count}/3 · unsaved");
        }
        let Some(coverage) = self
            .coverages
            .iter()
            .find(|coverage| coverage.priority == CoveragePriority::Primary.number())
            .or_else(|| self.coverages.first())
        else {
            return "Missing coverage".to_string();
        };
        format!(
            "{}/3 · {}",
            count.min(CoveragePriority::ALL.len()),
            billing_readiness_short_label(coverage.billing_readiness)
        )
    }

    pub(super) fn coverage_editor_lines(&self) -> Vec<Line<'static>> {
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
                PatientAdminEditMode::Coverage.title(),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            self.coverage_priority_tabs_line(),
            Line::from(vec![
                Span::styled("Billing readiness: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    billing_readiness_label(self.coverage_draft.billing_readiness),
                    coverage_readiness_style(self.coverage_draft.billing_readiness),
                ),
            ]),
            Line::from(Span::styled(
                billing_readiness_summary(self.coverage_draft.billing_readiness),
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "Coverage/member ID is separate from Patient ID / MRN. Billing status never blocks clinical chart saves.",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        for field in CoverageField::ALL {
            let heading = match field {
                CoverageField::PayerName => Some("Plan and member"),
                CoverageField::PatientRelationshipToSubscriber => Some("Subscriber identity"),
                CoverageField::SubscriberAddressSameAsPatient => Some("Subscriber address"),
                CoverageField::CoverageNotes => Some("Administrative notes"),
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
            let active = self.focus == WorkspaceFocus::Demographics && self.coverage_field == field;
            let value = if field == CoverageField::SubscriberAddressSameAsPatient {
                if self.coverage_draft.subscriber_address_same_as_patient {
                    "yes"
                } else {
                    "no"
                }
            } else {
                self.coverage_draft.text(field).unwrap_or_default()
            };
            lines.push(patient_admin_editor_field_line(
                field.label(),
                value,
                active,
                "not set",
            ));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Card comparison history",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        )));
        if self.coverage_verifications.is_empty() {
            lines.push(Line::from("  No human-confirmed card comparison on file."));
        } else {
            for verification in self.coverage_verifications.iter().take(3) {
                let result = match verification.match_result {
                    WorkspaceCoverageMatchResult::Match => "MATCH",
                    WorkspaceCoverageMatchResult::Mismatch => "MISMATCH",
                };
                let stale = if verification.is_stale {
                    " · stale"
                } else {
                    ""
                };
                let mismatches = if verification.mismatch_fields.is_empty() {
                    String::new()
                } else {
                    format!(" · fields {}", verification.mismatch_fields.join(", "))
                };
                lines.push(Line::from(format!(
                    "  {} · {} · {}{}{}",
                    epoch_seconds_date_label(verification.created_at),
                    result,
                    compact_preview(&verification.actor, 24),
                    stale,
                    mismatches
                )));
            }
        }
        lines.push(Line::from(Span::styled(
            "Ctrl-S saves coverage · :coverage verify records a human comparison · no OCR, eligibility, payer, claim, or EDI action",
            Style::default().fg(Color::DarkGray),
        )));
        lines
    }

    fn coverage_priority_tabs_line(&self) -> Line<'static> {
        let mut spans = vec![Span::styled("Priority: ", Style::default().fg(Color::Gray))];
        for (index, priority) in CoveragePriority::ALL.into_iter().enumerate() {
            if index > 0 {
                spans.push(Span::raw("  "));
            }
            let label = if priority == self.coverage_draft.priority {
                format!("[{}]", priority.label())
            } else {
                priority.label().to_string()
            };
            let style = if priority == self.coverage_draft.priority {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            spans.push(Span::styled(label, style));
        }
        spans.push(Span::styled(
            "  ←/→ switch",
            Style::default().fg(Color::DarkGray),
        ));
        Line::from(spans)
    }

    pub(super) fn card_verification_editor_lines(&self) -> Vec<Line<'static>> {
        let Some(draft) = self.card_verification_draft.as_ref() else {
            return self.coverage_editor_lines();
        };
        let source_label = self
            .documents
            .iter()
            .find(|document| document.id == draft.source_document_id)
            .map(|document| compact_preview(&document.title, 56))
            .unwrap_or_else(|| "source document not selected".to_string());
        let mut lines = vec![
            Line::from(Span::styled(
                "Human Coverage Card Comparison",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(self.coverage_draft.concise_summary()),
            Line::from(Span::styled(
                CARD_VERIFICATION_ENTRY_HELP,
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "This creates an append-only audit record pinned to the current patient and coverage versions.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(format!("Selected source: {source_label}")),
            Line::from(""),
        ];
        for field in CardVerificationField::ALL {
            let active =
                self.focus == WorkspaceFocus::Demographics && self.card_verification_field == field;
            let value = self.card_verification_field_value(field);
            let empty_label = match field {
                CardVerificationField::ComparedSubject => "beneficiary",
                CardVerificationField::PrintedMiddleName | CardVerificationField::PrintedSuffix => {
                    "optional"
                }
                _ => "required",
            };
            lines.push(patient_admin_editor_field_line(
                field.label(),
                &value,
                active,
                empty_label,
            ));
        }
        lines.extend([
            Line::from(""),
            Line::from(Span::styled(
                "Space cycles source/toggles subject · Ctrl-S records · Esc discards this unsaved comparison",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "No OCR/model extraction, eligibility check, payer message, claim, submission, or remote upload.",
                Style::default().fg(Color::DarkGray),
            )),
        ]);
        lines
    }

    pub(super) fn card_verification_field_value(&self, field: CardVerificationField) -> String {
        let Some(draft) = self.card_verification_draft.as_ref() else {
            return String::new();
        };
        match field {
            CardVerificationField::SourceDocument => draft.source_document_id.clone(),
            CardVerificationField::ComparedSubject => {
                verification_subject_label(draft.compared_subject).to_string()
            }
            CardVerificationField::PrintedFirstName => draft.observed_first_name.clone(),
            CardVerificationField::PrintedMiddleName => draft.observed_middle_name.clone(),
            CardVerificationField::PrintedLastName => draft.observed_last_name.clone(),
            CardVerificationField::PrintedSuffix => draft.observed_suffix.clone(),
            CardVerificationField::PrintedMemberId => draft.observed_member_id.clone(),
            CardVerificationField::ConfirmedBy => draft.actor.clone(),
        }
    }
}

pub(super) fn billing_readiness_short_label(readiness: WorkspaceBillingReadiness) -> &'static str {
    match readiness {
        WorkspaceBillingReadiness::Match => "match",
        WorkspaceBillingReadiness::Mismatch => "mismatch",
        WorkspaceBillingReadiness::Unverified => "unverified",
        WorkspaceBillingReadiness::Stale => "stale",
        WorkspaceBillingReadiness::Incomplete => "incomplete",
    }
}

fn coverage_readiness_style(readiness: WorkspaceBillingReadiness) -> Style {
    let color = match readiness {
        WorkspaceBillingReadiness::Match => Color::Green,
        WorkspaceBillingReadiness::Mismatch => Color::Red,
        WorkspaceBillingReadiness::Unverified
        | WorkspaceBillingReadiness::Stale
        | WorkspaceBillingReadiness::Incomplete => Color::Yellow,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}
