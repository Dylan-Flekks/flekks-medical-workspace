use super::PlanResult;
use super::not_found;
use super::required;
use super::transition;
use super::validation;
use crate::model::WorkspacePlanSessionRow;
use crate::model::datetime_to_epoch_millis;
use crate::runtime::workspace::WorkspaceStore;
use crate::runtime::workspace::insert_audit_event;
use crate::runtime::workspace_policy::WorkspacePolicyRequirementError;
use crate::runtime::workspace_policy::require_synthetic_workspace;
use chrono::Utc;
use uuid::Uuid;

impl WorkspaceStore {
    pub async fn open_plan_session(
        &self,
        input: crate::WorkspacePlanSessionOpen,
    ) -> PlanResult<crate::WorkspacePlanSession> {
        let client_id = required("session client id", &input.client_id)?;
        let actor = required("session creator", &input.created_by)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        require_synthetic_workspace(&mut tx)
            .await
            .map_err(|error| match error {
                WorkspacePolicyRequirementError::NotSynthetic => validation(error.to_string()),
                WorkspacePolicyRequirementError::Integrity(error) => error.into(),
            })?;
        let client_exists = sqlx::query_scalar::<_, i64>(
            "SELECT 1 FROM workspace_clients WHERE id = ? AND archived_at_ms IS NULL",
        )
        .bind(client_id)
        .fetch_optional(&mut *tx)
        .await?
        .is_some();
        if !client_exists {
            return Err(not_found(format!(
                "workspace plan client `{client_id}` was not found or is archived"
            )));
        }
        if let Some(existing) = sqlx::query_as::<_, WorkspacePlanSessionRow>(
            r#"
SELECT id, client_id, source_thread_id, status, latest_revision, created_by,
       created_at_ms, updated_at_ms, closed_at_ms
FROM workspace_plan_sessions
WHERE client_id = ? AND status = 'active'
            "#,
        )
        .bind(client_id)
        .fetch_optional(&mut *tx)
        .await?
        {
            tx.rollback().await?;
            return Ok(existing.try_into_model(true)?);
        }

        let id = Uuid::new_v4().to_string();
        let now_ms = datetime_to_epoch_millis(Utc::now());
        sqlx::query(
            "INSERT INTO workspace_plan_sessions (id, client_id, source_thread_id, status, latest_revision, created_by, created_at_ms, updated_at_ms, closed_at_ms) VALUES (?, ?, NULL, 'active', 0, ?, ?, ?, NULL)",
        )
        .bind(&id)
        .bind(client_id)
        .bind(actor)
        .bind(now_ms)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "plan_session".to_string(),
                entity_id: id.clone(),
                action: "opened".to_string(),
                actor: actor.to_string(),
                actor_kind: "human".to_string(),
                source: "workspace_plan".to_string(),
                client_id: Some(client_id.to_string()),
                success: true,
                summary: "persistent patient planning session opened".to_string(),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        let row = self
            .plan_session_row(&mut tx, &id)
            .await?
            .ok_or_else(|| not_found("inserted workspace plan session was not found"))?;
        tx.commit().await?;
        Ok(row.try_into_model(false)?)
    }

    pub async fn get_plan_session(
        &self,
        session_id: &str,
        client_id: &str,
    ) -> PlanResult<Option<crate::WorkspacePlanSession>> {
        let session_id = required("session id", session_id)?;
        let client_id = required("session client id", client_id)?;
        let mut tx = self.pool.begin().await?;
        let row = self.plan_session_row(&mut tx, session_id).await?;
        tx.rollback().await?;
        match row {
            Some(row) if row.client_id == client_id => Ok(Some(row.try_into_model(false)?)),
            Some(row) => Err(validation(format!(
                "workspace plan session `{session_id}` belongs to client `{}` not `{client_id}`",
                row.client_id
            ))),
            None => Ok(None),
        }
    }

    pub async fn get_active_plan_session(
        &self,
        client_id: &str,
    ) -> PlanResult<Option<crate::WorkspacePlanSession>> {
        let client_id = required("session client id", client_id)?;
        let row = sqlx::query_as::<_, WorkspacePlanSessionRow>(
            r#"
SELECT id, client_id, source_thread_id, status, latest_revision, created_by,
       created_at_ms, updated_at_ms, closed_at_ms
FROM workspace_plan_sessions
WHERE client_id = ? AND status = 'active'
            "#,
        )
        .bind(client_id)
        .fetch_optional(self.pool.as_ref())
        .await?;
        row.map(|row| row.try_into_model(false))
            .transpose()
            .map_err(Into::into)
    }

    /// Resolves the active persistent planning session bound to a dedicated model thread.
    ///
    /// The thread binding is durable, so callers can use this lookup after process restart to
    /// keep ordinary Codex turns out of a patient planning thread.
    pub async fn get_active_plan_session_by_thread(
        &self,
        source_thread_id: &str,
    ) -> PlanResult<Option<crate::WorkspacePlanSession>> {
        let source_thread_id = required("source thread id", source_thread_id)?;
        let row = sqlx::query_as::<_, WorkspacePlanSessionRow>(
            r#"
SELECT id, client_id, source_thread_id, status, latest_revision, created_by,
       created_at_ms, updated_at_ms, closed_at_ms
FROM workspace_plan_sessions
WHERE source_thread_id = ? AND status = 'active'
            "#,
        )
        .bind(source_thread_id)
        .fetch_optional(self.pool.as_ref())
        .await?;
        row.map(|row| row.try_into_model(false))
            .transpose()
            .map_err(Into::into)
    }

    pub async fn bind_plan_session_thread(
        &self,
        input: crate::WorkspacePlanSessionThreadBind,
    ) -> PlanResult<crate::WorkspacePlanSession> {
        let session_id = required("session id", &input.session_id)?;
        let client_id = required("session client id", &input.client_id)?;
        let next_thread = required("source thread id", &input.source_thread_id)?;
        let actor = required("thread binding actor", &input.actor)?;
        let expected = input
            .expected_thread_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let session = self
            .require_active_plan_session(&mut tx, session_id, client_id)
            .await?;
        if session.source_thread_id.as_deref() == Some(next_thread) {
            tx.rollback().await?;
            return Ok(session.try_into_model(true)?);
        }
        if let Some(bound_thread) = session.source_thread_id.as_deref() {
            return Err(validation(format!(
                "workspace plan session `{session_id}` is permanently bound to thread `{bound_thread}` and cannot be rebound"
            )));
        }
        if session.source_thread_id.as_deref() != expected {
            return Err(transition(format!(
                "workspace plan session `{session_id}` thread binding changed; expected `{}`, found `{}`",
                expected.unwrap_or("unbound"),
                session.source_thread_id.as_deref().unwrap_or("unbound")
            )));
        }
        let conflict = sqlx::query_scalar::<_, String>(
            "SELECT id FROM workspace_plan_sessions WHERE source_thread_id = ? AND id != ?",
        )
        .bind(next_thread)
        .bind(session_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(conflict) = conflict {
            return Err(validation(format!(
                "workspace plan thread `{next_thread}` is already bound to session `{conflict}`"
            )));
        }
        let now_ms = datetime_to_epoch_millis(Utc::now());
        sqlx::query(
            "UPDATE workspace_plan_sessions SET source_thread_id = ?, updated_at_ms = ? WHERE id = ? AND status = 'active'",
        )
        .bind(next_thread)
        .bind(now_ms)
        .bind(session_id)
        .execute(&mut *tx)
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "plan_session".to_string(),
                entity_id: session_id.to_string(),
                action: if expected.is_some() {
                    "thread_rebound".to_string()
                } else {
                    "thread_bound".to_string()
                },
                actor: actor.to_string(),
                actor_kind: "human".to_string(),
                source: "workspace_plan".to_string(),
                client_id: Some(client_id.to_string()),
                source_thread_id: Some(next_thread.to_string()),
                success: true,
                summary: "dedicated planning thread bound".to_string(),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        let row = self
            .plan_session_row(&mut tx, session_id)
            .await?
            .ok_or_else(|| not_found("updated workspace plan session was not found"))?;
        tx.commit().await?;
        Ok(row.try_into_model(false)?)
    }

    pub async fn close_plan_session(
        &self,
        input: crate::WorkspacePlanSessionClose,
    ) -> PlanResult<crate::WorkspacePlanSession> {
        let session_id = required("session id", &input.session_id)?;
        let client_id = required("session client id", &input.client_id)?;
        let actor = required("session close actor", &input.actor)?;
        let reason = required("session close reason", &input.reason)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let session = self
            .plan_session_row(&mut tx, session_id)
            .await?
            .ok_or_else(|| {
                not_found(format!(
                    "workspace plan session `{session_id}` was not found"
                ))
            })?;
        if session.client_id != client_id {
            return Err(validation(format!(
                "workspace plan session `{session_id}` belongs to client `{}` not `{client_id}`",
                session.client_id
            )));
        }
        if session.status == "closed" {
            tx.rollback().await?;
            return Ok(session.try_into_model(true)?);
        }
        let now_ms = datetime_to_epoch_millis(Utc::now());
        sqlx::query("UPDATE workspace_plan_revisions SET status = 'outdated' WHERE plan_session_id = ? AND status = 'current'")
            .bind(session_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE workspace_plan_proposals SET status = 'outdated' WHERE plan_session_id = ? AND status = 'pending'")
            .bind(session_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE workspace_plan_sessions SET status = 'closed', updated_at_ms = ?, closed_at_ms = ? WHERE id = ? AND status = 'active'")
            .bind(now_ms)
            .bind(now_ms)
            .bind(session_id)
            .execute(&mut *tx)
            .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "plan_session".to_string(),
                entity_id: session_id.to_string(),
                action: "closed".to_string(),
                actor: actor.to_string(),
                actor_kind: "human".to_string(),
                source: "workspace_plan".to_string(),
                client_id: Some(client_id.to_string()),
                source_thread_id: session.source_thread_id,
                success: true,
                summary: reason.to_string(),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        let row = self
            .plan_session_row(&mut tx, session_id)
            .await?
            .ok_or_else(|| not_found("closed workspace plan session was not found"))?;
        tx.commit().await?;
        Ok(row.try_into_model(false)?)
    }
}
