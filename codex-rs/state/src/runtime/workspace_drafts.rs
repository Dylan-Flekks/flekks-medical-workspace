use super::workspace::WorkspaceStore;
use crate::model::WorkspaceDraftCheckpointRow;
use crate::model::WorkspaceDraftSessionRow;
use crate::model::WorkspaceDraftSessionSnapshotRow;
use crate::model::datetime_to_epoch_millis;
use chrono::Utc;
use serde_json::Value;
use sha2::Digest;
use sha2::Sha256;
use sqlx::Sqlite;
use uuid::Uuid;

const DRAFT_SCHEMA_VERSION: i64 = 1;
const MAX_NORMALIZED_DRAFT_BYTES: usize = 1024 * 1024;

struct ExpectedCurrentCheckpoint<'a> {
    id: &'a str,
    revision: i64,
    content_sha256: &'a str,
}

impl WorkspaceStore {
    pub async fn create_draft_checkpoint(
        &self,
        input: crate::WorkspaceDraftCheckpointCreate,
    ) -> Result<crate::WorkspaceDraftCheckpoint, crate::WorkspaceDraftError> {
        let (draft_json, schema_version, content_sha256) = normalize_draft(&input.draft_json)?;
        let expected = expected_current_checkpoint(&input)?;
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
            return validation(format!(
                "workspace draft client `{}` was not found or is archived",
                input.client_id
            ));
        }

        let (session_id, current_checkpoint) = match input.session_id.as_deref().map(str::trim) {
            Some("") => return validation("workspace draft session id must not be empty"),
            Some(session_id) => {
                let session = session_by_id(&mut tx, session_id).await?.ok_or_else(|| {
                    crate::WorkspaceDraftError::Validation {
                        message: format!("workspace draft session `{session_id}` was not found"),
                    }
                })?;
                if session.client_id != input.client_id.trim() {
                    return validation(format!(
                        "workspace draft session `{session_id}` belongs to client `{}` not `{}`",
                        session.client_id, input.client_id
                    ));
                }
                if session.status != "active" {
                    return validation(format!(
                        "workspace draft session `{session_id}` is `{}` and cannot checkpoint",
                        session.status
                    ));
                }
                let expected = expected.as_ref().ok_or_else(|| {
                    crate::WorkspaceDraftError::Validation {
                        message: "workspace draft existing-session append requires expected current checkpoint id, revision, and SHA-256"
                            .to_string(),
                    }
                })?;
                let current_checkpoint = checkpoint_by_revision(
                    &mut tx,
                    session_id,
                    session.current_revision,
                )
                .await?
                .ok_or_else(|| crate::WorkspaceDraftError::Storage {
                    message: format!(
                        "workspace draft session `{session_id}` current checkpoint revision {} was not found",
                        session.current_revision
                    ),
                })?;
                if !checkpoint_matches_expected(&current_checkpoint, expected) {
                    if let Some(existing) =
                        checkpoint_by_hash(&mut tx, session_id, &content_sha256).await?
                    {
                        validate_replay(&existing, &input, schema_version, &draft_json)?;
                        if replay_matches_expected_predecessor(
                            &mut tx,
                            &existing,
                            &current_checkpoint,
                            expected,
                        )
                        .await?
                        {
                            tx.rollback().await?;
                            return Ok(existing.try_into_model(true)?);
                        }
                    }
                    return rollback_validation(
                        tx,
                        format!(
                            "workspace draft session `{session_id}` current checkpoint changed; expected `{}` revision {} with SHA-256 `{}`, found `{}` revision {} with SHA-256 `{}`",
                            expected.id,
                            expected.revision,
                            expected.content_sha256,
                            current_checkpoint.id,
                            current_checkpoint.revision,
                            current_checkpoint.content_sha256,
                        ),
                    )
                    .await;
                }
                (session_id.to_string(), Some(current_checkpoint))
            }
            None => {
                if expected.is_some() {
                    return rollback_validation(
                        tx,
                        "workspace draft new-session checkpoint must omit expected current checkpoint id, revision, and SHA-256",
                    )
                    .await;
                }
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
                (session_id, None)
            }
        };

        if let Some(existing) = checkpoint_by_hash(&mut tx, &session_id, &content_sha256).await? {
            validate_replay(&existing, &input, schema_version, &draft_json)?;
            if current_checkpoint
                .as_ref()
                .is_some_and(|current| current.id == existing.id)
            {
                tx.rollback().await?;
                return Ok(existing.try_into_model(true)?);
            }
            return rollback_validation(
                tx,
                format!(
                    "workspace draft checkpoint content already exists at non-current revision {} and cannot move the session head backward",
                    existing.revision
                ),
            )
            .await;
        }

        let previous_revision = current_checkpoint
            .as_ref()
            .map_or(0, |checkpoint| checkpoint.revision);
        let revision = previous_revision + 1;
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
        let updated = sqlx::query(
            "UPDATE workspace_draft_sessions SET current_revision = ?, updated_at_ms = ? WHERE id = ? AND status = 'active' AND current_revision = ?",
        )
        .bind(revision)
        .bind(now_ms)
        .bind(&session_id)
        .bind(previous_revision)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            return rollback_validation(
                tx,
                format!(
                    "workspace draft session `{session_id}` current checkpoint changed concurrently"
                ),
            )
            .await;
        }
        tx.commit().await?;
        Ok(checkpoint.try_into_model(false)?)
    }

    pub async fn list_draft_sessions(
        &self,
        filter: crate::WorkspaceDraftSessionFilter,
    ) -> anyhow::Result<Vec<crate::WorkspaceDraftSessionSnapshot>> {
        let client_id = required("draft session client id", &filter.client_id)?;
        let limit = filter.limit.unwrap_or(50).clamp(1, 200);
        let cursor_id = normalize_optional(filter.cursor_id.as_deref());
        if filter.cursor_updated_at_ms.is_some() != cursor_id.is_some() {
            anyhow::bail!(
                "workspace draft session cursor requires both updated time and session id"
            );
        }
        let rows = sqlx::query_as::<_, WorkspaceDraftSessionSnapshotRow>(
            r#"
SELECT
    session.id AS session_id,
    session.client_id AS session_client_id,
    session.status AS session_status,
    session.current_revision AS session_current_revision,
    session.created_by AS session_created_by,
    session.created_at_ms AS session_created_at_ms,
    session.updated_at_ms AS session_updated_at_ms,
    session.closed_at_ms AS session_closed_at_ms,
    checkpoint.id AS checkpoint_id,
    checkpoint.session_id AS checkpoint_session_id,
    checkpoint.client_id AS checkpoint_client_id,
    checkpoint.encounter_id AS checkpoint_encounter_id,
    checkpoint.note_id AS checkpoint_note_id,
    checkpoint.base_note_revision AS checkpoint_base_note_revision,
    checkpoint.schema_version AS checkpoint_schema_version,
    checkpoint.revision AS checkpoint_revision,
    checkpoint.draft_json AS checkpoint_draft_json,
    checkpoint.content_sha256 AS checkpoint_content_sha256,
    checkpoint.trigger AS checkpoint_trigger,
    checkpoint.actor AS checkpoint_actor,
    checkpoint.created_at_ms AS checkpoint_created_at_ms
FROM workspace_draft_sessions AS session
JOIN workspace_draft_checkpoints AS checkpoint
  ON checkpoint.session_id = session.id
 AND checkpoint.revision = session.current_revision
 AND checkpoint.client_id = session.client_id
WHERE session.client_id = ? AND (? OR session.status = 'active')
  AND (
    ? IS NULL OR session.updated_at_ms < ?
    OR (session.updated_at_ms = ? AND session.id < ?)
  )
ORDER BY session.updated_at_ms DESC, session.id DESC
LIMIT ?
            "#,
        )
        .bind(client_id)
        .bind(filter.include_closed)
        .bind(filter.cursor_updated_at_ms)
        .bind(filter.cursor_updated_at_ms)
        .bind(filter.cursor_updated_at_ms)
        .bind(cursor_id)
        .bind(i64::from(limit))
        .fetch_all(self.pool.as_ref())
        .await?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    pub async fn list_draft_checkpoints(
        &self,
        filter: crate::WorkspaceDraftCheckpointFilter,
    ) -> anyhow::Result<Vec<crate::WorkspaceDraftCheckpoint>> {
        let client_id = required("draft checkpoint client id", &filter.client_id)?;
        let limit = filter.limit.unwrap_or(50).clamp(1, 200);
        let rows = if let Some(session_id) = filter.session_id.as_deref() {
            let session_id = required("draft checkpoint session id", session_id)?;
            sqlx::query_as::<_, WorkspaceDraftCheckpointRow>(
                r#"
SELECT checkpoint.id, checkpoint.session_id, checkpoint.client_id,
       checkpoint.encounter_id, checkpoint.note_id, checkpoint.base_note_revision,
       checkpoint.schema_version, checkpoint.revision, checkpoint.draft_json,
       checkpoint.content_sha256, checkpoint.trigger, checkpoint.actor,
       checkpoint.created_at_ms
FROM workspace_draft_checkpoints AS checkpoint
JOIN workspace_draft_sessions AS session
  ON session.id = checkpoint.session_id
 AND session.client_id = checkpoint.client_id
WHERE checkpoint.client_id = ? AND checkpoint.session_id = ?
  AND (? IS NULL OR checkpoint.revision < ?)
ORDER BY checkpoint.revision DESC
LIMIT ?
                "#,
            )
            .bind(client_id)
            .bind(session_id)
            .bind(filter.cursor_before_revision)
            .bind(filter.cursor_before_revision)
            .bind(i64::from(limit))
            .fetch_all(self.pool.as_ref())
            .await?
        } else {
            if filter.cursor_before_revision.is_some() {
                anyhow::bail!("workspace draft checkpoint revision cursor requires a session id");
            }
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
 AND checkpoint.client_id = session.client_id
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

async fn checkpoint_by_revision(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    session_id: &str,
    revision: i64,
) -> anyhow::Result<Option<WorkspaceDraftCheckpointRow>> {
    sqlx::query_as::<_, WorkspaceDraftCheckpointRow>(
        r#"
SELECT id, session_id, client_id, encounter_id, note_id, base_note_revision,
       schema_version, revision, draft_json, content_sha256, trigger, actor, created_at_ms
FROM workspace_draft_checkpoints
WHERE session_id = ? AND revision = ?
        "#,
    )
    .bind(session_id)
    .bind(revision)
    .fetch_optional(&mut **tx)
    .await
    .map_err(Into::into)
}

async fn replay_matches_expected_predecessor(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    existing: &WorkspaceDraftCheckpointRow,
    current: &WorkspaceDraftCheckpointRow,
    expected: &ExpectedCurrentCheckpoint<'_>,
) -> anyhow::Result<bool> {
    if existing.id != current.id
        || expected.revision.checked_add(1) != Some(existing.revision)
    {
        return Ok(false);
    }
    Ok(checkpoint_by_revision(tx, &existing.session_id, expected.revision)
        .await?
        .as_ref()
        .is_some_and(|checkpoint| checkpoint_matches_expected(checkpoint, expected)))
}

fn checkpoint_matches_expected(
    checkpoint: &WorkspaceDraftCheckpointRow,
    expected: &ExpectedCurrentCheckpoint<'_>,
) -> bool {
    checkpoint.id == expected.id
        && checkpoint.revision == expected.revision
        && checkpoint.content_sha256 == expected.content_sha256
}

fn expected_current_checkpoint(
    input: &crate::WorkspaceDraftCheckpointCreate,
) -> Result<Option<ExpectedCurrentCheckpoint<'_>>, crate::WorkspaceDraftError> {
    match (
        input.expected_current_checkpoint_id.as_deref(),
        input.expected_current_checkpoint_revision,
        input.expected_current_checkpoint_sha256.as_deref(),
    ) {
        (None, None, None) => Ok(None),
        (Some(id), Some(revision), Some(content_sha256)) => {
            let id = required("draft expected current checkpoint id", id)?;
            if revision < 1 {
                return validation(
                    "workspace draft expected current checkpoint revision must be positive",
                );
            }
            let content_sha256 =
                required("draft expected current checkpoint SHA-256", content_sha256)?;
            if content_sha256.len() != 64
                || !content_sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return validation(
                    "workspace draft expected current checkpoint SHA-256 must be 64 lowercase hexadecimal characters",
                );
            }
            Ok(Some(ExpectedCurrentCheckpoint {
                id,
                revision,
                content_sha256,
            }))
        }
        _ => validation(
            "workspace draft expected current checkpoint id, revision, and SHA-256 must be provided together",
        ),
    }
}

fn normalize_draft(draft_json: &str) -> Result<(String, i64, String), crate::WorkspaceDraftError> {
    let value: Value = serde_json::from_str(draft_json.trim()).map_err(|error| {
        crate::WorkspaceDraftError::Validation {
            message: format!("workspace draft checkpoint must be valid JSON: {error}"),
        }
    })?;
    if !value.is_object() {
        return validation("workspace draft checkpoint must be a JSON object");
    }
    let schema_version = value
        .get("schemaVersion")
        .and_then(Value::as_i64)
        .ok_or_else(|| crate::WorkspaceDraftError::Validation {
            message: "workspace draft checkpoint schemaVersion is required".to_string(),
        })?;
    if schema_version != DRAFT_SCHEMA_VERSION {
        return validation(format!(
            "unsupported workspace draft checkpoint schemaVersion {schema_version}"
        ));
    }
    let normalized = serde_json::to_string(&value)?;
    if normalized.len() > MAX_NORMALIZED_DRAFT_BYTES {
        return validation(format!(
            "workspace draft checkpoint exceeds the {MAX_NORMALIZED_DRAFT_BYTES} byte normalized limit"
        ));
    }
    let hash = format!("{:x}", Sha256::digest(normalized.as_bytes()));
    Ok((normalized, schema_version, hash))
}

fn validate_replay(
    checkpoint: &WorkspaceDraftCheckpointRow,
    input: &crate::WorkspaceDraftCheckpointCreate,
    schema_version: i64,
    draft_json: &str,
) -> Result<(), crate::WorkspaceDraftError> {
    if checkpoint.client_id != input.client_id.trim()
        || checkpoint.encounter_id != normalized_owned(input.encounter_id.as_deref())
        || checkpoint.note_id != normalized_owned(input.note_id.as_deref())
        || checkpoint.base_note_revision != input.base_note_revision
        || checkpoint.schema_version != schema_version
        || checkpoint.draft_json != draft_json
    {
        return validation(
            "workspace draft checkpoint content hash was reused with different metadata",
        );
    }
    Ok(())
}

fn required<'a>(label: &str, value: &'a str) -> Result<&'a str, crate::WorkspaceDraftError> {
    let value = value.trim();
    if value.is_empty() {
        return validation(format!("workspace {label} must not be empty"));
    }
    Ok(value)
}

fn validation<T>(message: impl Into<String>) -> Result<T, crate::WorkspaceDraftError> {
    Err(crate::WorkspaceDraftError::Validation {
        message: message.into(),
    })
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

async fn rollback_validation<T>(
    tx: sqlx::Transaction<'_, Sqlite>,
    message: impl Into<String>,
) -> Result<T, crate::WorkspaceDraftError> {
    tx.rollback().await?;
    validation(message)
}

#[cfg(test)]
#[path = "workspace_drafts_tests.rs"]
mod tests;
