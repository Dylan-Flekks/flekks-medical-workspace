use super::workspace::WorkspaceStore;
use crate::model::WorkspaceDraftCheckpointRow;

#[path = "workspace_drafts/checkpoint_create.rs"]
mod checkpoint_create;
#[path = "workspace_drafts/session_list.rs"]
mod session_list;

impl WorkspaceStore {
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

fn normalize_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "workspace_drafts_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "workspace_draft_idempotency_tests.rs"]
mod idempotency_tests;

#[cfg(test)]
#[path = "workspace_draft_global_list_tests.rs"]
mod global_list_tests;
