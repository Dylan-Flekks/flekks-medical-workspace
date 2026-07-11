use sqlx::Sqlite;

use super::errors::GuideResult;
use super::errors::stale_checkpoint;
use super::errors::validation;

#[derive(sqlx::FromRow)]
pub(super) struct CheckpointBinding {
    pub(super) client_id: String,
    pub(super) session_id: String,
    pub(super) encounter_id: Option<String>,
    pub(super) note_id: Option<String>,
    pub(super) revision: i64,
    pub(super) content_sha256: String,
    pub(super) session_status: String,
    pub(super) current_id: String,
}

pub(super) async fn checkpoint_binding(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<Option<CheckpointBinding>> {
    Ok(sqlx::query_as(
        r#"
SELECT checkpoint.client_id, checkpoint.session_id, checkpoint.encounter_id,
       checkpoint.note_id, checkpoint.revision, checkpoint.content_sha256,
       session.status AS session_status,
       current.id AS current_id
FROM workspace_draft_checkpoints AS checkpoint
JOIN workspace_draft_sessions AS session ON session.id = checkpoint.session_id
JOIN workspace_draft_checkpoints AS current
  ON current.session_id = session.id AND current.revision = session.current_revision
JOIN workspace_clients AS client ON client.id = checkpoint.client_id
WHERE checkpoint.id = ? AND client.archived_at_ms IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?)
}

pub(super) fn validate_checkpoint(
    input: &crate::WorkspaceGuideRunStart,
    checkpoint: &CheckpointBinding,
) -> GuideResult<()> {
    if checkpoint.client_id != input.client_id.trim()
        || checkpoint.session_id != input.session_id.trim()
        || checkpoint.revision != input.source_checkpoint_revision
        || checkpoint.content_sha256 != input.source_checkpoint_sha256.trim()
    {
        return validation("workspace guide source checkpoint identity does not match");
    }
    if checkpoint.session_status != "active" {
        return validation("workspace guide runs require an active draft session");
    }
    if checkpoint.current_id != input.source_checkpoint_id.trim() {
        return stale_checkpoint("workspace guide source checkpoint is no longer current");
    }
    Ok(())
}
