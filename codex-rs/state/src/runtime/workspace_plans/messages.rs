use super::MAX_MESSAGE_BYTES;
use super::PlanResult;
use super::idempotency;
use super::required;
use super::source_pair;
use super::validate_bound_thread;
use super::validation;
use crate::model::WorkspacePlanMessageRow;
use crate::model::datetime_to_epoch_millis;
use crate::runtime::workspace::WorkspaceStore;
use crate::runtime::workspace::insert_audit_event;
use chrono::Utc;
use sha2::Digest;
use sha2::Sha256;
use uuid::Uuid;

impl WorkspaceStore {
    pub async fn append_plan_message(
        &self,
        input: crate::WorkspacePlanMessageAppend,
    ) -> PlanResult<crate::WorkspacePlanMessage> {
        let session_id = required("message session id", &input.plan_session_id)?;
        let client_id = required("message client id", &input.client_id)?;
        let guide_run_id = required("message guide run id", &input.guide_run_id)?;
        let content = required("message content", &input.content)?;
        let key = required("message idempotency key", &input.idempotency_key)?;
        if input.role != crate::WorkspacePlanMessageRole::Human {
            return Err(validation(
                "workspace plan message append accepts only human-authored messages; assistant, error, and status records are core-owned",
            ));
        }
        if content.len() > MAX_MESSAGE_BYTES {
            return Err(validation(format!(
                "workspace plan message exceeds the {MAX_MESSAGE_BYTES} byte limit"
            )));
        }
        let (thread_id, turn_id) = source_pair(
            input.source_thread_id.as_deref(),
            input.source_turn_id.as_deref(),
        )?;
        let content_sha256 = format!("{:x}", Sha256::digest(content.as_bytes()));
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let session = self
            .require_active_plan_session(&mut tx, session_id, client_id)
            .await?;
        if let Some(thread_id) = thread_id {
            validate_bound_thread(&session, thread_id)?;
        }
        let run = self
            .plan_run_binding(&mut tx, guide_run_id, client_id)
            .await?;
        if let Some(existing) = message_by_key(&mut tx, session_id, key).await? {
            if existing.guide_run_id != guide_run_id
                || existing.role != input.role.as_str()
                || existing.content != content
                || existing.content_sha256 != content_sha256
                || existing.source_checkpoint_id != run.source_checkpoint_id
                || existing.source_checkpoint_revision != run.source_checkpoint_revision
                || existing.source_checkpoint_sha256 != run.source_checkpoint_sha256
                || existing.source_thread_id.as_deref() != thread_id
                || existing.source_turn_id.as_deref() != turn_id
            {
                return Err(idempotency(format!(
                    "workspace plan message key `{key}` was reused with different content"
                )));
            }
            tx.rollback().await?;
            return Ok(existing.try_into_model(true)?);
        }
        if thread_id.is_some() || turn_id.is_some() {
            return Err(validation(
                "human workspace plan messages cannot claim model thread or turn provenance",
            ));
        }
        if run.status != "running" || run.source_thread_id.is_some() || run.source_turn_id.is_some()
        {
            return Err(validation(
                "new human workspace plan messages require an unclaimed running guide run",
            ));
        }
        let sequence = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM workspace_plan_messages WHERE plan_session_id = ?",
        )
        .bind(session_id)
        .fetch_one(&mut *tx)
        .await?;
        let id = Uuid::new_v4().to_string();
        let now_ms = datetime_to_epoch_millis(Utc::now());
        sqlx::query(
            r#"
INSERT INTO workspace_plan_messages (
    id, plan_session_id, client_id, guide_run_id, sequence, role, content,
    content_sha256, idempotency_key, source_checkpoint_id,
    source_checkpoint_revision, source_checkpoint_sha256, encounter_id, note_id,
    source_thread_id, source_turn_id, created_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(session_id)
        .bind(client_id)
        .bind(guide_run_id)
        .bind(sequence)
        .bind(input.role.as_str())
        .bind(content)
        .bind(&content_sha256)
        .bind(key)
        .bind(&run.source_checkpoint_id)
        .bind(run.source_checkpoint_revision)
        .bind(&run.source_checkpoint_sha256)
        .bind(&run.encounter_id)
        .bind(&run.note_id)
        .bind(thread_id)
        .bind(turn_id)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "plan_message".to_string(),
                entity_id: id.clone(),
                action: "appended".to_string(),
                actor: "local human".to_string(),
                actor_kind: "human".to_string(),
                source: "workspace_plan".to_string(),
                client_id: Some(client_id.to_string()),
                encounter_id: run.encounter_id,
                note_id: run.note_id,
                source_thread_id: thread_id.map(str::to_string),
                source_turn_id: turn_id.map(str::to_string),
                success: true,
                summary: format!("{} message sequence {sequence}", input.role.as_str()),
                metadata_json: Some(
                    serde_json::json!({
                        "guideRunId": guide_run_id,
                        "planSessionId": session_id,
                        "sourceCheckpointId": run.source_checkpoint_id,
                        "contentSha256": content_sha256,
                    })
                    .to_string(),
                ),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        let row = message_by_id(&mut tx, &id)
            .await?
            .ok_or_else(|| validation("inserted workspace plan message was not found"))?;
        tx.commit().await?;
        Ok(row.try_into_model(false)?)
    }

    pub async fn list_plan_messages(
        &self,
        filter: crate::WorkspacePlanMessageFilter,
    ) -> PlanResult<Vec<crate::WorkspacePlanMessage>> {
        let session_id = required("message session id", &filter.plan_session_id)?;
        let client_id = required("message client id", &filter.client_id)?;
        let limit = filter.limit.unwrap_or(100).clamp(1, 500);
        if filter.after_sequence.is_some_and(|sequence| sequence < 0) {
            return Err(validation(
                "workspace plan message cursor must not be negative",
            ));
        }
        let session_client = sqlx::query_scalar::<_, String>(
            "SELECT client_id FROM workspace_plan_sessions WHERE id = ?",
        )
        .bind(session_id)
        .fetch_optional(self.pool.as_ref())
        .await?;
        match session_client.as_deref() {
            Some(actual) if actual == client_id => {}
            Some(actual) => {
                return Err(validation(format!(
                    "workspace plan session `{session_id}` belongs to client `{actual}` not `{client_id}`"
                )));
            }
            None => return Ok(Vec::new()),
        }
        let rows = if let Some(after_sequence) = filter.after_sequence {
            sqlx::query_as::<_, WorkspacePlanMessageRow>(
                r#"
SELECT id, plan_session_id, client_id, guide_run_id, sequence, role, content,
       content_sha256, idempotency_key, source_checkpoint_id,
       source_checkpoint_revision, source_checkpoint_sha256, encounter_id,
       note_id, source_thread_id, source_turn_id, created_at_ms
FROM workspace_plan_messages
WHERE plan_session_id = ? AND client_id = ? AND sequence > ?
ORDER BY sequence ASC
LIMIT ?
                "#,
            )
            .bind(session_id)
            .bind(client_id)
            .bind(after_sequence)
            .bind(i64::from(limit))
            .fetch_all(self.pool.as_ref())
            .await?
        } else {
            let mut latest = sqlx::query_as::<_, WorkspacePlanMessageRow>(
                r#"
SELECT id, plan_session_id, client_id, guide_run_id, sequence, role, content,
       content_sha256, idempotency_key, source_checkpoint_id,
       source_checkpoint_revision, source_checkpoint_sha256, encounter_id,
       note_id, source_thread_id, source_turn_id, created_at_ms
FROM workspace_plan_messages
WHERE plan_session_id = ? AND client_id = ?
ORDER BY sequence DESC
LIMIT ?
                "#,
            )
            .bind(session_id)
            .bind(client_id)
            .bind(i64::from(limit))
            .fetch_all(self.pool.as_ref())
            .await?;
            latest.reverse();
            latest
        };
        rows.into_iter()
            .map(|row| row.try_into_model(false).map_err(Into::into))
            .collect()
    }
}

async fn message_by_key(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    session_id: &str,
    key: &str,
) -> PlanResult<Option<WorkspacePlanMessageRow>> {
    Ok(sqlx::query_as(
        r#"
SELECT id, plan_session_id, client_id, guide_run_id, sequence, role, content,
       content_sha256, idempotency_key, source_checkpoint_id,
       source_checkpoint_revision, source_checkpoint_sha256, encounter_id,
       note_id, source_thread_id, source_turn_id, created_at_ms
FROM workspace_plan_messages
WHERE plan_session_id = ? AND idempotency_key = ?
        "#,
    )
    .bind(session_id)
    .bind(key)
    .fetch_optional(&mut **tx)
    .await?)
}

pub(super) async fn message_by_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: &str,
) -> PlanResult<Option<WorkspacePlanMessageRow>> {
    Ok(sqlx::query_as(
        r#"
SELECT id, plan_session_id, client_id, guide_run_id, sequence, role, content,
       content_sha256, idempotency_key, source_checkpoint_id,
       source_checkpoint_revision, source_checkpoint_sha256, encounter_id,
       note_id, source_thread_id, source_turn_id, created_at_ms
FROM workspace_plan_messages
WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?)
}
