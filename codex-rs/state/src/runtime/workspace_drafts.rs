use super::workspace::WorkspaceStore;
use super::workspace::insert_audit_event;
use crate::model::WorkspaceDraftCheckpointRow;
use crate::model::WorkspaceDraftSessionRow;
use crate::model::datetime_to_epoch_millis;
use chrono::Utc;
use serde_json::Value;
use sha2::Digest;
use sha2::Sha256;
use sqlx::Sqlite;
use uuid::Uuid;

const DRAFT_SCHEMA_VERSION: i64 = 1;
const MAX_NORMALIZED_DRAFT_BYTES: usize = 1024 * 1024;

impl WorkspaceStore {
    pub async fn create_draft_checkpoint(
        &self,
        input: crate::WorkspaceDraftCheckpointCreate,
    ) -> anyhow::Result<crate::WorkspaceDraftCheckpoint> {
        let (draft_json, schema_version, content_sha256) = normalize_draft(&input.draft_json)?;
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let actor = nonempty_or(&input.actor, "local human");
        let trigger = nonempty_or(&input.trigger, "manual");
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let client_exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM workspace_clients WHERE id = ? AND archived_at_ms IS NULL",
        )
        .bind(input.client_id.trim())
        .fetch_optional(&mut *tx)
        .await?;
        if client_exists.is_none() {
            anyhow::bail!(
                "workspace draft client `{}` was not found or is archived",
                input.client_id
            );
        }

        let session_id = match input.session_id.as_deref().map(str::trim) {
            Some("") => anyhow::bail!("workspace draft session id must not be empty"),
            Some(session_id) => {
                let session = session_by_id(&mut tx, session_id).await?.ok_or_else(|| {
                    anyhow::anyhow!("workspace draft session `{session_id}` was not found")
                })?;
                if session.client_id != input.client_id.trim() {
                    anyhow::bail!(
                        "workspace draft session `{session_id}` belongs to client `{}` not `{}`",
                        session.client_id,
                        input.client_id
                    );
                }
                if session.status != "active" {
                    anyhow::bail!(
                        "workspace draft session `{session_id}` is `{}` and cannot checkpoint",
                        session.status
                    );
                }
                session_id.to_string()
            }
            None => {
                let session_id = Uuid::new_v4().to_string();
                sqlx::query(
                    r#"
INSERT INTO workspace_draft_sessions (
    id, client_id, status, current_revision, created_by,
    created_at_ms, updated_at_ms, closed_at_ms
) VALUES (?, ?, 'active', 0, ?, ?, ?, NULL)
                    "#,
                )
                .bind(&session_id)
                .bind(input.client_id.trim())
                .bind(&actor)
                .bind(now_ms)
                .bind(now_ms)
                .execute(&mut *tx)
                .await?;
                session_id
            }
        };

        if let Some(existing) = checkpoint_by_hash(&mut tx, &session_id, &content_sha256).await? {
            validate_replay(&existing, &input, schema_version, &draft_json)?;
            sqlx::query(
                "UPDATE workspace_draft_sessions SET current_revision = ?, updated_at_ms = ? WHERE id = ?",
            )
            .bind(existing.revision)
            .bind(now_ms)
            .bind(&session_id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return existing.try_into_model(true);
        }

        let revision: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(revision), 0) + 1 FROM workspace_draft_checkpoints WHERE session_id = ?",
        )
        .bind(&session_id)
        .fetch_one(&mut *tx)
        .await?;
        let checkpoint_id = Uuid::new_v4().to_string();
        let checkpoint = sqlx::query_as::<_, WorkspaceDraftCheckpointRow>(
            r#"
INSERT INTO workspace_draft_checkpoints (
    id, session_id, client_id, encounter_id, note_id, base_note_revision,
    schema_version, revision, draft_json, content_sha256, trigger, actor, created_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
RETURNING id, session_id, client_id, encounter_id, note_id, base_note_revision,
          schema_version, revision, draft_json, content_sha256, trigger, actor, created_at_ms
            "#,
        )
        .bind(&checkpoint_id)
        .bind(&session_id)
        .bind(input.client_id.trim())
        .bind(normalize_optional(input.encounter_id.as_deref()))
        .bind(normalize_optional(input.note_id.as_deref()))
        .bind(input.base_note_revision)
        .bind(schema_version)
        .bind(revision)
        .bind(&draft_json)
        .bind(&content_sha256)
        .bind(&trigger)
        .bind(&actor)
        .bind(now_ms)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE workspace_draft_sessions SET current_revision = ?, updated_at_ms = ? WHERE id = ?",
        )
        .bind(revision)
        .bind(now_ms)
        .bind(&session_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        checkpoint.try_into_model(false)
    }

    pub async fn list_draft_sessions(
        &self,
        filter: crate::WorkspaceDraftSessionFilter,
    ) -> anyhow::Result<Vec<crate::WorkspaceDraftSession>> {
        let client_id = required("draft session client id", &filter.client_id)?;
        let limit = filter.limit.unwrap_or(50).clamp(1, 200);
        let rows = sqlx::query_as::<_, WorkspaceDraftSessionRow>(
            r#"
SELECT id, client_id, status, current_revision, created_by,
       created_at_ms, updated_at_ms, closed_at_ms
FROM workspace_draft_sessions
WHERE client_id = ? AND (? OR status = 'active')
ORDER BY updated_at_ms DESC
LIMIT ?
            "#,
        )
        .bind(client_id)
        .bind(filter.include_closed)
        .bind(i64::from(limit))
        .fetch_all(self.pool.as_ref())
        .await?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    pub async fn close_draft_session(
        &self,
        input: crate::WorkspaceDraftSessionClose,
    ) -> anyhow::Result<crate::WorkspaceDraftSession> {
        let session_id = required("draft session id", &input.session_id)?;
        let client_id = required("draft session client id", &input.client_id)?;
        let target_status = input.status.as_str();
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let session = session_by_id(&mut tx, session_id).await?.ok_or_else(|| {
            anyhow::anyhow!("workspace draft session `{session_id}` was not found")
        })?;
        if session.client_id != client_id {
            anyhow::bail!(
                "workspace draft session `{session_id}` belongs to client `{}` not `{client_id}`",
                session.client_id
            );
        }
        if session.status == target_status {
            tx.rollback().await?;
            return session.try_into();
        }
        if session.status != "active" {
            anyhow::bail!(
                "workspace draft session `{session_id}` is already terminal with status `{}`",
                session.status
            );
        }
        let updated = sqlx::query(
            "UPDATE workspace_draft_sessions SET status = ?, updated_at_ms = ?, closed_at_ms = ? WHERE id = ? AND status = 'active'",
        )
        .bind(target_status)
        .bind(now_ms)
        .bind(now_ms)
        .bind(session_id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            anyhow::bail!("workspace draft session `{session_id}` lifecycle changed concurrently");
        }
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "draft_session".to_string(),
                entity_id: session_id.to_string(),
                action: target_status.to_string(),
                actor: nonempty_or(&input.actor, "local human"),
                actor_kind: "human".to_string(),
                source: "state".to_string(),
                client_id: Some(client_id.to_string()),
                success: true,
                summary: input.reason.trim().to_string(),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        let session = session_by_id(&mut tx, session_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("workspace draft session disappeared after close"))?;
        tx.commit().await?;
        session.try_into()
    }

    pub async fn list_draft_checkpoints(
        &self,
        filter: crate::WorkspaceDraftCheckpointFilter,
    ) -> anyhow::Result<Vec<crate::WorkspaceDraftCheckpoint>> {
        let client_id = required("draft checkpoint client id", &filter.client_id)?;
        let limit = filter.limit.unwrap_or(50).clamp(1, 200);
        let rows = if let Some(session_id) = filter.session_id.as_deref() {
            sqlx::query_as::<_, WorkspaceDraftCheckpointRow>(
                r#"
SELECT id, session_id, client_id, encounter_id, note_id, base_note_revision,
       schema_version, revision, draft_json, content_sha256, trigger, actor, created_at_ms
FROM workspace_draft_checkpoints
WHERE client_id = ? AND session_id = ?
ORDER BY revision DESC
LIMIT ?
                "#,
            )
            .bind(client_id)
            .bind(session_id.trim())
            .bind(i64::from(limit))
            .fetch_all(self.pool.as_ref())
            .await?
        } else {
            sqlx::query_as::<_, WorkspaceDraftCheckpointRow>(
                r#"
SELECT checkpoint.id, checkpoint.session_id, checkpoint.client_id,
       checkpoint.encounter_id, checkpoint.note_id, checkpoint.base_note_revision,
       checkpoint.schema_version, checkpoint.revision, checkpoint.draft_json,
       checkpoint.content_sha256, checkpoint.trigger, checkpoint.actor,
       checkpoint.created_at_ms
FROM workspace_draft_sessions AS session
JOIN workspace_draft_checkpoints AS checkpoint
  ON checkpoint.session_id = session.id
 AND checkpoint.revision = session.current_revision
WHERE session.client_id = ? AND session.status = 'active'
ORDER BY session.updated_at_ms DESC
LIMIT ?
                "#,
            )
            .bind(client_id)
            .bind(i64::from(limit))
            .fetch_all(self.pool.as_ref())
            .await?
        };
        rows.into_iter()
            .map(|row| row.try_into_model(false))
            .collect()
    }
}

async fn session_by_id(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<Option<WorkspaceDraftSessionRow>> {
    sqlx::query_as::<_, WorkspaceDraftSessionRow>(
        "SELECT id, client_id, status, current_revision, created_by, created_at_ms, updated_at_ms, closed_at_ms FROM workspace_draft_sessions WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(Into::into)
}

async fn checkpoint_by_hash(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    session_id: &str,
    hash: &str,
) -> anyhow::Result<Option<WorkspaceDraftCheckpointRow>> {
    sqlx::query_as::<_, WorkspaceDraftCheckpointRow>(
        r#"
SELECT id, session_id, client_id, encounter_id, note_id, base_note_revision,
       schema_version, revision, draft_json, content_sha256, trigger, actor, created_at_ms
FROM workspace_draft_checkpoints
WHERE session_id = ? AND content_sha256 = ?
        "#,
    )
    .bind(session_id)
    .bind(hash)
    .fetch_optional(&mut **tx)
    .await
    .map_err(Into::into)
}

fn normalize_draft(draft_json: &str) -> anyhow::Result<(String, i64, String)> {
    let value: Value = serde_json::from_str(draft_json.trim()).map_err(|error| {
        anyhow::anyhow!("workspace draft checkpoint must be valid JSON: {error}")
    })?;
    if !value.is_object() {
        anyhow::bail!("workspace draft checkpoint must be a JSON object");
    }
    let schema_version = value
        .get("schemaVersion")
        .and_then(Value::as_i64)
        .ok_or_else(|| anyhow::anyhow!("workspace draft checkpoint schemaVersion is required"))?;
    if schema_version != DRAFT_SCHEMA_VERSION {
        anyhow::bail!("unsupported workspace draft checkpoint schemaVersion {schema_version}");
    }
    let normalized = serde_json::to_string(&value)?;
    if normalized.len() > MAX_NORMALIZED_DRAFT_BYTES {
        anyhow::bail!(
            "workspace draft checkpoint exceeds the {MAX_NORMALIZED_DRAFT_BYTES} byte normalized limit"
        );
    }
    let hash = format!("{:x}", Sha256::digest(normalized.as_bytes()));
    Ok((normalized, schema_version, hash))
}

fn validate_replay(
    checkpoint: &WorkspaceDraftCheckpointRow,
    input: &crate::WorkspaceDraftCheckpointCreate,
    schema_version: i64,
    draft_json: &str,
) -> anyhow::Result<()> {
    if checkpoint.client_id != input.client_id.trim()
        || checkpoint.encounter_id != normalized_owned(input.encounter_id.as_deref())
        || checkpoint.note_id != normalized_owned(input.note_id.as_deref())
        || checkpoint.base_note_revision != input.base_note_revision
        || checkpoint.schema_version != schema_version
        || checkpoint.draft_json != draft_json
    {
        anyhow::bail!("workspace draft checkpoint content hash was reused with different metadata");
    }
    Ok(())
}

fn required<'a>(label: &str, value: &'a str) -> anyhow::Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        anyhow::bail!("workspace {label} must not be empty");
    }
    Ok(value)
}

fn nonempty_or(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

fn normalize_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn normalized_owned(value: Option<&str>) -> Option<String> {
    normalize_optional(value).map(str::to_string)
}

#[cfg(test)]
#[path = "workspace_drafts_tests.rs"]
mod tests;
