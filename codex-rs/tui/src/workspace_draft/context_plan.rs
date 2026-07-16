use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeSet;

use super::model::WorkspaceDraftError;

pub(crate) const MEDICAL_CONTEXT_PLAN_SCHEMA_VERSION: i64 = 1;
const DEFAULT_CONTEXT_READ_LIMIT: u32 = 25;
const MAX_CONTEXT_READ_LIMIT: u32 = 100;
const MAX_OBJECTIVE_BYTES: usize = 32 * 1024;
const MAX_WARNING_COUNT: usize = 100;
const MAX_WARNING_MESSAGE_BYTES: usize = 4 * 1024;
const MAX_ACKNOWLEDGEMENT_REASON_BYTES: usize = 4 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MedicalContextPlanWorkflowV2 {
    ClinicalDocumentation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MedicalContextPlanNoteKindV2 {
    Evaluation,
    Daily,
    Progress,
    Recertification,
    Discharge,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MedicalContextPlanAuthorizedCategoryV2 {
    VisitHistory,
    ProgressNotes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MedicalContextPlanAuthorizedScopeV2 {
    pub(crate) categories: Vec<MedicalContextPlanAuthorizedCategoryV2>,
    pub(crate) max_records: u32,
}

impl Default for MedicalContextPlanAuthorizedScopeV2 {
    fn default() -> Self {
        Self {
            categories: vec![
                MedicalContextPlanAuthorizedCategoryV2::VisitHistory,
                MedicalContextPlanAuthorizedCategoryV2::ProgressNotes,
            ],
            max_records: DEFAULT_CONTEXT_READ_LIMIT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MedicalContextPlanSelectedContextV2 {
    pub(crate) file_ids: Vec<String>,
    pub(crate) reviewed_text_ids: Vec<String>,
    pub(crate) clip_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MedicalContextPlanWarningV2 {
    pub(crate) code: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MedicalContextPlanAcknowledgementV2 {
    pub(crate) warning_code: String,
    pub(crate) checkpoint_sha256: String,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MedicalContextPlanV2 {
    schema_version: i64,
    pub(crate) objective: String,
    pub(crate) expected_output_kind: String,
    pub(crate) workflow: MedicalContextPlanWorkflowV2,
    pub(crate) note_kind: MedicalContextPlanNoteKindV2,
    pub(crate) authorized_scope: MedicalContextPlanAuthorizedScopeV2,
    pub(crate) selected_context: MedicalContextPlanSelectedContextV2,
    pub(crate) readiness_warnings: Vec<MedicalContextPlanWarningV2>,
    pub(crate) acknowledgements: Vec<MedicalContextPlanAcknowledgementV2>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MedicalContextPlanInput {
    pub(crate) objective: String,
    pub(crate) expected_output_kind: String,
    pub(crate) workflow: MedicalContextPlanWorkflowV2,
    pub(crate) note_kind: MedicalContextPlanNoteKindV2,
    pub(crate) authorized_scope: MedicalContextPlanAuthorizedScopeV2,
    pub(crate) readiness_warnings: Vec<MedicalContextPlanWarningV2>,
    pub(crate) acknowledgements: Vec<MedicalContextPlanAcknowledgementV2>,
}

impl MedicalContextPlanInput {
    pub(crate) fn for_note_proposal(objective: impl Into<String>) -> Self {
        Self {
            objective: objective.into(),
            expected_output_kind: "note_proposal".to_string(),
            workflow: MedicalContextPlanWorkflowV2::ClinicalDocumentation,
            note_kind: MedicalContextPlanNoteKindV2::Other,
            authorized_scope: MedicalContextPlanAuthorizedScopeV2::default(),
            readiness_warnings: Vec::new(),
            acknowledgements: Vec::new(),
        }
    }
}

impl MedicalContextPlanV2 {
    pub(super) fn from_input(
        input: MedicalContextPlanInput,
        file_ids: Vec<String>,
        reviewed_text_ids: Vec<String>,
        clip_ids: Vec<String>,
    ) -> Result<Self, WorkspaceDraftError> {
        let plan = Self {
            schema_version: MEDICAL_CONTEXT_PLAN_SCHEMA_VERSION,
            objective: input.objective,
            expected_output_kind: input.expected_output_kind.trim().to_string(),
            workflow: input.workflow,
            note_kind: input.note_kind,
            authorized_scope: input.authorized_scope,
            selected_context: MedicalContextPlanSelectedContextV2 {
                file_ids,
                reviewed_text_ids,
                clip_ids,
            },
            readiness_warnings: input.readiness_warnings,
            acknowledgements: input.acknowledgements,
        };
        plan.validate()?;
        Ok(plan)
    }

    pub(super) fn validate(&self) -> Result<(), WorkspaceDraftError> {
        if self.schema_version != MEDICAL_CONTEXT_PLAN_SCHEMA_VERSION {
            return invalid_plan(format!(
                "unsupported context plan schemaVersion {}",
                self.schema_version
            ));
        }
        if self.objective.len() > MAX_OBJECTIVE_BYTES {
            return invalid_plan(format!(
                "objective exceeds the {MAX_OBJECTIVE_BYTES} byte limit"
            ));
        }
        if self.expected_output_kind.is_empty() || self.expected_output_kind.len() > 128 {
            return invalid_plan("expected output kind must contain 1 to 128 bytes");
        }
        if self.authorized_scope.categories.is_empty() {
            return invalid_plan("authorized categories must not be empty");
        }
        let categories = self
            .authorized_scope
            .categories
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if categories.len() != self.authorized_scope.categories.len() {
            return invalid_plan("authorized categories must be unique");
        }
        if !(1..=MAX_CONTEXT_READ_LIMIT).contains(&self.authorized_scope.max_records) {
            return invalid_plan(format!(
                "authorized maxRecords must be between 1 and {MAX_CONTEXT_READ_LIMIT}"
            ));
        }
        if self.readiness_warnings.len() > MAX_WARNING_COUNT {
            return invalid_plan(format!(
                "readiness warnings exceed the {MAX_WARNING_COUNT} item limit"
            ));
        }
        let mut warning_codes = BTreeSet::new();
        for warning in &self.readiness_warnings {
            validate_code("readiness warning", &warning.code)?;
            if !warning_codes.insert(warning.code.as_str()) {
                return invalid_plan("readiness warning codes must be unique");
            }
            if warning.message.trim().is_empty()
                || warning.message.len() > MAX_WARNING_MESSAGE_BYTES
            {
                return invalid_plan(format!(
                    "readiness warning messages must contain 1 to {MAX_WARNING_MESSAGE_BYTES} bytes"
                ));
            }
        }
        let mut acknowledged_codes = BTreeSet::new();
        for acknowledgement in &self.acknowledgements {
            validate_code("acknowledgement warning", &acknowledgement.warning_code)?;
            if !warning_codes.contains(acknowledgement.warning_code.as_str()) {
                return invalid_plan(format!(
                    "acknowledgement references unknown warning `{}`",
                    acknowledgement.warning_code
                ));
            }
            if !acknowledged_codes.insert(acknowledgement.warning_code.as_str()) {
                return invalid_plan("each readiness warning may be acknowledged once");
            }
            if !is_lower_hex_sha256(&acknowledgement.checkpoint_sha256) {
                return invalid_plan(
                    "acknowledgement checkpoint SHA-256 must be 64 lowercase hexadecimal characters",
                );
            }
            if acknowledgement.reason.trim().is_empty()
                || acknowledgement.reason.len() > MAX_ACKNOWLEDGEMENT_REASON_BYTES
            {
                return invalid_plan(format!(
                    "acknowledgement reasons must contain 1 to {MAX_ACKNOWLEDGEMENT_REASON_BYTES} bytes"
                ));
            }
        }
        Ok(())
    }
}

fn validate_code(label: &str, code: &str) -> Result<(), WorkspaceDraftError> {
    if code.is_empty()
        || code.len() > 64
        || !code.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
    {
        return invalid_plan(format!(
            "{label} code must contain 1 to 64 lowercase letters, digits, underscores, or hyphens"
        ));
    }
    Ok(())
}

fn is_lower_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid_plan<T>(message: impl Into<String>) -> Result<T, WorkspaceDraftError> {
    Err(WorkspaceDraftError::InvalidDraft(format!(
        "context plan: {}",
        message.into()
    )))
}
