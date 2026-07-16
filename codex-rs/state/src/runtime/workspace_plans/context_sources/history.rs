use super::context_source;
use super::safe_body;
use super::safe_label;
use crate::runtime::workspace_agent::MAX_AGENT_CONTEXT_SNAPSHOT_BYTES;
use sha2::Digest;
use sha2::Sha256;
use sqlx::Row;

pub(super) async fn read_visit_history(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    client_id: &str,
    max_records: u32,
) -> super::super::PlanResult<Vec<crate::WorkspacePlanningContextSource>> {
    let rows = sqlx::query(
        r#"
SELECT id, client_id, kind, title, status, started_at_ms, ended_at_ms,
       archived_at_ms, created_at_ms, updated_at_ms
FROM workspace_encounters
WHERE client_id = ? AND archived_at_ms IS NULL
ORDER BY COALESCE(started_at_ms, updated_at_ms) DESC, title ASC, id ASC
LIMIT ?
        "#,
    )
    .bind(client_id)
    .bind(i64::from(max_records))
    .fetch_all(&mut **tx)
    .await?;
    rows.into_iter()
        .map(|row| {
            let id: String = row.try_get("id")?;
            let title: String = row.try_get("title")?;
            let (safe_title, title_paths_redacted, title_truncated) = safe_label(&title);
            let snapshot_json = serde_json::json!({
                "id": id,
                "client_id": row.try_get::<String, _>("client_id")?,
                "kind": row.try_get::<String, _>("kind")?,
                "title": safe_title,
                "title_truncated": title_truncated,
                "title_local_paths_redacted": title_paths_redacted,
                "status": row.try_get::<String, _>("status")?,
                "started_at_ms": row.try_get::<Option<i64>, _>("started_at_ms")?,
                "ended_at_ms": row.try_get::<Option<i64>, _>("ended_at_ms")?,
                "archived_at_ms": row.try_get::<Option<i64>, _>("archived_at_ms")?,
                "created_at_ms": row.try_get::<i64, _>("created_at_ms")?,
                "updated_at_ms": row.try_get::<i64, _>("updated_at_ms")?,
            })
            .to_string();
            Ok(context_source(
                "encounter",
                id,
                None,
                &safe_title,
                snapshot_json,
            ))
        })
        .collect()
}

pub(super) async fn read_progress_notes(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    client_id: &str,
    max_records: u32,
) -> super::super::PlanResult<Vec<crate::WorkspacePlanningContextSource>> {
    let rows = sqlx::query(
        r#"
SELECT id, client_id, encounter_id, title, kind, body, status,
       current_revision, archived_at_ms, created_at_ms, updated_at_ms
FROM workspace_notes
WHERE client_id = ? AND archived_at_ms IS NULL
  AND LOWER(kind) IN ('progress', 'progress_note', 'daily', 'daily_note')
ORDER BY updated_at_ms DESC, title ASC, id ASC
LIMIT ?
        "#,
    )
    .bind(client_id)
    .bind(i64::from(max_records))
    .fetch_all(&mut **tx)
    .await?;
    let mut sources = Vec::with_capacity(rows.len());
    let mut returned_bytes = 0usize;
    for row in rows {
        let id: String = row.try_get("id")?;
        let title: String = row.try_get("title")?;
        let body: String = row.try_get("body")?;
        let revision: i64 = row.try_get("current_revision")?;
        let original_body_sha256 = format!("{:x}", Sha256::digest(body.as_bytes()));
        let (safe_body, body_paths_redacted, body_truncated) = safe_body(&body);
        let (safe_title, title_paths_redacted, title_truncated) = safe_label(&title);
        let snapshot_json = serde_json::json!({
            "id": id,
            "client_id": row.try_get::<String, _>("client_id")?,
            "encounter_id": row.try_get::<Option<String>, _>("encounter_id")?,
            "title": safe_title,
            "title_truncated": title_truncated,
            "title_local_paths_redacted": title_paths_redacted,
            "kind": row.try_get::<String, _>("kind")?,
            "body": safe_body,
            "body_truncated": body_truncated,
            "body_local_paths_redacted": body_paths_redacted,
            "body_original_bytes": body.len(),
            "body_original_sha256": original_body_sha256,
            "status": row.try_get::<String, _>("status")?,
            "current_revision": revision,
            "archived_at_ms": row.try_get::<Option<i64>, _>("archived_at_ms")?,
            "created_at_ms": row.try_get::<i64, _>("created_at_ms")?,
            "updated_at_ms": row.try_get::<i64, _>("updated_at_ms")?,
        })
        .to_string();
        if returned_bytes.saturating_add(snapshot_json.len()) > MAX_AGENT_CONTEXT_SNAPSHOT_BYTES {
            break;
        }
        returned_bytes += snapshot_json.len();
        sources.push(context_source(
            "note_revision",
            id,
            Some(revision),
            &safe_title,
            snapshot_json,
        ));
    }
    Ok(sources)
}
