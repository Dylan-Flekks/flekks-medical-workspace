use crate::model::WorkspaceArtifactDerivativeRow;
use crate::model::WorkspaceClientRow;
use crate::model::WorkspaceContextClipRow;
use crate::model::WorkspaceDocumentRow;
use crate::model::WorkspaceEncounterRow;
use crate::model::WorkspaceNoteRow;
use crate::model::WorkspacePatientSafetyItemRow;
use crate::model::WorkspaceTaskRow;
use chrono::DateTime;
use chrono::Utc;
use sqlx::Sqlite;
use sqlx::Transaction;
use sqlx::sqlite::SqliteRow;

#[derive(Debug, Clone, Copy)]
pub(super) enum OwnedEntity {
    Encounter,
    Note,
    Document,
}

async fn fetch_optional_row(
    tx: &mut Transaction<'_, Sqlite>,
    query: &'static str,
    id: &str,
) -> anyhow::Result<Option<SqliteRow>> {
    sqlx::query(query)
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(Into::into)
}

pub(super) async fn client(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<Option<crate::WorkspaceClient>> {
    let row = fetch_optional_row(
        tx,
        r#"
SELECT
    client.id, client.display_name, client.legal_first_name,
    client.legal_middle_name, client.legal_last_name, client.legal_suffix,
    client.preferred_name, client.previous_name, client.date_of_birth,
    client.sex_or_gender, client.administrative_sex, client.preferred_language,
    client.interpreter_required, client.external_id, client.record_start_date,
    client.record_end_date, client.summary,
    contact.client_id AS contact_client_id, contact.primary_phone,
    contact.primary_phone_use, contact.secondary_phone, contact.secondary_phone_use,
    contact.email, contact.secondary_email, contact.preferred_contact_method,
    contact.address_line_1, contact.address_line_2, contact.city,
    contact.state_or_province, contact.postal_code, contact.country,
    contact.address_use,
    contact.emergency_contact_name, contact.emergency_contact_relationship,
    contact.emergency_contact_phone, contact.emergency_contact_email,
    contact.contact_notes,
    coverage.client_id AS coverage_client_id, coverage.payer_name,
    coverage.plan_name, coverage.member_id, coverage.group_number,
    coverage.coverage_type, coverage.coverage_status, coverage.coverage_notes,
    client.archived_at_ms, client.created_at_ms, client.updated_at_ms
FROM workspace_clients AS client
LEFT JOIN workspace_client_contacts AS contact ON contact.client_id = client.id
LEFT JOIN workspace_client_coverages AS coverage ON coverage.client_id = client.id
WHERE client.id = ?
        "#,
        id,
    )
    .await?;
    row.map(|row| WorkspaceClientRow::try_from_row(&row).and_then(TryInto::try_into))
        .transpose()
}

pub(super) async fn safety_item(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<Option<crate::WorkspacePatientSafetyItem>> {
    let row = fetch_optional_row(
        tx,
        r#"
SELECT id, client_id, category, name, reaction, severity, dose, route, frequency,
       status, recorded_date, notes, archived_at_ms, created_at_ms, updated_at_ms
FROM workspace_patient_safety_items WHERE id = ?
        "#,
        id,
    )
    .await?;
    row.map(|row| WorkspacePatientSafetyItemRow::try_from_row(&row).and_then(TryInto::try_into))
        .transpose()
}

pub(super) async fn encounter(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<Option<crate::WorkspaceEncounter>> {
    let row = fetch_optional_row(
        tx,
        r#"
SELECT id, client_id, kind, title, status, started_at_ms, ended_at_ms,
       archived_at_ms, created_at_ms, updated_at_ms
FROM workspace_encounters WHERE id = ?
        "#,
        id,
    )
    .await?;
    row.map(|row| WorkspaceEncounterRow::try_from_row(&row).and_then(TryInto::try_into))
        .transpose()
}

pub(super) async fn note(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<Option<crate::WorkspaceNote>> {
    let row = fetch_optional_row(
        tx,
        r#"
SELECT id, client_id, encounter_id, title, kind, body, status, current_revision,
       archived_at_ms, created_at_ms, updated_at_ms
FROM workspace_notes WHERE id = ?
        "#,
        id,
    )
    .await?;
    row.map(|row| WorkspaceNoteRow::try_from_row(&row).and_then(TryInto::try_into))
        .transpose()
}

pub(super) async fn document(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<Option<crate::WorkspaceDocument>> {
    let row = fetch_optional_row(
        tx,
        r#"
SELECT id, client_id, encounter_id, title, kind, local_path, notes, scope,
       detected_kind, mime_type, file_size_bytes, modified_at_ms, sha256, tags,
       source_label, existence_status, metadata_json, original_path,
       reference_kind, vault_path, content_sha256, thumbnail_path,
       thumbnail_status, thumbnail_mime_type, intake_source, imported_at_ms,
       archived_at_ms, created_at_ms, updated_at_ms
FROM workspace_documents WHERE id = ?
        "#,
        id,
    )
    .await?;
    row.map(|row| WorkspaceDocumentRow::try_from_row(&row).and_then(TryInto::try_into))
        .transpose()
}

pub(super) async fn derivative(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<Option<crate::WorkspaceArtifactDerivative>> {
    let row = fetch_optional_row(
        tx,
        r#"
SELECT id, document_id, client_id, encounter_id, note_id, kind, title, body,
       review_status, source_method, page_range, timestamp_range, segment_label,
       tags, metadata_json, archived_at_ms, created_at_ms, updated_at_ms
FROM workspace_artifact_derivatives WHERE id = ?
        "#,
        id,
    )
    .await?;
    row.map(|row| WorkspaceArtifactDerivativeRow::try_from_row(&row).and_then(TryInto::try_into))
        .transpose()
}

pub(super) async fn clip(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<Option<crate::WorkspaceContextClip>> {
    let row = fetch_optional_row(
        tx,
        r#"
SELECT id, derivative_id, document_id, client_id, encounter_id, note_id, kind,
       title, body, review_status, source_method, page_range, timestamp_range,
       line_range, segment_label, tags, metadata_json, archived_at_ms,
       created_at_ms, updated_at_ms
FROM workspace_context_clips WHERE id = ?
        "#,
        id,
    )
    .await?;
    row.map(|row| WorkspaceContextClipRow::try_from_row(&row).and_then(TryInto::try_into))
        .transpose()
}

pub(super) async fn task(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<Option<crate::WorkspaceTask>> {
    let row = fetch_optional_row(
        tx,
        r#"
SELECT id, client_id, encounter_id, note_id, document_id, title, details, kind,
       status, priority, due_date, assigned_to, completed_at_ms, archived_at_ms,
       created_at_ms, updated_at_ms
FROM workspace_tasks WHERE id = ?
        "#,
        id,
    )
    .await?;
    row.map(|row| WorkspaceTaskRow::try_from_row(&row).and_then(TryInto::try_into))
        .transpose()
}

pub(super) async fn put_client(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    input: &crate::WorkspaceClientUpsert,
    now_ms: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
INSERT INTO workspace_clients (
    id, display_name, legal_first_name, legal_middle_name, legal_last_name,
    legal_suffix, preferred_name, previous_name, date_of_birth, sex_or_gender,
    administrative_sex, preferred_language, interpreter_required, external_id,
    record_start_date, record_end_date, summary, archived_at_ms, created_at_ms,
    updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    display_name = excluded.display_name,
    legal_first_name = excluded.legal_first_name,
    legal_middle_name = excluded.legal_middle_name,
    legal_last_name = excluded.legal_last_name,
    legal_suffix = excluded.legal_suffix,
    preferred_name = excluded.preferred_name,
    previous_name = excluded.previous_name,
    date_of_birth = excluded.date_of_birth,
    sex_or_gender = excluded.sex_or_gender,
    administrative_sex = excluded.administrative_sex,
    preferred_language = excluded.preferred_language,
    interpreter_required = excluded.interpreter_required,
    external_id = excluded.external_id,
    record_start_date = excluded.record_start_date,
    record_end_date = excluded.record_end_date,
    summary = excluded.summary,
    archived_at_ms = NULL,
    updated_at_ms = excluded.updated_at_ms
        "#,
    )
    .bind(id)
    .bind(&input.display_name)
    .bind(&input.legal_first_name)
    .bind(&input.legal_middle_name)
    .bind(&input.legal_last_name)
    .bind(&input.legal_suffix)
    .bind(&input.preferred_name)
    .bind(&input.previous_name)
    .bind(&input.date_of_birth)
    .bind(&input.sex_or_gender)
    .bind(&input.administrative_sex)
    .bind(&input.preferred_language)
    .bind(input.interpreter_required)
    .bind(&input.external_id)
    .bind(&input.record_start_date)
    .bind(&input.record_end_date)
    .bind(&input.summary)
    .bind(now_ms)
    .bind(now_ms)
    .execute(&mut **tx)
    .await?;
    super::workspace::upsert_client_admin_metadata(tx, id, input, now_ms).await
}

pub(super) async fn put_safety_item(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    input: &crate::WorkspacePatientSafetyItemUpsert,
    now_ms: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
INSERT INTO workspace_patient_safety_items (
    id, client_id, category, name, reaction, severity, dose, route, frequency,
    status, recorded_date, notes, archived_at_ms, created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    client_id = excluded.client_id, category = excluded.category, name = excluded.name,
    reaction = excluded.reaction, severity = excluded.severity, dose = excluded.dose,
    route = excluded.route, frequency = excluded.frequency, status = excluded.status,
    recorded_date = excluded.recorded_date, notes = excluded.notes,
    archived_at_ms = NULL, updated_at_ms = excluded.updated_at_ms
        "#,
    )
    .bind(id)
    .bind(&input.client_id)
    .bind(&input.category)
    .bind(&input.name)
    .bind(&input.reaction)
    .bind(&input.severity)
    .bind(&input.dose)
    .bind(&input.route)
    .bind(&input.frequency)
    .bind(&input.status)
    .bind(&input.recorded_date)
    .bind(&input.notes)
    .bind(now_ms)
    .bind(now_ms)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(super) async fn put_encounter(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    input: &crate::WorkspaceEncounterUpsert,
    now_ms: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
INSERT INTO workspace_encounters (
    id, client_id, kind, title, status, started_at_ms, ended_at_ms,
    archived_at_ms, created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    client_id = excluded.client_id, kind = excluded.kind, title = excluded.title,
    status = excluded.status, started_at_ms = excluded.started_at_ms,
    ended_at_ms = excluded.ended_at_ms, archived_at_ms = NULL,
    updated_at_ms = excluded.updated_at_ms
        "#,
    )
    .bind(id)
    .bind(&input.client_id)
    .bind(&input.kind)
    .bind(&input.title)
    .bind(&input.status)
    .bind(input.started_at.map(epoch_millis))
    .bind(input.ended_at.map(epoch_millis))
    .bind(now_ms)
    .bind(now_ms)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(super) async fn put_note(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    input: &crate::WorkspaceNoteUpsert,
    revision: i64,
    now_ms: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
INSERT INTO workspace_notes (
    id, client_id, encounter_id, title, kind, body, status, current_revision,
    archived_at_ms, created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    client_id = excluded.client_id, encounter_id = excluded.encounter_id,
    title = excluded.title, kind = excluded.kind, body = excluded.body,
    status = excluded.status, current_revision = excluded.current_revision,
    archived_at_ms = NULL, updated_at_ms = excluded.updated_at_ms
        "#,
    )
    .bind(id)
    .bind(&input.client_id)
    .bind(&input.encounter_id)
    .bind(&input.title)
    .bind(&input.kind)
    .bind(&input.body)
    .bind(&input.status)
    .bind(revision)
    .bind(now_ms)
    .bind(now_ms)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        r#"
INSERT INTO workspace_note_revisions (
    note_id, revision, body, actor, source_thread_id, source_turn_id, summary,
    created_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(id)
    .bind(revision)
    .bind(&input.body)
    .bind(&input.actor)
    .bind(&input.source_thread_id)
    .bind(&input.source_turn_id)
    .bind(&input.summary)
    .bind(now_ms)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(super) async fn put_document(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    input: &crate::WorkspaceDocumentUpsert,
    now_ms: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
INSERT INTO workspace_documents (
    id, client_id, encounter_id, title, kind, local_path, notes, scope,
    detected_kind, mime_type, file_size_bytes, modified_at_ms, sha256, tags,
    source_label, existence_status, metadata_json, original_path, reference_kind,
    vault_path, content_sha256, thumbnail_path, thumbnail_status,
    thumbnail_mime_type, intake_source, imported_at_ms, archived_at_ms,
    created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    client_id = excluded.client_id, encounter_id = excluded.encounter_id,
    title = excluded.title, kind = excluded.kind, local_path = excluded.local_path,
    notes = excluded.notes, scope = excluded.scope,
    detected_kind = excluded.detected_kind, mime_type = excluded.mime_type,
    file_size_bytes = excluded.file_size_bytes, modified_at_ms = excluded.modified_at_ms,
    sha256 = excluded.sha256, tags = excluded.tags, source_label = excluded.source_label,
    existence_status = excluded.existence_status, metadata_json = excluded.metadata_json,
    original_path = excluded.original_path, reference_kind = excluded.reference_kind,
    vault_path = excluded.vault_path, content_sha256 = excluded.content_sha256,
    thumbnail_path = excluded.thumbnail_path,
    thumbnail_status = excluded.thumbnail_status,
    thumbnail_mime_type = excluded.thumbnail_mime_type,
    intake_source = excluded.intake_source, imported_at_ms = excluded.imported_at_ms,
    archived_at_ms = NULL, updated_at_ms = excluded.updated_at_ms
        "#,
    )
    .bind(id)
    .bind(&input.client_id)
    .bind(&input.encounter_id)
    .bind(&input.title)
    .bind(&input.kind)
    .bind(&input.local_path)
    .bind(&input.notes)
    .bind(&input.scope)
    .bind(&input.detected_kind)
    .bind(&input.mime_type)
    .bind(input.file_size_bytes)
    .bind(input.modified_at.map(epoch_millis))
    .bind(&input.sha256)
    .bind(&input.tags)
    .bind(&input.source_label)
    .bind(&input.existence_status)
    .bind(&input.metadata_json)
    .bind(&input.original_path)
    .bind(&input.reference_kind)
    .bind(&input.vault_path)
    .bind(&input.content_sha256)
    .bind(&input.thumbnail_path)
    .bind(&input.thumbnail_status)
    .bind(&input.thumbnail_mime_type)
    .bind(&input.intake_source)
    .bind(input.imported_at.map(epoch_millis))
    .bind(now_ms)
    .bind(now_ms)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(super) async fn put_derivative(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    input: &crate::WorkspaceArtifactDerivativeUpsert,
    now_ms: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
INSERT INTO workspace_artifact_derivatives (
    id, document_id, client_id, encounter_id, note_id, kind, title, body,
    review_status, source_method, page_range, timestamp_range, segment_label,
    tags, metadata_json, archived_at_ms, created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    document_id = excluded.document_id, client_id = excluded.client_id,
    encounter_id = excluded.encounter_id, note_id = excluded.note_id,
    kind = excluded.kind, title = excluded.title, body = excluded.body,
    review_status = excluded.review_status, source_method = excluded.source_method,
    page_range = excluded.page_range, timestamp_range = excluded.timestamp_range,
    segment_label = excluded.segment_label, tags = excluded.tags,
    metadata_json = excluded.metadata_json, archived_at_ms = NULL,
    updated_at_ms = excluded.updated_at_ms
        "#,
    )
    .bind(id)
    .bind(&input.document_id)
    .bind(&input.client_id)
    .bind(&input.encounter_id)
    .bind(&input.note_id)
    .bind(&input.kind)
    .bind(&input.title)
    .bind(&input.body)
    .bind(&input.review_status)
    .bind(&input.source_method)
    .bind(&input.page_range)
    .bind(&input.timestamp_range)
    .bind(&input.segment_label)
    .bind(&input.tags)
    .bind(&input.metadata_json)
    .bind(now_ms)
    .bind(now_ms)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(super) async fn put_clip(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    input: &crate::WorkspaceContextClipUpsert,
    now_ms: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
INSERT INTO workspace_context_clips (
    id, derivative_id, document_id, client_id, encounter_id, note_id, kind,
    title, body, review_status, source_method, page_range, timestamp_range,
    line_range, segment_label, tags, metadata_json, archived_at_ms, created_at_ms,
    updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    derivative_id = excluded.derivative_id, document_id = excluded.document_id,
    client_id = excluded.client_id, encounter_id = excluded.encounter_id,
    note_id = excluded.note_id, kind = excluded.kind, title = excluded.title,
    body = excluded.body, review_status = excluded.review_status,
    source_method = excluded.source_method, page_range = excluded.page_range,
    timestamp_range = excluded.timestamp_range, line_range = excluded.line_range,
    segment_label = excluded.segment_label, tags = excluded.tags,
    metadata_json = excluded.metadata_json, archived_at_ms = NULL,
    updated_at_ms = excluded.updated_at_ms
        "#,
    )
    .bind(id)
    .bind(&input.derivative_id)
    .bind(&input.document_id)
    .bind(&input.client_id)
    .bind(&input.encounter_id)
    .bind(&input.note_id)
    .bind(&input.kind)
    .bind(&input.title)
    .bind(&input.body)
    .bind(&input.review_status)
    .bind(&input.source_method)
    .bind(&input.page_range)
    .bind(&input.timestamp_range)
    .bind(&input.line_range)
    .bind(&input.segment_label)
    .bind(&input.tags)
    .bind(&input.metadata_json)
    .bind(now_ms)
    .bind(now_ms)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(super) async fn put_task(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    input: &crate::WorkspaceTaskUpsert,
    completed_at_ms: Option<i64>,
    now_ms: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
INSERT INTO workspace_tasks (
    id, client_id, encounter_id, note_id, document_id, title, details, kind,
    status, priority, due_date, assigned_to, completed_at_ms, archived_at_ms,
    created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    encounter_id = excluded.encounter_id, note_id = excluded.note_id,
    document_id = excluded.document_id, title = excluded.title,
    details = excluded.details, kind = excluded.kind, status = excluded.status,
    priority = excluded.priority, due_date = excluded.due_date,
    assigned_to = excluded.assigned_to, completed_at_ms = excluded.completed_at_ms,
    archived_at_ms = NULL, updated_at_ms = excluded.updated_at_ms
        "#,
    )
    .bind(id)
    .bind(&input.client_id)
    .bind(&input.encounter_id)
    .bind(&input.note_id)
    .bind(&input.document_id)
    .bind(&input.title)
    .bind(&input.details)
    .bind(&input.kind)
    .bind(input.status.as_str())
    .bind(input.priority.as_str())
    .bind(&input.due_date)
    .bind(&input.assigned_to)
    .bind(completed_at_ms)
    .bind(now_ms)
    .bind(now_ms)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn epoch_millis(value: DateTime<Utc>) -> i64 {
    value.timestamp_millis()
}

pub(super) async fn active_owned_entity(
    tx: &mut Transaction<'_, Sqlite>,
    entity: OwnedEntity,
    id: &str,
    client_id: &str,
) -> anyhow::Result<bool> {
    let query = match entity {
        OwnedEntity::Encounter => {
            "SELECT 1 FROM workspace_encounters WHERE id = ? AND client_id = ? AND archived_at_ms IS NULL"
        }
        OwnedEntity::Note => {
            "SELECT 1 FROM workspace_notes WHERE id = ? AND client_id = ? AND archived_at_ms IS NULL"
        }
        OwnedEntity::Document => {
            "SELECT 1 FROM workspace_documents WHERE id = ? AND client_id = ? AND archived_at_ms IS NULL"
        }
    };
    Ok(sqlx::query_scalar::<_, i64>(query)
        .bind(id)
        .bind(client_id)
        .fetch_optional(&mut **tx)
        .await?
        .is_some())
}

pub(super) async fn active_derivative_link(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    document_id: &str,
    client_id: &str,
) -> anyhow::Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        r#"
SELECT 1 FROM workspace_artifact_derivatives
WHERE id = ? AND document_id = ? AND client_id = ? AND archived_at_ms IS NULL
        "#,
    )
    .bind(id)
    .bind(document_id)
    .bind(client_id)
    .fetch_optional(&mut **tx)
    .await?
    .is_some())
}
