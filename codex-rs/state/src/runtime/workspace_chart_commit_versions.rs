use super::workspace_chart_commit::ExistingRecords;
use crate::WorkspaceChartCommitError;
use crate::WorkspaceChartCommitRequest;
use crate::WorkspaceChartEntityKind;

pub(super) fn validate(
    request: &WorkspaceChartCommitRequest,
    existing: &ExistingRecords,
    client_id: &str,
) -> Result<(), WorkspaceChartCommitError> {
    validate_one(
        WorkspaceChartEntityKind::Client,
        client_id,
        request.client.is_some(),
        existing.client.as_ref(),
        request.expected_versions.client.as_deref(),
        crate::WorkspaceClient::record_version,
    )?;
    validate_one(
        WorkspaceChartEntityKind::Coverage,
        request
            .coverage
            .as_ref()
            .and_then(|input| input.id.as_deref())
            .unwrap_or(""),
        request.coverage.is_some(),
        existing.coverage.as_ref(),
        request.expected_versions.coverage.as_deref(),
        crate::WorkspaceCoverage::record_version,
    )?;
    validate_one(
        WorkspaceChartEntityKind::SafetyItem,
        request
            .safety_item
            .as_ref()
            .and_then(|input| input.id.as_deref())
            .unwrap_or(""),
        request.safety_item.is_some(),
        existing.safety_item.as_ref(),
        request.expected_versions.safety_item.as_deref(),
        crate::WorkspacePatientSafetyItem::record_version,
    )?;
    validate_one(
        WorkspaceChartEntityKind::Encounter,
        request
            .encounter
            .as_ref()
            .and_then(|input| input.id.as_deref())
            .unwrap_or(""),
        request.encounter.is_some(),
        existing.encounter.as_ref(),
        request.expected_versions.encounter.as_deref(),
        crate::WorkspaceEncounter::record_version,
    )?;
    validate_one(
        WorkspaceChartEntityKind::Document,
        request
            .document
            .as_ref()
            .and_then(|input| input.id.as_deref())
            .unwrap_or(""),
        request.document.is_some(),
        existing.document.as_ref(),
        request.expected_versions.document.as_deref(),
        crate::WorkspaceDocument::record_version,
    )?;
    validate_one(
        WorkspaceChartEntityKind::ArtifactDerivative,
        request
            .artifact_derivative
            .as_ref()
            .and_then(|input| input.id.as_deref())
            .unwrap_or(""),
        request.artifact_derivative.is_some(),
        existing.derivative.as_ref(),
        request.expected_versions.artifact_derivative.as_deref(),
        crate::WorkspaceArtifactDerivative::record_version,
    )?;
    validate_one(
        WorkspaceChartEntityKind::ContextClip,
        request
            .context_clip
            .as_ref()
            .and_then(|input| input.id.as_deref())
            .unwrap_or(""),
        request.context_clip.is_some(),
        existing.clip.as_ref(),
        request.expected_versions.context_clip.as_deref(),
        crate::WorkspaceContextClip::record_version,
    )?;
    validate_one(
        WorkspaceChartEntityKind::Task,
        request
            .task
            .as_ref()
            .and_then(|input| input.id.as_deref())
            .unwrap_or(""),
        request.task.is_some(),
        existing.task.as_ref(),
        request.expected_versions.task.as_deref(),
        crate::WorkspaceTask::record_version,
    )?;
    Ok(())
}

fn validate_one<T>(
    kind: WorkspaceChartEntityKind,
    entity_id: &str,
    included: bool,
    existing: Option<&T>,
    expected: Option<&str>,
    record_version: fn(&T) -> anyhow::Result<String>,
) -> Result<(), WorkspaceChartCommitError> {
    if !included {
        if expected.is_some() {
            return validation(format!(
                "workspace {} expected version requires an included record",
                kind.as_str()
            ));
        }
        return Ok(());
    }
    match existing {
        Some(existing) => {
            let expected = expected.ok_or_else(|| WorkspaceChartCommitError::Validation {
                message: format!(
                    "workspace {} `{entity_id}` expected version is required",
                    kind.as_str()
                ),
            })?;
            let actual = record_version(existing)?;
            if expected != actual {
                return Err(WorkspaceChartCommitError::StaleEntityVersion {
                    entity_kind: kind,
                    entity_id: entity_id.to_string(),
                    expected: expected.to_string(),
                    actual,
                });
            }
        }
        None => {
            if expected.is_some() {
                return validation(format!(
                    "new workspace {} `{entity_id}` must not include an expected version",
                    kind.as_str()
                ));
            }
        }
    }
    Ok(())
}

fn validation<T>(message: impl Into<String>) -> Result<T, WorkspaceChartCommitError> {
    Err(WorkspaceChartCommitError::Validation {
        message: message.into(),
    })
}
