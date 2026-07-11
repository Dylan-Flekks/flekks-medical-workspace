use super::*;
use crate::model::WorkspaceAgentResultRow;
use crate::model::WorkspaceArtifactDerivativeRow;
use crate::model::WorkspaceAuditEventRow;
use crate::model::WorkspaceClientRow;
use crate::model::WorkspaceContextClipRow;
use crate::model::WorkspaceContextPacketRow;
use crate::model::WorkspaceDocumentRow;
use crate::model::WorkspaceEncounterRow;
use crate::model::WorkspaceNoteAddendumRow;
use crate::model::WorkspaceNoteProposalRow;
use crate::model::WorkspaceNoteRow;
use crate::model::WorkspaceNoteSignatureRow;
use crate::model::WorkspacePatientSafetyItemRow;
use crate::model::WorkspaceTaskRow;
use crate::model::legacy_client_admin_metadata_from_summary;
use sha2::Digest;
use sha2::Sha256;
use std::collections::BTreeMap;
use uuid::Uuid;

const NOTE_STATUS_SIGNED: &str = "signed";
const NOTE_STATUS_ADDENDED: &str = "addended";

#[derive(Clone)]
pub struct WorkspaceStore {
    pub(super) pool: Arc<SqlitePool>,
}

// Workspace graph invariants:
// - client rows own notes, artifacts/documents, jobs, packets, and agent results.
// - artifact/document rows are reference-only metadata; original files are not mutated here.
// - derivatives belong to active artifacts; clips belong to active derivatives and artifacts.
// - context packets snapshot selected IDs plus human-readable summaries for history.
// - agent-visible context is limited to explicit packet envelopes; replay is read-only history.
// - agent results are packet-bound outputs whose packet id controls client, note, and hash provenance.
// - conversions create reviewable drafts/proposals only and never sign, submit, contact, or overwrite.
// - archived artifacts, derivatives, and clips are historical records, not selectable context.
// - write-like operations are scoped to the owning graph and recorded as audit events.

impl WorkspaceStore {
    pub(crate) fn new(pool: Arc<SqlitePool>) -> Self {
        Self { pool }
    }
}

impl WorkspaceStore {
    pub async fn list_clients(&self) -> anyhow::Result<Vec<crate::WorkspaceClient>> {
        let rows = sqlx::query(
            r#"
SELECT
    client.id AS id,
    client.display_name AS display_name,
    client.preferred_name AS preferred_name,
    client.date_of_birth AS date_of_birth,
    client.sex_or_gender AS sex_or_gender,
    client.external_id AS external_id,
    client.record_start_date AS record_start_date,
    client.record_end_date AS record_end_date,
    client.summary AS summary,
    contact.client_id AS contact_client_id,
    contact.primary_phone AS primary_phone,
    contact.secondary_phone AS secondary_phone,
    contact.email AS email,
    contact.preferred_contact_method AS preferred_contact_method,
    contact.emergency_contact_name AS emergency_contact_name,
    contact.emergency_contact_relationship AS emergency_contact_relationship,
    contact.emergency_contact_phone AS emergency_contact_phone,
    contact.emergency_contact_email AS emergency_contact_email,
    contact.contact_notes AS contact_notes,
    coverage.client_id AS coverage_client_id,
    coverage.payer_name AS payer_name,
    coverage.plan_name AS plan_name,
    coverage.member_id AS member_id,
    coverage.group_number AS group_number,
    coverage.coverage_type AS coverage_type,
    coverage.coverage_status AS coverage_status,
    coverage.coverage_notes AS coverage_notes,
    client.archived_at_ms AS archived_at_ms,
    client.created_at_ms AS created_at_ms,
    client.updated_at_ms AS updated_at_ms
FROM workspace_clients AS client
LEFT JOIN workspace_client_contacts AS contact ON contact.client_id = client.id
LEFT JOIN workspace_client_coverages AS coverage ON coverage.client_id = client.id
WHERE client.archived_at_ms IS NULL
ORDER BY client.updated_at_ms DESC, client.display_name ASC
            "#,
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.into_iter()
            .map(|row| WorkspaceClientRow::try_from_row(&row).and_then(TryInto::try_into))
            .collect()
    }

    pub async fn get_client(&self, id: &str) -> anyhow::Result<Option<crate::WorkspaceClient>> {
        let row = sqlx::query(
            r#"
SELECT
    client.id AS id,
    client.display_name AS display_name,
    client.preferred_name AS preferred_name,
    client.date_of_birth AS date_of_birth,
    client.sex_or_gender AS sex_or_gender,
    client.external_id AS external_id,
    client.record_start_date AS record_start_date,
    client.record_end_date AS record_end_date,
    client.summary AS summary,
    contact.client_id AS contact_client_id,
    contact.primary_phone AS primary_phone,
    contact.secondary_phone AS secondary_phone,
    contact.email AS email,
    contact.preferred_contact_method AS preferred_contact_method,
    contact.emergency_contact_name AS emergency_contact_name,
    contact.emergency_contact_relationship AS emergency_contact_relationship,
    contact.emergency_contact_phone AS emergency_contact_phone,
    contact.emergency_contact_email AS emergency_contact_email,
    contact.contact_notes AS contact_notes,
    coverage.client_id AS coverage_client_id,
    coverage.payer_name AS payer_name,
    coverage.plan_name AS plan_name,
    coverage.member_id AS member_id,
    coverage.group_number AS group_number,
    coverage.coverage_type AS coverage_type,
    coverage.coverage_status AS coverage_status,
    coverage.coverage_notes AS coverage_notes,
    client.archived_at_ms AS archived_at_ms,
    client.created_at_ms AS created_at_ms,
    client.updated_at_ms AS updated_at_ms
FROM workspace_clients AS client
LEFT JOIN workspace_client_contacts AS contact ON contact.client_id = client.id
LEFT JOIN workspace_client_coverages AS coverage ON coverage.client_id = client.id
WHERE client.id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(self.pool.as_ref())
        .await?;

        row.map(|row| WorkspaceClientRow::try_from_row(&row).and_then(TryInto::try_into))
            .transpose()
    }

    pub async fn upsert_client(
        &self,
        input: crate::WorkspaceClientUpsert,
    ) -> anyhow::Result<crate::WorkspaceClient> {
        let id = input
            .id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let existed: Option<String> =
            sqlx::query_scalar("SELECT id FROM workspace_clients WHERE id = ?")
                .bind(&id)
                .fetch_optional(&mut *tx)
                .await?;
        sqlx::query(
            r#"
INSERT INTO workspace_clients (
    id,
    display_name,
    preferred_name,
    date_of_birth,
    sex_or_gender,
    external_id,
    record_start_date,
    record_end_date,
    summary,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    display_name = excluded.display_name,
    preferred_name = excluded.preferred_name,
    date_of_birth = excluded.date_of_birth,
    sex_or_gender = excluded.sex_or_gender,
    external_id = excluded.external_id,
    record_start_date = excluded.record_start_date,
    record_end_date = excluded.record_end_date,
    summary = excluded.summary,
    archived_at_ms = NULL,
    updated_at_ms = excluded.updated_at_ms
            "#,
        )
        .bind(&id)
        .bind(&input.display_name)
        .bind(&input.preferred_name)
        .bind(&input.date_of_birth)
        .bind(&input.sex_or_gender)
        .bind(&input.external_id)
        .bind(&input.record_start_date)
        .bind(&input.record_end_date)
        .bind(&input.summary)
        .bind(now_ms)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;

        upsert_client_admin_metadata(&mut tx, &id, &input, now_ms).await?;

        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "client".to_string(),
                entity_id: id.clone(),
                action: if existed.is_some() {
                    "updated".to_string()
                } else {
                    "created".to_string()
                },
                actor: "human".to_string(),
                actor_kind: "human".to_string(),
                source: "state".to_string(),
                client_id: Some(id.clone()),
                success: true,
                summary: input.display_name,
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        tx.commit().await?;
        self.get_client(&id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("workspace client missing after upsert: {id}"))
    }

    pub async fn backfill_legacy_client_admin_metadata(&self) -> anyhow::Result<()> {
        let rows = sqlx::query(
            r#"
SELECT id, summary
FROM workspace_clients
            "#,
        )
        .fetch_all(self.pool.as_ref())
        .await?;
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        for row in rows {
            let client_id: String = row.try_get("id")?;
            let summary: String = row.try_get("summary")?;
            let legacy = legacy_client_admin_metadata_from_summary(&summary);
            if legacy.has_contact_values() {
                sqlx::query(
                    r#"
INSERT INTO workspace_client_contacts (
    client_id,
    primary_phone,
    secondary_phone,
    email,
    preferred_contact_method,
    emergency_contact_name,
    emergency_contact_relationship,
    emergency_contact_phone,
    emergency_contact_email,
    contact_notes,
    created_at_ms,
    updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(client_id) DO NOTHING
                    "#,
                )
                .bind(&client_id)
                .bind(&legacy.primary_phone)
                .bind(&legacy.secondary_phone)
                .bind(&legacy.email)
                .bind(&legacy.preferred_contact_method)
                .bind(&legacy.emergency_contact_name)
                .bind(&legacy.emergency_contact_relationship)
                .bind(&legacy.emergency_contact_phone)
                .bind(&legacy.emergency_contact_email)
                .bind(&legacy.contact_notes)
                .bind(now_ms)
                .bind(now_ms)
                .execute(&mut *tx)
                .await?;
            }
            if legacy.has_coverage_values() {
                sqlx::query(
                    r#"
INSERT INTO workspace_client_coverages (
    client_id,
    payer_name,
    plan_name,
    member_id,
    group_number,
    coverage_type,
    coverage_status,
    coverage_notes,
    created_at_ms,
    updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(client_id) DO NOTHING
                    "#,
                )
                .bind(&client_id)
                .bind(&legacy.payer_name)
                .bind(&legacy.plan_name)
                .bind(&legacy.member_id)
                .bind(&legacy.group_number)
                .bind(&legacy.coverage_type)
                .bind(&legacy.coverage_status)
                .bind(&legacy.coverage_notes)
                .bind(now_ms)
                .bind(now_ms)
                .execute(&mut *tx)
                .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn archive_client(&self, id: &str) -> anyhow::Result<bool> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let display_name: Option<String> = sqlx::query_scalar(
            "SELECT display_name FROM workspace_clients WHERE id = ? AND archived_at_ms IS NULL",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(display_name) = display_name else {
            tx.rollback().await?;
            return Ok(false);
        };
        let result = sqlx::query(
            r#"
UPDATE workspace_clients
SET archived_at_ms = ?, updated_at_ms = ?
WHERE id = ? AND archived_at_ms IS NULL
            "#,
        )
        .bind(now_ms)
        .bind(now_ms)
        .bind(id)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() > 0 {
            insert_audit_event(
                &mut tx,
                crate::WorkspaceAuditEventCreate {
                    entity_type: "client".to_string(),
                    entity_id: id.to_string(),
                    action: "archived".to_string(),
                    actor: "human".to_string(),
                    actor_kind: "human".to_string(),
                    source: "state".to_string(),
                    client_id: Some(id.to_string()),
                    success: true,
                    summary: display_name,
                    ..Default::default()
                },
                now_ms,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_encounters(
        &self,
        client_id: &str,
    ) -> anyhow::Result<Vec<crate::WorkspaceEncounter>> {
        let rows = sqlx::query(
            r#"
SELECT
    id,
    client_id,
    kind,
    title,
    status,
    started_at_ms,
    ended_at_ms,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
FROM workspace_encounters
WHERE client_id = ? AND archived_at_ms IS NULL
ORDER BY COALESCE(started_at_ms, updated_at_ms) DESC, title ASC
            "#,
        )
        .bind(client_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.into_iter()
            .map(|row| WorkspaceEncounterRow::try_from_row(&row).and_then(TryInto::try_into))
            .collect()
    }

    pub async fn upsert_encounter(
        &self,
        input: crate::WorkspaceEncounterUpsert,
    ) -> anyhow::Result<crate::WorkspaceEncounter> {
        let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let started_at_ms = input.started_at.map(datetime_to_epoch_millis);
        let ended_at_ms = input.ended_at.map(datetime_to_epoch_millis);
        let mut tx = self.pool.begin().await?;
        let existed: Option<String> =
            sqlx::query_scalar("SELECT id FROM workspace_encounters WHERE id = ?")
                .bind(&id)
                .fetch_optional(&mut *tx)
                .await?;
        let row = sqlx::query(
            r#"
INSERT INTO workspace_encounters (
    id,
    client_id,
    kind,
    title,
    status,
    started_at_ms,
    ended_at_ms,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    client_id = excluded.client_id,
    kind = excluded.kind,
    title = excluded.title,
    status = excluded.status,
    started_at_ms = excluded.started_at_ms,
    ended_at_ms = excluded.ended_at_ms,
    archived_at_ms = NULL,
    updated_at_ms = excluded.updated_at_ms
RETURNING
    id,
    client_id,
    kind,
    title,
    status,
    started_at_ms,
    ended_at_ms,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
            "#,
        )
        .bind(&id)
        .bind(&input.client_id)
        .bind(&input.kind)
        .bind(&input.title)
        .bind(&input.status)
        .bind(started_at_ms)
        .bind(ended_at_ms)
        .bind(now_ms)
        .bind(now_ms)
        .fetch_one(&mut *tx)
        .await?;

        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "encounter".to_string(),
                entity_id: id.clone(),
                action: if existed.is_some() {
                    "updated".to_string()
                } else {
                    "created".to_string()
                },
                actor: "human".to_string(),
                actor_kind: "human".to_string(),
                source: "state".to_string(),
                client_id: Some(input.client_id),
                encounter_id: Some(id),
                success: true,
                summary: input.title,
                ..Default::default()
            },
            now_ms,
        )
        .await?;

        tx.commit().await?;
        WorkspaceEncounterRow::try_from_row(&row).and_then(TryInto::try_into)
    }

    pub async fn list_documents(
        &self,
        client_id: &str,
    ) -> anyhow::Result<Vec<crate::WorkspaceDocument>> {
        let rows = sqlx::query(
            r#"
SELECT
    id,
    client_id,
    encounter_id,
    title,
    kind,
    local_path,
    notes,
    scope,
    detected_kind,
    mime_type,
    file_size_bytes,
    modified_at_ms,
    sha256,
    tags,
    source_label,
    existence_status,
    metadata_json,
    original_path,
    reference_kind,
    vault_path,
    content_sha256,
    thumbnail_path,
    thumbnail_status,
    thumbnail_mime_type,
    intake_source,
    imported_at_ms,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
FROM workspace_documents
WHERE client_id = ? AND archived_at_ms IS NULL
ORDER BY updated_at_ms DESC, title ASC
            "#,
        )
        .bind(client_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.into_iter()
            .map(|row| WorkspaceDocumentRow::try_from_row(&row).and_then(TryInto::try_into))
            .collect()
    }

    pub async fn list_practice_library_items(
        &self,
        filter: crate::WorkspacePracticeLibraryFilter,
    ) -> anyhow::Result<Vec<crate::WorkspacePracticeLibraryItem>> {
        let active_links = if let Some(active_client_id) = filter.active_client_id.as_deref() {
            self.active_patient_library_links(active_client_id).await?
        } else {
            BTreeMap::new()
        };
        let query = filter
            .query
            .as_deref()
            .map(str::trim)
            .filter(|query| !query.is_empty())
            .map(str::to_ascii_lowercase);
        let limit = filter.limit.unwrap_or(100).clamp(1, 500) as usize;
        let rows = sqlx::query(
            r#"
SELECT
    document.id,
    document.client_id,
    document.encounter_id,
    document.title,
    document.kind,
    document.local_path,
    document.notes,
    document.scope,
    document.detected_kind,
    document.mime_type,
    document.file_size_bytes,
    document.modified_at_ms,
    document.sha256,
    document.tags,
    document.source_label,
    document.existence_status,
    document.metadata_json,
    document.original_path,
    document.reference_kind,
    document.vault_path,
    document.content_sha256,
    document.thumbnail_path,
    document.thumbnail_status,
    document.thumbnail_mime_type,
    document.intake_source,
    document.imported_at_ms,
    document.archived_at_ms,
    document.created_at_ms,
    document.updated_at_ms,
    client.id AS owner_client_id,
    client.display_name AS owner_display_name,
    (
        SELECT COUNT(*)
        FROM workspace_artifact_derivatives AS derivative
        WHERE derivative.document_id = document.id
          AND derivative.client_id = document.client_id
          AND derivative.archived_at_ms IS NULL
    ) AS reviewed_text_count,
    (
        SELECT COUNT(*)
        FROM workspace_context_clips AS clip
        WHERE clip.document_id = document.id
          AND clip.client_id = document.client_id
          AND clip.archived_at_ms IS NULL
    ) AS clip_count
FROM workspace_documents AS document
JOIN workspace_clients AS client
  ON client.id = document.client_id
 AND client.archived_at_ms IS NULL
WHERE document.archived_at_ms IS NULL
ORDER BY document.updated_at_ms DESC, document.title ASC
            "#,
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        let mut items = Vec::new();
        for row in rows {
            let document: crate::WorkspaceDocument =
                WorkspaceDocumentRow::try_from_row(&row)?.try_into()?;
            let Some(scope_reason) = practice_library_scope_reason(&document) else {
                continue;
            };
            let owner_client_id: String = row.try_get("owner_client_id")?;
            let owner_display_name: String = row.try_get("owner_display_name")?;
            if let Some(query) = query.as_deref()
                && !practice_library_item_matches_query(&document, &owner_display_name, query)
            {
                continue;
            }
            let linked_document_id = active_links.get(&document.id).cloned();
            items.push(crate::WorkspacePracticeLibraryItem {
                document,
                owner_client_id,
                owner_display_name,
                linked_to_active_client: linked_document_id.is_some(),
                linked_document_id,
                scope_reason,
                reviewed_text_count: row.try_get("reviewed_text_count")?,
                clip_count: row.try_get("clip_count")?,
            });
            if items.len() >= limit {
                break;
            }
        }
        Ok(items)
    }

    async fn active_patient_library_links(
        &self,
        active_client_id: &str,
    ) -> anyhow::Result<BTreeMap<String, String>> {
        let rows = sqlx::query(
            r#"
SELECT id, metadata_json
FROM workspace_documents
WHERE client_id = ? AND archived_at_ms IS NULL
            "#,
        )
        .bind(active_client_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        let mut links = BTreeMap::new();
        for row in rows {
            let document_id: String = row.try_get("id")?;
            let metadata_json: String = row.try_get("metadata_json")?;
            if let Some(source_id) = associated_from_document_id(&metadata_json) {
                links.entry(source_id).or_insert(document_id);
            }
        }
        Ok(links)
    }

    pub async fn get_document(&self, id: &str) -> anyhow::Result<Option<crate::WorkspaceDocument>> {
        let row = sqlx::query(
            r#"
SELECT
    id,
    client_id,
    encounter_id,
    title,
    kind,
    local_path,
    notes,
    scope,
    detected_kind,
    mime_type,
    file_size_bytes,
    modified_at_ms,
    sha256,
    tags,
    source_label,
    existence_status,
    metadata_json,
    original_path,
    reference_kind,
    vault_path,
    content_sha256,
    thumbnail_path,
    thumbnail_status,
    thumbnail_mime_type,
    intake_source,
    imported_at_ms,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
FROM workspace_documents
WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(self.pool.as_ref())
        .await?;

        row.map(|row| WorkspaceDocumentRow::try_from_row(&row).and_then(TryInto::try_into))
            .transpose()
    }

    pub async fn upsert_document(
        &self,
        input: crate::WorkspaceDocumentUpsert,
    ) -> anyhow::Result<crate::WorkspaceDocument> {
        let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let existed: Option<String> =
            sqlx::query_scalar("SELECT id FROM workspace_documents WHERE id = ?")
                .bind(&id)
                .fetch_optional(&mut *tx)
                .await?;
        let row = sqlx::query(
            r#"
INSERT INTO workspace_documents (
    id,
    client_id,
    encounter_id,
    title,
    kind,
    local_path,
    notes,
    scope,
    detected_kind,
    mime_type,
    file_size_bytes,
    modified_at_ms,
    sha256,
    tags,
    source_label,
    existence_status,
    metadata_json,
    original_path,
    reference_kind,
    vault_path,
    content_sha256,
    thumbnail_path,
    thumbnail_status,
    thumbnail_mime_type,
    intake_source,
    imported_at_ms,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    client_id = excluded.client_id,
    encounter_id = excluded.encounter_id,
    title = excluded.title,
    kind = excluded.kind,
    local_path = excluded.local_path,
    notes = excluded.notes,
    scope = excluded.scope,
    detected_kind = excluded.detected_kind,
    mime_type = excluded.mime_type,
    file_size_bytes = excluded.file_size_bytes,
    modified_at_ms = excluded.modified_at_ms,
    sha256 = excluded.sha256,
    tags = excluded.tags,
    source_label = excluded.source_label,
    existence_status = excluded.existence_status,
    metadata_json = excluded.metadata_json,
    original_path = excluded.original_path,
    reference_kind = excluded.reference_kind,
    vault_path = excluded.vault_path,
    content_sha256 = excluded.content_sha256,
    thumbnail_path = excluded.thumbnail_path,
    thumbnail_status = excluded.thumbnail_status,
    thumbnail_mime_type = excluded.thumbnail_mime_type,
    intake_source = excluded.intake_source,
    imported_at_ms = excluded.imported_at_ms,
    archived_at_ms = NULL,
    updated_at_ms = excluded.updated_at_ms
RETURNING
    id,
    client_id,
    encounter_id,
    title,
    kind,
    local_path,
    notes,
    scope,
    detected_kind,
    mime_type,
    file_size_bytes,
    modified_at_ms,
    sha256,
    tags,
    source_label,
    existence_status,
    metadata_json,
    original_path,
    reference_kind,
    vault_path,
    content_sha256,
    thumbnail_path,
    thumbnail_status,
    thumbnail_mime_type,
    intake_source,
    imported_at_ms,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
            "#,
        )
        .bind(&id)
        .bind(&input.client_id)
        .bind(&input.encounter_id)
        .bind(&input.title)
        .bind(&input.kind)
        .bind(&input.local_path)
        .bind(&input.notes)
        .bind(nonempty_or_string(&input.scope, "patient"))
        .bind(&input.detected_kind)
        .bind(&input.mime_type)
        .bind(input.file_size_bytes)
        .bind(input.modified_at.map(datetime_to_epoch_millis))
        .bind(&input.sha256)
        .bind(&input.tags)
        .bind(&input.source_label)
        .bind(nonempty_or_string(&input.existence_status, "unknown"))
        .bind(nonempty_or_string(&input.metadata_json, "{}"))
        .bind(nonempty_or_string(&input.original_path, &input.local_path))
        .bind(nonempty_or_string(&input.reference_kind, "local_reference"))
        .bind(&input.vault_path)
        .bind(&input.content_sha256)
        .bind(&input.thumbnail_path)
        .bind(nonempty_or_string(&input.thumbnail_status, "none"))
        .bind(&input.thumbnail_mime_type)
        .bind(&input.intake_source)
        .bind(input.imported_at.map(datetime_to_epoch_millis))
        .bind(now_ms)
        .bind(now_ms)
        .fetch_one(&mut *tx)
        .await?;

        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "document".to_string(),
                entity_id: id.clone(),
                action: if existed.is_some() {
                    "updated".to_string()
                } else {
                    "created".to_string()
                },
                actor: "human".to_string(),
                actor_kind: "human".to_string(),
                source: "state".to_string(),
                client_id: Some(input.client_id),
                encounter_id: input.encounter_id,
                document_id: Some(id),
                success: true,
                summary: input.title,
                ..Default::default()
            },
            now_ms,
        )
        .await?;

        tx.commit().await?;
        WorkspaceDocumentRow::try_from_row(&row).and_then(TryInto::try_into)
    }

    pub async fn archive_document(&self, id: &str) -> anyhow::Result<bool> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let existing: Option<(String, Option<String>, String)> = sqlx::query_as(
            "SELECT client_id, encounter_id, title FROM workspace_documents WHERE id = ? AND archived_at_ms IS NULL",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((client_id, encounter_id, title)) = existing else {
            tx.rollback().await?;
            return Ok(false);
        };
        let result = sqlx::query(
            r#"
UPDATE workspace_documents
SET archived_at_ms = ?, updated_at_ms = ?
WHERE id = ? AND archived_at_ms IS NULL
            "#,
        )
        .bind(now_ms)
        .bind(now_ms)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() > 0 {
            insert_audit_event(
                &mut tx,
                crate::WorkspaceAuditEventCreate {
                    entity_type: "document".to_string(),
                    entity_id: id.to_string(),
                    action: "archived".to_string(),
                    actor: "human".to_string(),
                    actor_kind: "human".to_string(),
                    source: "state".to_string(),
                    client_id: Some(client_id),
                    encounter_id,
                    document_id: Some(id.to_string()),
                    success: true,
                    summary: title,
                    ..Default::default()
                },
                now_ms,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_patient_safety_items(
        &self,
        client_id: &str,
    ) -> anyhow::Result<Vec<crate::WorkspacePatientSafetyItem>> {
        let rows = sqlx::query(
            r#"
SELECT
    id,
    client_id,
    category,
    name,
    reaction,
    severity,
    dose,
    route,
    frequency,
    status,
    recorded_date,
    notes,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
FROM workspace_patient_safety_items
WHERE client_id = ? AND archived_at_ms IS NULL
ORDER BY
    CASE category
        WHEN 'allergy' THEN 0
        WHEN 'medication' THEN 1
        WHEN 'condition' THEN 2
        WHEN 'precaution' THEN 3
        ELSE 4
    END,
    updated_at_ms DESC,
    name ASC
            "#,
        )
        .bind(client_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.into_iter()
            .map(|row| {
                WorkspacePatientSafetyItemRow::try_from_row(&row).and_then(TryInto::try_into)
            })
            .collect()
    }

    pub async fn upsert_patient_safety_item(
        &self,
        input: crate::WorkspacePatientSafetyItemUpsert,
    ) -> anyhow::Result<crate::WorkspacePatientSafetyItem> {
        let category = normalize_patient_safety_category(&input.category)?;
        let name = input.name.trim();
        if name.is_empty() {
            anyhow::bail!("workspace patient safety item name must not be empty");
        }
        let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        validate_task_links(&mut tx, &input.client_id, None, None, None).await?;
        let existing_client_id: Option<String> =
            sqlx::query_scalar("SELECT client_id FROM workspace_patient_safety_items WHERE id = ?")
                .bind(&id)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some(existing_client_id) = existing_client_id.as_deref()
            && existing_client_id != input.client_id
        {
            tx.rollback().await?;
            anyhow::bail!(
                "workspace patient safety item `{id}` was not found for client `{}`",
                input.client_id
            );
        }
        let row = sqlx::query(
            r#"
INSERT INTO workspace_patient_safety_items (
    id,
    client_id,
    category,
    name,
    reaction,
    severity,
    dose,
    route,
    frequency,
    status,
    recorded_date,
    notes,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    client_id = excluded.client_id,
    category = excluded.category,
    name = excluded.name,
    reaction = excluded.reaction,
    severity = excluded.severity,
    dose = excluded.dose,
    route = excluded.route,
    frequency = excluded.frequency,
    status = excluded.status,
    recorded_date = excluded.recorded_date,
    notes = excluded.notes,
    archived_at_ms = NULL,
    updated_at_ms = excluded.updated_at_ms
RETURNING
    id,
    client_id,
    category,
    name,
    reaction,
    severity,
    dose,
    route,
    frequency,
    status,
    recorded_date,
    notes,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
            "#,
        )
        .bind(&id)
        .bind(&input.client_id)
        .bind(category)
        .bind(name)
        .bind(input.reaction)
        .bind(input.severity)
        .bind(input.dose)
        .bind(input.route)
        .bind(input.frequency)
        .bind(input.status)
        .bind(input.recorded_date)
        .bind(input.notes)
        .bind(now_ms)
        .bind(now_ms)
        .fetch_one(&mut *tx)
        .await?;

        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "patient_safety_item".to_string(),
                entity_id: id.clone(),
                action: if existing_client_id.is_some() {
                    "updated".to_string()
                } else {
                    "created".to_string()
                },
                actor: "human".to_string(),
                actor_kind: "human".to_string(),
                source: "state".to_string(),
                client_id: Some(input.client_id),
                success: true,
                summary: name.to_string(),
                ..Default::default()
            },
            now_ms,
        )
        .await?;

        tx.commit().await?;
        WorkspacePatientSafetyItemRow::try_from_row(&row).and_then(TryInto::try_into)
    }

    pub async fn archive_patient_safety_item(&self, id: &str) -> anyhow::Result<bool> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let existing: Option<(String, String)> = sqlx::query_as(
            "SELECT client_id, name FROM workspace_patient_safety_items WHERE id = ? AND archived_at_ms IS NULL",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((client_id, name)) = existing else {
            tx.rollback().await?;
            return Ok(false);
        };
        let result = sqlx::query(
            r#"
UPDATE workspace_patient_safety_items
SET archived_at_ms = ?, updated_at_ms = ?
WHERE id = ? AND archived_at_ms IS NULL
            "#,
        )
        .bind(now_ms)
        .bind(now_ms)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() > 0 {
            insert_audit_event(
                &mut tx,
                crate::WorkspaceAuditEventCreate {
                    entity_type: "patient_safety_item".to_string(),
                    entity_id: id.to_string(),
                    action: "archived".to_string(),
                    actor: "human".to_string(),
                    actor_kind: "human".to_string(),
                    source: "state".to_string(),
                    client_id: Some(client_id),
                    success: true,
                    summary: name,
                    ..Default::default()
                },
                now_ms,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_artifact_derivatives(
        &self,
        filter: crate::WorkspaceArtifactDerivativeFilter,
    ) -> anyhow::Result<Vec<crate::WorkspaceArtifactDerivative>> {
        let limit = filter.limit.unwrap_or(100).clamp(1, 200);
        let rows = sqlx::query(
            r#"
SELECT
    derivative.id,
    derivative.document_id,
    derivative.client_id,
    derivative.encounter_id,
    derivative.note_id,
    derivative.kind,
    derivative.title,
    derivative.body,
    derivative.review_status,
    derivative.source_method,
    derivative.page_range,
    derivative.timestamp_range,
    derivative.segment_label,
    derivative.tags,
    derivative.metadata_json,
    derivative.archived_at_ms,
    derivative.created_at_ms,
    derivative.updated_at_ms
FROM workspace_artifact_derivatives AS derivative
JOIN workspace_documents AS document
  ON document.id = derivative.document_id
 AND document.client_id = derivative.client_id
 AND document.archived_at_ms IS NULL
WHERE derivative.client_id = ?
  AND (? IS NULL OR derivative.document_id = ?)
  AND (? IS NULL OR derivative.note_id = ?)
  AND derivative.archived_at_ms IS NULL
ORDER BY derivative.updated_at_ms DESC, derivative.title ASC
LIMIT ?
            "#,
        )
        .bind(filter.client_id)
        .bind(&filter.document_id)
        .bind(&filter.document_id)
        .bind(&filter.note_id)
        .bind(&filter.note_id)
        .bind(i64::from(limit))
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.into_iter()
            .map(|row| {
                WorkspaceArtifactDerivativeRow::try_from_row(&row).and_then(TryInto::try_into)
            })
            .collect()
    }

    pub async fn upsert_artifact_derivative(
        &self,
        input: crate::WorkspaceArtifactDerivativeUpsert,
    ) -> anyhow::Result<crate::WorkspaceArtifactDerivative> {
        let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        validate_task_links(
            &mut tx,
            &input.client_id,
            input.encounter_id.as_deref(),
            input.note_id.as_deref(),
            Some(input.document_id.as_str()),
        )
        .await?;
        let existed: Option<String> =
            sqlx::query_scalar("SELECT id FROM workspace_artifact_derivatives WHERE id = ?")
                .bind(&id)
                .fetch_optional(&mut *tx)
                .await?;
        let kind = nonempty_or_string(&input.kind, "human annotation");
        let review_status = nonempty_or_string(&input.review_status, "draft");
        let source_method = nonempty_or_string(&input.source_method, "human_typed");
        let metadata_json = nonempty_or_string(&input.metadata_json, "{}");
        let row = sqlx::query(
            r#"
INSERT INTO workspace_artifact_derivatives (
    id,
    document_id,
    client_id,
    encounter_id,
    note_id,
    kind,
    title,
    body,
    review_status,
    source_method,
    page_range,
    timestamp_range,
    segment_label,
    tags,
    metadata_json,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    document_id = excluded.document_id,
    client_id = excluded.client_id,
    encounter_id = excluded.encounter_id,
    note_id = excluded.note_id,
    kind = excluded.kind,
    title = excluded.title,
    body = excluded.body,
    review_status = excluded.review_status,
    source_method = excluded.source_method,
    page_range = excluded.page_range,
    timestamp_range = excluded.timestamp_range,
    segment_label = excluded.segment_label,
    tags = excluded.tags,
    metadata_json = excluded.metadata_json,
    archived_at_ms = NULL,
    updated_at_ms = excluded.updated_at_ms
RETURNING
    id,
    document_id,
    client_id,
    encounter_id,
    note_id,
    kind,
    title,
    body,
    review_status,
    source_method,
    page_range,
    timestamp_range,
    segment_label,
    tags,
    metadata_json,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
            "#,
        )
        .bind(&id)
        .bind(&input.document_id)
        .bind(&input.client_id)
        .bind(&input.encounter_id)
        .bind(&input.note_id)
        .bind(&kind)
        .bind(&input.title)
        .bind(&input.body)
        .bind(&review_status)
        .bind(&source_method)
        .bind(&input.page_range)
        .bind(&input.timestamp_range)
        .bind(&input.segment_label)
        .bind(&input.tags)
        .bind(&metadata_json)
        .bind(now_ms)
        .bind(now_ms)
        .fetch_one(&mut *tx)
        .await?;

        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "artifact_derivative".to_string(),
                entity_id: id.clone(),
                action: if existed.is_some() {
                    "updated".to_string()
                } else {
                    "created".to_string()
                },
                actor: input.actor,
                actor_kind: "human".to_string(),
                source: "state".to_string(),
                client_id: Some(input.client_id),
                encounter_id: input.encounter_id,
                note_id: input.note_id,
                document_id: Some(input.document_id),
                success: true,
                summary: input.title,
                metadata_json: Some(format!(
                    r#"{{"kind":"{}","review_status":"{}","source_method":"{}"}}"#,
                    json_escape(&kind),
                    json_escape(&review_status),
                    json_escape(&source_method)
                )),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        tx.commit().await?;
        WorkspaceArtifactDerivativeRow::try_from_row(&row).and_then(TryInto::try_into)
    }

    pub async fn update_artifact_derivative_status(
        &self,
        input: crate::WorkspaceArtifactDerivativeStatusUpdate,
    ) -> anyhow::Result<Option<crate::WorkspaceArtifactDerivative>> {
        let review_status = input.review_status.trim().to_string();
        if review_status.is_empty() {
            anyhow::bail!("workspace artifact derivative review status must not be empty");
        }
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let existing_row = sqlx::query(
            r#"
SELECT
    id,
    document_id,
    client_id,
    encounter_id,
    note_id,
    kind,
    title,
    body,
    review_status,
    source_method,
    page_range,
    timestamp_range,
    segment_label,
    tags,
    metadata_json,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
FROM workspace_artifact_derivatives
WHERE id = ?
            "#,
        )
        .bind(&input.derivative_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(existing_row) = existing_row else {
            tx.rollback().await?;
            return Ok(None);
        };
        let existing: crate::WorkspaceArtifactDerivative =
            WorkspaceArtifactDerivativeRow::try_from_row(&existing_row)?.try_into()?;
        if existing.review_status == review_status {
            tx.rollback().await?;
            return Ok(Some(existing));
        }
        let archived_at_ms = (review_status == "archived").then_some(now_ms);
        let row = sqlx::query(
            r#"
UPDATE workspace_artifact_derivatives
SET review_status = ?, archived_at_ms = ?, updated_at_ms = ?
WHERE id = ?
RETURNING
    id,
    document_id,
    client_id,
    encounter_id,
    note_id,
    kind,
    title,
    body,
    review_status,
    source_method,
    page_range,
    timestamp_range,
    segment_label,
    tags,
    metadata_json,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
            "#,
        )
        .bind(&review_status)
        .bind(archived_at_ms)
        .bind(now_ms)
        .bind(&input.derivative_id)
        .fetch_one(&mut *tx)
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "artifact_derivative".to_string(),
                entity_id: input.derivative_id,
                action: "status_changed".to_string(),
                actor: input.actor,
                actor_kind: "human".to_string(),
                source: "state".to_string(),
                client_id: Some(existing.client_id),
                encounter_id: existing.encounter_id,
                note_id: existing.note_id,
                document_id: Some(existing.document_id),
                success: true,
                summary: format!("{} -> {}", existing.review_status, review_status),
                metadata_json: Some(format!(
                    r#"{{"from":"{}","to":"{}"}}"#,
                    json_escape(&existing.review_status),
                    json_escape(&review_status)
                )),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        tx.commit().await?;
        WorkspaceArtifactDerivativeRow::try_from_row(&row)
            .and_then(TryInto::try_into)
            .map(Some)
    }

    pub async fn list_context_clips(
        &self,
        filter: crate::WorkspaceContextClipFilter,
    ) -> anyhow::Result<Vec<crate::WorkspaceContextClip>> {
        let limit = filter.limit.unwrap_or(100).clamp(1, 200);
        let rows = sqlx::query(
            r#"
SELECT
    clip.id,
    clip.derivative_id,
    clip.document_id,
    clip.client_id,
    clip.encounter_id,
    clip.note_id,
    clip.kind,
    clip.title,
    clip.body,
    clip.review_status,
    clip.source_method,
    clip.page_range,
    clip.timestamp_range,
    clip.line_range,
    clip.segment_label,
    clip.tags,
    clip.metadata_json,
    clip.archived_at_ms,
    clip.created_at_ms,
    clip.updated_at_ms
FROM workspace_context_clips AS clip
JOIN workspace_artifact_derivatives AS derivative
  ON derivative.id = clip.derivative_id
 AND derivative.document_id = clip.document_id
 AND derivative.client_id = clip.client_id
 AND derivative.archived_at_ms IS NULL
JOIN workspace_documents AS document
  ON document.id = clip.document_id
 AND document.client_id = clip.client_id
 AND document.archived_at_ms IS NULL
WHERE clip.client_id = ?
  AND (? IS NULL OR clip.derivative_id = ?)
  AND (? IS NULL OR clip.document_id = ?)
  AND (? IS NULL OR clip.note_id = ?)
  AND clip.archived_at_ms IS NULL
ORDER BY clip.updated_at_ms DESC, clip.title ASC
LIMIT ?
            "#,
        )
        .bind(filter.client_id)
        .bind(&filter.derivative_id)
        .bind(&filter.derivative_id)
        .bind(&filter.document_id)
        .bind(&filter.document_id)
        .bind(&filter.note_id)
        .bind(&filter.note_id)
        .bind(i64::from(limit))
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.into_iter()
            .map(|row| WorkspaceContextClipRow::try_from_row(&row).and_then(TryInto::try_into))
            .collect()
    }

    pub async fn upsert_context_clip(
        &self,
        input: crate::WorkspaceContextClipUpsert,
    ) -> anyhow::Result<crate::WorkspaceContextClip> {
        let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        validate_task_links(
            &mut tx,
            &input.client_id,
            input.encounter_id.as_deref(),
            input.note_id.as_deref(),
            Some(input.document_id.as_str()),
        )
        .await?;
        let derivative_exists: Option<String> = sqlx::query_scalar(
            r#"
SELECT id
FROM workspace_artifact_derivatives
WHERE id = ?
  AND document_id = ?
  AND client_id = ?
  AND archived_at_ms IS NULL
            "#,
        )
        .bind(&input.derivative_id)
        .bind(&input.document_id)
        .bind(&input.client_id)
        .fetch_optional(&mut *tx)
        .await?;
        if derivative_exists.is_none() {
            anyhow::bail!("workspace context clip derivative link is invalid");
        }
        let existed: Option<String> =
            sqlx::query_scalar("SELECT id FROM workspace_context_clips WHERE id = ?")
                .bind(&id)
                .fetch_optional(&mut *tx)
                .await?;
        let kind = nonempty_or_string(&input.kind, "generic excerpt");
        let review_status = nonempty_or_string(&input.review_status, "draft");
        let source_method = nonempty_or_string(&input.source_method, "human_selected");
        let metadata_json = nonempty_or_string(&input.metadata_json, "{}");
        let row = sqlx::query(
            r#"
INSERT INTO workspace_context_clips (
    id,
    derivative_id,
    document_id,
    client_id,
    encounter_id,
    note_id,
    kind,
    title,
    body,
    review_status,
    source_method,
    page_range,
    timestamp_range,
    line_range,
    segment_label,
    tags,
    metadata_json,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    derivative_id = excluded.derivative_id,
    document_id = excluded.document_id,
    client_id = excluded.client_id,
    encounter_id = excluded.encounter_id,
    note_id = excluded.note_id,
    kind = excluded.kind,
    title = excluded.title,
    body = excluded.body,
    review_status = excluded.review_status,
    source_method = excluded.source_method,
    page_range = excluded.page_range,
    timestamp_range = excluded.timestamp_range,
    line_range = excluded.line_range,
    segment_label = excluded.segment_label,
    tags = excluded.tags,
    metadata_json = excluded.metadata_json,
    archived_at_ms = NULL,
    updated_at_ms = excluded.updated_at_ms
RETURNING
    id,
    derivative_id,
    document_id,
    client_id,
    encounter_id,
    note_id,
    kind,
    title,
    body,
    review_status,
    source_method,
    page_range,
    timestamp_range,
    line_range,
    segment_label,
    tags,
    metadata_json,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
            "#,
        )
        .bind(&id)
        .bind(&input.derivative_id)
        .bind(&input.document_id)
        .bind(&input.client_id)
        .bind(&input.encounter_id)
        .bind(&input.note_id)
        .bind(&kind)
        .bind(&input.title)
        .bind(&input.body)
        .bind(&review_status)
        .bind(&source_method)
        .bind(&input.page_range)
        .bind(&input.timestamp_range)
        .bind(&input.line_range)
        .bind(&input.segment_label)
        .bind(&input.tags)
        .bind(&metadata_json)
        .bind(now_ms)
        .bind(now_ms)
        .fetch_one(&mut *tx)
        .await?;

        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "context_clip".to_string(),
                entity_id: id.clone(),
                action: if existed.is_some() {
                    "updated".to_string()
                } else {
                    "created".to_string()
                },
                actor: input.actor,
                actor_kind: "human".to_string(),
                source: "state".to_string(),
                client_id: Some(input.client_id),
                encounter_id: input.encounter_id,
                note_id: input.note_id,
                document_id: Some(input.document_id),
                success: true,
                summary: input.title,
                metadata_json: Some(format!(
                    r#"{{"derivative_id":"{}","kind":"{}","review_status":"{}","source_method":"{}"}}"#,
                    json_escape(&input.derivative_id),
                    json_escape(&kind),
                    json_escape(&review_status),
                    json_escape(&source_method)
                )),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        tx.commit().await?;
        WorkspaceContextClipRow::try_from_row(&row).and_then(TryInto::try_into)
    }

    pub async fn update_context_clip_status(
        &self,
        input: crate::WorkspaceContextClipStatusUpdate,
    ) -> anyhow::Result<Option<crate::WorkspaceContextClip>> {
        let review_status = input.review_status.trim().to_string();
        if review_status.is_empty() {
            anyhow::bail!("workspace context clip review status must not be empty");
        }
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let existing_row = sqlx::query(
            r#"
SELECT
    id,
    derivative_id,
    document_id,
    client_id,
    encounter_id,
    note_id,
    kind,
    title,
    body,
    review_status,
    source_method,
    page_range,
    timestamp_range,
    line_range,
    segment_label,
    tags,
    metadata_json,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
FROM workspace_context_clips
WHERE id = ?
            "#,
        )
        .bind(&input.clip_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(existing_row) = existing_row else {
            tx.rollback().await?;
            return Ok(None);
        };
        let existing: crate::WorkspaceContextClip =
            WorkspaceContextClipRow::try_from_row(&existing_row)?.try_into()?;
        if existing.review_status == review_status {
            tx.rollback().await?;
            return Ok(Some(existing));
        }
        let archived_at_ms = (review_status == "archived").then_some(now_ms);
        let row = sqlx::query(
            r#"
UPDATE workspace_context_clips
SET review_status = ?, archived_at_ms = ?, updated_at_ms = ?
WHERE id = ?
RETURNING
    id,
    derivative_id,
    document_id,
    client_id,
    encounter_id,
    note_id,
    kind,
    title,
    body,
    review_status,
    source_method,
    page_range,
    timestamp_range,
    line_range,
    segment_label,
    tags,
    metadata_json,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
            "#,
        )
        .bind(&review_status)
        .bind(archived_at_ms)
        .bind(now_ms)
        .bind(&input.clip_id)
        .fetch_one(&mut *tx)
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "context_clip".to_string(),
                entity_id: input.clip_id,
                action: "status_changed".to_string(),
                actor: input.actor,
                actor_kind: "human".to_string(),
                source: "state".to_string(),
                client_id: Some(existing.client_id),
                encounter_id: existing.encounter_id,
                note_id: existing.note_id,
                document_id: Some(existing.document_id),
                success: true,
                summary: format!("{} -> {}", existing.review_status, review_status),
                metadata_json: Some(format!(
                    r#"{{"derivative_id":"{}","from":"{}","to":"{}"}}"#,
                    json_escape(&existing.derivative_id),
                    json_escape(&existing.review_status),
                    json_escape(&review_status)
                )),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        tx.commit().await?;
        WorkspaceContextClipRow::try_from_row(&row)
            .and_then(TryInto::try_into)
            .map(Some)
    }

    pub async fn list_tasks(&self, client_id: &str) -> anyhow::Result<Vec<crate::WorkspaceTask>> {
        let rows = sqlx::query(
            r#"
SELECT
    id,
    client_id,
    encounter_id,
    note_id,
    document_id,
    title,
    details,
    kind,
    status,
    priority,
    due_date,
    assigned_to,
    completed_at_ms,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
FROM workspace_tasks
WHERE client_id = ? AND archived_at_ms IS NULL
ORDER BY
    CASE status
        WHEN 'open' THEN 0
        WHEN 'in_progress' THEN 1
        WHEN 'blocked' THEN 2
        WHEN 'done' THEN 3
        WHEN 'canceled' THEN 4
        ELSE 5
    END,
    CASE WHEN due_date IS NULL OR due_date = '' THEN 1 ELSE 0 END,
    due_date ASC,
    updated_at_ms DESC,
    title ASC
            "#,
        )
        .bind(client_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.into_iter()
            .map(|row| WorkspaceTaskRow::try_from_row(&row).and_then(TryInto::try_into))
            .collect()
    }

    pub async fn list_open_tasks(
        &self,
        client_id: &str,
    ) -> anyhow::Result<Vec<crate::WorkspaceTask>> {
        Ok(self
            .list_tasks(client_id)
            .await?
            .into_iter()
            .filter(|task| task.status.is_active())
            .collect())
    }

    pub async fn upsert_task(
        &self,
        input: crate::WorkspaceTaskUpsert,
    ) -> anyhow::Result<crate::WorkspaceTask> {
        let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        validate_task_links(
            &mut tx,
            &input.client_id,
            input.encounter_id.as_deref(),
            input.note_id.as_deref(),
            input.document_id.as_deref(),
        )
        .await?;
        let existing: Option<(String, Option<i64>)> =
            sqlx::query_as("SELECT client_id, completed_at_ms FROM workspace_tasks WHERE id = ?")
                .bind(&id)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some((existing_client_id, _)) = &existing
            && existing_client_id != &input.client_id
        {
            tx.rollback().await?;
            anyhow::bail!(
                "workspace task `{id}` was not found for client `{}`",
                input.client_id
            );
        }
        let completed_at_ms = if input.status == crate::WorkspaceTaskStatus::Done {
            existing
                .as_ref()
                .and_then(|(_, completed_at_ms)| *completed_at_ms)
                .or(Some(now_ms))
        } else {
            None
        };
        let row = sqlx::query(
            r#"
INSERT INTO workspace_tasks (
    id,
    client_id,
    encounter_id,
    note_id,
    document_id,
    title,
    details,
    kind,
    status,
    priority,
    due_date,
    assigned_to,
    completed_at_ms,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    encounter_id = excluded.encounter_id,
    note_id = excluded.note_id,
    document_id = excluded.document_id,
    title = excluded.title,
    details = excluded.details,
    kind = excluded.kind,
    status = excluded.status,
    priority = excluded.priority,
    due_date = excluded.due_date,
    assigned_to = excluded.assigned_to,
    completed_at_ms = excluded.completed_at_ms,
    archived_at_ms = NULL,
    updated_at_ms = excluded.updated_at_ms
RETURNING
    id,
    client_id,
    encounter_id,
    note_id,
    document_id,
    title,
    details,
    kind,
    status,
    priority,
    due_date,
    assigned_to,
    completed_at_ms,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
            "#,
        )
        .bind(&id)
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
        .fetch_one(&mut *tx)
        .await?;

        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "task".to_string(),
                entity_id: id.clone(),
                action: if existing.is_some() {
                    "updated".to_string()
                } else {
                    "created".to_string()
                },
                actor: input.actor,
                actor_kind: "human".to_string(),
                source: "state".to_string(),
                client_id: Some(input.client_id),
                encounter_id: input.encounter_id,
                note_id: input.note_id,
                document_id: input.document_id,
                success: true,
                summary: input.title,
                metadata_json: Some(format!(
                    r#"{{"status":"{}","priority":"{}"}}"#,
                    input.status.as_str(),
                    input.priority.as_str()
                )),
                ..Default::default()
            },
            now_ms,
        )
        .await?;

        tx.commit().await?;
        WorkspaceTaskRow::try_from_row(&row).and_then(TryInto::try_into)
    }

    pub async fn update_task_status(
        &self,
        input: crate::WorkspaceTaskStatusUpdate,
    ) -> anyhow::Result<Option<crate::WorkspaceTask>> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let existing_row = sqlx::query(
            r#"
SELECT
    id,
    client_id,
    encounter_id,
    note_id,
    document_id,
    title,
    details,
    kind,
    status,
    priority,
    due_date,
    assigned_to,
    completed_at_ms,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
FROM workspace_tasks
WHERE id = ? AND client_id = ? AND archived_at_ms IS NULL
            "#,
        )
        .bind(&input.task_id)
        .bind(&input.client_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(existing_row) = existing_row else {
            tx.rollback().await?;
            return Ok(None);
        };
        let existing_task: crate::WorkspaceTask =
            WorkspaceTaskRow::try_from_row(&existing_row)?.try_into()?;
        if existing_task.status == input.status {
            tx.rollback().await?;
            return Ok(Some(existing_task));
        }
        let completed_at_ms = if input.status == crate::WorkspaceTaskStatus::Done {
            Some(now_ms)
        } else {
            None
        };
        let row = sqlx::query(
            r#"
UPDATE workspace_tasks
SET status = ?, completed_at_ms = ?, updated_at_ms = ?
WHERE id = ? AND client_id = ? AND archived_at_ms IS NULL
RETURNING
    id,
    client_id,
    encounter_id,
    note_id,
    document_id,
    title,
    details,
    kind,
    status,
    priority,
    due_date,
    assigned_to,
    completed_at_ms,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
            "#,
        )
        .bind(input.status.as_str())
        .bind(completed_at_ms)
        .bind(now_ms)
        .bind(&input.task_id)
        .bind(&input.client_id)
        .fetch_one(&mut *tx)
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "task".to_string(),
                entity_id: input.task_id,
                action: "status_changed".to_string(),
                actor: input.actor,
                actor_kind: "human".to_string(),
                source: "state".to_string(),
                client_id: Some(input.client_id),
                encounter_id: existing_task.encounter_id,
                note_id: existing_task.note_id,
                document_id: existing_task.document_id,
                success: true,
                summary: format!(
                    "{} -> {}",
                    existing_task.status.as_str(),
                    input.status.as_str()
                ),
                metadata_json: Some(format!(
                    r#"{{"from":"{}","to":"{}"}}"#,
                    existing_task.status.as_str(),
                    input.status.as_str()
                )),
                ..Default::default()
            },
            now_ms,
        )
        .await?;

        tx.commit().await?;
        WorkspaceTaskRow::try_from_row(&row)
            .and_then(TryInto::try_into)
            .map(Some)
    }

    pub async fn list_notes(&self, client_id: &str) -> anyhow::Result<Vec<crate::WorkspaceNote>> {
        let rows = sqlx::query(
            r#"
SELECT
    id,
    client_id,
    encounter_id,
    title,
    kind,
    body,
    status,
    current_revision,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
FROM workspace_notes
WHERE client_id = ? AND archived_at_ms IS NULL
ORDER BY updated_at_ms DESC, title ASC
            "#,
        )
        .bind(client_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.into_iter()
            .map(|row| WorkspaceNoteRow::try_from_row(&row).and_then(TryInto::try_into))
            .collect()
    }

    pub async fn get_note(&self, id: &str) -> anyhow::Result<Option<crate::WorkspaceNote>> {
        let row = sqlx::query(
            r#"
SELECT
    id,
    client_id,
    encounter_id,
    title,
    kind,
    body,
    status,
    current_revision,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
FROM workspace_notes
WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(self.pool.as_ref())
        .await?;

        row.map(|row| WorkspaceNoteRow::try_from_row(&row).and_then(TryInto::try_into))
            .transpose()
    }

    pub async fn upsert_note(
        &self,
        input: crate::WorkspaceNoteUpsert,
    ) -> anyhow::Result<crate::WorkspaceNote> {
        let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        if note_status_is_locked(&input.status) {
            anyhow::bail!(
                "workspace note status `{}` requires a dedicated workflow",
                input.status
            );
        }
        let existing_note: Option<(i64, String)> =
            sqlx::query_as("SELECT current_revision, status FROM workspace_notes WHERE id = ?")
                .bind(&id)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some((_, status)) = &existing_note
            && note_status_is_locked(status)
        {
            anyhow::bail!("signed workspace notes require an addendum instead of direct edits");
        }
        let existing_revision = existing_note.as_ref().map(|(revision, _)| *revision);
        let next_revision = existing_revision.unwrap_or(0) + 1;
        let row = sqlx::query(
            r#"
INSERT INTO workspace_notes (
    id,
    client_id,
    encounter_id,
    title,
    kind,
    body,
    status,
    current_revision,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
ON CONFLICT(id) DO UPDATE SET
    client_id = excluded.client_id,
    encounter_id = excluded.encounter_id,
    title = excluded.title,
    kind = excluded.kind,
    body = excluded.body,
    status = excluded.status,
    current_revision = excluded.current_revision,
    archived_at_ms = NULL,
    updated_at_ms = excluded.updated_at_ms
RETURNING
    id,
    client_id,
    encounter_id,
    title,
    kind,
    body,
    status,
    current_revision,
    archived_at_ms,
    created_at_ms,
    updated_at_ms
            "#,
        )
        .bind(&id)
        .bind(&input.client_id)
        .bind(&input.encounter_id)
        .bind(&input.title)
        .bind(&input.kind)
        .bind(&input.body)
        .bind(&input.status)
        .bind(next_revision)
        .bind(now_ms)
        .bind(now_ms)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            r#"
INSERT INTO workspace_note_revisions (
    note_id,
    revision,
    body,
    actor,
    source_thread_id,
    source_turn_id,
    summary,
    created_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(next_revision)
        .bind(&input.body)
        .bind(&input.actor)
        .bind(&input.source_thread_id)
        .bind(&input.source_turn_id)
        .bind(&input.summary)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;

        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "note".to_string(),
                entity_id: id.clone(),
                action: if existing_revision.is_some() {
                    "updated".to_string()
                } else {
                    "created".to_string()
                },
                actor: input.actor,
                actor_kind: "human".to_string(),
                source: "state".to_string(),
                client_id: Some(input.client_id),
                encounter_id: input.encounter_id,
                note_id: Some(id),
                source_thread_id: input.source_thread_id,
                source_turn_id: input.source_turn_id,
                success: true,
                summary: input.summary.unwrap_or_default(),
                ..Default::default()
            },
            now_ms,
        )
        .await?;

        tx.commit().await?;
        WorkspaceNoteRow::try_from_row(&row).and_then(TryInto::try_into)
    }

    pub async fn archive_note(&self, id: &str, actor: &str) -> anyhow::Result<bool> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let existing: Option<(String, Option<String>, String)> = sqlx::query_as(
            "SELECT client_id, encounter_id, title FROM workspace_notes WHERE id = ? AND archived_at_ms IS NULL",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((client_id, encounter_id, title)) = existing else {
            tx.rollback().await?;
            return Ok(false);
        };
        let result = sqlx::query(
            r#"
UPDATE workspace_notes
SET archived_at_ms = ?, updated_at_ms = ?
WHERE id = ? AND archived_at_ms IS NULL
            "#,
        )
        .bind(now_ms)
        .bind(now_ms)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() > 0 {
            insert_audit_event(
                &mut tx,
                crate::WorkspaceAuditEventCreate {
                    entity_type: "note".to_string(),
                    entity_id: id.to_string(),
                    action: "archived".to_string(),
                    actor: actor.to_string(),
                    actor_kind: "human".to_string(),
                    source: "state".to_string(),
                    client_id: Some(client_id),
                    encounter_id,
                    note_id: Some(id.to_string()),
                    success: true,
                    summary: title,
                    ..Default::default()
                },
                now_ms,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn sign_note(
        &self,
        input: crate::WorkspaceNoteSign,
    ) -> anyhow::Result<crate::WorkspaceNoteSignature> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let note: Option<(String, Option<String>, String, String, i64)> = sqlx::query_as(
            "SELECT client_id, encounter_id, status, body, current_revision FROM workspace_notes WHERE id = ? AND archived_at_ms IS NULL",
        )
        .bind(&input.note_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((client_id, encounter_id, status, body, revision)) = note else {
            tx.rollback().await?;
            anyhow::bail!("workspace note `{}` was not found", input.note_id);
        };
        if note_status_is_locked(&status) {
            tx.rollback().await?;
            anyhow::bail!("workspace note `{}` is already signed", input.note_id);
        }
        let body_sha256 = format!("{:x}", Sha256::digest(body.as_bytes()));
        let id = Uuid::new_v4().to_string();
        let row = sqlx::query(
            r#"
INSERT INTO workspace_note_signatures (
    id,
    note_id,
    revision,
    signer,
    body_sha256,
    signed_at_ms
) VALUES (?, ?, ?, ?, ?, ?)
RETURNING
    id,
    note_id,
    revision,
    signer,
    body_sha256,
    signed_at_ms
            "#,
        )
        .bind(&id)
        .bind(&input.note_id)
        .bind(revision)
        .bind(&input.signer)
        .bind(&body_sha256)
        .bind(now_ms)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            r#"
UPDATE workspace_notes
SET status = ?, updated_at_ms = ?
WHERE id = ?
            "#,
        )
        .bind(NOTE_STATUS_SIGNED)
        .bind(now_ms)
        .bind(&input.note_id)
        .execute(&mut *tx)
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "note".to_string(),
                entity_id: input.note_id.clone(),
                action: "signed".to_string(),
                actor: input.signer,
                actor_kind: "human".to_string(),
                source: "state".to_string(),
                client_id: Some(client_id),
                encounter_id,
                note_id: Some(input.note_id),
                success: true,
                summary: format!("signed revision {revision}"),
                metadata_json: Some(format!(
                    r#"{{"revision":{revision},"body_sha256":"{body_sha256}"}}"#
                )),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        tx.commit().await?;
        WorkspaceNoteSignatureRow::try_from_row(&row).and_then(TryInto::try_into)
    }

    pub async fn list_note_signatures(
        &self,
        note_id: &str,
    ) -> anyhow::Result<Vec<crate::WorkspaceNoteSignature>> {
        let rows = sqlx::query(
            r#"
SELECT
    id,
    note_id,
    revision,
    signer,
    body_sha256,
    signed_at_ms
FROM workspace_note_signatures
WHERE note_id = ?
ORDER BY signed_at_ms DESC
            "#,
        )
        .bind(note_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.into_iter()
            .map(|row| WorkspaceNoteSignatureRow::try_from_row(&row).and_then(TryInto::try_into))
            .collect()
    }

    pub async fn create_note_addendum(
        &self,
        input: crate::WorkspaceNoteAddendumCreate,
    ) -> anyhow::Result<crate::WorkspaceNoteAddendum> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let note: Option<(String, Option<String>, String, i64)> = sqlx::query_as(
            "SELECT client_id, encounter_id, status, current_revision FROM workspace_notes WHERE id = ? AND archived_at_ms IS NULL",
        )
        .bind(&input.note_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((client_id, encounter_id, status, current_revision)) = note else {
            tx.rollback().await?;
            anyhow::bail!("workspace note `{}` was not found", input.note_id);
        };
        if !note_status_is_locked(&status) {
            tx.rollback().await?;
            anyhow::bail!(
                "workspace note `{}` must be signed before addenda",
                input.note_id
            );
        }
        if current_revision != input.base_revision {
            tx.rollback().await?;
            anyhow::bail!(
                "cannot add addendum based on revision {} because note is at revision {}",
                input.base_revision,
                current_revision
            );
        }
        let signed_revision: Option<String> = sqlx::query_scalar(
            "SELECT id FROM workspace_note_signatures WHERE note_id = ? AND revision = ?",
        )
        .bind(&input.note_id)
        .bind(input.base_revision)
        .fetch_optional(&mut *tx)
        .await?;
        if signed_revision.is_none() {
            tx.rollback().await?;
            anyhow::bail!(
                "cannot add addendum because revision {} is not signed",
                input.base_revision
            );
        }
        let id = Uuid::new_v4().to_string();
        let row = sqlx::query(
            r#"
INSERT INTO workspace_note_addenda (
    id,
    note_id,
    base_revision,
    body,
    author,
    source_thread_id,
    source_turn_id,
    created_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
RETURNING
    id,
    note_id,
    base_revision,
    body,
    author,
    source_thread_id,
    source_turn_id,
    created_at_ms
            "#,
        )
        .bind(&id)
        .bind(&input.note_id)
        .bind(input.base_revision)
        .bind(&input.body)
        .bind(&input.author)
        .bind(&input.source_thread_id)
        .bind(&input.source_turn_id)
        .bind(now_ms)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            r#"
UPDATE workspace_notes
SET status = ?, updated_at_ms = ?
WHERE id = ?
            "#,
        )
        .bind(NOTE_STATUS_ADDENDED)
        .bind(now_ms)
        .bind(&input.note_id)
        .execute(&mut *tx)
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "note_addendum".to_string(),
                entity_id: id,
                action: "created".to_string(),
                actor: input.author,
                actor_kind: "human".to_string(),
                source: "state".to_string(),
                client_id: Some(client_id),
                encounter_id,
                note_id: Some(input.note_id),
                source_thread_id: input.source_thread_id,
                source_turn_id: input.source_turn_id,
                success: true,
                summary: format!("addendum on revision {}", input.base_revision),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        tx.commit().await?;
        WorkspaceNoteAddendumRow::try_from_row(&row).and_then(TryInto::try_into)
    }

    pub async fn list_note_addenda(
        &self,
        note_id: &str,
    ) -> anyhow::Result<Vec<crate::WorkspaceNoteAddendum>> {
        let rows = sqlx::query(
            r#"
SELECT
    id,
    note_id,
    base_revision,
    body,
    author,
    source_thread_id,
    source_turn_id,
    created_at_ms
FROM workspace_note_addenda
WHERE note_id = ?
ORDER BY created_at_ms DESC
            "#,
        )
        .bind(note_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.into_iter()
            .map(|row| WorkspaceNoteAddendumRow::try_from_row(&row).and_then(TryInto::try_into))
            .collect()
    }

    pub async fn create_note_proposal(
        &self,
        mut input: crate::WorkspaceNoteProposalCreate,
    ) -> anyhow::Result<crate::WorkspaceNoteProposal> {
        let id = Uuid::new_v4().to_string();
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let note: Option<(String, Option<String>, String, i64)> = sqlx::query_as(
            "SELECT client_id, encounter_id, status, current_revision FROM workspace_notes WHERE id = ? AND archived_at_ms IS NULL",
        )
        .bind(&input.note_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((client_id, encounter_id, note_status, current_revision)) = note else {
            tx.rollback().await?;
            anyhow::bail!("workspace note `{}` was not found", input.note_id);
        };
        if note_status_is_locked(&note_status) {
            tx.rollback().await?;
            anyhow::bail!(
                "signed workspace notes require an addendum instead of replacement proposals"
            );
        }
        if let Some(result_id) = input.agent_result_id.as_deref() {
            let linked = sqlx::query(
                r#"
SELECT
    result.client_id AS result_client_id,
    result.note_id AS result_note_id,
    result.base_note_revision AS result_base_note_revision,
    result.packet_id AS result_packet_id,
    result.packet_context_sha256 AS result_packet_context_sha256,
    run.id AS run_id,
    run.packet_id AS run_packet_id,
    run.client_id AS run_client_id,
    run.note_id AS run_note_id,
    run.base_note_revision AS run_base_note_revision,
    run.context_envelope_sha256 AS run_context_envelope_sha256,
    run.source_thread_id AS run_source_thread_id,
    run.source_turn_id AS run_source_turn_id
FROM workspace_agent_results AS result
LEFT JOIN workspace_agent_runs AS run ON run.id = result.run_id
WHERE result.id = ?
                "#,
            )
            .bind(result_id)
            .fetch_optional(&mut *tx)
            .await?;
            let Some(linked) = linked else {
                anyhow::bail!("workspace agent result `{result_id}` was not found");
            };
            let result_client_id: String = linked.try_get("result_client_id")?;
            let result_note_id: Option<String> = linked.try_get("result_note_id")?;
            let result_base_revision: Option<i64> = linked.try_get("result_base_note_revision")?;
            let result_packet_id: String = linked.try_get("result_packet_id")?;
            let result_packet_hash: String = linked.try_get("result_packet_context_sha256")?;
            let run_id: Option<String> = linked.try_get("run_id")?;
            let run_packet_id: Option<String> = linked.try_get("run_packet_id")?;
            let run_client_id: Option<String> = linked.try_get("run_client_id")?;
            let run_note_id: Option<String> = linked.try_get("run_note_id")?;
            let run_base_revision: Option<i64> = linked.try_get("run_base_note_revision")?;
            let run_packet_hash: Option<String> = linked.try_get("run_context_envelope_sha256")?;
            let source_thread_id: Option<String> = linked.try_get("run_source_thread_id")?;
            let source_turn_id: Option<String> = linked.try_get("run_source_turn_id")?;
            let result_base_revision = result_base_revision.ok_or_else(|| {
                anyhow::anyhow!(
                    "workspace agent result `{result_id}` has no durable base note revision"
                )
            })?;
            if result_client_id != client_id
                || result_note_id.as_deref() != Some(input.note_id.as_str())
            {
                anyhow::bail!(
                    "workspace agent result `{result_id}` does not belong to note `{}`",
                    input.note_id
                );
            }
            let run_id = run_id.ok_or_else(|| {
                anyhow::anyhow!(
                    "workspace agent result `{result_id}` is not linked to a durable run"
                )
            })?;
            if run_packet_id.as_deref() != Some(result_packet_id.as_str())
                || run_client_id.as_deref() != Some(result_client_id.as_str())
                || run_note_id != result_note_id
                || run_base_revision != Some(result_base_revision)
                || run_packet_hash.as_deref() != Some(result_packet_hash.as_str())
            {
                anyhow::bail!(
                    "workspace agent result `{result_id}` provenance does not match run `{run_id}`"
                );
            }
            input.base_revision = result_base_revision;
            if input.source_thread_id.is_none() {
                input.source_thread_id = source_thread_id;
            }
            if input.source_turn_id.is_none() {
                input.source_turn_id = source_turn_id.or_else(|| Some(result_id.to_string()));
            }
        } else if current_revision != input.base_revision {
            tx.rollback().await?;
            anyhow::bail!(
                "cannot create proposal based on revision {} because note is at revision {}",
                input.base_revision,
                current_revision
            );
        }
        let row = sqlx::query(
            r#"
INSERT INTO workspace_note_proposals (
    id,
    note_id,
    base_revision,
    agent_result_id,
    proposed_body,
    summary,
    status,
    source_thread_id,
    source_turn_id,
    created_at_ms,
    resolved_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)
RETURNING
    id,
    note_id,
    base_revision,
    agent_result_id,
    proposed_body,
    summary,
    status,
    source_thread_id,
    source_turn_id,
    created_at_ms,
    resolved_at_ms
            "#,
        )
        .bind(&id)
        .bind(&input.note_id)
        .bind(input.base_revision)
        .bind(&input.agent_result_id)
        .bind(&input.proposed_body)
        .bind(&input.summary)
        .bind(crate::WorkspaceNoteProposalStatus::Pending.as_str())
        .bind(&input.source_thread_id)
        .bind(&input.source_turn_id)
        .bind(now_ms)
        .fetch_one(&mut *tx)
        .await?;
        if let Some(result_id) = input.agent_result_id.as_deref() {
            sqlx::query(
                "UPDATE workspace_agent_results SET status = 'converted', updated_at_ms = ? WHERE id = ?",
            )
            .bind(now_ms)
            .bind(result_id)
            .execute(&mut *tx)
            .await?;
        }
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "note_proposal".to_string(),
                entity_id: id.clone(),
                action: "created".to_string(),
                actor: "agent".to_string(),
                actor_kind: "agent".to_string(),
                source: "state".to_string(),
                client_id: Some(client_id),
                encounter_id,
                note_id: Some(input.note_id),
                source_thread_id: input.source_thread_id,
                source_turn_id: input.source_turn_id,
                success: true,
                summary: input.summary,
                metadata_json: input
                    .agent_result_id
                    .map(|result_id| format!(r#"{{"agent_result_id":"{result_id}"}}"#)),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        tx.commit().await?;
        WorkspaceNoteProposalRow::try_from_row(&row).and_then(TryInto::try_into)
    }

    pub async fn create_note_proposal_from_agent_result(
        &self,
        input: crate::WorkspaceNoteProposalCreate,
    ) -> anyhow::Result<crate::WorkspaceNoteProposal> {
        if input.agent_result_id.is_none() {
            anyhow::bail!("linked workspace note proposal requires an agent result id");
        }
        self.create_note_proposal(input).await
    }

    pub async fn list_note_proposals(
        &self,
        note_id: &str,
    ) -> anyhow::Result<Vec<crate::WorkspaceNoteProposal>> {
        let rows = sqlx::query(
            r#"
SELECT
    id,
    note_id,
    base_revision,
    agent_result_id,
    proposed_body,
    summary,
    status,
    source_thread_id,
    source_turn_id,
    created_at_ms,
    resolved_at_ms
FROM workspace_note_proposals
WHERE note_id = ?
ORDER BY created_at_ms DESC
            "#,
        )
        .bind(note_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.into_iter()
            .map(|row| WorkspaceNoteProposalRow::try_from_row(&row).and_then(TryInto::try_into))
            .collect()
    }

    pub async fn resolve_note_proposal(
        &self,
        proposal_id: &str,
        accept: bool,
        actor: &str,
    ) -> anyhow::Result<Option<crate::WorkspaceNoteProposal>> {
        let resolution = if accept {
            crate::WorkspaceNoteProposalResolution::Accept
        } else {
            crate::WorkspaceNoteProposalResolution::Decline
        };
        self.resolve_note_proposal_with(crate::WorkspaceNoteProposalResolve {
            proposal_id: proposal_id.to_string(),
            resolution,
            actor: actor.to_string(),
            reason: String::new(),
        })
        .await
    }

    pub async fn create_context_packet(
        &self,
        input: crate::WorkspaceContextPacketCreate,
    ) -> anyhow::Result<crate::WorkspaceContextPacket> {
        let id = Uuid::new_v4().to_string();
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        validate_packet_links(
            &mut tx,
            &input.client_id,
            input.encounter_id.as_deref(),
            input.note_id.as_deref(),
        )
        .await?;
        validate_packet_selected_context(
            &mut tx,
            &input.client_id,
            &input.selected_artifact_ids_json,
            &input.selected_derivative_ids_json,
            &input.selected_clip_ids_json,
        )
        .await?;
        validate_packet_context_envelope(&input)?;
        let base_note_revision = resolve_packet_base_note_revision(&mut tx, &input).await?;
        let context_envelope_sha256 = context_envelope_sha256(&input.context_envelope_json);
        let status = if input.status.trim().is_empty() {
            "prepared".to_string()
        } else {
            input.status.trim().to_string()
        };
        if !matches!(
            status.as_str(),
            "prepared" | "submitted" | "canceled" | "sent" | "result_saved"
        ) {
            anyhow::bail!("unsupported workspace context packet status `{status}`");
        }
        let clinician_actor = if input.actor.trim().is_empty() {
            "local human".to_string()
        } else {
            input.actor.trim().to_string()
        };
        let authorized_scope_json = if input.authorized_scope_json.trim().is_empty() {
            legacy_packet_authorized_scope_json(&input)
        } else {
            let value: serde_json::Value = serde_json::from_str(&input.authorized_scope_json)
                .map_err(|err| {
                    anyhow::anyhow!(
                        "workspace context packet authorized scope must be valid JSON: {err}"
                    )
                })?;
            if !value.is_object() {
                anyhow::bail!("workspace context packet authorized scope must be a JSON object");
            }
            validate_agent_visible_json("context packet authorized scope", &value)?;
            input.authorized_scope_json.trim().to_string()
        };
        let expected_output_kind = if input.expected_output_kind.trim().is_empty() {
            "recommendation".to_string()
        } else {
            input.expected_output_kind.trim().to_string()
        };
        let submitted_at_ms =
            matches!(status.as_str(), "submitted" | "sent" | "result_saved").then_some(now_ms);
        let canceled_at_ms = (status == "canceled").then_some(now_ms);
        let row = sqlx::query(
            r#"
INSERT INTO workspace_context_packets (
    id,
    client_id,
    encounter_id,
    note_id,
    human_request,
    selected_artifact_ids_json,
    selected_derivative_ids_json,
    selected_clip_ids_json,
    artifact_summary,
    derivative_summary,
    clip_summary,
    chart_context_summary,
    context_envelope_json,
    context_envelope_sha256,
    clinician_actor,
    base_note_revision,
    authorized_scope_json,
    expected_output_kind,
    status,
    created_at_ms,
    sent_at_ms,
    submitted_at_ms,
    canceled_at_ms,
    updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
RETURNING
    id,
    client_id,
    encounter_id,
    note_id,
    human_request,
    selected_artifact_ids_json,
    selected_derivative_ids_json,
    selected_clip_ids_json,
    artifact_summary,
    derivative_summary,
    clip_summary,
    chart_context_summary,
    context_envelope_json,
    context_envelope_sha256,
    clinician_actor,
    base_note_revision,
    authorized_scope_json,
    expected_output_kind,
    status,
    created_at_ms,
    sent_at_ms,
    submitted_at_ms,
    canceled_at_ms,
    updated_at_ms
            "#,
        )
        .bind(&id)
        .bind(&input.client_id)
        .bind(&input.encounter_id)
        .bind(&input.note_id)
        .bind(&input.human_request)
        .bind(&input.selected_artifact_ids_json)
        .bind(&input.selected_derivative_ids_json)
        .bind(&input.selected_clip_ids_json)
        .bind(&input.artifact_summary)
        .bind(&input.derivative_summary)
        .bind(&input.clip_summary)
        .bind(&input.chart_context_summary)
        .bind(&input.context_envelope_json)
        .bind(&context_envelope_sha256)
        .bind(&clinician_actor)
        .bind(base_note_revision)
        .bind(&authorized_scope_json)
        .bind(&expected_output_kind)
        .bind(&status)
        .bind(now_ms)
        .bind(now_ms)
        .bind(submitted_at_ms)
        .bind(canceled_at_ms)
        .bind(now_ms)
        .fetch_one(&mut *tx)
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "context_packet".to_string(),
                entity_id: id.clone(),
                action: status.clone(),
                actor: clinician_actor,
                actor_kind: "human".to_string(),
                source: "state".to_string(),
                client_id: Some(input.client_id),
                encounter_id: input.encounter_id,
                note_id: input.note_id,
                success: true,
                summary: input.chart_context_summary,
                metadata_json: Some(format!(
                    r#"{{"artifact_summary":"{}","derivative_summary":"{}","clip_summary":"{}","context_envelope":"present"}}"#,
                    json_escape(&input.artifact_summary),
                    json_escape(&input.derivative_summary),
                    json_escape(&input.clip_summary)
                )),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        tx.commit().await?;
        WorkspaceContextPacketRow::try_from_row(&row).and_then(TryInto::try_into)
    }

    pub async fn list_context_packets(
        &self,
        filter: crate::WorkspaceContextPacketFilter,
    ) -> anyhow::Result<Vec<crate::WorkspaceContextPacket>> {
        let limit = filter.limit.unwrap_or(20).clamp(1, 100);
        let rows = sqlx::query(
            r#"
SELECT
    id,
    client_id,
    encounter_id,
    note_id,
    human_request,
    selected_artifact_ids_json,
    selected_derivative_ids_json,
    selected_clip_ids_json,
    artifact_summary,
    derivative_summary,
    clip_summary,
    chart_context_summary,
    context_envelope_json,
    context_envelope_sha256,
    clinician_actor,
    base_note_revision,
    authorized_scope_json,
    expected_output_kind,
    status,
    created_at_ms,
    sent_at_ms,
    submitted_at_ms,
    canceled_at_ms,
    updated_at_ms
FROM workspace_context_packets
WHERE client_id = ?
  AND (? IS NULL OR note_id = ?)
ORDER BY COALESCE(submitted_at_ms, created_at_ms) DESC
LIMIT ?
            "#,
        )
        .bind(filter.client_id)
        .bind(&filter.note_id)
        .bind(&filter.note_id)
        .bind(i64::from(limit))
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.into_iter()
            .map(|row| WorkspaceContextPacketRow::try_from_row(&row).and_then(TryInto::try_into))
            .collect()
    }

    pub async fn get_context_packet_replay(
        &self,
        filter: crate::WorkspaceContextPacketReplayFilter,
    ) -> anyhow::Result<Option<crate::WorkspaceContextPacket>> {
        let expected_hash = filter.context_envelope_sha256.trim().to_string();
        // Agent-visible packet replay is intentionally limited to the stored packet row.
        // Do not join current artifact, derivative, clip, task, or note rows here.
        let row = sqlx::query(
            r#"
SELECT
    id,
    client_id,
    encounter_id,
    note_id,
    human_request,
    selected_artifact_ids_json,
    selected_derivative_ids_json,
    selected_clip_ids_json,
    artifact_summary,
    derivative_summary,
    clip_summary,
    chart_context_summary,
    context_envelope_json,
    context_envelope_sha256,
    clinician_actor,
    base_note_revision,
    authorized_scope_json,
    expected_output_kind,
    status,
    created_at_ms,
    sent_at_ms,
    submitted_at_ms,
    canceled_at_ms,
    updated_at_ms
FROM workspace_context_packets
WHERE id = ?
  AND client_id = ?
  AND status IN ('submitted', 'sent', 'result_saved')
LIMIT 1
            "#,
        )
        .bind(filter.packet_id)
        .bind(filter.client_id)
        .fetch_optional(self.pool.as_ref())
        .await?;

        let packet = row
            .map(|row| WorkspaceContextPacketRow::try_from_row(&row).and_then(TryInto::try_into))
            .transpose()?;
        if !expected_hash.is_empty()
            && packet
                .as_ref()
                .is_some_and(|packet: &crate::WorkspaceContextPacket| {
                    packet.context_envelope_sha256 != expected_hash
                })
        {
            return Ok(None);
        }
        Ok(packet)
    }

    pub async fn create_agent_result(
        &self,
        mut input: crate::WorkspaceAgentResultCreate,
    ) -> anyhow::Result<crate::WorkspaceAgentResult> {
        if input.run_id.is_none() {
            let run = self
                .start_agent_run(crate::WorkspaceAgentRunStart {
                    packet_id: input.packet_id.clone(),
                    expected_client_id: input.expected_client_id.clone().unwrap_or_default(),
                    expected_context_envelope_sha256: input
                        .expected_context_envelope_sha256
                        .clone(),
                    run_kind: "manual_import".to_string(),
                    idempotency_key: format!("manual-import:{}", Uuid::new_v4()),
                    provider: "manual".to_string(),
                    model: String::new(),
                    source_thread_id: None,
                    source_turn_id: None,
                    actor: input.actor.clone(),
                })
                .await?;
            input.run_id = Some(run.id);
        }
        self.complete_agent_run_with_result(input).await
    }

    pub async fn update_agent_result_status(
        &self,
        input: crate::WorkspaceAgentResultStatusUpdate,
    ) -> anyhow::Result<Option<crate::WorkspaceAgentResult>> {
        let status = input.status.trim().to_string();
        if status.is_empty() {
            anyhow::bail!("workspace agent result status must not be empty");
        }
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let existing_row = sqlx::query(
            r#"
SELECT
    r.id,
    r.packet_id,
    r.client_id,
    r.note_id,
    r.run_id,
    r.base_note_revision,
    p.context_envelope_sha256,
    COALESCE(NULLIF(r.packet_context_sha256, ''), p.context_envelope_sha256)
        AS packet_context_sha256,
    r.body,
    r.summary,
    r.result_kind,
    r.structured_changes_json,
    r.rationale_summary,
    r.status,
    r.created_at_ms,
    r.updated_at_ms
FROM workspace_agent_results r
JOIN workspace_context_packets p ON p.id = r.packet_id
WHERE r.id = ?
            "#,
        )
        .bind(&input.result_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(existing_row) = existing_row else {
            tx.rollback().await?;
            return Ok(None);
        };
        let existing_result: crate::WorkspaceAgentResult =
            WorkspaceAgentResultRow::try_from_row(&existing_row)?.try_into()?;
        if existing_result.status == status {
            tx.rollback().await?;
            return Ok(Some(existing_result));
        }
        let transition_allowed = matches!(
            (existing_result.status.as_str(), status.as_str()),
            ("review_pending", "reviewed" | "dismissed") | ("reviewed", "dismissed")
        );
        if !transition_allowed {
            anyhow::bail!(
                "workspace agent result cannot transition from `{}` to `{status}`",
                existing_result.status
            );
        }
        sqlx::query(
            r#"
UPDATE workspace_agent_results
SET status = ?, updated_at_ms = ?
WHERE id = ?
            "#,
        )
        .bind(&status)
        .bind(now_ms)
        .bind(&input.result_id)
        .execute(&mut *tx)
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "agent_result".to_string(),
                entity_id: input.result_id.clone(),
                action: "status_changed".to_string(),
                actor: input.actor,
                actor_kind: "human".to_string(),
                source: "state".to_string(),
                client_id: Some(existing_result.client_id),
                note_id: existing_result.note_id,
                source_turn_id: Some(existing_result.id),
                success: true,
                summary: format!("{} -> {}", existing_result.status, status),
                metadata_json: Some(format!(
                    r#"{{"from":"{}","to":"{}","packet_id":"{}","context_envelope_sha256":"{}"}}"#,
                    json_escape(&existing_result.status),
                    json_escape(&status),
                    json_escape(&existing_result.packet_id),
                    json_escape(&existing_result.context_envelope_sha256)
                )),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        let row = workspace_agent_result_row_by_id(&mut tx, &input.result_id).await?;
        tx.commit().await?;
        WorkspaceAgentResultRow::try_from_row(&row)
            .and_then(TryInto::try_into)
            .map(Some)
    }

    pub async fn list_agent_results(
        &self,
        filter: crate::WorkspaceAgentResultFilter,
    ) -> anyhow::Result<Vec<crate::WorkspaceAgentResult>> {
        let limit = filter.limit.unwrap_or(20).clamp(1, 100);
        let rows = sqlx::query(
            r#"
SELECT
    r.id,
    r.packet_id,
    r.client_id,
    r.note_id,
    r.run_id,
    r.base_note_revision,
    p.context_envelope_sha256,
    COALESCE(NULLIF(r.packet_context_sha256, ''), p.context_envelope_sha256)
        AS packet_context_sha256,
    r.body,
    r.summary,
    r.result_kind,
    r.structured_changes_json,
    r.rationale_summary,
    r.status,
    r.created_at_ms,
    r.updated_at_ms
FROM workspace_agent_results r
JOIN workspace_context_packets p ON p.id = r.packet_id
WHERE r.client_id = ?
  AND (? IS NULL OR r.note_id = ?)
  AND (? IS NULL OR r.packet_id = ?)
ORDER BY r.created_at_ms DESC
LIMIT ?
            "#,
        )
        .bind(filter.client_id)
        .bind(&filter.note_id)
        .bind(&filter.note_id)
        .bind(&filter.packet_id)
        .bind(&filter.packet_id)
        .bind(i64::from(limit))
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.into_iter()
            .map(|row| WorkspaceAgentResultRow::try_from_row(&row).and_then(TryInto::try_into))
            .collect()
    }

    pub async fn list_audit_events(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> anyhow::Result<Vec<crate::WorkspaceAuditEvent>> {
        let rows = sqlx::query(
            r#"
SELECT
    id,
    entity_type,
    entity_id,
    action,
    actor,
    actor_kind,
    source,
    client_id,
    encounter_id,
    note_id,
    document_id,
    source_thread_id,
    source_turn_id,
    success,
    summary,
    metadata_json,
    created_at_ms
FROM workspace_audit_events
WHERE entity_type = ? AND entity_id = ?
ORDER BY created_at_ms DESC
            "#,
        )
        .bind(entity_type)
        .bind(entity_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.into_iter()
            .map(|row| WorkspaceAuditEventRow::try_from_row(&row).and_then(TryInto::try_into))
            .collect()
    }

    pub async fn list_audit_events_filtered(
        &self,
        filter: crate::WorkspaceAuditEventFilter,
    ) -> anyhow::Result<Vec<crate::WorkspaceAuditEvent>> {
        let limit = filter.limit.unwrap_or(50).clamp(1, 200);
        let rows = sqlx::query(
            r#"
SELECT
    id,
    entity_type,
    entity_id,
    action,
    actor,
    actor_kind,
    source,
    client_id,
    encounter_id,
    note_id,
    document_id,
    source_thread_id,
    source_turn_id,
    success,
    summary,
    metadata_json,
    created_at_ms
FROM workspace_audit_events
WHERE (? IS NULL OR entity_type = ?)
  AND (? IS NULL OR entity_id = ?)
  AND (? IS NULL OR client_id = ?)
  AND (? IS NULL OR note_id = ?)
  AND (? IS NULL OR created_at_ms < ?)
ORDER BY created_at_ms DESC
LIMIT ?
            "#,
        )
        .bind(&filter.entity_type)
        .bind(&filter.entity_type)
        .bind(&filter.entity_id)
        .bind(&filter.entity_id)
        .bind(&filter.client_id)
        .bind(&filter.client_id)
        .bind(&filter.note_id)
        .bind(&filter.note_id)
        .bind(filter.cursor_created_at_ms)
        .bind(filter.cursor_created_at_ms)
        .bind(i64::from(limit))
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.into_iter()
            .map(|row| WorkspaceAuditEventRow::try_from_row(&row).and_then(TryInto::try_into))
            .collect()
    }

    pub async fn record_audit_event(
        &self,
        input: crate::WorkspaceAuditEventCreate,
    ) -> anyhow::Result<crate::WorkspaceAuditEvent> {
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin().await?;
        let id = insert_audit_event(&mut tx, input, now_ms).await?;
        tx.commit().await?;
        let row = sqlx::query(
            r#"
SELECT
    id,
    entity_type,
    entity_id,
    action,
    actor,
    actor_kind,
    source,
    client_id,
    encounter_id,
    note_id,
    document_id,
    source_thread_id,
    source_turn_id,
    success,
    summary,
    metadata_json,
    created_at_ms
FROM workspace_audit_events
WHERE id = ?
            "#,
        )
        .bind(&id)
        .fetch_one(self.pool.as_ref())
        .await?;
        WorkspaceAuditEventRow::try_from_row(&row).and_then(TryInto::try_into)
    }
}

pub(super) async fn upsert_client_admin_metadata(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    client_id: &str,
    input: &crate::WorkspaceClientUpsert,
    now_ms: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
INSERT INTO workspace_client_contacts (
    client_id,
    primary_phone,
    secondary_phone,
    email,
    preferred_contact_method,
    emergency_contact_name,
    emergency_contact_relationship,
    emergency_contact_phone,
    emergency_contact_email,
    contact_notes,
    created_at_ms,
    updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(client_id) DO UPDATE SET
    primary_phone = excluded.primary_phone,
    secondary_phone = excluded.secondary_phone,
    email = excluded.email,
    preferred_contact_method = excluded.preferred_contact_method,
    emergency_contact_name = excluded.emergency_contact_name,
    emergency_contact_relationship = excluded.emergency_contact_relationship,
    emergency_contact_phone = excluded.emergency_contact_phone,
    emergency_contact_email = excluded.emergency_contact_email,
    contact_notes = excluded.contact_notes,
    updated_at_ms = excluded.updated_at_ms
        "#,
    )
    .bind(client_id)
    .bind(&input.primary_phone)
    .bind(&input.secondary_phone)
    .bind(&input.email)
    .bind(&input.preferred_contact_method)
    .bind(&input.emergency_contact_name)
    .bind(&input.emergency_contact_relationship)
    .bind(&input.emergency_contact_phone)
    .bind(&input.emergency_contact_email)
    .bind(&input.contact_notes)
    .bind(now_ms)
    .bind(now_ms)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"
INSERT INTO workspace_client_coverages (
    client_id,
    payer_name,
    plan_name,
    member_id,
    group_number,
    coverage_type,
    coverage_status,
    coverage_notes,
    created_at_ms,
    updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
ON CONFLICT(client_id) DO UPDATE SET
    payer_name = excluded.payer_name,
    plan_name = excluded.plan_name,
    member_id = excluded.member_id,
    group_number = excluded.group_number,
    coverage_type = excluded.coverage_type,
    coverage_status = excluded.coverage_status,
    coverage_notes = excluded.coverage_notes,
    updated_at_ms = excluded.updated_at_ms
        "#,
    )
    .bind(client_id)
    .bind(&input.payer_name)
    .bind(&input.plan_name)
    .bind(&input.member_id)
    .bind(&input.group_number)
    .bind(&input.coverage_type)
    .bind(&input.coverage_status)
    .bind(&input.coverage_notes)
    .bind(now_ms)
    .bind(now_ms)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn workspace_agent_result_row_by_id(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<sqlx::sqlite::SqliteRow> {
    sqlx::query(
        r#"
SELECT
    r.id,
    r.packet_id,
    r.client_id,
    r.note_id,
    r.run_id,
    r.base_note_revision,
    p.context_envelope_sha256,
    COALESCE(NULLIF(r.packet_context_sha256, ''), p.context_envelope_sha256)
        AS packet_context_sha256,
    r.body,
    r.summary,
    r.result_kind,
    r.structured_changes_json,
    r.rationale_summary,
    r.status,
    r.created_at_ms,
    r.updated_at_ms
FROM workspace_agent_results r
JOIN workspace_context_packets p ON p.id = r.packet_id
WHERE r.id = ?
            "#,
    )
    .bind(id)
    .fetch_one(&mut **tx)
    .await
    .map_err(Into::into)
}

pub(super) async fn insert_audit_event(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    input: crate::WorkspaceAuditEventCreate,
    created_at_ms: i64,
) -> anyhow::Result<String> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
INSERT INTO workspace_audit_events (
    id,
    entity_type,
    entity_id,
    action,
    actor,
    actor_kind,
    source,
    client_id,
    encounter_id,
    note_id,
    document_id,
    source_thread_id,
    source_turn_id,
    success,
    summary,
    metadata_json,
    created_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(input.entity_type)
    .bind(input.entity_id)
    .bind(input.action)
    .bind(input.actor)
    .bind(input.actor_kind)
    .bind(input.source)
    .bind(input.client_id)
    .bind(input.encounter_id)
    .bind(input.note_id)
    .bind(input.document_id)
    .bind(input.source_thread_id)
    .bind(input.source_turn_id)
    .bind(if input.success { 1_i64 } else { 0_i64 })
    .bind(input.summary)
    .bind(input.metadata_json)
    .bind(created_at_ms)
    .execute(&mut **tx)
    .await?;
    Ok(id)
}

async fn validate_task_links(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    client_id: &str,
    encounter_id: Option<&str>,
    note_id: Option<&str>,
    document_id: Option<&str>,
) -> anyhow::Result<()> {
    let client_exists: Option<String> = sqlx::query_scalar(
        "SELECT id FROM workspace_clients WHERE id = ? AND archived_at_ms IS NULL",
    )
    .bind(client_id)
    .fetch_optional(&mut **tx)
    .await?;
    if client_exists.is_none() {
        anyhow::bail!("workspace client `{client_id}` was not found");
    }

    if let Some(encounter_id) = encounter_id {
        let exists: Option<String> = sqlx::query_scalar(
            "SELECT id FROM workspace_encounters WHERE id = ? AND client_id = ? AND archived_at_ms IS NULL",
        )
        .bind(encounter_id)
        .bind(client_id)
        .fetch_optional(&mut **tx)
        .await?;
        if exists.is_none() {
            anyhow::bail!(
                "workspace encounter `{encounter_id}` was not found for client `{client_id}`"
            );
        }
    }

    if let Some(note_id) = note_id {
        let exists: Option<String> = sqlx::query_scalar(
            "SELECT id FROM workspace_notes WHERE id = ? AND client_id = ? AND archived_at_ms IS NULL",
        )
        .bind(note_id)
        .bind(client_id)
        .fetch_optional(&mut **tx)
        .await?;
        if exists.is_none() {
            anyhow::bail!("workspace note `{note_id}` was not found for client `{client_id}`");
        }
    }

    if let Some(document_id) = document_id {
        let exists: Option<String> = sqlx::query_scalar(
            "SELECT id FROM workspace_documents WHERE id = ? AND client_id = ? AND archived_at_ms IS NULL",
        )
        .bind(document_id)
        .bind(client_id)
        .fetch_optional(&mut **tx)
        .await?;
        if exists.is_none() {
            anyhow::bail!(
                "workspace document `{document_id}` was not found for client `{client_id}`"
            );
        }
    }

    Ok(())
}

async fn validate_packet_links(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    client_id: &str,
    encounter_id: Option<&str>,
    note_id: Option<&str>,
) -> anyhow::Result<()> {
    validate_task_links(tx, client_id, encounter_id, note_id, None).await
}

async fn validate_packet_selected_context(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    client_id: &str,
    selected_artifact_ids_json: &str,
    selected_derivative_ids_json: &str,
    selected_clip_ids_json: &str,
) -> anyhow::Result<()> {
    for artifact_id in parse_selected_ids_json(
        "workspace context packet selected artifact ids",
        selected_artifact_ids_json,
    )? {
        let exists: Option<String> = sqlx::query_scalar(
            "SELECT id FROM workspace_documents WHERE id = ? AND client_id = ? AND archived_at_ms IS NULL",
        )
        .bind(&artifact_id)
        .bind(client_id)
        .fetch_optional(&mut **tx)
        .await?;
        if exists.is_none() {
            anyhow::bail!(
                "workspace context packet selected artifact `{artifact_id}` is not available for client `{client_id}`"
            );
        }
    }

    for derivative_id in parse_selected_ids_json(
        "workspace context packet selected derivative ids",
        selected_derivative_ids_json,
    )? {
        let exists: Option<String> = sqlx::query_scalar(
            r#"
SELECT derivative.id
FROM workspace_artifact_derivatives AS derivative
JOIN workspace_documents AS document
  ON document.id = derivative.document_id
 AND document.client_id = derivative.client_id
 AND document.archived_at_ms IS NULL
WHERE derivative.id = ?
  AND derivative.client_id = ?
  AND derivative.archived_at_ms IS NULL
            "#,
        )
        .bind(&derivative_id)
        .bind(client_id)
        .fetch_optional(&mut **tx)
        .await?;
        if exists.is_none() {
            anyhow::bail!(
                "workspace context packet selected derivative `{derivative_id}` is not available for client `{client_id}`"
            );
        }
    }

    for clip_id in parse_selected_ids_json(
        "workspace context packet selected clip ids",
        selected_clip_ids_json,
    )? {
        let exists: Option<String> = sqlx::query_scalar(
            r#"
SELECT clip.id
FROM workspace_context_clips AS clip
JOIN workspace_artifact_derivatives AS derivative
  ON derivative.id = clip.derivative_id
 AND derivative.document_id = clip.document_id
 AND derivative.client_id = clip.client_id
 AND derivative.archived_at_ms IS NULL
JOIN workspace_documents AS document
  ON document.id = clip.document_id
 AND document.client_id = clip.client_id
 AND document.archived_at_ms IS NULL
WHERE clip.id = ?
  AND clip.client_id = ?
  AND clip.archived_at_ms IS NULL
            "#,
        )
        .bind(&clip_id)
        .bind(client_id)
        .fetch_optional(&mut **tx)
        .await?;
        if exists.is_none() {
            anyhow::bail!(
                "workspace context packet selected clip `{clip_id}` is not available for client `{client_id}`"
            );
        }
    }

    Ok(())
}

fn parse_selected_ids_json(label: &str, value: &str) -> anyhow::Result<Vec<String>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<String> = serde_json::from_str(value)
        .map_err(|err| anyhow::anyhow!("{label} must be a JSON string array: {err}"))?;
    let mut unique_ids = BTreeSet::new();
    let mut parsed = Vec::new();
    for id in ids {
        let id = id.trim();
        if id.is_empty() {
            anyhow::bail!("{label} must not contain an empty id");
        }
        if unique_ids.insert(id.to_string()) {
            parsed.push(id.to_string());
        }
    }
    Ok(parsed)
}

fn validate_packet_context_envelope(
    input: &crate::WorkspaceContextPacketCreate,
) -> anyhow::Result<()> {
    let envelope_json = input.context_envelope_json.trim();
    if envelope_json.is_empty() || envelope_json == "{}" {
        anyhow::bail!("workspace context packet envelope must not be empty");
    }
    let envelope: serde_json::Value = serde_json::from_str(envelope_json).map_err(|err| {
        anyhow::anyhow!("workspace context packet envelope must be valid JSON: {err}")
    })?;
    if !envelope.is_object() {
        anyhow::bail!("workspace context packet envelope must be a JSON object");
    }
    if let Some(path_key) = forbidden_packet_path_key(&envelope) {
        anyhow::bail!(
            "workspace context packet envelope must not contain path-bearing key `{path_key}`"
        );
    }
    validate_agent_visible_json("context packet envelope", &envelope)?;
    let assembly_version = envelope
        .get("assemblyVersion")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim();
    if assembly_version.is_empty() {
        anyhow::bail!("workspace context packet envelope assemblyVersion is required");
    }
    if envelope
        .get("includeDocuments")
        .and_then(serde_json::Value::as_bool)
        != Some(false)
    {
        anyhow::bail!("workspace context packet envelope includeDocuments must be false");
    }
    let envelope_request = envelope
        .get("humanRequest")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim();
    if envelope_request != input.human_request.trim() {
        anyhow::bail!("workspace context packet envelope humanRequest does not match packet");
    }
    validate_envelope_selected_ids(
        &envelope,
        "selected artifact",
        "/ids/selectedArtifactIds",
        &input.selected_artifact_ids_json,
    )?;
    validate_envelope_selected_ids(
        &envelope,
        "selected derivative",
        "/ids/selectedDerivativeIds",
        &input.selected_derivative_ids_json,
    )?;
    validate_envelope_selected_ids(
        &envelope,
        "selected clip",
        "/ids/selectedClipIds",
        &input.selected_clip_ids_json,
    )?;
    let safety = envelope
        .get("safety")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("workspace context packet envelope safety is required"))?;
    let has_read_only = safety.iter().any(|value| {
        value
            .as_str()
            .is_some_and(|line| line.contains("read-only context packet"))
    });
    let has_no_sign_submit = safety.iter().any(|value| {
        value.as_str().is_some_and(|line| {
            line.contains("do not sign notes")
                && line.contains("submit claims")
                && line.contains("send payer communications")
        })
    });
    if !has_read_only || !has_no_sign_submit {
        anyhow::bail!("workspace context packet envelope safety wording is incomplete");
    }
    let prompt_snapshot = envelope
        .get("promptSnapshot")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim();
    if prompt_snapshot.is_empty() {
        anyhow::bail!("workspace context packet envelope promptSnapshot is required");
    }
    Ok(())
}

fn forbidden_packet_path_key(value: &serde_json::Value) -> Option<&str> {
    match value {
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                let normalized_key = key
                    .chars()
                    .filter(char::is_ascii_alphanumeric)
                    .map(|character| character.to_ascii_lowercase())
                    .collect::<String>();
                if matches!(
                    normalized_key.as_str(),
                    "localpath"
                        | "originalpath"
                        | "sourcepath"
                        | "vaultpath"
                        | "thumbnailpath"
                        | "previewcachepath"
                ) {
                    return Some(key.as_str());
                }
                if let Some(key) = forbidden_packet_path_key(value) {
                    return Some(key);
                }
            }
            None
        }
        serde_json::Value::Array(values) => values.iter().find_map(forbidden_packet_path_key),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => None,
    }
}

pub(super) fn validate_agent_visible_json(
    label: &str,
    value: &serde_json::Value,
) -> anyhow::Result<()> {
    if let Some(path_key) = forbidden_packet_path_key(value) {
        anyhow::bail!("workspace {label} must not contain path-bearing key `{path_key}`");
    }
    if contains_absolute_path_value(value) {
        anyhow::bail!("workspace {label} must not contain absolute filesystem path values");
    }
    Ok(())
}

fn contains_absolute_path_value(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(object) => object.values().any(contains_absolute_path_value),
        serde_json::Value::Array(values) => values.iter().any(contains_absolute_path_value),
        serde_json::Value::String(value) => string_contains_absolute_path(value),
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            false
        }
    }
}

fn string_contains_absolute_path(value: &str) -> bool {
    if value.contains("file://") {
        return true;
    }
    if value.as_bytes().windows(3).any(|window| {
        window[0].is_ascii_alphabetic() && window[1] == b':' && matches!(window[2], b'/' | b'\\')
    }) {
        return true;
    }
    value.split_whitespace().any(|token| {
        let token = token
            .rsplit_once('=')
            .map_or(token, |(_, candidate)| candidate)
            .trim_matches(|character: char| {
                matches!(
                    character,
                    '\'' | '"' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
                )
            });
        token.starts_with("~/")
            || token.starts_with("\\\\")
            || token.starts_with("//")
            || (token.starts_with('/') && token != "/workspacemedical")
    })
}

async fn resolve_packet_base_note_revision(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    input: &crate::WorkspaceContextPacketCreate,
) -> anyhow::Result<Option<i64>> {
    let Some(note_id) = input.note_id.as_deref() else {
        if input.base_note_revision.is_some() {
            anyhow::bail!("workspace context packet base note revision requires a linked note");
        }
        return Ok(None);
    };
    let current_revision: Option<i64> =
        sqlx::query_scalar("SELECT current_revision FROM workspace_notes WHERE id = ?")
            .bind(note_id)
            .fetch_optional(&mut **tx)
            .await?;
    let current_revision = current_revision
        .ok_or_else(|| anyhow::anyhow!("workspace note `{note_id}` was not found"))?;
    if let Some(expected_revision) = input.base_note_revision
        && expected_revision != current_revision
    {
        anyhow::bail!(
            "workspace context packet expected note revision {expected_revision} but note `{note_id}` is at revision {current_revision}"
        );
    }
    let envelope: serde_json::Value = serde_json::from_str(&input.context_envelope_json)?;
    if let Some(envelope_revision) = envelope
        .pointer("/note/revision")
        .and_then(serde_json::Value::as_i64)
        && envelope_revision != current_revision
    {
        anyhow::bail!(
            "workspace context packet envelope note revision {envelope_revision} does not match saved revision {current_revision}"
        );
    }
    Ok(Some(current_revision))
}

fn legacy_packet_authorized_scope_json(input: &crate::WorkspaceContextPacketCreate) -> String {
    let selected_artifact_ids =
        serde_json::from_str::<serde_json::Value>(&input.selected_artifact_ids_json)
            .unwrap_or_else(|_| serde_json::json!([]));
    let selected_derivative_ids =
        serde_json::from_str::<serde_json::Value>(&input.selected_derivative_ids_json)
            .unwrap_or_else(|_| serde_json::json!([]));
    let selected_clip_ids =
        serde_json::from_str::<serde_json::Value>(&input.selected_clip_ids_json)
            .unwrap_or_else(|_| serde_json::json!([]));
    serde_json::json!({
        "version": 1,
        "categories": ["packet_snapshot"],
        "legacy": true,
        "selectedArtifactIds": selected_artifact_ids,
        "selectedDerivativeIds": selected_derivative_ids,
        "selectedClipIds": selected_clip_ids,
    })
    .to_string()
}

fn validate_envelope_selected_ids(
    envelope: &serde_json::Value,
    label: &str,
    pointer: &str,
    selected_ids_json: &str,
) -> anyhow::Result<()> {
    let expected = parse_selected_ids_json(
        &format!("workspace context packet {label} ids"),
        selected_ids_json,
    )?
    .into_iter()
    .collect::<BTreeSet<_>>();
    let Some(values) = envelope
        .pointer(pointer)
        .and_then(serde_json::Value::as_array)
    else {
        anyhow::bail!("workspace context packet envelope {label} ids are required");
    };
    let mut actual = BTreeSet::new();
    for value in values {
        let Some(id) = value.as_str().map(str::trim).filter(|id| !id.is_empty()) else {
            anyhow::bail!("workspace context packet envelope {label} ids must be strings");
        };
        actual.insert(id.to_string());
    }
    if actual != expected {
        anyhow::bail!("workspace context packet envelope {label} ids do not match packet");
    }
    Ok(())
}

fn context_envelope_sha256(context_envelope_json: &str) -> String {
    format!("{:x}", Sha256::digest(context_envelope_json.as_bytes()))
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn associated_from_document_id(metadata_json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(metadata_json)
        .ok()
        .and_then(|metadata| {
            metadata
                .get("associatedFromDocumentId")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(str::to_string)
        })
}

fn practice_library_scope_reason(document: &crate::WorkspaceDocument) -> Option<String> {
    if associated_from_document_id(&document.metadata_json).is_some() {
        return None;
    }
    let scope = document.scope.trim().to_ascii_lowercase();
    if matches!(
        scope.as_str(),
        "practice" | "practice-wide" | "billing" | "payer"
    ) {
        return Some("explicit practice scope".to_string());
    }
    let haystack = practice_library_haystack(document);
    if haystack.contains("x12")
        || haystack.contains("ansi")
        || haystack.contains("edi")
        || haystack.contains("837")
        || haystack.contains("835")
        || haystack.contains("270")
        || haystack.contains("271")
        || haystack.contains("277")
        || haystack.contains("999")
    {
        return Some("EDI/ANSI metadata".to_string());
    }
    if haystack.contains("payer")
        || haystack.contains("billing")
        || haystack.contains("claim")
        || haystack.contains("remittance")
        || haystack.contains("eligibility")
        || haystack.contains("fee schedule")
    {
        return Some("billing/payer metadata".to_string());
    }
    if haystack.contains("practice") || haystack.contains("batch") || haystack.contains("facility")
    {
        return Some("practice metadata".to_string());
    }
    None
}

fn practice_library_item_matches_query(
    document: &crate::WorkspaceDocument,
    owner_display_name: &str,
    query: &str,
) -> bool {
    practice_library_haystack(document).contains(query)
        || owner_display_name.to_ascii_lowercase().contains(query)
}

fn practice_library_haystack(document: &crate::WorkspaceDocument) -> String {
    format!(
        "{} {} {} {} {} {} {} {} {}",
        document.scope,
        document.kind,
        document.title,
        document.local_path,
        document.notes,
        document.tags,
        document.source_label,
        document.detected_kind,
        document.metadata_json
    )
    .to_ascii_lowercase()
}

fn nonempty_or_string(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

fn normalize_patient_safety_category(value: &str) -> anyhow::Result<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "allergy" | "allergies" => Ok("allergy"),
        "medication" | "medications" | "med" | "meds" => Ok("medication"),
        "condition" | "conditions" | "problem" | "problems" => Ok("condition"),
        "precaution" | "precautions" | "restriction" | "restrictions" => Ok("precaution"),
        other => anyhow::bail!("unsupported workspace patient safety category `{other}`"),
    }
}

fn note_status_is_locked(status: &str) -> bool {
    matches!(status.trim(), NOTE_STATUS_SIGNED | NOTE_STATUS_ADDENDED)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::test_support::unique_temp_dir;
    use pretty_assertions::assert_eq;

    async fn test_runtime() -> std::sync::Arc<StateRuntime> {
        StateRuntime::init(unique_temp_dir(), "test-provider".to_string())
            .await
            .expect("state db should initialize")
    }

    #[tokio::test]
    async fn client_note_revision_and_proposal_flow() {
        let runtime = test_runtime().await;
        let client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Ada Lovelace".to_string(),
                preferred_name: Some("Ada".to_string()),
                summary: "Mathematics consultation".to_string(),
                ..Default::default()
            })
            .await
            .expect("client upsert");

        let note = runtime
            .workspace()
            .upsert_note(crate::WorkspaceNoteUpsert {
                client_id: client.id.clone(),
                title: "Initial note".to_string(),
                kind: "progress".to_string(),
                body: "Human note".to_string(),
                status: "draft".to_string(),
                actor: "human".to_string(),
                summary: Some("created".to_string()),
                ..Default::default()
            })
            .await
            .expect("note upsert");
        assert_eq!(note.current_revision, 1);

        let proposal = runtime
            .workspace()
            .create_note_proposal(crate::WorkspaceNoteProposalCreate {
                note_id: note.id.clone(),
                base_revision: note.current_revision,
                proposed_body: "Agent-edited note".to_string(),
                summary: "tighten wording".to_string(),
                source_thread_id: Some("thread-1".to_string()),
                source_turn_id: Some("turn-1".to_string()),
                ..Default::default()
            })
            .await
            .expect("proposal create");
        assert_eq!(proposal.status, crate::WorkspaceNoteProposalStatus::Pending);

        let resolved = runtime
            .workspace()
            .resolve_note_proposal(&proposal.id, /*accept*/ true, "human")
            .await
            .expect("proposal accept")
            .expect("proposal exists");
        assert_eq!(
            resolved.status,
            crate::WorkspaceNoteProposalStatus::Accepted
        );

        let updated = runtime
            .workspace()
            .get_note(&note.id)
            .await
            .expect("note get")
            .expect("note exists");
        assert_eq!(updated.body, "Agent-edited note");
        assert_eq!(updated.current_revision, 2);

        let audit = runtime
            .workspace()
            .list_audit_events("note_proposal", &proposal.id)
            .await
            .expect("audit list");
        assert_eq!(audit.len(), 2);
    }

    #[tokio::test]
    async fn client_upsert_and_archive_emit_scoped_audit_events() {
        let runtime = test_runtime().await;
        let client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Jordan Client".to_string(),
                summary: "initial fake chart".to_string(),
                ..Default::default()
            })
            .await
            .expect("client create");
        let updated = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                id: Some(client.id.clone()),
                display_name: "Jordan Updated".to_string(),
                summary: "updated fake chart".to_string(),
                ..Default::default()
            })
            .await
            .expect("client update");

        assert!(
            runtime
                .workspace()
                .archive_client(&updated.id)
                .await
                .expect("client archive")
        );

        let audit = runtime
            .workspace()
            .list_audit_events("client", &updated.id)
            .await
            .expect("client audit");
        assert!(audit.iter().any(|event| {
            event.action == "created"
                && event.client_id.as_deref() == Some(updated.id.as_str())
                && event.summary == "Jordan Client"
        }));
        assert!(audit.iter().any(|event| {
            event.action == "updated"
                && event.client_id.as_deref() == Some(updated.id.as_str())
                && event.summary == "Jordan Updated"
        }));
        assert!(audit.iter().any(|event| {
            event.action == "archived"
                && event.client_id.as_deref() == Some(updated.id.as_str())
                && event.summary == "Jordan Updated"
        }));
    }

    #[tokio::test]
    async fn client_admin_metadata_schema_enforces_one_row_and_foreign_keys() {
        let runtime = test_runtime().await;
        let pool = runtime.workspace().pool.as_ref();
        let contact_schema: Option<String> = sqlx::query_scalar(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'workspace_client_contacts'",
        )
        .fetch_optional(pool)
        .await
        .expect("contact schema query");
        let coverage_schema: Option<String> = sqlx::query_scalar(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'workspace_client_coverages'",
        )
        .fetch_optional(pool)
        .await
        .expect("coverage schema query");
        assert!(
            contact_schema
                .as_deref()
                .is_some_and(|schema| schema.contains("PRIMARY KEY"))
        );
        assert!(
            coverage_schema
                .as_deref()
                .is_some_and(|schema| schema.contains("PRIMARY KEY"))
        );

        let contact_fks = sqlx::query("PRAGMA foreign_key_list(workspace_client_contacts)")
            .fetch_all(pool)
            .await
            .expect("contact fk list");
        assert!(contact_fks.iter().any(|row| {
            row.try_get::<String, _>("table")
                .is_ok_and(|table| table == "workspace_clients")
        }));
        let coverage_fks = sqlx::query("PRAGMA foreign_key_list(workspace_client_coverages)")
            .fetch_all(pool)
            .await
            .expect("coverage fk list");
        assert!(coverage_fks.iter().any(|row| {
            row.try_get::<String, _>("table")
                .is_ok_and(|table| table == "workspace_clients")
        }));

        let client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Constraint Patient".to_string(),
                ..Default::default()
            })
            .await
            .expect("client upsert");
        let duplicate_contact = sqlx::query(
            "INSERT INTO workspace_client_contacts (client_id, created_at_ms, updated_at_ms) VALUES (?, 1, 1)",
        )
        .bind(&client.id)
        .execute(pool)
        .await;
        assert!(duplicate_contact.is_err());
        let duplicate_coverage = sqlx::query(
            "INSERT INTO workspace_client_coverages (client_id, created_at_ms, updated_at_ms) VALUES (?, 1, 1)",
        )
        .bind(&client.id)
        .execute(pool)
        .await;
        assert!(duplicate_coverage.is_err());
        let orphan_contact = sqlx::query(
            "INSERT INTO workspace_client_contacts (client_id, created_at_ms, updated_at_ms) VALUES ('missing-client', 1, 1)",
        )
        .execute(pool)
        .await;
        assert!(orphan_contact.is_err());
        let orphan_coverage = sqlx::query(
            "INSERT INTO workspace_client_coverages (client_id, created_at_ms, updated_at_ms) VALUES ('missing-client', 1, 1)",
        )
        .execute(pool)
        .await;
        assert!(orphan_coverage.is_err());
    }

    #[tokio::test]
    async fn patient_safety_items_schema_persists_and_archives_by_client() {
        let state_dir = unique_temp_dir();
        let runtime = StateRuntime::init(state_dir.clone(), "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let pool = runtime.workspace().pool.as_ref();
        let safety_schema: Option<String> = sqlx::query_scalar(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'workspace_patient_safety_items'",
        )
        .fetch_optional(pool)
        .await
        .expect("safety schema query");
        assert!(
            safety_schema
                .as_deref()
                .is_some_and(|schema| schema.contains("FOREIGN KEY"))
        );
        assert!(
            safety_schema
                .as_deref()
                .is_some_and(|schema| schema.contains("CHECK"))
        );
        let safety_fks = sqlx::query("PRAGMA foreign_key_list(workspace_patient_safety_items)")
            .fetch_all(pool)
            .await
            .expect("safety fk list");
        assert!(safety_fks.iter().any(|row| {
            row.try_get::<String, _>("table")
                .is_ok_and(|table| table == "workspace_clients")
        }));
        let orphan_safety = sqlx::query(
            "INSERT INTO workspace_patient_safety_items (id, client_id, category, name, created_at_ms, updated_at_ms) VALUES ('orphan', 'missing-client', 'allergy', 'Peanut', 1, 1)",
        )
        .execute(pool)
        .await;
        assert!(orphan_safety.is_err());

        let client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Safety Patient".to_string(),
                ..Default::default()
            })
            .await
            .expect("client upsert");
        let allergy = runtime
            .workspace()
            .upsert_patient_safety_item(crate::WorkspacePatientSafetyItemUpsert {
                client_id: client.id.clone(),
                category: "allergy".to_string(),
                name: "Peanut".to_string(),
                reaction: Some("hives".to_string()),
                severity: Some("severe".to_string()),
                status: Some("active".to_string()),
                recorded_date: Some("2026-06-22".to_string()),
                notes: "fake allergy row".to_string(),
                ..Default::default()
            })
            .await
            .expect("allergy upsert");
        runtime
            .workspace()
            .upsert_patient_safety_item(crate::WorkspacePatientSafetyItemUpsert {
                client_id: client.id.clone(),
                category: "medication".to_string(),
                name: "Fakeformin".to_string(),
                dose: Some("500 mg".to_string()),
                route: Some("PO".to_string()),
                frequency: Some("daily".to_string()),
                status: Some("active".to_string()),
                notes: "fake medication row".to_string(),
                ..Default::default()
            })
            .await
            .expect("medication upsert");
        drop(runtime);

        let reloaded = StateRuntime::init(state_dir, "test-provider".to_string())
            .await
            .expect("state db should reload");
        let items = reloaded
            .workspace()
            .list_patient_safety_items(&client.id)
            .await
            .expect("safety list");
        assert_eq!(items.len(), 2);
        let reloaded_allergy = items
            .iter()
            .find(|item| item.category == "allergy")
            .expect("allergy present");
        assert_eq!(reloaded_allergy.name, "Peanut");
        assert_eq!(reloaded_allergy.reaction.as_deref(), Some("hives"));
        assert_eq!(reloaded_allergy.severity.as_deref(), Some("severe"));
        let medication = items
            .iter()
            .find(|item| item.category == "medication")
            .expect("medication present");
        assert_eq!(medication.dose.as_deref(), Some("500 mg"));
        assert_eq!(medication.route.as_deref(), Some("PO"));
        assert_eq!(medication.frequency.as_deref(), Some("daily"));

        assert!(
            reloaded
                .workspace()
                .archive_patient_safety_item(&allergy.id)
                .await
                .expect("archive allergy")
        );
        let after_archive = reloaded
            .workspace()
            .list_patient_safety_items(&client.id)
            .await
            .expect("safety list after archive");
        assert_eq!(after_archive.len(), 1);
        assert_eq!(after_archive[0].category, "medication");
    }

    #[tokio::test]
    async fn client_structured_admin_metadata_persists_and_clears_through_sqlite_reload() {
        let state_dir = unique_temp_dir();
        let runtime = StateRuntime::init(state_dir.clone(), "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Jordan Patient".to_string(),
                external_id: Some("MRN-123".to_string()),
                summary: "Narrative patient summary only.".to_string(),
                primary_phone: Some("555-0101".to_string()),
                secondary_phone: Some("555-0102".to_string()),
                email: Some("jordan.fake@example.test".to_string()),
                preferred_contact_method: Some("phone".to_string()),
                emergency_contact_name: Some("Maya Contact".to_string()),
                emergency_contact_relationship: Some("sibling".to_string()),
                emergency_contact_phone: Some("555-0199".to_string()),
                emergency_contact_email: Some("maya.fake@example.test".to_string()),
                contact_notes: Some("fake contact note".to_string()),
                payer_name: Some("Fake Medicare".to_string()),
                plan_name: Some("Fake Plan A".to_string()),
                member_id: Some("MED-777".to_string()),
                group_number: Some("GRP-1".to_string()),
                coverage_type: Some("Medicare".to_string()),
                coverage_status: Some("active".to_string()),
                coverage_notes: Some("fake coverage note".to_string()),
                ..Default::default()
            })
            .await
            .expect("client upsert");
        drop(runtime);

        let reloaded = StateRuntime::init(state_dir.clone(), "test-provider".to_string())
            .await
            .expect("state db should reload");
        let reloaded_client = reloaded
            .workspace()
            .get_client(&client.id)
            .await
            .expect("client get")
            .expect("client exists");
        assert_eq!(reloaded_client.external_id.as_deref(), Some("MRN-123"));
        assert_eq!(reloaded_client.primary_phone.as_deref(), Some("555-0101"));
        assert_eq!(reloaded_client.secondary_phone.as_deref(), Some("555-0102"));
        assert_eq!(
            reloaded_client.email.as_deref(),
            Some("jordan.fake@example.test")
        );
        assert_eq!(
            reloaded_client.preferred_contact_method.as_deref(),
            Some("phone")
        );
        assert_eq!(
            reloaded_client.emergency_contact_name.as_deref(),
            Some("Maya Contact")
        );
        assert_eq!(
            reloaded_client.emergency_contact_relationship.as_deref(),
            Some("sibling")
        );
        assert_eq!(
            reloaded_client.emergency_contact_phone.as_deref(),
            Some("555-0199")
        );
        assert_eq!(
            reloaded_client.emergency_contact_email.as_deref(),
            Some("maya.fake@example.test")
        );
        assert_eq!(
            reloaded_client.contact_notes.as_deref(),
            Some("fake contact note")
        );
        assert_eq!(reloaded_client.payer_name.as_deref(), Some("Fake Medicare"));
        assert_eq!(reloaded_client.plan_name.as_deref(), Some("Fake Plan A"));
        assert_eq!(reloaded_client.member_id.as_deref(), Some("MED-777"));
        assert_eq!(reloaded_client.group_number.as_deref(), Some("GRP-1"));
        assert_eq!(reloaded_client.coverage_type.as_deref(), Some("Medicare"));
        assert_eq!(reloaded_client.coverage_status.as_deref(), Some("active"));
        assert_eq!(
            reloaded_client.coverage_notes.as_deref(),
            Some("fake coverage note")
        );

        reloaded
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                id: Some(client.id.clone()),
                display_name: "Jordan Patient".to_string(),
                summary: "Primary phone: 555-LEGACY\nMember ID: LEGACY-MEMBER".to_string(),
                primary_phone: None,
                member_id: None,
                ..Default::default()
            })
            .await
            .expect("client clear");
        drop(reloaded);

        let cleared_runtime = StateRuntime::init(state_dir, "test-provider".to_string())
            .await
            .expect("state db should reload after clear");
        let cleared = cleared_runtime
            .workspace()
            .get_client(&client.id)
            .await
            .expect("client get")
            .expect("client exists");
        assert_eq!(cleared.primary_phone, None);
        assert_eq!(cleared.member_id, None);
        assert!(cleared.summary.contains("555-LEGACY"));
        assert!(cleared.summary.contains("LEGACY-MEMBER"));
    }

    #[tokio::test]
    async fn client_legacy_summary_markers_backfill_into_structured_admin_metadata() {
        let state_dir = unique_temp_dir();
        let summary = "\
Primary phone: 555-0101
Secondary phone: 555-0102
Email: jordan.fake@example.test
Preferred contact method: phone
Emergency contact name: Maya Contact
Emergency contact relationship: sibling
Emergency contact phone: 555-0199
Emergency contact email: maya.fake@example.test
Contact notes: fake contact note
Payer: Fake Medicare
Plan name: Fake Plan A
Member ID: MED-777
Group number: GRP-1
Coverage type: Medicare
Coverage status: active
Coverage notes: fake coverage note";
        let runtime = StateRuntime::init(state_dir.clone(), "test-provider".to_string())
            .await
            .expect("state db should initialize");
        let client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Jordan Patient".to_string(),
                external_id: Some("MRN-123".to_string()),
                summary: summary.to_string(),
                ..Default::default()
            })
            .await
            .expect("client upsert");
        sqlx::query("DELETE FROM workspace_client_contacts WHERE client_id = ?")
            .bind(&client.id)
            .execute(runtime.workspace().pool.as_ref())
            .await
            .expect("remove normalized contact row to simulate legacy db");
        sqlx::query("DELETE FROM workspace_client_coverages WHERE client_id = ?")
            .bind(&client.id)
            .execute(runtime.workspace().pool.as_ref())
            .await
            .expect("remove normalized coverage row to simulate legacy db");
        drop(runtime);

        let reloaded = StateRuntime::init(state_dir, "test-provider".to_string())
            .await
            .expect("state db should reload");
        let clients = reloaded
            .workspace()
            .list_clients()
            .await
            .expect("client list");
        let reloaded_client = clients
            .into_iter()
            .find(|candidate| candidate.id == client.id)
            .expect("reloaded client exists");

        assert_eq!(reloaded_client.external_id.as_deref(), Some("MRN-123"));
        assert_eq!(reloaded_client.primary_phone.as_deref(), Some("555-0101"));
        assert_eq!(reloaded_client.secondary_phone.as_deref(), Some("555-0102"));
        assert_eq!(
            reloaded_client.email.as_deref(),
            Some("jordan.fake@example.test")
        );
        assert_eq!(
            reloaded_client.preferred_contact_method.as_deref(),
            Some("phone")
        );
        assert_eq!(
            reloaded_client.emergency_contact_name.as_deref(),
            Some("Maya Contact")
        );
        assert_eq!(
            reloaded_client.emergency_contact_relationship.as_deref(),
            Some("sibling")
        );
        assert_eq!(
            reloaded_client.emergency_contact_phone.as_deref(),
            Some("555-0199")
        );
        assert_eq!(
            reloaded_client.emergency_contact_email.as_deref(),
            Some("maya.fake@example.test")
        );
        assert_eq!(
            reloaded_client.contact_notes.as_deref(),
            Some("fake contact note")
        );
        assert_eq!(reloaded_client.payer_name.as_deref(), Some("Fake Medicare"));
        assert_eq!(reloaded_client.plan_name.as_deref(), Some("Fake Plan A"));
        assert_eq!(reloaded_client.member_id.as_deref(), Some("MED-777"));
        assert_eq!(reloaded_client.group_number.as_deref(), Some("GRP-1"));
        assert_eq!(reloaded_client.coverage_type.as_deref(), Some("Medicare"));
        assert_eq!(reloaded_client.coverage_status.as_deref(), Some("active"));
        assert_eq!(
            reloaded_client.coverage_notes.as_deref(),
            Some("fake coverage note")
        );
    }

    #[tokio::test]
    async fn workspace_agent_result_status_update_is_persisted_and_audited() {
        let runtime = test_runtime().await;
        let client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Jordan Patient".to_string(),
                ..Default::default()
            })
            .await
            .expect("client upsert");
        let note = runtime
            .workspace()
            .upsert_note(crate::WorkspaceNoteUpsert {
                client_id: client.id.clone(),
                title: "Visit note".to_string(),
                kind: "progress".to_string(),
                body: "Human note.".to_string(),
                status: "draft".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("note upsert");
        let packet = runtime
            .workspace()
            .create_context_packet(crate::WorkspaceContextPacketCreate {
                client_id: client.id.clone(),
                encounter_id: None,
                note_id: Some(note.id.clone()),
                human_request: "Draft follow-up plan.".to_string(),
                selected_artifact_ids_json: "[]".to_string(),
                selected_derivative_ids_json: "[]".to_string(),
                selected_clip_ids_json: "[]".to_string(),
                artifact_summary: "0 selected artifacts".to_string(),
                derivative_summary: "0 selected derivatives".to_string(),
                clip_summary: "0 selected clips".to_string(),
                chart_context_summary: "patient Jordan Patient".to_string(),
                context_envelope_json: valid_packet_envelope(
                    "Draft follow-up plan.",
                    "[]",
                    "[]",
                    "[]",
                ),
                status: "sent".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("packet create");
        let result = runtime
            .workspace()
            .create_agent_result(crate::WorkspaceAgentResultCreate {
                packet_id: packet.id.clone(),
                body: "Returned work.".to_string(),
                summary: "Returned work.".to_string(),
                status: "review_pending".to_string(),
                actor: "human".to_string(),
                expected_client_id: Some(client.id.clone()),
                expected_note_id: Some(note.id.clone()),
                expected_context_envelope_sha256: packet.context_envelope_sha256.clone(),
                ..Default::default()
            })
            .await
            .expect("agent result create");
        assert_eq!(
            result.context_envelope_sha256,
            packet.context_envelope_sha256
        );
        let wrong_client = runtime
            .workspace()
            .create_agent_result(crate::WorkspaceAgentResultCreate {
                packet_id: packet.id.clone(),
                body: "Cross-client work.".to_string(),
                summary: "Cross-client work.".to_string(),
                status: "review_pending".to_string(),
                actor: "human".to_string(),
                expected_client_id: Some("client-other".to_string()),
                expected_note_id: Some(note.id.clone()),
                expected_context_envelope_sha256: packet.context_envelope_sha256.clone(),
                ..Default::default()
            })
            .await;
        assert!(wrong_client.is_err());
        let wrong_note = runtime
            .workspace()
            .create_agent_result(crate::WorkspaceAgentResultCreate {
                packet_id: packet.id.clone(),
                body: "Wrong-note work.".to_string(),
                summary: "Wrong-note work.".to_string(),
                status: "review_pending".to_string(),
                actor: "human".to_string(),
                expected_client_id: Some(client.id.clone()),
                expected_note_id: Some("note-other".to_string()),
                expected_context_envelope_sha256: packet.context_envelope_sha256.clone(),
                ..Default::default()
            })
            .await;
        assert!(wrong_note.is_err());
        let wrong_hash = runtime
            .workspace()
            .create_agent_result(crate::WorkspaceAgentResultCreate {
                packet_id: packet.id.clone(),
                body: "Wrong-hash work.".to_string(),
                summary: "Wrong-hash work.".to_string(),
                status: "review_pending".to_string(),
                actor: "human".to_string(),
                expected_client_id: Some(client.id.clone()),
                expected_note_id: Some(note.id.clone()),
                expected_context_envelope_sha256: "wrong-envelope-hash".to_string(),
                ..Default::default()
            })
            .await;
        assert!(wrong_hash.is_err());
        let other_client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Riley Other".to_string(),
                ..Default::default()
            })
            .await
            .expect("other client upsert");
        let other_note = runtime
            .workspace()
            .upsert_note(crate::WorkspaceNoteUpsert {
                client_id: other_client.id.clone(),
                title: "Other note".to_string(),
                kind: "progress".to_string(),
                body: "Other note body.".to_string(),
                status: "draft".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("other note upsert");
        let other_packet = runtime
            .workspace()
            .create_context_packet(crate::WorkspaceContextPacketCreate {
                client_id: other_client.id.clone(),
                encounter_id: None,
                note_id: Some(other_note.id.clone()),
                human_request: "OTHER_CLIENT_PACKET_SENTINEL".to_string(),
                selected_artifact_ids_json: "[]".to_string(),
                selected_derivative_ids_json: "[]".to_string(),
                selected_clip_ids_json: "[]".to_string(),
                artifact_summary: "0 selected artifacts".to_string(),
                derivative_summary: "0 selected derivatives".to_string(),
                clip_summary: "0 selected clips".to_string(),
                chart_context_summary: "patient Riley Other".to_string(),
                context_envelope_json: valid_packet_envelope(
                    "OTHER_CLIENT_PACKET_SENTINEL",
                    "[]",
                    "[]",
                    "[]",
                ),
                status: "sent".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("other packet create");
        runtime
            .workspace()
            .create_agent_result(crate::WorkspaceAgentResultCreate {
                packet_id: other_packet.id.clone(),
                body: "OTHER_CLIENT_RESULT_SENTINEL".to_string(),
                summary: "OTHER_CLIENT_RESULT_SENTINEL".to_string(),
                status: "review_pending".to_string(),
                actor: "human".to_string(),
                expected_client_id: Some(other_client.id.clone()),
                expected_note_id: Some(other_note.id.clone()),
                expected_context_envelope_sha256: other_packet.context_envelope_sha256,
                ..Default::default()
            })
            .await
            .expect("other agent result create");
        let client_results = runtime
            .workspace()
            .list_agent_results(crate::WorkspaceAgentResultFilter {
                client_id: client.id.clone(),
                note_id: None,
                packet_id: None,
                limit: Some(10),
            })
            .await
            .expect("client result list");
        let client_result_text = client_results
            .iter()
            .map(|entry| format!("{} {}", entry.summary, entry.body))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!client_result_text.contains("OTHER_CLIENT_PACKET_SENTINEL"));
        assert!(!client_result_text.contains("OTHER_CLIENT_RESULT_SENTINEL"));

        let reviewed = runtime
            .workspace()
            .update_agent_result_status(crate::WorkspaceAgentResultStatusUpdate {
                result_id: result.id.clone(),
                status: "reviewed".to_string(),
                actor: "human".to_string(),
            })
            .await
            .expect("status update")
            .expect("result exists");
        assert_eq!(reviewed.status, "reviewed");
        assert_eq!(reviewed.body, "Returned work.");
        let invalid_conversion = runtime
            .workspace()
            .update_agent_result_status(crate::WorkspaceAgentResultStatusUpdate {
                result_id: result.id.clone(),
                status: "converted".to_string(),
                actor: "human".to_string(),
            })
            .await
            .expect_err("only proposal creation may mark a result converted")
            .to_string();
        assert!(invalid_conversion.contains("cannot transition"));
        let listed = runtime
            .workspace()
            .list_agent_results(crate::WorkspaceAgentResultFilter {
                client_id: client.id,
                note_id: Some(note.id),
                packet_id: Some(packet.id),
                limit: Some(10),
            })
            .await
            .expect("list results");
        assert_eq!(listed[0].status, "reviewed");
        assert_eq!(listed[0].body, "Returned work.");
        let audit = runtime
            .workspace()
            .list_audit_events("agent_result", &result.id)
            .await
            .expect("agent result audit");
        assert!(audit.iter().any(|event| event.action == "saved"));
        assert!(audit.iter().any(|event| {
            event.action == "status_changed" && event.summary == "review_pending -> reviewed"
        }));
    }

    #[tokio::test]
    async fn declining_note_proposal_does_not_mutate_note() {
        let runtime = test_runtime().await;
        let client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Ada Lovelace".to_string(),
                ..Default::default()
            })
            .await
            .expect("client upsert");
        let note = runtime
            .workspace()
            .upsert_note(crate::WorkspaceNoteUpsert {
                client_id: client.id.clone(),
                title: "Initial note".to_string(),
                kind: "progress".to_string(),
                body: "Human note".to_string(),
                status: "draft".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("note upsert");
        let proposal = runtime
            .workspace()
            .create_note_proposal(crate::WorkspaceNoteProposalCreate {
                note_id: note.id.clone(),
                base_revision: note.current_revision,
                proposed_body: "Declined body".to_string(),
                summary: "decline".to_string(),
                source_thread_id: None,
                source_turn_id: None,
                ..Default::default()
            })
            .await
            .expect("proposal create");

        let resolved = runtime
            .workspace()
            .resolve_note_proposal(&proposal.id, /*accept*/ false, "human")
            .await
            .expect("proposal decline")
            .expect("proposal exists");
        assert_eq!(
            resolved.status,
            crate::WorkspaceNoteProposalStatus::Declined
        );
        let unchanged = runtime
            .workspace()
            .get_note(&note.id)
            .await
            .expect("note get")
            .expect("note exists");
        assert_eq!(unchanged.body, "Human note");
        assert_eq!(unchanged.current_revision, note.current_revision);
        let audit = runtime
            .workspace()
            .list_audit_events("note_proposal", &proposal.id)
            .await
            .expect("proposal audit");
        let declined = audit
            .iter()
            .find(|event| event.action == "declined")
            .expect("decline audit");
        assert_eq!(declined.client_id.as_deref(), Some(client.id.as_str()));
        assert_eq!(declined.note_id.as_deref(), Some(note.id.as_str()));
    }

    #[tokio::test]
    async fn stale_note_proposal_accept_is_rejected() {
        let runtime = test_runtime().await;
        let client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Ada Lovelace".to_string(),
                ..Default::default()
            })
            .await
            .expect("client upsert");
        let note = runtime
            .workspace()
            .upsert_note(crate::WorkspaceNoteUpsert {
                client_id: client.id.clone(),
                title: "Initial note".to_string(),
                kind: "progress".to_string(),
                body: "Human note".to_string(),
                status: "draft".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("note upsert");
        let proposal = runtime
            .workspace()
            .create_note_proposal(crate::WorkspaceNoteProposalCreate {
                note_id: note.id.clone(),
                base_revision: note.current_revision,
                proposed_body: "Stale body".to_string(),
                summary: "stale".to_string(),
                source_thread_id: None,
                source_turn_id: None,
                ..Default::default()
            })
            .await
            .expect("proposal create");
        runtime
            .workspace()
            .upsert_note(crate::WorkspaceNoteUpsert {
                id: Some(note.id.clone()),
                client_id: client.id,
                title: note.title.clone(),
                kind: note.kind.clone(),
                body: "Human changed body".to_string(),
                status: "draft".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("note update");

        let accept = runtime
            .workspace()
            .resolve_note_proposal(&proposal.id, /*accept*/ true, "human")
            .await;
        assert!(accept.is_err());
        let unchanged = runtime
            .workspace()
            .get_note(&note.id)
            .await
            .expect("note get")
            .expect("note exists");
        assert_eq!(unchanged.body, "Human changed body");
        assert_eq!(unchanged.current_revision, note.current_revision + 1);
    }

    #[tokio::test]
    async fn document_metadata_persists_and_archives() {
        let runtime = test_runtime().await;
        let client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Jordan Client".to_string(),
                record_start_date: Some("2026-01-01".to_string()),
                record_end_date: Some("2026-06-09".to_string()),
                ..Default::default()
            })
            .await
            .expect("client upsert");

        let document = runtime
            .workspace()
            .upsert_document(crate::WorkspaceDocumentUpsert {
                client_id: client.id.clone(),
                title: "Outside referral PDF".to_string(),
                kind: "referral".to_string(),
                local_path: "/tmp/referral.pdf".to_string(),
                notes: "metadata only".to_string(),
                scope: "patient".to_string(),
                detected_kind: "PDF".to_string(),
                mime_type: Some("application/pdf".to_string()),
                file_size_bytes: Some(2048),
                sha256: Some("abc123".to_string()),
                tags: "outside-record,review".to_string(),
                source_label: "referring clinic".to_string(),
                existence_status: "missing".to_string(),
                metadata_json: r#"{"referenceOnly":true}"#.to_string(),
                original_path: "/tmp/original-referral.pdf".to_string(),
                reference_kind: "vault_copy".to_string(),
                vault_path: "/tmp/codex-vault/referral.pdf".to_string(),
                content_sha256: Some("content123".to_string()),
                thumbnail_path: "/tmp/codex-thumbnails/referral.png".to_string(),
                thumbnail_status: "ready".to_string(),
                thumbnail_mime_type: Some("image/png".to_string()),
                intake_source: "manual_file_import".to_string(),
                ..Default::default()
            })
            .await
            .expect("document upsert");

        let documents = runtime
            .workspace()
            .list_documents(&client.id)
            .await
            .expect("document list");
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].title, "Outside referral PDF");
        assert_eq!(documents[0].local_path, "/tmp/referral.pdf");
        assert_eq!(documents[0].scope, "patient");
        assert_eq!(documents[0].detected_kind, "PDF");
        assert_eq!(documents[0].mime_type.as_deref(), Some("application/pdf"));
        assert_eq!(documents[0].file_size_bytes, Some(2048));
        assert_eq!(documents[0].sha256.as_deref(), Some("abc123"));
        assert_eq!(documents[0].tags, "outside-record,review");
        assert_eq!(documents[0].source_label, "referring clinic");
        assert_eq!(documents[0].existence_status, "missing");
        assert_eq!(documents[0].original_path, "/tmp/original-referral.pdf");
        assert_eq!(documents[0].reference_kind, "vault_copy");
        assert_eq!(documents[0].vault_path, "/tmp/codex-vault/referral.pdf");
        assert_eq!(documents[0].content_sha256.as_deref(), Some("content123"));
        assert_eq!(
            documents[0].thumbnail_path,
            "/tmp/codex-thumbnails/referral.png"
        );
        assert_eq!(documents[0].thumbnail_status, "ready");
        assert_eq!(
            documents[0].thumbnail_mime_type.as_deref(),
            Some("image/png")
        );
        assert_eq!(documents[0].intake_source, "manual_file_import");

        let fetched = runtime
            .workspace()
            .get_document(&document.id)
            .await
            .expect("document get")
            .expect("document exists");
        assert_eq!(fetched.kind, "referral");

        assert!(
            runtime
                .workspace()
                .archive_document(&document.id)
                .await
                .expect("document archive")
        );
        let documents = runtime
            .workspace()
            .list_documents(&client.id)
            .await
            .expect("document list after archive");
        assert!(documents.is_empty());
        let audit = runtime
            .workspace()
            .list_audit_events("document", &document.id)
            .await
            .expect("document audit");
        let archived = audit
            .iter()
            .find(|event| event.action == "archived")
            .expect("document archive audit");
        assert_eq!(archived.client_id.as_deref(), Some(client.id.as_str()));
        assert_eq!(archived.document_id.as_deref(), Some(document.id.as_str()));
        assert_eq!(archived.summary, "Outside referral PDF");
    }

    #[tokio::test]
    async fn document_intake_metadata_defaults_and_backfills_local_path() {
        let runtime = test_runtime().await;
        let client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "File Intake Patient".to_string(),
                ..Default::default()
            })
            .await
            .expect("client upsert");

        let document = runtime
            .workspace()
            .upsert_document(crate::WorkspaceDocumentUpsert {
                client_id: client.id.clone(),
                title: "Scanned Medicare Card JPG".to_string(),
                kind: "medicare insurance card image".to_string(),
                local_path: "/tmp/scanned-medicare-card.jpg".to_string(),
                notes: "metadata only".to_string(),
                ..Default::default()
            })
            .await
            .expect("document upsert");

        assert_eq!(document.original_path, "/tmp/scanned-medicare-card.jpg");
        assert_eq!(document.reference_kind, "local_reference");
        assert!(document.vault_path.is_empty());
        assert_eq!(document.thumbnail_status, "none");
        assert!(document.thumbnail_path.is_empty());
        assert!(document.content_sha256.is_none());
        assert!(document.imported_at.is_none());

        let pool = runtime.workspace().pool.as_ref();
        let schema: Option<String> = sqlx::query_scalar(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'workspace_documents'",
        )
        .fetch_optional(pool)
        .await
        .expect("document schema query");
        let schema = schema.expect("workspace documents schema exists");
        for column in [
            "original_path",
            "reference_kind",
            "vault_path",
            "content_sha256",
            "thumbnail_path",
            "thumbnail_status",
            "thumbnail_mime_type",
            "intake_source",
            "imported_at_ms",
        ] {
            assert!(schema.contains(column), "schema should include {column}");
        }
    }

    #[tokio::test]
    async fn practice_library_lists_practice_records_and_active_patient_links() {
        let runtime = test_runtime().await;
        let active_client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Jordan Client".to_string(),
                ..Default::default()
            })
            .await
            .expect("active client upsert");
        let other_client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Payer Import Owner".to_string(),
                ..Default::default()
            })
            .await
            .expect("other client upsert");

        let active_practice = runtime
            .workspace()
            .upsert_document(crate::WorkspaceDocumentUpsert {
                client_id: active_client.id.clone(),
                title: "Active 837P batch".to_string(),
                kind: "x12 edi".to_string(),
                local_path: "/tmp/active-837p.x12".to_string(),
                scope: "practice".to_string(),
                detected_kind: "EDI 837P".to_string(),
                existence_status: "missing".to_string(),
                metadata_json: r#"{"ediTransaction":"837P professional claim"}"#.to_string(),
                ..Default::default()
            })
            .await
            .expect("active practice document");
        let other_practice = runtime
            .workspace()
            .upsert_document(crate::WorkspaceDocumentUpsert {
                client_id: other_client.id.clone(),
                title: "Clearinghouse 999 acknowledgment".to_string(),
                kind: "x12 edi".to_string(),
                local_path: "/tmp/other-999.x12".to_string(),
                scope: "practice".to_string(),
                detected_kind: "EDI 999".to_string(),
                existence_status: "missing".to_string(),
                metadata_json: r#"{"ediTransaction":"999 implementation ack"}"#.to_string(),
                ..Default::default()
            })
            .await
            .expect("other practice document");
        let patient_doc = runtime
            .workspace()
            .upsert_document(crate::WorkspaceDocumentUpsert {
                client_id: active_client.id.clone(),
                title: "Patient referral PDF".to_string(),
                kind: "pdf".to_string(),
                local_path: "/tmp/referral.pdf".to_string(),
                scope: "patient".to_string(),
                detected_kind: "PDF".to_string(),
                existence_status: "missing".to_string(),
                metadata_json: r#"{"referenceOnly":true}"#.to_string(),
                ..Default::default()
            })
            .await
            .expect("patient document");
        let linked_copy = runtime
            .workspace()
            .upsert_document(crate::WorkspaceDocumentUpsert {
                client_id: active_client.id.clone(),
                title: "Associated 999 acknowledgment".to_string(),
                kind: "x12 edi".to_string(),
                local_path: other_practice.local_path.clone(),
                scope: "patient".to_string(),
                detected_kind: "EDI 999".to_string(),
                existence_status: "missing".to_string(),
                metadata_json: serde_json::json!({
                    "referenceOnly": true,
                    "associatedFromDocumentId": other_practice.id,
                    "associationReviewedBy": "human"
                })
                .to_string(),
                ..Default::default()
            })
            .await
            .expect("linked patient copy");
        let archived_practice = runtime
            .workspace()
            .upsert_document(crate::WorkspaceDocumentUpsert {
                client_id: other_client.id.clone(),
                title: "Archived 835 remittance".to_string(),
                kind: "x12 edi".to_string(),
                local_path: "/tmp/archive-835.x12".to_string(),
                scope: "practice".to_string(),
                detected_kind: "EDI 835".to_string(),
                existence_status: "missing".to_string(),
                ..Default::default()
            })
            .await
            .expect("archived practice document");
        assert!(
            runtime
                .workspace()
                .archive_document(&archived_practice.id)
                .await
                .expect("archive practice document")
        );

        let items = runtime
            .workspace()
            .list_practice_library_items(crate::WorkspacePracticeLibraryFilter {
                active_client_id: Some(active_client.id.clone()),
                query: None,
                limit: Some(20),
            })
            .await
            .expect("practice library list");
        let ids = items
            .iter()
            .map(|item| item.document.id.as_str())
            .collect::<BTreeSet<_>>();
        assert!(ids.contains(active_practice.id.as_str()));
        assert!(ids.contains(other_practice.id.as_str()));
        assert!(!ids.contains(patient_doc.id.as_str()));
        assert!(!ids.contains(linked_copy.id.as_str()));
        assert!(!ids.contains(archived_practice.id.as_str()));

        let linked_item = items
            .iter()
            .find(|item| item.document.id == other_practice.id)
            .expect("linked practice source");
        assert!(linked_item.linked_to_active_client);
        assert_eq!(
            linked_item.linked_document_id.as_deref(),
            Some(linked_copy.id.as_str())
        );
        assert_eq!(linked_item.owner_display_name, "Payer Import Owner");
        assert_eq!(linked_item.reviewed_text_count, 0);
        assert_eq!(linked_item.clip_count, 0);

        let queried = runtime
            .workspace()
            .list_practice_library_items(crate::WorkspacePracticeLibraryFilter {
                active_client_id: Some(active_client.id),
                query: Some("999".to_string()),
                limit: Some(20),
            })
            .await
            .expect("practice library query");
        assert_eq!(queried.len(), 1);
        assert_eq!(queried[0].document.id, other_practice.id);
    }

    #[tokio::test]
    async fn note_archive_audit_keeps_patient_scope() {
        let runtime = test_runtime().await;
        let client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Jordan Client".to_string(),
                ..Default::default()
            })
            .await
            .expect("client upsert");
        let note = runtime
            .workspace()
            .upsert_note(crate::WorkspaceNoteUpsert {
                client_id: client.id.clone(),
                title: "Archive note".to_string(),
                kind: "progress".to_string(),
                body: "Fake note for archive audit.".to_string(),
                status: "draft".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("note upsert");

        assert!(
            runtime
                .workspace()
                .archive_note(&note.id, "human")
                .await
                .expect("note archive")
        );

        let notes = runtime
            .workspace()
            .list_notes(&client.id)
            .await
            .expect("notes after archive");
        assert!(notes.is_empty());
        let audit = runtime
            .workspace()
            .list_audit_events("note", &note.id)
            .await
            .expect("note audit");
        let archived = audit
            .iter()
            .find(|event| event.action == "archived")
            .expect("note archive audit");
        assert_eq!(archived.client_id.as_deref(), Some(client.id.as_str()));
        assert_eq!(archived.note_id.as_deref(), Some(note.id.as_str()));
        assert_eq!(archived.summary, "Archive note");
    }

    #[tokio::test]
    async fn artifact_derivative_persists_status_and_packet_trace() {
        let runtime = test_runtime().await;
        let client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Jordan Client".to_string(),
                ..Default::default()
            })
            .await
            .expect("client upsert");
        let note = runtime
            .workspace()
            .upsert_note(crate::WorkspaceNoteUpsert {
                client_id: client.id.clone(),
                title: "Visit note".to_string(),
                kind: "progress".to_string(),
                body: "Human note.".to_string(),
                status: "draft".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("note upsert");
        let document = runtime
            .workspace()
            .upsert_document(crate::WorkspaceDocumentUpsert {
                client_id: client.id.clone(),
                title: "Fake dictation audio".to_string(),
                kind: "audio".to_string(),
                local_path: "/tmp/fake-dictation.m4a".to_string(),
                scope: "patient".to_string(),
                detected_kind: "audio".to_string(),
                mime_type: Some("audio/mp4".to_string()),
                existence_status: "missing".to_string(),
                metadata_json: r#"{"referenceOnly":true}"#.to_string(),
                ..Default::default()
            })
            .await
            .expect("document upsert");

        let derivative = runtime
            .workspace()
            .upsert_artifact_derivative(crate::WorkspaceArtifactDerivativeUpsert {
                document_id: document.id.clone(),
                client_id: client.id.clone(),
                note_id: Some(note.id.clone()),
                kind: "transcript".to_string(),
                title: "Dictation transcript excerpt".to_string(),
                body: "Human pasted transcript for follow-up planning.".to_string(),
                review_status: "draft".to_string(),
                source_method: "human_pasted".to_string(),
                timestamp_range: "00:10-00:42".to_string(),
                tags: "dictation,follow-up".to_string(),
                metadata_json: r#"{"humanProvided":true}"#.to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("derivative upsert");
        assert_eq!(derivative.document_id, document.id);
        assert_eq!(derivative.review_status, "draft");
        assert_eq!(derivative.source_method, "human_pasted");

        let derivatives = runtime
            .workspace()
            .list_artifact_derivatives(crate::WorkspaceArtifactDerivativeFilter {
                client_id: client.id.clone(),
                document_id: Some(document.id.clone()),
                note_id: Some(note.id.clone()),
                limit: Some(10),
            })
            .await
            .expect("derivative list");
        assert_eq!(derivatives.len(), 1);
        assert_eq!(
            derivatives[0].body,
            "Human pasted transcript for follow-up planning."
        );

        let reviewed = runtime
            .workspace()
            .update_artifact_derivative_status(crate::WorkspaceArtifactDerivativeStatusUpdate {
                derivative_id: derivative.id.clone(),
                review_status: "human_reviewed".to_string(),
                actor: "human".to_string(),
            })
            .await
            .expect("derivative status")
            .expect("derivative exists");
        assert_eq!(reviewed.review_status, "human_reviewed");

        let packet =
            runtime
                .workspace()
                .create_context_packet(crate::WorkspaceContextPacketCreate {
                    client_id: client.id.clone(),
                    encounter_id: None,
                    note_id: Some(note.id.clone()),
                    human_request: "Draft a follow-up plan from selected context.".to_string(),
                    selected_artifact_ids_json: format!(r#"["{}"]"#, document.id),
                    selected_derivative_ids_json: format!(r#"["{}"]"#, derivative.id),
                    selected_clip_ids_json: "[]".to_string(),
                    artifact_summary: "1 selected artifact(s): patient audio: Fake dictation audio"
                        .to_string(),
                    derivative_summary:
                        "1 selected derivative(s): Transcript: Dictation transcript excerpt"
                            .to_string(),
                    clip_summary: "0 selected clips".to_string(),
                    chart_context_summary: "patient Jordan Client; note Visit note".to_string(),
                    context_envelope_json: valid_packet_envelope(
                        "Draft a follow-up plan from selected context.",
                        &format!(r#"["{}"]"#, document.id),
                        &format!(r#"["{}"]"#, derivative.id),
                        "[]",
                    ),
                    status: "sent".to_string(),
                    actor: "human".to_string(),
                    ..Default::default()
                })
                .await
                .expect("packet create");
        assert_eq!(
            packet.selected_derivative_ids_json,
            format!(r#"["{}"]"#, derivative.id)
        );
        assert!(packet.derivative_summary.contains("Transcript"));
        assert_eq!(packet.context_envelope_sha256.len(), 64);

        let packets = runtime
            .workspace()
            .list_context_packets(crate::WorkspaceContextPacketFilter {
                client_id: client.id.clone(),
                note_id: Some(note.id.clone()),
                limit: Some(10),
            })
            .await
            .expect("packet list");
        assert_eq!(packets.len(), 1);
        assert_eq!(
            packets[0].selected_derivative_ids_json,
            packet.selected_derivative_ids_json
        );
        assert!(
            packets[0]
                .derivative_summary
                .contains("Dictation transcript")
        );
        assert!(
            packets[0]
                .context_envelope_json
                .contains(r#""assemblyVersion":"test-v1""#)
        );
        assert_eq!(
            packets[0].context_envelope_sha256,
            packet.context_envelope_sha256
        );
        let replay = runtime
            .workspace()
            .get_context_packet_replay(crate::WorkspaceContextPacketReplayFilter {
                client_id: client.id.clone(),
                packet_id: packet.id.clone(),
                context_envelope_sha256: packet.context_envelope_sha256.clone(),
            })
            .await
            .expect("packet replay")
            .expect("packet replay exists");
        assert_eq!(replay.context_envelope_json, packet.context_envelope_json);
        assert_eq!(
            replay.context_envelope_sha256,
            packet.context_envelope_sha256
        );
        let wrong_hash = runtime
            .workspace()
            .get_context_packet_replay(crate::WorkspaceContextPacketReplayFilter {
                client_id: client.id.clone(),
                packet_id: packet.id.clone(),
                context_envelope_sha256: "wrong-envelope-hash".to_string(),
            })
            .await
            .expect("packet replay with wrong hash");
        assert!(wrong_hash.is_none());

        let audit = runtime
            .workspace()
            .list_audit_events("artifact_derivative", &derivative.id)
            .await
            .expect("audit list");
        assert_eq!(audit.len(), 2);
        assert!(audit.iter().any(|event| event.action == "created"));
        assert!(audit.iter().any(|event| event.action == "status_changed"));
    }

    #[tokio::test]
    async fn context_clip_persists_status_and_packet_trace() {
        let runtime = test_runtime().await;
        let client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Jordan Client".to_string(),
                ..Default::default()
            })
            .await
            .expect("client upsert");
        let note = runtime
            .workspace()
            .upsert_note(crate::WorkspaceNoteUpsert {
                client_id: client.id.clone(),
                title: "Visit note".to_string(),
                kind: "progress".to_string(),
                body: "Human note.".to_string(),
                status: "draft".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("note upsert");
        let document = runtime
            .workspace()
            .upsert_document(crate::WorkspaceDocumentUpsert {
                client_id: client.id.clone(),
                encounter_id: None,
                title: "Fake 837P batch".to_string(),
                kind: "EDI".to_string(),
                local_path: "/tmp/fake-837p.x12".to_string(),
                scope: "practice".to_string(),
                detected_kind: "EDI".to_string(),
                existence_status: "missing".to_string(),
                ..Default::default()
            })
            .await
            .expect("document upsert");
        let derivative = runtime
            .workspace()
            .upsert_artifact_derivative(crate::WorkspaceArtifactDerivativeUpsert {
                client_id: client.id.clone(),
                document_id: document.id.clone(),
                note_id: Some(note.id.clone()),
                kind: "EDI summary".to_string(),
                title: "Billing team 837P note".to_string(),
                body: "Fake 837P batch accepted for QA only. Claim count looked complete."
                    .to_string(),
                review_status: "human_reviewed".to_string(),
                source_method: "human_typed".to_string(),
                segment_label: "ST*837".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("derivative upsert");
        let clip = runtime
            .workspace()
            .upsert_context_clip(crate::WorkspaceContextClipUpsert {
                client_id: client.id.clone(),
                document_id: document.id.clone(),
                derivative_id: derivative.id.clone(),
                note_id: Some(note.id.clone()),
                kind: "EDI summary excerpt".to_string(),
                title: "837P acceptance excerpt".to_string(),
                body: "Fake 837P batch accepted for QA only.".to_string(),
                review_status: "draft".to_string(),
                source_method: "human_selected".to_string(),
                line_range: "1-1".to_string(),
                segment_label: "ST*837".to_string(),
                tags: "billing,qa".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("clip upsert");
        assert_eq!(clip.derivative_id, derivative.id);
        assert_eq!(clip.document_id, document.id);
        assert_eq!(clip.body, "Fake 837P batch accepted for QA only.");

        let clips = runtime
            .workspace()
            .list_context_clips(crate::WorkspaceContextClipFilter {
                client_id: client.id.clone(),
                derivative_id: Some(derivative.id.clone()),
                document_id: Some(document.id.clone()),
                note_id: Some(note.id.clone()),
                limit: Some(10),
            })
            .await
            .expect("clip list");
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].title, "837P acceptance excerpt");

        let reviewed = runtime
            .workspace()
            .update_context_clip_status(crate::WorkspaceContextClipStatusUpdate {
                clip_id: clip.id.clone(),
                review_status: "human_reviewed".to_string(),
                actor: "human".to_string(),
            })
            .await
            .expect("clip status")
            .expect("clip exists");
        assert_eq!(reviewed.review_status, "human_reviewed");

        let packet = runtime
            .workspace()
            .create_context_packet(crate::WorkspaceContextPacketCreate {
                client_id: client.id.clone(),
                encounter_id: None,
                note_id: Some(note.id.clone()),
                human_request: "Review selected billing excerpt.".to_string(),
                selected_artifact_ids_json: "[]".to_string(),
                selected_derivative_ids_json: "[]".to_string(),
                selected_clip_ids_json: format!(r#"["{}"]"#, clip.id),
                artifact_summary: "0 selected artifacts".to_string(),
                derivative_summary: "0 selected derivatives".to_string(),
                clip_summary: "1 selected clip(s): 837P acceptance excerpt".to_string(),
                chart_context_summary: "patient Jordan Client; note Visit note".to_string(),
                context_envelope_json: valid_packet_envelope(
                    "Review selected billing excerpt.",
                    "[]",
                    "[]",
                    &format!(r#"["{}"]"#, clip.id),
                ),
                status: "sent".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("packet create");
        assert_eq!(packet.selected_clip_ids_json, format!(r#"["{}"]"#, clip.id));
        assert!(packet.clip_summary.contains("837P acceptance excerpt"));
        assert!(packet.context_envelope_json.contains("ack accepted"));

        let audit = runtime
            .workspace()
            .list_audit_events("context_clip", &clip.id)
            .await
            .expect("audit list");
        assert_eq!(audit.len(), 2);
        assert!(audit.iter().any(|event| event.action == "created"));
        assert!(audit.iter().any(|event| event.action == "status_changed"));
    }

    #[tokio::test]
    async fn derivative_and_clip_lists_require_active_parent_graph() {
        let runtime = test_runtime().await;
        let client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Jordan Client".to_string(),
                ..Default::default()
            })
            .await
            .expect("client upsert");
        let note = runtime
            .workspace()
            .upsert_note(crate::WorkspaceNoteUpsert {
                client_id: client.id.clone(),
                title: "Visit note".to_string(),
                kind: "progress".to_string(),
                body: "Human note.".to_string(),
                status: "draft".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("note upsert");
        let document = runtime
            .workspace()
            .upsert_document(crate::WorkspaceDocumentUpsert {
                client_id: client.id.clone(),
                title: "Fake 837P batch".to_string(),
                kind: "EDI".to_string(),
                local_path: "/tmp/fake-837p.x12".to_string(),
                scope: "practice".to_string(),
                detected_kind: "EDI".to_string(),
                existence_status: "missing".to_string(),
                ..Default::default()
            })
            .await
            .expect("document upsert");
        let derivative = runtime
            .workspace()
            .upsert_artifact_derivative(crate::WorkspaceArtifactDerivativeUpsert {
                client_id: client.id.clone(),
                document_id: document.id.clone(),
                note_id: Some(note.id.clone()),
                kind: "EDI summary".to_string(),
                title: "Billing team note".to_string(),
                body: "Fake billing summary.".to_string(),
                review_status: "draft".to_string(),
                source_method: "human_typed".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("derivative upsert");
        let clip = runtime
            .workspace()
            .upsert_context_clip(crate::WorkspaceContextClipUpsert {
                client_id: client.id.clone(),
                document_id: document.id.clone(),
                derivative_id: derivative.id.clone(),
                note_id: Some(note.id.clone()),
                kind: "EDI summary excerpt".to_string(),
                title: "Acceptance excerpt".to_string(),
                body: "Fake accepted excerpt.".to_string(),
                review_status: "draft".to_string(),
                source_method: "human_selected".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("clip upsert");

        assert_eq!(
            runtime
                .workspace()
                .list_artifact_derivatives(crate::WorkspaceArtifactDerivativeFilter {
                    client_id: client.id.clone(),
                    document_id: None,
                    note_id: None,
                    limit: Some(10),
                })
                .await
                .expect("derivative list")
                .len(),
            1
        );
        assert_eq!(
            runtime
                .workspace()
                .list_context_clips(crate::WorkspaceContextClipFilter {
                    client_id: client.id.clone(),
                    derivative_id: None,
                    document_id: None,
                    note_id: None,
                    limit: Some(10),
                })
                .await
                .expect("clip list")
                .len(),
            1
        );

        runtime
            .workspace()
            .update_artifact_derivative_status(crate::WorkspaceArtifactDerivativeStatusUpdate {
                derivative_id: derivative.id.clone(),
                review_status: "archived".to_string(),
                actor: "human".to_string(),
            })
            .await
            .expect("archive derivative")
            .expect("derivative exists");
        assert_eq!(
            runtime
                .workspace()
                .list_context_clips(crate::WorkspaceContextClipFilter {
                    client_id: client.id.clone(),
                    derivative_id: Some(derivative.id.clone()),
                    document_id: Some(document.id.clone()),
                    note_id: Some(note.id.clone()),
                    limit: Some(10),
                })
                .await
                .expect("clip list after derivative archive")
                .len(),
            0,
            "clip {} should not be selectable after its derivative is archived",
            clip.id
        );

        let document_two = runtime
            .workspace()
            .upsert_document(crate::WorkspaceDocumentUpsert {
                client_id: client.id.clone(),
                title: "Fake 999 ack".to_string(),
                kind: "EDI".to_string(),
                local_path: "/tmp/fake-999.x12".to_string(),
                scope: "practice".to_string(),
                detected_kind: "EDI 999".to_string(),
                existence_status: "missing".to_string(),
                ..Default::default()
            })
            .await
            .expect("document two upsert");
        runtime
            .workspace()
            .upsert_artifact_derivative(crate::WorkspaceArtifactDerivativeUpsert {
                client_id: client.id.clone(),
                document_id: document_two.id.clone(),
                note_id: Some(note.id.clone()),
                kind: "EDI summary".to_string(),
                title: "999 ack summary".to_string(),
                body: "Fake ack summary.".to_string(),
                review_status: "draft".to_string(),
                source_method: "human_typed".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("derivative two upsert");
        assert!(
            runtime
                .workspace()
                .archive_document(&document_two.id)
                .await
                .expect("archive document")
        );
        assert_eq!(
            runtime
                .workspace()
                .list_artifact_derivatives(crate::WorkspaceArtifactDerivativeFilter {
                    client_id: client.id.clone(),
                    document_id: Some(document_two.id.clone()),
                    note_id: Some(note.id.clone()),
                    limit: Some(10),
                })
                .await
                .expect("derivative list after document archive")
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn context_packet_rejects_stale_or_cross_client_selected_context() {
        let runtime = test_runtime().await;
        let client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Jordan Client".to_string(),
                ..Default::default()
            })
            .await
            .expect("client upsert");
        let note = runtime
            .workspace()
            .upsert_note(crate::WorkspaceNoteUpsert {
                client_id: client.id.clone(),
                title: "Visit note".to_string(),
                kind: "progress".to_string(),
                body: "Human note.".to_string(),
                status: "draft".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("note upsert");
        let other_client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Other Client".to_string(),
                ..Default::default()
            })
            .await
            .expect("other client upsert");
        let other_document = runtime
            .workspace()
            .upsert_document(crate::WorkspaceDocumentUpsert {
                client_id: other_client.id.clone(),
                title: "Other fake claim batch".to_string(),
                kind: "EDI".to_string(),
                local_path: "/tmp/other-837p.x12".to_string(),
                scope: "practice".to_string(),
                detected_kind: "EDI".to_string(),
                existence_status: "missing".to_string(),
                ..Default::default()
            })
            .await
            .expect("other document upsert");
        let other_derivative = runtime
            .workspace()
            .upsert_artifact_derivative(crate::WorkspaceArtifactDerivativeUpsert {
                client_id: other_client.id.clone(),
                document_id: other_document.id.clone(),
                kind: "EDI summary".to_string(),
                title: "Other fake summary".to_string(),
                body: "Other fake summary.".to_string(),
                review_status: "draft".to_string(),
                source_method: "human_typed".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("other derivative upsert");
        let other_clip = runtime
            .workspace()
            .upsert_context_clip(crate::WorkspaceContextClipUpsert {
                client_id: other_client.id.clone(),
                document_id: other_document.id.clone(),
                derivative_id: other_derivative.id.clone(),
                kind: "EDI summary excerpt".to_string(),
                title: "Other fake clip".to_string(),
                body: "Other fake clip.".to_string(),
                review_status: "draft".to_string(),
                source_method: "human_selected".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("other clip upsert");

        let packet = |artifact_ids: String, derivative_ids: String, clip_ids: String| {
            crate::WorkspaceContextPacketCreate {
                client_id: client.id.clone(),
                encounter_id: None,
                note_id: Some(note.id.clone()),
                human_request: "Review selected context.".to_string(),
                selected_artifact_ids_json: artifact_ids.clone(),
                selected_derivative_ids_json: derivative_ids.clone(),
                selected_clip_ids_json: clip_ids.clone(),
                artifact_summary: "test artifacts".to_string(),
                derivative_summary: "test derivatives".to_string(),
                clip_summary: "test clips".to_string(),
                chart_context_summary: "patient Jordan Client; note Visit note".to_string(),
                context_envelope_json: valid_packet_envelope(
                    "Review selected context.",
                    &artifact_ids,
                    &derivative_ids,
                    &clip_ids,
                ),
                status: "sent".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            }
        };

        let err = runtime
            .workspace()
            .create_context_packet(packet(
                format!(r#"["{}"]"#, other_document.id),
                "[]".to_string(),
                "[]".to_string(),
            ))
            .await
            .expect_err("cross-client artifact should be rejected")
            .to_string();
        assert!(err.contains("selected artifact"));

        let err = runtime
            .workspace()
            .create_context_packet(packet(
                "[]".to_string(),
                format!(r#"["{}"]"#, other_derivative.id),
                "[]".to_string(),
            ))
            .await
            .expect_err("cross-client derivative should be rejected")
            .to_string();
        assert!(err.contains("selected derivative"));

        let err = runtime
            .workspace()
            .create_context_packet(packet(
                "[]".to_string(),
                "[]".to_string(),
                format!(r#"["{}"]"#, other_clip.id),
            ))
            .await
            .expect_err("cross-client clip should be rejected")
            .to_string();
        assert!(err.contains("selected clip"));

        let document = runtime
            .workspace()
            .upsert_document(crate::WorkspaceDocumentUpsert {
                client_id: client.id.clone(),
                title: "Fake local 837P batch".to_string(),
                kind: "EDI".to_string(),
                local_path: "/tmp/fake-local-837p.x12".to_string(),
                scope: "practice".to_string(),
                detected_kind: "EDI".to_string(),
                existence_status: "missing".to_string(),
                ..Default::default()
            })
            .await
            .expect("document upsert");
        let derivative = runtime
            .workspace()
            .upsert_artifact_derivative(crate::WorkspaceArtifactDerivativeUpsert {
                client_id: client.id.clone(),
                document_id: document.id.clone(),
                kind: "EDI summary".to_string(),
                title: "Local fake summary".to_string(),
                body: "Local fake summary.".to_string(),
                review_status: "draft".to_string(),
                source_method: "human_typed".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("derivative upsert");
        let clip = runtime
            .workspace()
            .upsert_context_clip(crate::WorkspaceContextClipUpsert {
                client_id: client.id.clone(),
                document_id: document.id.clone(),
                derivative_id: derivative.id.clone(),
                kind: "EDI summary excerpt".to_string(),
                title: "Local fake clip".to_string(),
                body: "Local fake clip.".to_string(),
                review_status: "draft".to_string(),
                source_method: "human_selected".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("clip upsert");
        assert!(
            runtime
                .workspace()
                .archive_document(&document.id)
                .await
                .expect("archive document")
        );
        let err = runtime
            .workspace()
            .create_context_packet(packet(
                "[]".to_string(),
                "[]".to_string(),
                format!(r#"["{}"]"#, clip.id),
            ))
            .await
            .expect_err("clip with archived parent document should be rejected")
            .to_string();
        assert!(err.contains("selected clip"));
    }

    #[tokio::test]
    async fn context_packet_rejects_invalid_or_mismatched_envelopes() {
        let runtime = test_runtime().await;
        let client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Jordan Client".to_string(),
                ..Default::default()
            })
            .await
            .expect("client upsert");
        let note = runtime
            .workspace()
            .upsert_note(crate::WorkspaceNoteUpsert {
                client_id: client.id.clone(),
                title: "Visit note".to_string(),
                kind: "progress".to_string(),
                body: "Human note.".to_string(),
                status: "draft".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("note upsert");
        let packet = |context_envelope_json: String| crate::WorkspaceContextPacketCreate {
            client_id: client.id.clone(),
            encounter_id: None,
            note_id: Some(note.id.clone()),
            human_request: "Review selected context.".to_string(),
            selected_artifact_ids_json: "[]".to_string(),
            selected_derivative_ids_json: "[]".to_string(),
            selected_clip_ids_json: "[]".to_string(),
            artifact_summary: "0 selected artifacts".to_string(),
            derivative_summary: "0 selected derivatives".to_string(),
            clip_summary: "0 selected clips".to_string(),
            chart_context_summary: "patient Jordan Client; note Visit note".to_string(),
            context_envelope_json,
            status: "sent".to_string(),
            actor: "human".to_string(),
            ..Default::default()
        };

        let err = runtime
            .workspace()
            .create_context_packet(packet("{not json".to_string()))
            .await
            .expect_err("invalid JSON envelope should be rejected")
            .to_string();
        assert!(err.contains("valid JSON"));

        let err = runtime
            .workspace()
            .create_context_packet(packet(valid_packet_envelope(
                "Different request.",
                "[]",
                "[]",
                "[]",
            )))
            .await
            .expect_err("mismatched request should be rejected")
            .to_string();
        assert!(err.contains("humanRequest"));

        let mut include_documents = serde_json::from_str::<serde_json::Value>(
            &valid_packet_envelope("Review selected context.", "[]", "[]", "[]"),
        )
        .expect("valid envelope");
        include_documents["includeDocuments"] = serde_json::Value::Bool(true);
        let err = runtime
            .workspace()
            .create_context_packet(packet(include_documents.to_string()))
            .await
            .expect_err("includeDocuments true should be rejected")
            .to_string();
        assert!(err.contains("includeDocuments"));

        let err = runtime
            .workspace()
            .create_context_packet(packet(valid_packet_envelope(
                "Review selected context.",
                r#"["doc-mismatch"]"#,
                "[]",
                "[]",
            )))
            .await
            .expect_err("mismatched selected ids should be rejected")
            .to_string();
        assert!(err.contains("selected artifact"));
    }

    #[tokio::test]
    async fn workspace_task_create_list_and_status_update_persists() {
        let runtime = test_runtime().await;
        let client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Jordan Client".to_string(),
                ..Default::default()
            })
            .await
            .expect("client upsert");
        let note = runtime
            .workspace()
            .upsert_note(crate::WorkspaceNoteUpsert {
                client_id: client.id.clone(),
                title: "Visit note".to_string(),
                kind: "note".to_string(),
                body: "Human note body.".to_string(),
                status: "draft".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("note upsert");
        let document = runtime
            .workspace()
            .upsert_document(crate::WorkspaceDocumentUpsert {
                client_id: client.id.clone(),
                title: "Referral metadata".to_string(),
                kind: "referral".to_string(),
                local_path: "/tmp/referral.pdf".to_string(),
                ..Default::default()
            })
            .await
            .expect("document upsert");

        let task = runtime
            .workspace()
            .upsert_task(crate::WorkspaceTaskUpsert {
                client_id: client.id.clone(),
                note_id: Some(note.id.clone()),
                document_id: Some(document.id.clone()),
                title: "Request outside records".to_string(),
                details: "Call the referring office.".to_string(),
                kind: "follow-up".to_string(),
                status: crate::WorkspaceTaskStatus::Open,
                priority: crate::WorkspaceTaskPriority::High,
                due_date: Some("2026-06-12".to_string()),
                assigned_to: Some("front desk".to_string()),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("task upsert");

        assert_eq!(
            runtime
                .workspace()
                .list_tasks(&client.id)
                .await
                .expect("task list"),
            vec![task.clone()]
        );

        let done = runtime
            .workspace()
            .update_task_status(crate::WorkspaceTaskStatusUpdate {
                client_id: client.id.clone(),
                task_id: task.id.clone(),
                status: crate::WorkspaceTaskStatus::Done,
                actor: "human".to_string(),
            })
            .await
            .expect("status update")
            .expect("task exists");
        assert_eq!(done.status, crate::WorkspaceTaskStatus::Done);
        assert!(done.completed_at.is_some());
        assert_eq!(
            runtime
                .workspace()
                .list_tasks(&client.id)
                .await
                .expect("task list after update"),
            vec![done]
        );
        let audit = runtime
            .workspace()
            .list_audit_events("task", &task.id)
            .await
            .expect("task audit");
        assert!(audit.iter().any(|event| event.action == "created"));
        assert!(audit.iter().any(|event| event.action == "status_changed"));
    }

    #[tokio::test]
    async fn workspace_task_status_update_does_not_cross_client_boundary() {
        let runtime = test_runtime().await;
        let client_a = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Client A".to_string(),
                ..Default::default()
            })
            .await
            .expect("client a upsert");
        let client_b = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Client B".to_string(),
                ..Default::default()
            })
            .await
            .expect("client b upsert");
        let task = runtime
            .workspace()
            .upsert_task(crate::WorkspaceTaskUpsert {
                client_id: client_a.id.clone(),
                title: "Client A task".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("task upsert");

        let cross_client = runtime
            .workspace()
            .update_task_status(crate::WorkspaceTaskStatusUpdate {
                client_id: client_b.id.clone(),
                task_id: task.id.clone(),
                status: crate::WorkspaceTaskStatus::Done,
                actor: "human".to_string(),
            })
            .await
            .expect("cross-client status update should be safe");
        assert!(cross_client.is_none());
        assert!(
            runtime
                .workspace()
                .list_tasks(&client_b.id)
                .await
                .expect("client b task list")
                .is_empty()
        );
        assert_eq!(
            runtime
                .workspace()
                .list_tasks(&client_a.id)
                .await
                .expect("client a task list")[0]
                .status,
            crate::WorkspaceTaskStatus::Open
        );
    }

    #[tokio::test]
    async fn encounter_signing_addendum_and_audit_flow() {
        let runtime = test_runtime().await;
        let client = runtime
            .workspace()
            .upsert_client(crate::WorkspaceClientUpsert {
                display_name: "Jordan Patient".to_string(),
                ..Default::default()
            })
            .await
            .expect("client upsert");
        let encounter = runtime
            .workspace()
            .upsert_encounter(crate::WorkspaceEncounterUpsert {
                client_id: client.id.clone(),
                kind: "visit".to_string(),
                title: "Initial visit".to_string(),
                status: "open".to_string(),
                ..Default::default()
            })
            .await
            .expect("encounter upsert");
        let note = runtime
            .workspace()
            .upsert_note(crate::WorkspaceNoteUpsert {
                client_id: client.id.clone(),
                encounter_id: Some(encounter.id.clone()),
                title: "Initial note".to_string(),
                kind: "progress".to_string(),
                body: "Signed note body".to_string(),
                status: "draft".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await
            .expect("note upsert");

        let signature = runtime
            .workspace()
            .sign_note(crate::WorkspaceNoteSign {
                note_id: note.id.clone(),
                signer: "local human".to_string(),
            })
            .await
            .expect("note sign");
        assert_eq!(signature.note_id, note.id);
        assert_eq!(signature.revision, note.current_revision);
        assert_eq!(signature.body_sha256.len(), 64);

        let signed = runtime
            .workspace()
            .get_note(&note.id)
            .await
            .expect("note get")
            .expect("note exists");
        assert_eq!(signed.status, NOTE_STATUS_SIGNED);

        let overwrite = runtime
            .workspace()
            .upsert_note(crate::WorkspaceNoteUpsert {
                id: Some(note.id.clone()),
                client_id: client.id.clone(),
                encounter_id: Some(encounter.id.clone()),
                title: "Initial note".to_string(),
                kind: "progress".to_string(),
                body: "Should be rejected".to_string(),
                status: "draft".to_string(),
                actor: "human".to_string(),
                ..Default::default()
            })
            .await;
        assert!(overwrite.is_err());

        let proposal = runtime
            .workspace()
            .create_note_proposal(crate::WorkspaceNoteProposalCreate {
                note_id: note.id.clone(),
                base_revision: note.current_revision,
                proposed_body: "Replacement proposal".to_string(),
                summary: "unsafe replacement".to_string(),
                source_thread_id: Some("thread-1".to_string()),
                source_turn_id: Some("turn-1".to_string()),
                ..Default::default()
            })
            .await;
        assert!(proposal.is_err());

        let addendum = runtime
            .workspace()
            .create_note_addendum(crate::WorkspaceNoteAddendumCreate {
                note_id: note.id.clone(),
                base_revision: note.current_revision,
                body: "Addendum text".to_string(),
                author: "local human".to_string(),
                source_thread_id: None,
                source_turn_id: None,
            })
            .await
            .expect("addendum create");
        assert_eq!(addendum.base_revision, note.current_revision);
        assert_eq!(addendum.body, "Addendum text");

        let addended = runtime
            .workspace()
            .get_note(&note.id)
            .await
            .expect("note get")
            .expect("note exists");
        assert_eq!(addended.status, NOTE_STATUS_ADDENDED);
        assert_eq!(addended.body, "Signed note body");

        let audit = runtime
            .workspace()
            .list_audit_events_filtered(crate::WorkspaceAuditEventFilter {
                client_id: Some(client.id.clone()),
                note_id: Some(note.id.clone()),
                ..Default::default()
            })
            .await
            .expect("audit list");
        assert!(audit.iter().any(|event| event.action == "signed"));
        assert!(
            audit
                .iter()
                .any(|event| event.action == "created" && event.entity_type == "note_addendum")
        );
    }

    fn valid_packet_envelope(
        human_request: &str,
        selected_artifact_ids_json: &str,
        selected_derivative_ids_json: &str,
        selected_clip_ids_json: &str,
    ) -> String {
        let selected_artifact_ids: Vec<String> =
            serde_json::from_str(selected_artifact_ids_json).expect("artifact ids json");
        let selected_derivative_ids: Vec<String> =
            serde_json::from_str(selected_derivative_ids_json).expect("derivative ids json");
        let selected_clip_ids: Vec<String> =
            serde_json::from_str(selected_clip_ids_json).expect("clip ids json");
        let selected_clips = selected_clip_ids
            .iter()
            .map(|id| serde_json::json!({ "id": id, "body": "ack accepted" }))
            .collect::<Vec<_>>();
        serde_json::json!({
            "assemblyVersion": "test-v1",
            "sourceMode": "ctrl_g_handoff",
            "includeDocuments": false,
            "humanRequest": human_request,
            "ids": {
                "selectedArtifactIds": selected_artifact_ids,
                "selectedDerivativeIds": selected_derivative_ids,
                "selectedClipIds": selected_clip_ids,
            },
            "safety": [
                "read-only context packet; do not mutate workspace records",
                "do not sign notes, submit claims, send payer communications, or overwrite saved data"
            ],
            "promptSnapshot": "Medical workspace context selected. Read-only packet.",
            "selectedClips": selected_clips,
        })
        .to_string()
    }
}
