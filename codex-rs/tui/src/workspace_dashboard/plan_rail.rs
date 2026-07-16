use super::*;
use codex_app_server_protocol::WorkspacePlanMessageRole;
use codex_app_server_protocol::WorkspacePlanRevisionStatus;
use codex_app_server_protocol::WorkspacePlanSubmissionReceipt;

impl WorkspaceDashboard {
    pub(super) fn agent_workpane_chat_lines(&self) -> Vec<Line<'static>> {
        let patient = nonempty_or(&self.draft_client.display_name, "Unsaved patient");
        let selected_file_count = self.selected_artifacts().len();
        let selected_text_count = self.selected_derivatives().len();
        let selected_clip_count = self.selected_clips().len();
        let active_snapshot = self.active_plan_snapshot();
        let current_revision = self.active_plan_revision_with_status(
            active_snapshot,
            WorkspacePlanRevisionStatus::Current,
        );
        let submitted_revision = self.active_plan_revision_with_status(
            active_snapshot,
            WorkspacePlanRevisionStatus::Submitted,
        );
        let outdated_revision = self.active_plan_revision_with_status(
            active_snapshot,
            WorkspacePlanRevisionStatus::Outdated,
        );
        let display_revision = current_revision
            .or(submitted_revision)
            .or(outdated_revision);
        let composer_state = self.agent_composer_state();
        let (state_tone, state_label) = if composer_state == AgentComposerState::DraftRecovery {
            (
                WorkflowTone::Attention,
                "blocked · recover local draft".to_string(),
            )
        } else if composer_state == AgentComposerState::UnsavedPatient {
            (
                WorkflowTone::Attention,
                "blocked · save patient first".to_string(),
            )
        } else if active_snapshot.is_some()
            && let Some(status) = self.plan_streaming_status.as_deref()
        {
            (WorkflowTone::Agent, status.to_string())
        } else if let Some(revision) = display_revision {
            match revision.status {
                WorkspacePlanRevisionStatus::Current => (
                    WorkflowTone::Ready,
                    format!("plan r{} current", revision.revision),
                ),
                WorkspacePlanRevisionStatus::Outdated => (
                    WorkflowTone::Caution,
                    format!("plan r{} outdated · Refresh needed", revision.revision),
                ),
                WorkspacePlanRevisionStatus::Submitted => (
                    WorkflowTone::Review,
                    format!("plan r{} submitted to master", revision.revision),
                ),
            }
        } else {
            match self.agent_workpane_state() {
                AgentWorkpaneState::ContextPlan => {
                    (WorkflowTone::Ready, "ready to plan".to_string())
                }
                AgentWorkpaneState::CodexRunning => {
                    (WorkflowTone::Agent, "Codex is working".to_string())
                }
                AgentWorkpaneState::AgentReview => (
                    WorkflowTone::Review,
                    "recommendation ready for review".to_string(),
                ),
                AgentWorkpaneState::Outdated => (
                    WorkflowTone::Caution,
                    "context changed · Refresh needed".to_string(),
                ),
            }
        };
        let mut lines = vec![
            Line::from(vec![
                Span::styled(
                    "Plan with Codex",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" · {}", compact_preview(&patient, 28)),
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Line::from(format!(
                "  Context  {selected_file_count} files · {selected_text_count} text · {selected_clip_count} clips"
            )),
            workflow_context_signal_line(state_tone, "plan", state_label),
            Line::from(""),
        ];

        if composer_state == AgentComposerState::DraftRecovery {
            lines.extend([
                workflow_header_line("Recovery Required", WorkflowTone::Attention),
                Line::from(
                    "  A recoverable local draft must be resolved before planning continues.",
                ),
                workflow_action_hint_line("Ctrl-P → Restore local draft or Discard local draft"),
                Line::from("  Messaging, chart edits, and master handoff remain locked."),
            ]);
        } else if composer_state == AgentComposerState::UnsavedPatient {
            lines.extend([
                workflow_header_line("Save Patient First", WorkflowTone::Attention),
                Line::from("  Persistent Plan with Codex needs a saved patient record."),
                workflow_action_hint_line("Ctrl-S saves this patient and note locally"),
                Line::from("  After save, return here to message Codex."),
            ]);
        } else if let Some(snapshot) = active_snapshot {
            self.append_plan_snapshot_conversation(&mut lines, snapshot);
        } else if self.context_packets.is_empty() && self.agent_results.is_empty() {
            lines.extend([
                Line::from(Span::styled(
                    "Codex",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from("  I can help connect today's note to the patient's longer-term"),
                Line::from("  goals, prior visits, referrals, and selected files."),
                Line::from(""),
                Line::from(Span::styled(
                    "  Try: What context is missing before I finish this note?",
                    Style::default().fg(Color::DarkGray),
                )),
            ]);
        } else {
            lines.push(Line::from(Span::styled(
                "Conversation",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            for packet in self.context_packets.iter().take(3).rev() {
                lines.push(Line::from(vec![
                    Span::styled("You", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(
                        format!(" · {}", epoch_seconds_date_label(packet.sent_at)),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
                lines.push(Line::from(format!(
                    "  {}",
                    compact_preview(&packet.human_request, 68)
                )));
                if let Some(result) = self.latest_result_for_packet(&packet.id) {
                    lines.push(Line::from(vec![
                        Span::styled("Codex", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled(
                            format!(" · {}", agent_result_status_label(&result.status)),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]));
                    lines.push(Line::from(format!(
                        "  {}",
                        compact_preview(&result.summary, 68)
                    )));
                } else {
                    lines.push(Line::from(Span::styled(
                        "Codex · master analysis pending",
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                lines.push(Line::from(""));
            }
        }

        let plan_needs_refresh = (current_revision.is_none()
            && submitted_revision.is_none()
            && outdated_revision.is_some())
            || (active_snapshot.is_none() && self.context_packet_is_outdated());
        if plan_needs_refresh {
            lines.push(workflow_action_hint_line(
                "Refresh needed: the chart changed after this plan context was captured",
            ));
        }
        let handoff_hint = if composer_state == AgentComposerState::DraftRecovery {
            "  Resolve draft recovery before messaging Codex or using Ctrl-G.".to_string()
        } else if composer_state == AgentComposerState::UnsavedPatient {
            "  Save the patient before messaging Codex or building an audited plan.".to_string()
        } else if let Some(revision) = current_revision {
            format!(
                "  Master handoff: Ctrl-G submits reviewed plan r{} with its evidence receipt.",
                revision.revision
            )
        } else if let Some(revision) = submitted_revision {
            format!(
                "  Plan r{} is submitted history; Ctrl-G retries only its exact audited handoff.",
                revision.revision
            )
        } else {
            "  Ask Codex to publish an evidence-linked plan before Ctrl-G handoff.".to_string()
        };
        lines.push(Line::from(Span::styled(
            handoff_hint,
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "  Recommendations remain review-only until a clinician approves a change.",
            Style::default().fg(Color::DarkGray),
        )));
        lines
    }

    fn active_plan_snapshot(&self) -> Option<&WorkspacePlanSnapshotGetResponse> {
        let patient_id = self.draft_client.id.as_deref()?;
        self.plan_snapshot
            .as_ref()
            .filter(|snapshot| match &snapshot.session {
                Some(session) => session.client_id == patient_id,
                None => {
                    let has_scoped_content = !snapshot.messages.is_empty()
                        || !snapshot.revisions.is_empty()
                        || !snapshot.submission_receipts.is_empty()
                        || !snapshot.proposals.is_empty();
                    has_scoped_content
                        && snapshot
                            .messages
                            .iter()
                            .all(|message| message.client_id == patient_id)
                        && snapshot
                            .revisions
                            .iter()
                            .all(|revision| revision.client_id == patient_id)
                        && snapshot
                            .submission_receipts
                            .iter()
                            .all(|receipt| receipt.client_id == patient_id)
                        && snapshot
                            .proposals
                            .iter()
                            .all(|proposal| proposal.client_id == patient_id)
                }
            })
    }

    fn active_plan_revision_with_status<'a>(
        &self,
        snapshot: Option<&'a WorkspacePlanSnapshotGetResponse>,
        status: WorkspacePlanRevisionStatus,
    ) -> Option<&'a WorkspacePlanRevision> {
        let patient_id = self.draft_client.id.as_deref()?;
        snapshot?
            .revisions
            .iter()
            .filter(|revision| {
                revision.status == status
                    && revision.client_id == patient_id
                    && revision.note_id == self.draft_note.id
                    && revision.encounter_id.as_deref() == self.active_encounter_id()
            })
            .max_by_key(|revision| revision.revision)
    }

    pub(crate) fn active_plan_revision_for_handoff(&self) -> Option<WorkspacePlanRevision> {
        let snapshot = self.active_plan_snapshot();
        self.active_plan_revision_with_status(snapshot, WorkspacePlanRevisionStatus::Current)
            .or_else(|| {
                self.active_plan_revision_with_status(
                    snapshot,
                    WorkspacePlanRevisionStatus::Submitted,
                )
            })
            .cloned()
    }

    pub(crate) fn submitted_plan_revision_for_handoff(&self) -> Option<WorkspacePlanRevision> {
        self.active_plan_revision_with_status(
            self.active_plan_snapshot(),
            WorkspacePlanRevisionStatus::Submitted,
        )
        .cloned()
    }

    pub(crate) fn submission_receipt_for_plan_revision(
        &self,
        revision_id: &str,
    ) -> Option<WorkspacePlanSubmissionReceipt> {
        let snapshot = self.active_plan_snapshot()?;
        snapshot
            .submission_receipts
            .iter()
            .find(|receipt| receipt.plan_revision_id == revision_id)
            .cloned()
    }

    fn append_plan_snapshot_conversation(
        &self,
        lines: &mut Vec<Line<'static>>,
        snapshot: &WorkspacePlanSnapshotGetResponse,
    ) {
        if snapshot.messages.is_empty() && self.plan_stream_delta.is_empty() {
            lines.extend([
                Line::from(Span::styled(
                    "Codex",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from("  This patient has a persistent planning thread."),
                Line::from("  Ask what is missing, what changed, or how today's note fits."),
            ]);
            return;
        }

        lines.push(Line::from(Span::styled(
            "Conversation",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        let first_message = snapshot.messages.len().saturating_sub(20);
        for message in &snapshot.messages[first_message..] {
            let (label, style) = match message.role {
                WorkspacePlanMessageRole::Human => {
                    ("You", Style::default().add_modifier(Modifier::BOLD))
                }
                WorkspacePlanMessageRole::Assistant => {
                    ("Codex", Style::default().add_modifier(Modifier::BOLD))
                }
                WorkspacePlanMessageRole::Question => (
                    "Codex · question",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                WorkspacePlanMessageRole::Answer => (
                    "You · answer",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                WorkspacePlanMessageRole::Error => (
                    "Codex · error",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                WorkspacePlanMessageRole::SystemStatus => {
                    ("Plan status", Style::default().fg(Color::DarkGray))
                }
            };
            lines.push(Line::from(Span::styled(label, style)));
            let mut content_lines = message.content.lines();
            for content in content_lines.by_ref().take(8) {
                lines.push(Line::from(format!("  {content}")));
            }
            if content_lines.next().is_some() {
                lines.push(Line::from(Span::styled(
                    "  … message shortened in Chat",
                    Style::default().fg(Color::DarkGray),
                )));
            }
            lines.push(Line::from(""));
        }

        if !self.plan_stream_delta.is_empty() || self.plan_streaming_status.is_some() {
            let status = self
                .plan_streaming_status
                .as_deref()
                .unwrap_or("responding");
            lines.push(Line::from(vec![
                Span::styled("Codex", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(format!(" · {status}"), Style::default().fg(Color::DarkGray)),
            ]));
            let mut delta_lines = self.plan_stream_delta.lines();
            for content in delta_lines.by_ref().take(12) {
                lines.push(Line::from(format!("  {content}")));
            }
            if delta_lines.next().is_some() {
                lines.push(Line::from(Span::styled(
                    "  …",
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
    }

    pub(super) fn agent_workpane_context_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![
            Line::from(self.workflow_single_line_chart_context()),
            self.agent_workpane_bridge_line(),
            Line::from(""),
        ];
        let packet_review_open = self.workflow_section == MedicalWorkflowSection::ContextPacket;
        if packet_review_open {
            lines.extend(self.agent_workpane_handoff_lines());
            lines.push(Line::from(""));
        }
        lines.push(workflow_header_line(
            "Selected Context",
            WorkflowTone::Agent,
        ));
        lines.extend(self.agent_workpane_selected_context_lines());
        lines.extend([
            Line::from(format!(
                "  Note: {} · r{}",
                compact_preview(&nonempty_or(&self.draft_note.title, "Untitled note"), 44),
                self.draft_note.current_revision
            )),
            Line::from("  Context is patient-scoped and remains review-only."),
            Line::from(""),
        ]);
        if let Some(snapshot) = self.active_plan_snapshot() {
            lines.extend([
                workflow_header_line("Persistent Plan", WorkflowTone::Agent),
                Line::from(format!(
                    "  {} messages · {} revisions · {} proposals",
                    snapshot.messages.len(),
                    snapshot.revisions.len(),
                    snapshot.proposals.len()
                )),
            ]);
            let revision = self
                .active_plan_revision_with_status(
                    Some(snapshot),
                    WorkspacePlanRevisionStatus::Current,
                )
                .or_else(|| {
                    self.active_plan_revision_with_status(
                        Some(snapshot),
                        WorkspacePlanRevisionStatus::Submitted,
                    )
                })
                .or_else(|| {
                    self.active_plan_revision_with_status(
                        Some(snapshot),
                        WorkspacePlanRevisionStatus::Outdated,
                    )
                });
            if let Some(revision) = revision {
                let status = match revision.status {
                    WorkspacePlanRevisionStatus::Current => "current",
                    WorkspacePlanRevisionStatus::Outdated => "outdated",
                    WorkspacePlanRevisionStatus::Submitted => "submitted",
                };
                lines.push(Line::from(format!(
                    "  Plan r{} · {status} · checkpoint r{}",
                    revision.revision, revision.source_checkpoint_revision
                )));
                lines.extend(
                    revision
                        .plan_markdown
                        .lines()
                        .take(12)
                        .map(|line| Line::from(format!("    {line}"))),
                );
            }
            lines.push(Line::from(""));
        }
        lines.extend(self.agent_workpane_history_lines());
        if packet_review_open {
            return lines;
        }
        lines.extend([
            Line::from(""),
            workflow_header_line("Master Handoff", WorkflowTone::Review),
            Line::from("  Ctrl-G submits the reviewed Plan to the parent harness."),
            Line::from("  No recommendation changes the chart without clinician approval."),
        ]);
        lines
    }

    pub(super) fn agent_workpane_plan_audit_lines(&self) -> Vec<Line<'static>> {
        let Some(snapshot) = self.active_plan_snapshot() else {
            return Vec::new();
        };
        let revision = self
            .active_plan_revision_with_status(Some(snapshot), WorkspacePlanRevisionStatus::Current)
            .or_else(|| {
                self.active_plan_revision_with_status(
                    Some(snapshot),
                    WorkspacePlanRevisionStatus::Submitted,
                )
            })
            .or_else(|| {
                self.active_plan_revision_with_status(
                    Some(snapshot),
                    WorkspacePlanRevisionStatus::Outdated,
                )
            });
        let Some(revision) = revision else {
            return Vec::new();
        };
        let status = match revision.status {
            WorkspacePlanRevisionStatus::Current => "current",
            WorkspacePlanRevisionStatus::Outdated => "outdated",
            WorkspacePlanRevisionStatus::Submitted => "submitted",
        };
        let mut lines = vec![
            workflow_header_line("Plan Provenance", WorkflowTone::Agent),
            Line::from(format!(
                "  Plan r{} · {status} · checkpoint r{}",
                revision.revision, revision.source_checkpoint_revision
            )),
            Line::from(format!(
                "  Content {} · evidence {} ({} reads)",
                compact_preview(&revision.content_sha256, 14),
                compact_preview(&revision.evidence_manifest_sha256, 14),
                revision.evidence_read_count
            )),
            Line::from(format!(
                "  Source thread {} · turn {}",
                compact_preview(&revision.source_thread_id, 16),
                compact_preview(&revision.source_turn_id, 16)
            )),
            Line::from(""),
        ];
        match serde_json::from_str::<Vec<serde_json::Value>>(&revision.evidence_manifest_json) {
            Ok(evidence) if !evidence.is_empty() => {
                lines.push(workflow_header_line("Evidence Reads", WorkflowTone::Review));
                for item in evidence {
                    let ordinal = item
                        .get("ordinal")
                        .and_then(serde_json::Value::as_u64)
                        .map(|value| value + 1)
                        .unwrap_or(0);
                    let category = item
                        .get("category")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("unknown");
                    let read_id = item
                        .get("contextReadId")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("missing");
                    let response_hash = item
                        .get("responseSha256")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("missing");
                    lines.push(Line::from(format!(
                        "  [{ordinal}] {category} · read {read_id}"
                    )));
                    lines.push(Line::from(format!("      response {response_hash}")));
                    if let Some(source_hashes) = item
                        .get("sourceContentSha256")
                        .and_then(serde_json::Value::as_array)
                    {
                        for (source_index, source_hash) in source_hashes.iter().enumerate() {
                            let source_hash = source_hash.as_str().unwrap_or("missing");
                            lines.push(Line::from(format!(
                                "      source {} {source_hash}",
                                source_index + 1
                            )));
                        }
                    }
                }
                lines.push(Line::from(""));
            }
            Ok(_) => {}
            Err(_) => lines.push(Line::from(Span::styled(
                "  Evidence manifest could not be decoded; integrity review required.",
                Style::default().fg(Color::Red),
            ))),
        }
        lines
    }
}
