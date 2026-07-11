use super::workspace::insert_audit_event;
use super::workspace_chart_commit::ExistingRecords;
use super::workspace_chart_commit_sql as chart_sql;
use super::workspace_chart_commit_validate;
use crate::WorkspaceChartCommitError;
use crate::WorkspaceChartCommitRequest;
use crate::WorkspaceChartEntityKind;
use sqlx::Sqlite;
use sqlx::Transaction;

struct AuditTarget<'a> {
    kind: WorkspaceChartEntityKind,
    entity_id: &'a str,
    action: &'static str,
    encounter_id: Option<&'a str>,
    note_id: Option<&'a str>,
}

pub(super) fn validation<T>(message: impl Into<String>) -> Result<T, WorkspaceChartCommitError> {
    Err(WorkspaceChartCommitError::Validation {
        message: message.into(),
    })
}

pub(super) fn required<T>(value: Option<T>, label: &str) -> Result<T, WorkspaceChartCommitError> {
    value.ok_or_else(|| WorkspaceChartCommitError::Storage {
        message: format!("workspace chart {label} missing after commit"),
    })
}

pub(super) async fn fetch_existing(
    tx: &mut Transaction<'_, Sqlite>,
    request: &WorkspaceChartCommitRequest,
    client: Option<crate::WorkspaceClient>,
) -> Result<ExistingRecords, WorkspaceChartCommitError> {
    Ok(ExistingRecords {
        client,
        safety_item: match request.safety_item.as_ref() {
            Some(input) => chart_sql::safety_item(tx, id(&input.id, "safety item")?).await?,
            None => None,
        },
        encounter: match request.encounter.as_ref() {
            Some(input) => chart_sql::encounter(tx, id(&input.id, "encounter")?).await?,
            None => None,
        },
        note: match request.note.as_ref() {
            Some(change) => chart_sql::note(tx, id(&change.upsert.id, "note")?).await?,
            None => None,
        },
        document: match request.document.as_ref() {
            Some(input) => chart_sql::document(tx, id(&input.id, "document")?).await?,
            None => None,
        },
        derivative: match request.artifact_derivative.as_ref() {
            Some(input) => chart_sql::derivative(tx, id(&input.id, "artifact derivative")?).await?,
            None => None,
        },
        clip: match request.context_clip.as_ref() {
            Some(input) => chart_sql::clip(tx, id(&input.id, "context clip")?).await?,
            None => None,
        },
        task: match request.task.as_ref() {
            Some(input) => chart_sql::task(tx, id(&input.id, "task")?).await?,
            None => None,
        },
    })
}

pub(super) fn validate_existing_ownership(
    existing: &ExistingRecords,
    client_id: &str,
) -> Result<(), WorkspaceChartCommitError> {
    if let Some(value) = existing.safety_item.as_ref() {
        validate_owner(
            "safety item",
            &value.id,
            &value.client_id,
            value.archived_at.is_some(),
            client_id,
        )?;
    }
    if let Some(value) = existing.encounter.as_ref() {
        validate_owner(
            "encounter",
            &value.id,
            &value.client_id,
            value.archived_at.is_some(),
            client_id,
        )?;
    }
    if let Some(value) = existing.note.as_ref() {
        validate_owner(
            "note",
            &value.id,
            &value.client_id,
            value.archived_at.is_some(),
            client_id,
        )?;
    }
    if let Some(value) = existing.document.as_ref() {
        validate_owner(
            "document",
            &value.id,
            &value.client_id,
            value.archived_at.is_some(),
            client_id,
        )?;
    }
    if let Some(value) = existing.derivative.as_ref() {
        validate_owner(
            "artifact derivative",
            &value.id,
            &value.client_id,
            value.archived_at.is_some(),
            client_id,
        )?;
    }
    if let Some(value) = existing.clip.as_ref() {
        validate_owner(
            "context clip",
            &value.id,
            &value.client_id,
            value.archived_at.is_some(),
            client_id,
        )?;
    }
    if let Some(value) = existing.task.as_ref() {
        validate_owner(
            "task",
            &value.id,
            &value.client_id,
            value.archived_at.is_some(),
            client_id,
        )?;
    }
    Ok(())
}

fn validate_owner(
    label: &str,
    id: &str,
    owner: &str,
    archived: bool,
    client_id: &str,
) -> Result<(), WorkspaceChartCommitError> {
    if owner != client_id {
        return validation(format!(
            "workspace {label} `{id}` was not found for client `{client_id}`"
        ));
    }
    if archived {
        return validation(format!(
            "workspace {label} `{id}` is archived and cannot be committed"
        ));
    }
    Ok(())
}

pub(super) fn validate_note_revision(
    request: &WorkspaceChartCommitRequest,
    existing: Option<&crate::WorkspaceNote>,
) -> Result<(), WorkspaceChartCommitError> {
    let Some(change) = request.note.as_ref() else {
        return Ok(());
    };
    match existing {
        Some(note) => {
            if workspace_chart_commit_validate::note_status_is_locked(&note.status) {
                return validation(
                    "signed workspace notes require an addendum instead of direct edits",
                );
            }
            let expected = change.expected_base_revision.ok_or_else(|| {
                WorkspaceChartCommitError::Validation {
                    message: format!(
                        "expected base revision is required for existing workspace note `{}`",
                        note.id
                    ),
                }
            })?;
            if expected != note.current_revision {
                return Err(WorkspaceChartCommitError::StaleNoteRevision {
                    note_id: note.id.clone(),
                    expected,
                    actual: note.current_revision,
                });
            }
        }
        None => {
            if change
                .expected_base_revision
                .is_some_and(|revision| revision != 0)
            {
                return validation("a new workspace note cannot have a nonzero base revision");
            }
        }
    }
    Ok(())
}

pub(super) async fn validate_relations(
    tx: &mut Transaction<'_, Sqlite>,
    request: &WorkspaceChartCommitRequest,
    client_id: &str,
) -> Result<(), WorkspaceChartCommitError> {
    let commit_encounter = request
        .encounter
        .as_ref()
        .and_then(|value| value.id.as_deref());
    let commit_note = request
        .note
        .as_ref()
        .and_then(|value| value.upsert.id.as_deref());
    let commit_document = request
        .document
        .as_ref()
        .and_then(|value| value.id.as_deref());

    if let Some(change) = request.note.as_ref() {
        validate_owned_link(
            tx,
            chart_sql::OwnedEntity::Encounter,
            change.upsert.encounter_id.as_deref(),
            commit_encounter,
            client_id,
        )
        .await?;
    }
    if let Some(input) = request.document.as_ref() {
        validate_owned_link(
            tx,
            chart_sql::OwnedEntity::Encounter,
            input.encounter_id.as_deref(),
            commit_encounter,
            client_id,
        )
        .await?;
    }
    if let Some(input) = request.artifact_derivative.as_ref() {
        validate_required_owned_link(
            tx,
            chart_sql::OwnedEntity::Document,
            &input.document_id,
            commit_document,
            client_id,
        )
        .await?;
        validate_owned_link(
            tx,
            chart_sql::OwnedEntity::Encounter,
            input.encounter_id.as_deref(),
            commit_encounter,
            client_id,
        )
        .await?;
        validate_owned_link(
            tx,
            chart_sql::OwnedEntity::Note,
            input.note_id.as_deref(),
            commit_note,
            client_id,
        )
        .await?;
    }
    if let Some(input) = request.context_clip.as_ref() {
        validate_required_owned_link(
            tx,
            chart_sql::OwnedEntity::Document,
            &input.document_id,
            commit_document,
            client_id,
        )
        .await?;
        validate_owned_link(
            tx,
            chart_sql::OwnedEntity::Encounter,
            input.encounter_id.as_deref(),
            commit_encounter,
            client_id,
        )
        .await?;
        validate_owned_link(
            tx,
            chart_sql::OwnedEntity::Note,
            input.note_id.as_deref(),
            commit_note,
            client_id,
        )
        .await?;
        let same_commit_derivative =
            request
                .artifact_derivative
                .as_ref()
                .is_some_and(|derivative| {
                    derivative.id.as_deref() == Some(input.derivative_id.as_str())
                        && derivative.document_id == input.document_id
                });
        if !same_commit_derivative
            && !chart_sql::active_derivative_link(
                tx,
                &input.derivative_id,
                &input.document_id,
                client_id,
            )
            .await?
        {
            return validation("workspace context clip derivative link is invalid");
        }
    }
    if let Some(input) = request.task.as_ref() {
        validate_owned_link(
            tx,
            chart_sql::OwnedEntity::Encounter,
            input.encounter_id.as_deref(),
            commit_encounter,
            client_id,
        )
        .await?;
        validate_owned_link(
            tx,
            chart_sql::OwnedEntity::Note,
            input.note_id.as_deref(),
            commit_note,
            client_id,
        )
        .await?;
        validate_owned_link(
            tx,
            chart_sql::OwnedEntity::Document,
            input.document_id.as_deref(),
            commit_document,
            client_id,
        )
        .await?;
    }
    Ok(())
}

async fn validate_required_owned_link(
    tx: &mut Transaction<'_, Sqlite>,
    entity: chart_sql::OwnedEntity,
    id: &str,
    same_commit_id: Option<&str>,
    client_id: &str,
) -> Result<(), WorkspaceChartCommitError> {
    if id.trim().is_empty() {
        return validation("workspace chart required relationship must not be empty");
    }
    validate_owned_link(tx, entity, Some(id), same_commit_id, client_id).await
}

async fn validate_owned_link(
    tx: &mut Transaction<'_, Sqlite>,
    entity: chart_sql::OwnedEntity,
    id: Option<&str>,
    same_commit_id: Option<&str>,
    client_id: &str,
) -> Result<(), WorkspaceChartCommitError> {
    let Some(id) = id else {
        return Ok(());
    };
    if same_commit_id == Some(id) {
        return Ok(());
    }
    if !chart_sql::active_owned_entity(tx, entity, id, client_id).await? {
        return validation(format!(
            "workspace chart relationship `{id}` was not found for client `{client_id}`"
        ));
    }
    Ok(())
}

pub(super) fn push_if_changed<I, E>(
    changed: &mut Vec<WorkspaceChartEntityKind>,
    kind: WorkspaceChartEntityKind,
    input: Option<&I>,
    existing: Option<&E>,
    matches: fn(&E, &I) -> bool,
) {
    if input.is_some_and(|input| existing.is_none_or(|existing| !matches(existing, input))) {
        changed.push(kind);
    }
}

pub(super) async fn apply_changes(
    tx: &mut Transaction<'_, Sqlite>,
    request: &WorkspaceChartCommitRequest,
    existing: &ExistingRecords,
    changed: &[WorkspaceChartEntityKind],
    commit_id: &str,
    client_id: &str,
    now_ms: i64,
) -> Result<(), WorkspaceChartCommitError> {
    if has(changed, WorkspaceChartEntityKind::Client) {
        let input = requested(request.client.as_ref(), "client")?;
        chart_sql::put_client(tx, client_id, input, now_ms).await?;
        audit(
            tx,
            request,
            commit_id,
            client_id,
            AuditTarget {
                kind: WorkspaceChartEntityKind::Client,
                entity_id: client_id,
                action: action(existing.client.is_some()),
                encounter_id: None,
                note_id: None,
            },
            now_ms,
        )
        .await?;
    }
    if has(changed, WorkspaceChartEntityKind::SafetyItem) {
        let input = requested(request.safety_item.as_ref(), "safety item")?;
        let entity_id = id(&input.id, "safety item")?;
        chart_sql::put_safety_item(tx, entity_id, input, now_ms).await?;
        audit(
            tx,
            request,
            commit_id,
            client_id,
            AuditTarget {
                kind: WorkspaceChartEntityKind::SafetyItem,
                entity_id,
                action: action(existing.safety_item.is_some()),
                encounter_id: None,
                note_id: None,
            },
            now_ms,
        )
        .await?;
    }
    if has(changed, WorkspaceChartEntityKind::Encounter) {
        let input = requested(request.encounter.as_ref(), "encounter")?;
        let entity_id = id(&input.id, "encounter")?;
        chart_sql::put_encounter(tx, entity_id, input, now_ms).await?;
        audit(
            tx,
            request,
            commit_id,
            client_id,
            AuditTarget {
                kind: WorkspaceChartEntityKind::Encounter,
                entity_id,
                action: action(existing.encounter.is_some()),
                encounter_id: Some(entity_id),
                note_id: None,
            },
            now_ms,
        )
        .await?;
    }
    if has(changed, WorkspaceChartEntityKind::Note) {
        let input = &requested(request.note.as_ref(), "note")?.upsert;
        let entity_id = id(&input.id, "note")?;
        let revision = existing
            .note
            .as_ref()
            .map_or(1, |note| note.current_revision + 1);
        chart_sql::put_note(tx, entity_id, input, revision, now_ms).await?;
        audit(
            tx,
            request,
            commit_id,
            client_id,
            AuditTarget {
                kind: WorkspaceChartEntityKind::Note,
                entity_id,
                action: action(existing.note.is_some()),
                encounter_id: input.encounter_id.as_deref(),
                note_id: Some(entity_id),
            },
            now_ms,
        )
        .await?;
    }
    if has(changed, WorkspaceChartEntityKind::Document) {
        let input = requested(request.document.as_ref(), "document")?;
        let entity_id = id(&input.id, "document")?;
        chart_sql::put_document(tx, entity_id, input, now_ms).await?;
        audit(
            tx,
            request,
            commit_id,
            client_id,
            AuditTarget {
                kind: WorkspaceChartEntityKind::Document,
                entity_id,
                action: action(existing.document.is_some()),
                encounter_id: input.encounter_id.as_deref(),
                note_id: None,
            },
            now_ms,
        )
        .await?;
    }
    if has(changed, WorkspaceChartEntityKind::ArtifactDerivative) {
        let input = requested(request.artifact_derivative.as_ref(), "artifact derivative")?;
        let entity_id = id(&input.id, "artifact derivative")?;
        chart_sql::put_derivative(tx, entity_id, input, now_ms).await?;
        audit(
            tx,
            request,
            commit_id,
            client_id,
            AuditTarget {
                kind: WorkspaceChartEntityKind::ArtifactDerivative,
                entity_id,
                action: action(existing.derivative.is_some()),
                encounter_id: input.encounter_id.as_deref(),
                note_id: input.note_id.as_deref(),
            },
            now_ms,
        )
        .await?;
    }
    if has(changed, WorkspaceChartEntityKind::ContextClip) {
        let input = requested(request.context_clip.as_ref(), "context clip")?;
        let entity_id = id(&input.id, "context clip")?;
        chart_sql::put_clip(tx, entity_id, input, now_ms).await?;
        audit(
            tx,
            request,
            commit_id,
            client_id,
            AuditTarget {
                kind: WorkspaceChartEntityKind::ContextClip,
                entity_id,
                action: action(existing.clip.is_some()),
                encounter_id: input.encounter_id.as_deref(),
                note_id: input.note_id.as_deref(),
            },
            now_ms,
        )
        .await?;
    }
    if has(changed, WorkspaceChartEntityKind::Task) {
        let input = requested(request.task.as_ref(), "task")?;
        let entity_id = id(&input.id, "task")?;
        let completed_at_ms = if input.status == crate::WorkspaceTaskStatus::Done {
            existing
                .task
                .as_ref()
                .and_then(|task| task.completed_at)
                .map(|value| value.timestamp_millis())
                .or(Some(now_ms))
        } else {
            None
        };
        chart_sql::put_task(tx, entity_id, input, completed_at_ms, now_ms).await?;
        audit(
            tx,
            request,
            commit_id,
            client_id,
            AuditTarget {
                kind: WorkspaceChartEntityKind::Task,
                entity_id,
                action: action(existing.task.is_some()),
                encounter_id: input.encounter_id.as_deref(),
                note_id: input.note_id.as_deref(),
            },
            now_ms,
        )
        .await?;
    }
    if !changed.is_empty() {
        let metadata_json = serde_json::json!({
            "changed_entity_kinds": changed,
            "reason": request.reason,
        })
        .to_string();
        insert_audit_event(
            tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "chart_commit".to_string(),
                entity_id: commit_id.to_string(),
                action: "committed".to_string(),
                actor: request.actor.clone(),
                actor_kind: "human".to_string(),
                source: "workspace_chart_commit".to_string(),
                client_id: Some(client_id.to_string()),
                source_thread_id: request.source_thread_id.clone(),
                source_turn_id: request.source_turn_id.clone(),
                success: true,
                summary: request.reason.clone(),
                metadata_json: Some(metadata_json),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
    }
    Ok(())
}

async fn audit(
    tx: &mut Transaction<'_, Sqlite>,
    request: &WorkspaceChartCommitRequest,
    commit_id: &str,
    client_id: &str,
    target: AuditTarget<'_>,
    now_ms: i64,
) -> anyhow::Result<()> {
    let document_id = match target.kind {
        WorkspaceChartEntityKind::Document => Some(target.entity_id.to_string()),
        WorkspaceChartEntityKind::ArtifactDerivative => request
            .artifact_derivative
            .as_ref()
            .map(|input| input.document_id.clone()),
        WorkspaceChartEntityKind::ContextClip => request
            .context_clip
            .as_ref()
            .map(|input| input.document_id.clone()),
        WorkspaceChartEntityKind::Task => request
            .task
            .as_ref()
            .and_then(|input| input.document_id.clone()),
        WorkspaceChartEntityKind::Client
        | WorkspaceChartEntityKind::SafetyItem
        | WorkspaceChartEntityKind::Encounter
        | WorkspaceChartEntityKind::Note => None,
    };
    let entity_type = match target.kind {
        WorkspaceChartEntityKind::SafetyItem => "patient_safety_item",
        other => other.as_str(),
    };
    insert_audit_event(
        tx,
        crate::WorkspaceAuditEventCreate {
            entity_type: entity_type.to_string(),
            entity_id: target.entity_id.to_string(),
            action: target.action.to_string(),
            actor: request.actor.clone(),
            actor_kind: "human".to_string(),
            source: "workspace_chart_commit".to_string(),
            client_id: Some(client_id.to_string()),
            encounter_id: target.encounter_id.map(str::to_string),
            note_id: target.note_id.map(str::to_string),
            document_id,
            source_thread_id: request.source_thread_id.clone(),
            source_turn_id: request.source_turn_id.clone(),
            success: true,
            summary: request.reason.clone(),
            metadata_json: Some(serde_json::json!({ "commit_id": commit_id }).to_string()),
        },
        now_ms,
    )
    .await?;
    Ok(())
}

fn action(existed: bool) -> &'static str {
    if existed { "updated" } else { "created" }
}

fn has(changed: &[WorkspaceChartEntityKind], kind: WorkspaceChartEntityKind) -> bool {
    changed.contains(&kind)
}

fn requested<'a, T>(value: Option<&'a T>, label: &str) -> Result<&'a T, WorkspaceChartCommitError> {
    value.ok_or_else(|| WorkspaceChartCommitError::Storage {
        message: format!("workspace chart changed {label} is missing from its request"),
    })
}

fn id<'a>(value: &'a Option<String>, label: &str) -> Result<&'a str, WorkspaceChartCommitError> {
    value
        .as_deref()
        .ok_or_else(|| WorkspaceChartCommitError::Storage {
            message: format!("workspace chart {label} id was not allocated"),
        })
}

pub(super) async fn fetch_requested_safety(
    tx: &mut Transaction<'_, Sqlite>,
    request: &WorkspaceChartCommitRequest,
) -> Result<Option<crate::WorkspacePatientSafetyItem>, WorkspaceChartCommitError> {
    match request.safety_item.as_ref() {
        Some(input) => Ok(Some(required(
            chart_sql::safety_item(tx, id(&input.id, "safety item")?).await?,
            "safety item",
        )?)),
        None => Ok(None),
    }
}

pub(super) async fn fetch_requested_encounter(
    tx: &mut Transaction<'_, Sqlite>,
    request: &WorkspaceChartCommitRequest,
) -> Result<Option<crate::WorkspaceEncounter>, WorkspaceChartCommitError> {
    match request.encounter.as_ref() {
        Some(input) => Ok(Some(required(
            chart_sql::encounter(tx, id(&input.id, "encounter")?).await?,
            "encounter",
        )?)),
        None => Ok(None),
    }
}

pub(super) async fn fetch_requested_note(
    tx: &mut Transaction<'_, Sqlite>,
    request: &WorkspaceChartCommitRequest,
) -> Result<Option<crate::WorkspaceNote>, WorkspaceChartCommitError> {
    match request.note.as_ref() {
        Some(change) => Ok(Some(required(
            chart_sql::note(tx, id(&change.upsert.id, "note")?).await?,
            "note",
        )?)),
        None => Ok(None),
    }
}

pub(super) async fn fetch_requested_document(
    tx: &mut Transaction<'_, Sqlite>,
    request: &WorkspaceChartCommitRequest,
) -> Result<Option<crate::WorkspaceDocument>, WorkspaceChartCommitError> {
    match request.document.as_ref() {
        Some(input) => Ok(Some(required(
            chart_sql::document(tx, id(&input.id, "document")?).await?,
            "document",
        )?)),
        None => Ok(None),
    }
}

pub(super) async fn fetch_requested_derivative(
    tx: &mut Transaction<'_, Sqlite>,
    request: &WorkspaceChartCommitRequest,
) -> Result<Option<crate::WorkspaceArtifactDerivative>, WorkspaceChartCommitError> {
    match request.artifact_derivative.as_ref() {
        Some(input) => Ok(Some(required(
            chart_sql::derivative(tx, id(&input.id, "artifact derivative")?).await?,
            "artifact derivative",
        )?)),
        None => Ok(None),
    }
}

pub(super) async fn fetch_requested_clip(
    tx: &mut Transaction<'_, Sqlite>,
    request: &WorkspaceChartCommitRequest,
) -> Result<Option<crate::WorkspaceContextClip>, WorkspaceChartCommitError> {
    match request.context_clip.as_ref() {
        Some(input) => Ok(Some(required(
            chart_sql::clip(tx, id(&input.id, "context clip")?).await?,
            "context clip",
        )?)),
        None => Ok(None),
    }
}

pub(super) async fn fetch_requested_task(
    tx: &mut Transaction<'_, Sqlite>,
    request: &WorkspaceChartCommitRequest,
) -> Result<Option<crate::WorkspaceTask>, WorkspaceChartCommitError> {
    match request.task.as_ref() {
        Some(input) => Ok(Some(required(
            chart_sql::task(tx, id(&input.id, "task")?).await?,
            "task",
        )?)),
        None => Ok(None),
    }
}
