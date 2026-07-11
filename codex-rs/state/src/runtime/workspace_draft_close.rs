use super::workspace::WorkspaceStore;
use super::workspace::insert_audit_event;
use crate::model::WorkspaceDraftSessionSnapshotRow;
use crate::model::datetime_to_epoch_millis;
use chrono::Utc;
use sqlx::Sqlite;

struct ExpectedCurrentCheckpoint<'a> {
    id: &'a str,
    revision: i64,
    content_sha256: &'a str,
}

impl WorkspaceStore {
    pub async fn close_draft_session(
        &self,
        input: crate::WorkspaceDraftSessionClose,
    ) -> Result<crate::WorkspaceDraftSessionSnapshot, crate::WorkspaceDraftError> {
        let session_id = required("draft session id", &input.session_id)?;
        let client_id = required("draft session client id", &input.client_id)?;
        let expected = expected_current_checkpoint(&input)?;
        let target_status = input.status.as_str();
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let Some(snapshot) = session_snapshot_by_id(&mut tx, session_id).await? else {
            return rollback_validation(
                tx,
                format!("workspace draft session `{session_id}` was not found"),
            )
            .await;
        };
        let session = &snapshot.session;
        if session.client_id != client_id {
            return rollback_validation(
                tx,
                format!(
                    "workspace draft session `{session_id}` belongs to client `{}` not `{client_id}`",
                    session.client_id
                ),
            )
            .await;
        }
        if let Some(expected) = expected
            && (snapshot.current_checkpoint.id != expected.id
                || snapshot.current_checkpoint.revision != expected.revision
                || snapshot.current_checkpoint.content_sha256 != expected.content_sha256)
        {
            return rollback_validation(
                tx,
                format!(
                    "workspace draft session `{session_id}` current checkpoint changed; expected `{}` revision {} with SHA-256 `{}`, found `{}` revision {} with SHA-256 `{}`",
                    expected.id,
                    expected.revision,
                    expected.content_sha256,
                    snapshot.current_checkpoint.id,
                    snapshot.current_checkpoint.revision,
                    snapshot.current_checkpoint.content_sha256,
                ),
            )
            .await;
        }
        if session.status == target_status {
            tx.rollback().await?;
            return Ok(snapshot);
        }
        if session.status != "active" {
            return rollback_validation(
                tx,
                format!(
                    "workspace draft session `{session_id}` is already terminal with status `{}`",
                    session.status
                ),
            )
            .await;
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
            return rollback_validation(
                tx,
                format!("workspace draft session `{session_id}` lifecycle changed concurrently"),
            )
            .await;
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
        let snapshot = session_snapshot_by_id(&mut tx, session_id)
            .await?
            .ok_or_else(|| crate::WorkspaceDraftError::Storage {
                message: "workspace draft session disappeared after close".to_string(),
            })?;
        tx.commit().await?;
        Ok(snapshot)
    }
}

fn expected_current_checkpoint(
    input: &crate::WorkspaceDraftSessionClose,
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

async fn session_snapshot_by_id(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<Option<crate::WorkspaceDraftSessionSnapshot>> {
    let row = sqlx::query_as::<_, WorkspaceDraftSessionSnapshotRow>(
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
WHERE session.id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(TryInto::try_into).transpose()
}

async fn rollback_validation<T>(
    tx: sqlx::Transaction<'_, Sqlite>,
    message: impl Into<String>,
) -> Result<T, crate::WorkspaceDraftError> {
    tx.rollback().await?;
    validation(message)
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
