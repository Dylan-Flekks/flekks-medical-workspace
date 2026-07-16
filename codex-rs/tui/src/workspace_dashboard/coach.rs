#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct WorkspaceCoachState {
    pub patient_saved: bool,
    pub note_saved: bool,
    pub note_has_title: bool,
    pub note_has_body: bool,
    pub chart_has_unsaved_changes: bool,
    pub selected_file_count: usize,
    pub selected_text_count: usize,
    pub selected_clip_count: usize,
    pub request_has_text: bool,
    pub packet_review_open: bool,
    pub packet_submitted: bool,
    pub packet_waiting_for_result: bool,
    pub returned_work_available: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WorkspaceCoachStage {
    Patient,
    Note,
    SaveChart,
    Evidence,
    Request,
    ReviewPacket,
    SendPacket,
    AwaitReturnedWork,
    ReviewReturnedWork,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorkspaceCoachAdvice {
    pub stage: WorkspaceCoachStage,
    pub title: &'static str,
    pub detail: &'static str,
    pub next: &'static str,
}

impl WorkspaceCoachAdvice {
    fn new(
        stage: WorkspaceCoachStage,
        title: &'static str,
        detail: &'static str,
        next: &'static str,
    ) -> Self {
        Self {
            stage,
            title,
            detail,
            next,
        }
    }
}

pub(super) fn workspace_coach_advice(state: WorkspaceCoachState) -> WorkspaceCoachAdvice {
    if !state.patient_saved {
        return WorkspaceCoachAdvice::new(
            WorkspaceCoachStage::Patient,
            "Start with the patient",
            "Open an existing patient or save the new patient before building agent context.",
            "Save patient",
        );
    }

    if !state.note_has_title {
        return WorkspaceCoachAdvice::new(
            WorkspaceCoachStage::Note,
            "Name the clinical work item",
            "A specific note title helps bind the draft, packet, returned work, and audit history.",
            "Name the note",
        );
    }

    if state.chart_has_unsaved_changes || !state.note_saved {
        return WorkspaceCoachAdvice::new(
            WorkspaceCoachStage::SaveChart,
            "Save the canonical chart",
            "Draft recovery protects typing, but only Ctrl-S commits the human chart revision.",
            "Ctrl-S saves chart",
        );
    }

    let selected_context_count =
        state.selected_file_count + state.selected_text_count + state.selected_clip_count;
    if selected_context_count == 0 {
        return WorkspaceCoachAdvice::new(
            WorkspaceCoachStage::Evidence,
            "Choose supporting context",
            "Select only the files, reviewed text, and clips the agent needs for this request.",
            "Mark agent context",
        );
    }

    if !state.request_has_text {
        return WorkspaceCoachAdvice::new(
            WorkspaceCoachStage::Request,
            "Give the agent a focused job",
            if state.note_has_body {
                "State the task, expected output, and constraints; the saved note and selected context stay reviewable."
            } else {
                "The note is blank: ask for a template or outline, expected output, and clinical constraints."
            },
            "Type agent instructions",
        );
    }

    if !state.packet_submitted && !state.packet_review_open {
        return WorkspaceCoachAdvice::new(
            WorkspaceCoachStage::ReviewPacket,
            "Review the context packet",
            "Confirm the patient, note revision, instructions, and selected evidence before handoff.",
            "Preview packet",
        );
    }

    if !state.packet_submitted {
        return WorkspaceCoachAdvice::new(
            WorkspaceCoachStage::SendPacket,
            "Packet is ready for handoff",
            "Ctrl-G sends the reviewed packet to the master Codex harness without changing the chart.",
            "Ctrl-G opens agent",
        );
    }

    if state.packet_waiting_for_result {
        return WorkspaceCoachAdvice::new(
            WorkspaceCoachStage::AwaitReturnedWork,
            "Agent plan submitted",
            "The immutable packet is in history; return here after the Codex harness produces work.",
            "Paste returned work",
        );
    }

    if state.returned_work_available {
        return WorkspaceCoachAdvice::new(
            WorkspaceCoachStage::ReviewReturnedWork,
            "Compare returned work",
            "Agent output is a proposal. Review it against the chart before accepting any clinical change.",
            "Inspect returned work",
        );
    }

    WorkspaceCoachAdvice::new(
        WorkspaceCoachStage::ReviewReturnedWork,
        "Review the agent history",
        "The immutable packet is available for audit even when no returned work has been saved.",
        "Open agent history",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet_ready_state() -> WorkspaceCoachState {
        WorkspaceCoachState {
            patient_saved: true,
            note_saved: true,
            note_has_title: true,
            note_has_body: true,
            selected_file_count: 1,
            request_has_text: true,
            ..WorkspaceCoachState::default()
        }
    }

    #[test]
    fn coach_teaches_the_packet_sequence_without_skipping_canonical_save() {
        let mut state = WorkspaceCoachState::default();
        assert_eq!(
            workspace_coach_advice(state).stage,
            WorkspaceCoachStage::Patient
        );

        state.patient_saved = true;
        assert_eq!(
            workspace_coach_advice(state).stage,
            WorkspaceCoachStage::Note
        );

        state.note_has_title = true;
        state.chart_has_unsaved_changes = true;
        assert_eq!(
            workspace_coach_advice(state).stage,
            WorkspaceCoachStage::SaveChart
        );

        state.chart_has_unsaved_changes = false;
        state.note_saved = true;
        assert_eq!(
            workspace_coach_advice(state).stage,
            WorkspaceCoachStage::Evidence
        );

        state.selected_text_count = 1;
        assert_eq!(
            workspace_coach_advice(state).stage,
            WorkspaceCoachStage::Request
        );

        state.request_has_text = true;
        assert_eq!(
            workspace_coach_advice(state).stage,
            WorkspaceCoachStage::ReviewPacket
        );

        state.packet_review_open = true;
        assert_eq!(
            workspace_coach_advice(state).stage,
            WorkspaceCoachStage::SendPacket
        );
    }

    #[test]
    fn coach_waits_for_and_then_surfaces_returned_work() {
        let mut state = packet_ready_state();
        state.packet_submitted = true;
        state.packet_waiting_for_result = true;
        assert_eq!(
            workspace_coach_advice(state).stage,
            WorkspaceCoachStage::AwaitReturnedWork
        );

        state.packet_waiting_for_result = false;
        state.returned_work_available = true;
        assert_eq!(
            workspace_coach_advice(state).stage,
            WorkspaceCoachStage::ReviewReturnedWork
        );
    }

    #[test]
    fn coach_counts_each_supported_context_kind_as_evidence() {
        for state in [
            WorkspaceCoachState {
                selected_file_count: 1,
                ..packet_ready_state()
            },
            WorkspaceCoachState {
                selected_file_count: 0,
                selected_text_count: 1,
                ..packet_ready_state()
            },
            WorkspaceCoachState {
                selected_file_count: 0,
                selected_clip_count: 1,
                ..packet_ready_state()
            },
        ] {
            assert_eq!(
                workspace_coach_advice(state).stage,
                WorkspaceCoachStage::ReviewPacket
            );
        }
    }
}
