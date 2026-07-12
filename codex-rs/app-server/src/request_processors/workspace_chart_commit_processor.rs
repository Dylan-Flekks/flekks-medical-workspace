use super::*;
use codex_app_server_protocol::WorkspaceChartCommitParams;
use codex_app_server_protocol::WorkspaceChartCommitResponse;
use codex_app_server_protocol::WorkspaceChartEntityKind;
use codex_app_server_protocol::WorkspaceChartNoteChange;
use serde_json::json;

impl WorkspaceRequestProcessor {
    pub(crate) async fn chart_commit(
        &self,
        params: WorkspaceChartCommitParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let state_db = self.state_db()?;
        let idempotency_key = required_commit_text("idempotencyKey", params.idempotency_key)?;
        let actor = required_commit_text("actor", params.actor)?;
        let reason = required_commit_text("reason", params.reason)?;
        let source_thread_id = empty_to_none(params.source_thread_id);
        let source_turn_id = empty_to_none(params.source_turn_id);
        let client_id = empty_to_none(params.client_id);
        let existing_client_id = client_id.as_deref().or_else(|| {
            params
                .client
                .as_ref()
                .and_then(|client| client.id.as_deref())
        });
        let existing_client = match (params.client.as_ref(), existing_client_id) {
            (Some(_), Some(client_id)) => state_db
                .workspace()
                .get_client(client_id)
                .await
                .map_err(|err| {
                    internal_error(format!("failed to read workspace client: {err}"))
                })?,
            (Some(_), None) | (None, _) => None,
        };

        let request = codex_state::WorkspaceChartCommitRequest {
            idempotency_key,
            actor: actor.clone(),
            reason,
            source_thread_id: source_thread_id.clone(),
            source_turn_id: source_turn_id.clone(),
            client_id,
            client: params
                .client
                .map(|value| {
                    super::client_patch::state_client_upsert(value, existing_client.as_ref())
                })
                .transpose()?,
            coverage: params.coverage.map(super::coverage::state_coverage_upsert),
            expected_versions: state_expected_versions(params.expected_versions),
            safety_item: params.safety_item.map(state_safety_item_upsert),
            encounter: params.encounter.map(state_encounter_upsert).transpose()?,
            note: params.note.map(|change| {
                state_note_change(change, &actor, &source_thread_id, &source_turn_id)
            }),
            document: params.document.map(state_document_upsert).transpose()?,
            artifact_derivative: params
                .artifact_derivative
                .map(|upsert| state_derivative_upsert(upsert, &actor)),
            context_clip: params
                .context_clip
                .map(|upsert| state_context_clip_upsert(upsert, &actor)),
            task: params.task.map(|upsert| state_task_upsert(upsert, &actor)),
        };

        let result = state_db
            .workspace()
            .commit_chart(request)
            .await
            .map_err(chart_commit_error)?;
        let committed_coverage_readiness = result.coverage_billing_readiness;
        let coverage = match result.coverage {
            Some(coverage) => {
                let readiness = match committed_coverage_readiness {
                    Some(readiness) => readiness,
                    None => state_db
                        .workspace()
                        .coverage_billing_readiness(&coverage)
                        .await
                        .map_err(|err| {
                            internal_error(format!(
                                "failed to derive workspace billing readiness: {err}"
                            ))
                        })?,
                };
                Some(super::coverage::api_coverage_from_state(
                    coverage, readiness,
                )?)
            }
            None => None,
        };

        Ok(Some(
            WorkspaceChartCommitResponse {
                commit_id: result.commit_id,
                idempotency_key: result.idempotency_key,
                replayed: result.replayed,
                changed_entity_kinds: result
                    .changed_entity_kinds
                    .into_iter()
                    .map(api_entity_kind)
                    .collect(),
                client: api_workspace_client_from_state(result.client)?,
                coverage,
                safety_item: result
                    .safety_item
                    .map(api_workspace_patient_safety_item_from_state)
                    .transpose()?,
                encounter: result
                    .encounter
                    .map(api_workspace_encounter_from_state)
                    .transpose()?,
                note: result.note.map(api_workspace_note_from_state),
                document: result
                    .document
                    .map(api_workspace_document_from_state)
                    .transpose()?,
                artifact_derivative: result
                    .artifact_derivative
                    .map(api_workspace_artifact_derivative_from_state)
                    .transpose()?,
                context_clip: result
                    .context_clip
                    .map(api_workspace_context_clip_from_state)
                    .transpose()?,
                task: result.task.map(api_workspace_task_from_state).transpose()?,
                resulting_note_revision: result.resulting_note_revision,
                committed_at: result.committed_at.timestamp(),
            }
            .into(),
        ))
    }
}

fn state_expected_versions(
    value: Option<codex_app_server_protocol::WorkspaceChartExpectedVersions>,
) -> codex_state::WorkspaceChartExpectedVersions {
    let value = value.unwrap_or_default();
    codex_state::WorkspaceChartExpectedVersions {
        client: empty_to_none(value.client),
        coverage: empty_to_none(value.coverage),
        safety_item: empty_to_none(value.safety_item),
        encounter: empty_to_none(value.encounter),
        document: empty_to_none(value.document),
        artifact_derivative: empty_to_none(value.artifact_derivative),
        context_clip: empty_to_none(value.context_clip),
        task: empty_to_none(value.task),
    }
}

fn state_safety_item_upsert(
    value: WorkspacePatientSafetyItemUpsertParams,
) -> codex_state::WorkspacePatientSafetyItemUpsert {
    codex_state::WorkspacePatientSafetyItemUpsert {
        id: value.id,
        client_id: value.client_id,
        category: value.category,
        name: value.name,
        reaction: value.reaction,
        severity: value.severity,
        dose: value.dose,
        route: value.route,
        frequency: value.frequency,
        status: value.status,
        recorded_date: value.recorded_date,
        notes: value.notes,
    }
}

fn state_encounter_upsert(
    value: WorkspaceEncounterUpsertParams,
) -> Result<codex_state::WorkspaceEncounterUpsert, JSONRPCErrorError> {
    Ok(codex_state::WorkspaceEncounterUpsert {
        id: value.id,
        client_id: value.client_id,
        kind: value.kind,
        title: value.title,
        status: value.status,
        started_at: value.started_at.map(chart_commit_timestamp).transpose()?,
        ended_at: value.ended_at.map(chart_commit_timestamp).transpose()?,
    })
}

fn state_note_change(
    value: WorkspaceChartNoteChange,
    actor: &str,
    source_thread_id: &Option<String>,
    source_turn_id: &Option<String>,
) -> codex_state::WorkspaceChartNoteChange {
    let upsert = value.upsert;
    codex_state::WorkspaceChartNoteChange {
        upsert: codex_state::WorkspaceNoteUpsert {
            id: upsert.id,
            client_id: upsert.client_id,
            encounter_id: upsert.encounter_id,
            title: upsert.title,
            kind: upsert.kind,
            body: upsert.body,
            status: upsert.status,
            actor: actor.to_string(),
            source_thread_id: source_thread_id.clone(),
            source_turn_id: source_turn_id.clone(),
            summary: upsert.summary,
        },
        expected_base_revision: value.expected_base_revision,
    }
}

fn state_document_upsert(
    value: WorkspaceDocumentUpsertParams,
) -> Result<codex_state::WorkspaceDocumentUpsert, JSONRPCErrorError> {
    Ok(codex_state::WorkspaceDocumentUpsert {
        id: value.id,
        client_id: value.client_id,
        encounter_id: value.encounter_id,
        title: value.title,
        kind: value.kind,
        local_path: value.local_path,
        notes: value.notes,
        scope: value.scope,
        detected_kind: value.detected_kind,
        mime_type: value.mime_type,
        file_size_bytes: value.file_size_bytes,
        modified_at: value.modified_at.map(chart_commit_timestamp).transpose()?,
        sha256: value.sha256,
        tags: value.tags,
        source_label: value.source_label,
        existence_status: value.existence_status,
        metadata_json: value.metadata_json,
        original_path: value.original_path,
        reference_kind: value.reference_kind,
        vault_path: value.vault_path,
        content_sha256: value.content_sha256,
        thumbnail_path: value.thumbnail_path,
        thumbnail_status: value.thumbnail_status,
        thumbnail_mime_type: value.thumbnail_mime_type,
        intake_source: value.intake_source,
        imported_at: value.imported_at.map(chart_commit_timestamp).transpose()?,
    })
}

fn state_derivative_upsert(
    value: WorkspaceArtifactDerivativeUpsertParams,
    actor: &str,
) -> codex_state::WorkspaceArtifactDerivativeUpsert {
    codex_state::WorkspaceArtifactDerivativeUpsert {
        id: value.id,
        document_id: value.document_id,
        client_id: value.client_id,
        encounter_id: value.encounter_id,
        note_id: value.note_id,
        kind: value.kind,
        title: value.title,
        body: value.body,
        review_status: value.review_status,
        source_method: value.source_method,
        page_range: value.page_range,
        timestamp_range: value.timestamp_range,
        segment_label: value.segment_label,
        tags: value.tags,
        metadata_json: value.metadata_json,
        actor: actor.to_string(),
    }
}

fn state_context_clip_upsert(
    value: WorkspaceContextClipUpsertParams,
    actor: &str,
) -> codex_state::WorkspaceContextClipUpsert {
    codex_state::WorkspaceContextClipUpsert {
        id: value.id,
        derivative_id: value.derivative_id,
        document_id: value.document_id,
        client_id: value.client_id,
        encounter_id: value.encounter_id,
        note_id: value.note_id,
        kind: value.kind,
        title: value.title,
        body: value.body,
        review_status: value.review_status,
        source_method: value.source_method,
        page_range: value.page_range,
        timestamp_range: value.timestamp_range,
        line_range: value.line_range,
        segment_label: value.segment_label,
        tags: value.tags,
        metadata_json: value.metadata_json,
        actor: actor.to_string(),
    }
}

fn state_task_upsert(
    value: WorkspaceTaskUpsertParams,
    actor: &str,
) -> codex_state::WorkspaceTaskUpsert {
    codex_state::WorkspaceTaskUpsert {
        id: value.id,
        client_id: value.client_id,
        encounter_id: value.encounter_id,
        note_id: value.note_id,
        document_id: value.document_id,
        title: value.title,
        details: value.details,
        kind: value.kind,
        status: state_workspace_task_status_from_api(value.status),
        priority: state_workspace_task_priority_from_api(value.priority),
        due_date: value.due_date,
        assigned_to: value.assigned_to,
        actor: actor.to_string(),
    }
}

fn api_entity_kind(value: codex_state::WorkspaceChartEntityKind) -> WorkspaceChartEntityKind {
    match value {
        codex_state::WorkspaceChartEntityKind::Client => WorkspaceChartEntityKind::Client,
        codex_state::WorkspaceChartEntityKind::Coverage => WorkspaceChartEntityKind::Coverage,
        codex_state::WorkspaceChartEntityKind::SafetyItem => WorkspaceChartEntityKind::SafetyItem,
        codex_state::WorkspaceChartEntityKind::Encounter => WorkspaceChartEntityKind::Encounter,
        codex_state::WorkspaceChartEntityKind::Note => WorkspaceChartEntityKind::Note,
        codex_state::WorkspaceChartEntityKind::Document => WorkspaceChartEntityKind::Document,
        codex_state::WorkspaceChartEntityKind::ArtifactDerivative => {
            WorkspaceChartEntityKind::ArtifactDerivative
        }
        codex_state::WorkspaceChartEntityKind::ContextClip => WorkspaceChartEntityKind::ContextClip,
        codex_state::WorkspaceChartEntityKind::Task => WorkspaceChartEntityKind::Task,
    }
}

fn required_commit_text(field_name: &str, value: String) -> Result<String, JSONRPCErrorError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(chart_commit_validation_error(format!(
            "workspace chart commit {field_name} must not be empty"
        )));
    }
    Ok(value.to_string())
}

fn chart_commit_timestamp(value: i64) -> Result<DateTime<Utc>, JSONRPCErrorError> {
    DateTime::<Utc>::from_timestamp(value, 0).ok_or_else(|| {
        chart_commit_validation_error(format!("invalid workspace timestamp `{value}`"))
    })
}

fn chart_commit_error(error: codex_state::WorkspaceChartCommitError) -> JSONRPCErrorError {
    let message = error.to_string();
    let data = match error {
        codex_state::WorkspaceChartCommitError::IdempotencyConflict { idempotency_key } => {
            json!({
                "kind": "idempotencyConflict",
                "idempotencyKey": idempotency_key,
            })
        }
        codex_state::WorkspaceChartCommitError::Validation { .. } => chart_validation_data(),
        codex_state::WorkspaceChartCommitError::StaleNoteRevision {
            note_id,
            expected,
            actual,
        } => json!({
            "kind": "staleNoteRevision",
            "noteId": note_id,
            "expectedRevision": expected,
            "actualRevision": actual,
        }),
        codex_state::WorkspaceChartCommitError::StaleEntityVersion {
            entity_kind,
            entity_id,
            expected,
            actual,
        } => json!({
            "kind": "staleEntityVersion",
            "entityKind": api_entity_kind(entity_kind),
            "entityId": entity_id,
            "expectedVersion": expected,
            "actualVersion": actual,
        }),
        codex_state::WorkspaceChartCommitError::Storage { .. } => {
            return internal_error(format!("failed to commit workspace chart: {message}"));
        }
    };
    JSONRPCErrorError {
        code: crate::error_code::INVALID_REQUEST_ERROR_CODE,
        message,
        data: Some(data),
    }
}

fn chart_commit_validation_error(message: impl Into<String>) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: crate::error_code::INVALID_REQUEST_ERROR_CODE,
        message: message.into(),
        data: Some(chart_validation_data()),
    }
}

fn chart_validation_data() -> serde_json::Value {
    json!({ "kind": "validation" })
}
