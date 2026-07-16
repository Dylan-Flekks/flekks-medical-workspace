use super::super::PlanResult;
use super::super::stale;
use super::super::validation;
use super::context_source;
use super::safe_body;
use super::safe_label;
use serde_json::Value;
use sha2::Digest;
use sha2::Sha256;
use sqlx::Row;

pub(super) async fn read_selected_context(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    execution: &crate::WorkspacePlanningGuideExecutionBinding,
    max_records: u32,
) -> PlanResult<Vec<crate::WorkspacePlanningContextSource>> {
    let checkpoint = sqlx::query(
        "SELECT revision, draft_json, content_sha256 FROM workspace_draft_checkpoints WHERE id = ? AND client_id = ?",
    )
    .bind(&execution.source_checkpoint_id)
    .bind(&execution.client_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| stale("workspace planning source checkpoint is no longer available"))?;
    let revision: i64 = checkpoint.try_get("revision")?;
    let draft_json: String = checkpoint.try_get("draft_json")?;
    let checkpoint_sha256: String = checkpoint.try_get("content_sha256")?;
    if revision != execution.source_checkpoint_revision
        || checkpoint_sha256 != execution.source_checkpoint_sha256
    {
        return Err(stale(
            "workspace planning selected context no longer matches the claimed checkpoint",
        ));
    }
    let draft: Value = serde_json::from_str(&draft_json)?;
    let file_ids = selected_ids(&draft, "selectedFileIds", "fileIds")?;
    let derivative_ids = selected_ids(&draft, "selectedReviewedTextIds", "reviewedTextIds")?;
    let clip_ids = selected_ids(&draft, "selectedClipIds", "clipIds")?;
    let selected_count = file_ids.len() + derivative_ids.len() + clip_ids.len();
    if selected_count > max_records as usize {
        return Err(validation(format!(
            "checkpoint selects {selected_count} context records but this read permits {max_records}; request a larger bounded read or reduce the checkpoint selection"
        )));
    }

    let mut sources = vec![crate::WorkspacePlanningContextSource {
        source_entity_type: "draft_checkpoint".to_string(),
        source_entity_id: execution.source_checkpoint_id.clone(),
        source_revision: Some(revision),
        display_label: "Exact planning checkpoint".to_string(),
        snapshot_json: draft_json,
        content_sha256: checkpoint_sha256,
    }];
    for id in file_ids {
        sources.push(document_source(tx, execution, &id).await?);
    }
    for id in derivative_ids {
        sources.push(derivative_source(tx, execution, &id).await?);
    }
    for id in clip_ids {
        sources.push(clip_source(tx, execution, &id).await?);
    }
    Ok(sources)
}

fn selected_ids(draft: &Value, top_level: &str, nested: &str) -> PlanResult<Vec<String>> {
    let value = draft.get(top_level).or_else(|| {
        draft
            .get("contextPlan")?
            .get("selectedContext")?
            .get(nested)
    });
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values = value.as_array().ok_or_else(|| {
        validation(format!(
            "workspace planning checkpoint field `{top_level}` must be an array"
        ))
    })?;
    let mut ids = Vec::with_capacity(values.len());
    for value in values {
        let id = value
            .as_str()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                validation(format!(
                    "workspace planning checkpoint field `{top_level}` contains an invalid id"
                ))
            })?;
        if ids.iter().any(|existing| existing == id) {
            return Err(validation(format!(
                "workspace planning checkpoint field `{top_level}` contains duplicate id `{id}`"
            )));
        }
        ids.push(id.to_string());
    }
    Ok(ids)
}

async fn document_source(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    execution: &crate::WorkspacePlanningGuideExecutionBinding,
    id: &str,
) -> PlanResult<crate::WorkspacePlanningContextSource> {
    let row = sqlx::query(
        r#"
SELECT id, encounter_id, title, kind, notes, detected_kind, mime_type,
       file_size_bytes, modified_at_ms, sha256, tags, source_label,
       existence_status, metadata_json, reference_kind, content_sha256,
       intake_source, imported_at_ms, created_at_ms, updated_at_ms
FROM workspace_documents
WHERE id = ? AND client_id = ? AND archived_at_ms IS NULL
        "#,
    )
    .bind(id)
    .bind(&execution.client_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        stale(format!(
            "selected document `{id}` is unavailable for this patient"
        ))
    })?;
    let title: String = row.try_get("title")?;
    let notes: String = row.try_get("notes")?;
    let metadata: String = row.try_get("metadata_json")?;
    let (title, title_paths_redacted, title_truncated) = safe_label(&title);
    let (notes, notes_paths_redacted, notes_truncated) = safe_body(&notes);
    let (metadata, metadata_paths_redacted, metadata_truncated) = safe_body(&metadata);
    let snapshot = serde_json::json!({
        "id": id,
        "client_id": execution.client_id,
        "encounter_id": row.try_get::<Option<String>, _>("encounter_id")?,
        "title": title,
        "title_truncated": title_truncated,
        "title_local_paths_redacted": title_paths_redacted,
        "kind": row.try_get::<String, _>("kind")?,
        "detected_kind": row.try_get::<String, _>("detected_kind")?,
        "mime_type": row.try_get::<Option<String>, _>("mime_type")?,
        "file_size_bytes": row.try_get::<Option<i64>, _>("file_size_bytes")?,
        "modified_at_ms": row.try_get::<Option<i64>, _>("modified_at_ms")?,
        "sha256": row.try_get::<Option<String>, _>("sha256")?,
        "content_sha256": row.try_get::<Option<String>, _>("content_sha256")?,
        "reference_kind": row.try_get::<String, _>("reference_kind")?,
        "existence_status": row.try_get::<String, _>("existence_status")?,
        "tags": row.try_get::<String, _>("tags")?,
        "source_label": row.try_get::<String, _>("source_label")?,
        "notes": notes,
        "notes_truncated": notes_truncated,
        "notes_local_paths_redacted": notes_paths_redacted,
        "metadata_json": metadata,
        "metadata_truncated": metadata_truncated,
        "metadata_local_paths_redacted": metadata_paths_redacted,
        "intake_source": row.try_get::<String, _>("intake_source")?,
        "imported_at_ms": row.try_get::<Option<i64>, _>("imported_at_ms")?,
        "created_at_ms": row.try_get::<i64, _>("created_at_ms")?,
        "updated_at_ms": row.try_get::<i64, _>("updated_at_ms")?,
        "raw_file_bytes_included": false,
    })
    .to_string();
    Ok(context_source(
        "document_metadata",
        id.to_string(),
        None,
        &title,
        snapshot,
    ))
}

async fn derivative_source(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    execution: &crate::WorkspacePlanningGuideExecutionBinding,
    id: &str,
) -> PlanResult<crate::WorkspacePlanningContextSource> {
    let row = sqlx::query(
        r#"
SELECT id, document_id, encounter_id, note_id, kind, title, body, review_status,
       source_method, page_range, timestamp_range, segment_label, tags,
       metadata_json, created_at_ms, updated_at_ms
FROM workspace_artifact_derivatives
WHERE id = ? AND client_id = ? AND archived_at_ms IS NULL
        "#,
    )
    .bind(id)
    .bind(&execution.client_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        stale(format!(
            "selected reviewed text `{id}` is unavailable for this patient"
        ))
    })?;
    reviewed_text_source(row, execution, id, "reviewed_text_derivative")
}

async fn clip_source(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    execution: &crate::WorkspacePlanningGuideExecutionBinding,
    id: &str,
) -> PlanResult<crate::WorkspacePlanningContextSource> {
    let row = sqlx::query(
        r#"
SELECT id, document_id, encounter_id, note_id, kind, title, body, review_status,
       source_method, page_range, timestamp_range, line_range, segment_label,
       tags, metadata_json, created_at_ms, updated_at_ms
FROM workspace_context_clips
WHERE id = ? AND client_id = ? AND archived_at_ms IS NULL
        "#,
    )
    .bind(id)
    .bind(&execution.client_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        stale(format!(
            "selected context clip `{id}` is unavailable for this patient"
        ))
    })?;
    reviewed_text_source(row, execution, id, "context_clip_transcript")
}

fn reviewed_text_source(
    row: sqlx::sqlite::SqliteRow,
    execution: &crate::WorkspacePlanningGuideExecutionBinding,
    id: &str,
    source_type: &str,
) -> PlanResult<crate::WorkspacePlanningContextSource> {
    let title: String = row.try_get("title")?;
    let body: String = row.try_get("body")?;
    let metadata: String = row.try_get("metadata_json")?;
    let original_body_sha256 = format!("{:x}", Sha256::digest(body.as_bytes()));
    let (title, title_paths_redacted, title_truncated) = safe_label(&title);
    let (body, body_paths_redacted, body_truncated) = safe_body(&body);
    let (metadata, metadata_paths_redacted, metadata_truncated) = safe_body(&metadata);
    let line_range = row
        .try_get::<Option<String>, _>("line_range")
        .unwrap_or(None);
    let snapshot = serde_json::json!({
        "id": id,
        "client_id": execution.client_id,
        "document_id": row.try_get::<String, _>("document_id")?,
        "encounter_id": row.try_get::<Option<String>, _>("encounter_id")?,
        "note_id": row.try_get::<Option<String>, _>("note_id")?,
        "kind": row.try_get::<String, _>("kind")?,
        "title": title,
        "title_truncated": title_truncated,
        "title_local_paths_redacted": title_paths_redacted,
        "body": body,
        "body_truncated": body_truncated,
        "body_local_paths_redacted": body_paths_redacted,
        "body_original_sha256": original_body_sha256,
        "review_status": row.try_get::<String, _>("review_status")?,
        "source_method": row.try_get::<String, _>("source_method")?,
        "page_range": row.try_get::<String, _>("page_range")?,
        "timestamp_range": row.try_get::<String, _>("timestamp_range")?,
        "line_range": line_range,
        "segment_label": row.try_get::<String, _>("segment_label")?,
        "tags": row.try_get::<String, _>("tags")?,
        "metadata_json": metadata,
        "metadata_truncated": metadata_truncated,
        "metadata_local_paths_redacted": metadata_paths_redacted,
        "created_at_ms": row.try_get::<i64, _>("created_at_ms")?,
        "updated_at_ms": row.try_get::<i64, _>("updated_at_ms")?,
        "raw_file_bytes_included": false,
    })
    .to_string();
    Ok(context_source(
        source_type,
        id.to_string(),
        None,
        &title,
        snapshot,
    ))
}
