use super::*;

const NO_SUPPORTING_CONTEXT_CODE: &str = "no_supporting_context";
const UNSAVED_CHART_DRAFT_CODE: &str = "unsaved_chart_draft";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ContextPlanReadinessItem {
    pub(super) code: &'static str,
    pub(super) message: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ContextPlanReadiness {
    pub(super) blockers: Vec<ContextPlanReadinessItem>,
    pub(super) warnings: Vec<MedicalContextPlanWarningV2>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ContextPlanReadinessInput {
    pub(super) patient_saved: bool,
    pub(super) request_present: bool,
    pub(super) recovery_pending: bool,
    pub(super) chart_changeset_pending: bool,
    pub(super) stale_context_selection: bool,
    pub(super) selected_context_count: usize,
    pub(super) unsaved_chart_draft: bool,
}

impl ContextPlanReadiness {
    pub(super) fn evaluate(input: ContextPlanReadinessInput) -> Self {
        let mut blockers = Vec::new();
        if !input.patient_saved {
            blockers.push(ContextPlanReadinessItem {
                code: "patient_unsaved",
                message: "Save the patient before submitting a Context Plan.",
            });
        }
        if !input.request_present {
            blockers.push(ContextPlanReadinessItem {
                code: "instructions_empty",
                message: "Add instructions describing what Codex should analyze.",
            });
        }
        if input.recovery_pending {
            blockers.push(ContextPlanReadinessItem {
                code: "draft_recovery_pending",
                message: "Restore or discard the offered local recovery draft first.",
            });
        }
        if input.chart_changeset_pending {
            blockers.push(ContextPlanReadinessItem {
                code: "chart_changeset_pending",
                message: "Save, reconcile, or discard the pending chart changeset first.",
            });
        }
        if input.stale_context_selection {
            blockers.push(ContextPlanReadinessItem {
                code: "stale_context_selection",
                message: "Review the pruned context selection before submitting.",
            });
        }

        let mut warnings = Vec::new();
        if input.selected_context_count == 0 {
            warnings.push(MedicalContextPlanWarningV2 {
                code: NO_SUPPORTING_CONTEXT_CODE.to_string(),
                message: "No patient files, reviewed text, or clips are selected; Codex will receive chart context only."
                    .to_string(),
            });
        }
        if input.unsaved_chart_draft {
            warnings.push(MedicalContextPlanWarningV2 {
                code: UNSAVED_CHART_DRAFT_CODE.to_string(),
                message: "The Context Plan includes a local chart draft that is not yet canonical."
                    .to_string(),
            });
        }

        Self { blockers, warnings }
    }

    pub(super) fn is_blocked(&self) -> bool {
        !self.blockers.is_empty()
    }

    pub(super) fn requires_acknowledgement(&self) -> bool {
        !self.warnings.is_empty()
    }

    pub(super) fn submission_readiness_json(&self, checkpoint_sha256: &str) -> serde_json::Value {
        let acknowledgements = self
            .warnings
            .iter()
            .map(|warning| {
                serde_json::json!({
                    "warningCode": warning.code,
                    "checkpointSha256": checkpoint_sha256,
                    "reason": "Clinician reviewed this warning and explicitly submitted the Context Plan with Ctrl-G.",
                })
            })
            .collect::<Vec<_>>();
        serde_json::json!({
            "version": 1,
            "warnings": self.warnings,
            "acknowledgements": acknowledgements,
            "legacy": false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_separates_hard_gates_from_acknowledgeable_warnings() {
        let readiness = ContextPlanReadiness::evaluate(ContextPlanReadinessInput {
            patient_saved: false,
            request_present: false,
            recovery_pending: true,
            chart_changeset_pending: true,
            stale_context_selection: true,
            selected_context_count: 0,
            unsaved_chart_draft: true,
        });

        assert_eq!(readiness.blockers.len(), 5);
        assert_eq!(readiness.warnings.len(), 2);
        assert!(readiness.is_blocked());
        assert!(readiness.requires_acknowledgement());
    }

    #[test]
    fn submission_acknowledgements_bind_every_warning_to_the_checkpoint() {
        let checkpoint_sha256 = "a".repeat(64);
        let readiness = ContextPlanReadiness::evaluate(ContextPlanReadinessInput {
            patient_saved: true,
            request_present: true,
            recovery_pending: false,
            chart_changeset_pending: false,
            stale_context_selection: false,
            selected_context_count: 0,
            unsaved_chart_draft: false,
        });

        let json = readiness.submission_readiness_json(&checkpoint_sha256);
        assert_eq!(json["warnings"].as_array().map(Vec::len), Some(1));
        assert_eq!(
            json["acknowledgements"][0]["checkpointSha256"],
            checkpoint_sha256
        );
        assert_eq!(json["legacy"], false);
    }
}
