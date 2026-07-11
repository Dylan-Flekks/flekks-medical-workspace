use crate::model::WorkspaceDraftSessionSnapshotRow;

use super::super::workspace::WorkspaceStore;
use super::normalize_optional;
use super::required;

macro_rules! draft_session_query {
    ($scope:literal) => {
        concat!(
            "SELECT ",
            "session.id AS session_id, ",
            "session.client_id AS session_client_id, ",
            "session.status AS session_status, ",
            "session.current_revision AS session_current_revision, ",
            "session.created_by AS session_created_by, ",
            "session.created_at_ms AS session_created_at_ms, ",
            "session.updated_at_ms AS session_updated_at_ms, ",
            "session.closed_at_ms AS session_closed_at_ms, ",
            "checkpoint.id AS checkpoint_id, ",
            "checkpoint.session_id AS checkpoint_session_id, ",
            "checkpoint.client_id AS checkpoint_client_id, ",
            "checkpoint.encounter_id AS checkpoint_encounter_id, ",
            "checkpoint.note_id AS checkpoint_note_id, ",
            "checkpoint.base_note_revision AS checkpoint_base_note_revision, ",
            "checkpoint.schema_version AS checkpoint_schema_version, ",
            "checkpoint.revision AS checkpoint_revision, ",
            "checkpoint.draft_json AS checkpoint_draft_json, ",
            "checkpoint.content_sha256 AS checkpoint_content_sha256, ",
            "checkpoint.trigger AS checkpoint_trigger, ",
            "checkpoint.actor AS checkpoint_actor, ",
            "checkpoint.created_at_ms AS checkpoint_created_at_ms ",
            "FROM workspace_draft_sessions AS session ",
            "JOIN workspace_draft_checkpoints AS checkpoint ",
            "ON checkpoint.session_id = session.id ",
            "AND checkpoint.revision = session.current_revision ",
            "AND checkpoint.client_id = session.client_id ",
            $scope,
            " AND (? IS NULL OR session.updated_at_ms < ? ",
            "OR (session.updated_at_ms = ? AND session.id < ?)) ",
            "ORDER BY session.updated_at_ms DESC, session.id DESC LIMIT ?"
        )
    };
}

impl WorkspaceStore {
    pub async fn list_draft_sessions(
        &self,
        filter: crate::WorkspaceDraftSessionFilter,
    ) -> anyhow::Result<Vec<crate::WorkspaceDraftSessionSnapshot>> {
        let cursor_id = normalize_optional(filter.cursor_id.as_deref());
        if filter.cursor_updated_at_ms.is_some() != cursor_id.is_some() {
            anyhow::bail!(
                "workspace draft session cursor requires both updated time and session id"
            );
        }
        let rows = match filter.scope {
            crate::WorkspaceDraftSessionScope::Client(client_id) => {
                let client_id = required("draft session client id", &client_id)?;
                let limit = filter.limit.unwrap_or(50).clamp(1, 200);
                sqlx::query_as::<_, WorkspaceDraftSessionSnapshotRow>(draft_session_query!(
                    "WHERE session.client_id = ? AND (? OR session.status = 'active')"
                ))
                .bind(client_id)
                .bind(filter.include_closed)
                .bind(filter.cursor_updated_at_ms)
                .bind(filter.cursor_updated_at_ms)
                .bind(filter.cursor_updated_at_ms)
                .bind(cursor_id)
                .bind(i64::from(limit))
                .fetch_all(self.pool.as_ref())
                .await?
            }
            crate::WorkspaceDraftSessionScope::AllActiveClients => {
                if filter.include_closed {
                    anyhow::bail!(
                        "workspace global draft discovery cannot include closed sessions"
                    );
                }
                // The API fetches two rows internally to return one record plus an exact cursor.
                let limit = filter.limit.unwrap_or(2).clamp(1, 2);
                sqlx::query_as::<_, WorkspaceDraftSessionSnapshotRow>(draft_session_query!(
                    "JOIN workspace_clients AS client ON client.id = session.client_id \
                     WHERE client.archived_at_ms IS NULL AND session.status = 'active'"
                ))
                .bind(filter.cursor_updated_at_ms)
                .bind(filter.cursor_updated_at_ms)
                .bind(filter.cursor_updated_at_ms)
                .bind(cursor_id)
                .bind(i64::from(limit))
                .fetch_all(self.pool.as_ref())
                .await?
            }
        };
        rows.into_iter().map(TryInto::try_into).collect()
    }
}
