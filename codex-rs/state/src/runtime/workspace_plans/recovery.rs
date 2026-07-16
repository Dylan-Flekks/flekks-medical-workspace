use sha2::Digest;
use sha2::Sha256;
use sqlx::Sqlite;
use uuid::Uuid;

use super::PlanResult;
use super::required;
use super::validation;
use crate::model::WorkspacePlanMessageRow;
use crate::model::WorkspacePlanRevisionRow;
use crate::model::datetime_to_epoch_millis;
use crate::model::epoch_millis_to_datetime;
use crate::runtime::workspace::WorkspaceStore;
use crate::runtime::workspace::insert_audit_event;
use chrono::Utc;

const INTERRUPTED_BEFORE_CLAIM_REASON: &str =
    "planning process ended before the model turn was durably claimed";
const INTERRUPTED_BEFORE_CLAIM_MESSAGE: &str = "Codex planning stopped before the model turn was attached. Your message is saved; send it again to continue.";

#[derive(sqlx::FromRow)]
struct ActiveRunClaimRow {
    guide_run_id: String,
    plan_session_id: String,
    source_thread_id: String,
    source_turn_id: String,
    provider: String,
    model: String,
    prompt_sha256: String,
    claimed_at_ms: i64,
    context_read_count: i64,
}

#[derive(sqlx::FromRow)]
struct UnclaimedPlanRunRow {
    id: String,
    source_checkpoint_id: String,
    source_checkpoint_revision: i64,
    source_checkpoint_sha256: String,
    encounter_id: Option<String>,
    note_id: Option<String>,
}

impl WorkspaceStore {
    pub async fn list_active_plan_runs(
        &self,
        filter: crate::WorkspacePlanActiveRunFilter,
    ) -> PlanResult<Vec<crate::WorkspacePlanActiveRun>> {
        let plan_session_id = required("active run plan session id", &filter.plan_session_id)?;
        let client_id = required("active run client id", &filter.client_id)?;
        let mut tx = self.pool.begin().await?;
        self.require_active_plan_session(&mut tx, plan_session_id, client_id)
            .await?;
        let rows = active_run_rows(&mut tx, plan_session_id, client_id).await?;
        let result = active_runs_from_rows(&mut tx, rows).await?;
        tx.rollback().await?;
        Ok(result)
    }

    pub async fn list_pending_plan_questions(
        &self,
        filter: crate::WorkspacePlanPendingQuestionFilter,
    ) -> PlanResult<Vec<crate::WorkspacePlanMessage>> {
        let plan_session_id =
            required("pending question plan session id", &filter.plan_session_id)?;
        let client_id = required("pending question client id", &filter.client_id)?;
        let mut tx = self.pool.begin().await?;
        self.require_active_plan_session(&mut tx, plan_session_id, client_id)
            .await?;
        let result = pending_questions(&mut tx, plan_session_id, client_id).await?;
        tx.rollback().await?;
        Ok(result)
    }

    /// Rebuilds the persistent plan rail from a single consistent SQLite snapshot.
    pub async fn reconcile_plan_session(
        &self,
        input: crate::WorkspacePlanRecoveryRequest,
    ) -> PlanResult<crate::WorkspacePlanRecoveryState> {
        let plan_session_id = required("recovery plan session id", &input.plan_session_id)?;
        let client_id = required("recovery client id", &input.client_id)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        self.require_active_plan_session(&mut tx, plan_session_id, client_id)
            .await?;
        cancel_unclaimed_plan_runs(&mut tx, plan_session_id, client_id).await?;
        let session = self
            .require_active_plan_session(&mut tx, plan_session_id, client_id)
            .await?;
        let active_rows = active_run_rows(&mut tx, plan_session_id, client_id).await?;
        let active_runs = active_runs_from_rows(&mut tx, active_rows).await?;
        let pending_questions = pending_questions(&mut tx, plan_session_id, client_id).await?;
        let current_revision = current_revision(&mut tx, plan_session_id, client_id).await?;
        let last_completion =
            super::completion::last_completion_for_session(&mut tx, plan_session_id, client_id)
                .await?
                .map(|row| row.try_into_receipt(false))
                .transpose()?;
        let session = session.try_into_model(false)?;
        tx.commit().await?;
        Ok(crate::WorkspacePlanRecoveryState {
            session,
            active_runs,
            pending_questions,
            current_revision,
            last_completion,
        })
    }
}

async fn cancel_unclaimed_plan_runs(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    plan_session_id: &str,
    client_id: &str,
) -> PlanResult<()> {
    let rows = sqlx::query_as::<_, UnclaimedPlanRunRow>(
        r#"
SELECT run.id, run.source_checkpoint_id, run.source_checkpoint_revision,
       run.source_checkpoint_sha256, checkpoint.encounter_id, checkpoint.note_id
FROM workspace_guide_runs AS run
JOIN workspace_draft_checkpoints AS checkpoint
  ON checkpoint.id = run.source_checkpoint_id
 AND checkpoint.session_id = run.session_id
 AND checkpoint.client_id = run.client_id
WHERE run.client_id = ? AND run.model_tool_mode = 'workspace_planning_only'
  AND run.status = 'running'
  AND run.source_thread_id IS NULL AND run.source_turn_id IS NULL
  AND NOT EXISTS (
      SELECT 1
      FROM workspace_planning_turn_claims AS claim
      WHERE claim.guide_run_id = run.id
  )
  AND (
      json_extract(run.request_envelope_json, '$.request.planSessionId') = ?
      OR EXISTS (
          SELECT 1
          FROM workspace_plan_messages AS message
          WHERE message.guide_run_id = run.id
            AND message.plan_session_id = ? AND message.client_id = ?
      )
  )
ORDER BY run.created_at_ms ASC, run.id ASC
        "#,
    )
    .bind(client_id)
    .bind(plan_session_id)
    .bind(plan_session_id)
    .bind(client_id)
    .fetch_all(&mut **tx)
    .await?;
    if rows.is_empty() {
        return Ok(());
    }

    let terminal_envelope_json = serde_json::to_string(&serde_json::json!({
        "schemaVersion": 1,
        "type": "canceled",
        "reason": INTERRUPTED_BEFORE_CLAIM_REASON,
    }))?;
    let terminal_envelope_sha256 =
        format!("{:x}", Sha256::digest(terminal_envelope_json.as_bytes()));
    let now_ms = datetime_to_epoch_millis(Utc::now());

    for row in rows {
        let updated = sqlx::query(
            r#"
UPDATE workspace_guide_runs
SET status = 'canceled', terminal_envelope_json = ?,
    terminal_envelope_sha256 = ?, updated_at_ms = ?, terminal_at_ms = ?
WHERE id = ? AND status = 'running'
  AND source_thread_id IS NULL AND source_turn_id IS NULL
  AND NOT EXISTS (
      SELECT 1
      FROM workspace_planning_turn_claims AS claim
      WHERE claim.guide_run_id = ?
  )
            "#,
        )
        .bind(&terminal_envelope_json)
        .bind(&terminal_envelope_sha256)
        .bind(now_ms)
        .bind(now_ms)
        .bind(&row.id)
        .bind(&row.id)
        .execute(&mut **tx)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(anyhow::anyhow!(
                "unclaimed workspace planning guide run `{}` changed during recovery",
                row.id
            )
            .into());
        }

        insert_audit_event(
            tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "guide_run".to_string(),
                entity_id: row.id.clone(),
                action: "canceled".to_string(),
                actor: "workspace plan recovery".to_string(),
                actor_kind: "system".to_string(),
                source: "workspace_plan".to_string(),
                client_id: Some(client_id.to_string()),
                encounter_id: row.encounter_id.clone(),
                note_id: row.note_id.clone(),
                success: true,
                summary: "unclaimed workspace planning run canceled during recovery".to_string(),
                metadata_json: Some(
                    serde_json::json!({
                        "planSessionId": plan_session_id,
                        "reason": INTERRUPTED_BEFORE_CLAIM_REASON,
                    })
                    .to_string(),
                ),
                ..Default::default()
            },
            now_ms,
        )
        .await?;

        if has_persisted_human_message(tx, plan_session_id, client_id, &row.id).await? {
            append_recovery_error_message(tx, plan_session_id, client_id, &row, now_ms).await?;
        }
    }

    sqlx::query(
        r#"
UPDATE workspace_plan_sessions
SET updated_at_ms = CASE
    WHEN updated_at_ms >= ? THEN updated_at_ms + 1
    ELSE ?
END
WHERE id = ? AND client_id = ? AND status = 'active'
        "#,
    )
    .bind(now_ms)
    .bind(now_ms)
    .bind(plan_session_id)
    .bind(client_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn has_persisted_human_message(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    plan_session_id: &str,
    client_id: &str,
    guide_run_id: &str,
) -> PlanResult<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        r#"
SELECT EXISTS (
    SELECT 1
    FROM workspace_plan_messages
    WHERE plan_session_id = ? AND client_id = ? AND guide_run_id = ?
      AND role IN ('human', 'answer')
)
        "#,
    )
    .bind(plan_session_id)
    .bind(client_id)
    .bind(guide_run_id)
    .fetch_one(&mut **tx)
    .await?
        != 0)
}

async fn append_recovery_error_message(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    plan_session_id: &str,
    client_id: &str,
    run: &UnclaimedPlanRunRow,
    now_ms: i64,
) -> PlanResult<()> {
    let idempotency_key = format!("workspace-plan-recovery:{}", run.id);
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT EXISTS (SELECT 1 FROM workspace_plan_messages WHERE plan_session_id = ? AND idempotency_key = ?)",
    )
    .bind(plan_session_id)
    .bind(&idempotency_key)
    .fetch_one(&mut **tx)
    .await?
        != 0;
    if exists {
        return Ok(());
    }

    let sequence = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(MAX(sequence), 0) + 1 FROM workspace_plan_messages WHERE plan_session_id = ?",
    )
    .bind(plan_session_id)
    .fetch_one(&mut **tx)
    .await?;
    let id = Uuid::new_v4().to_string();
    let content_sha256 = format!(
        "{:x}",
        Sha256::digest(INTERRUPTED_BEFORE_CLAIM_MESSAGE.as_bytes())
    );
    sqlx::query(
        r#"
INSERT INTO workspace_plan_messages (
    id, plan_session_id, client_id, guide_run_id, sequence, role, content,
    content_sha256, idempotency_key, source_checkpoint_id,
    source_checkpoint_revision, source_checkpoint_sha256, encounter_id, note_id,
    source_thread_id, source_turn_id, created_at_ms
) VALUES (?, ?, ?, ?, ?, 'error', ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, ?)
        "#,
    )
    .bind(&id)
    .bind(plan_session_id)
    .bind(client_id)
    .bind(&run.id)
    .bind(sequence)
    .bind(INTERRUPTED_BEFORE_CLAIM_MESSAGE)
    .bind(&content_sha256)
    .bind(&idempotency_key)
    .bind(&run.source_checkpoint_id)
    .bind(run.source_checkpoint_revision)
    .bind(&run.source_checkpoint_sha256)
    .bind(&run.encounter_id)
    .bind(&run.note_id)
    .bind(now_ms)
    .execute(&mut **tx)
    .await?;
    insert_audit_event(
        tx,
        crate::WorkspaceAuditEventCreate {
            entity_type: "plan_message".to_string(),
            entity_id: id,
            action: "appended".to_string(),
            actor: "workspace plan recovery".to_string(),
            actor_kind: "system".to_string(),
            source: "workspace_plan".to_string(),
            client_id: Some(client_id.to_string()),
            encounter_id: run.encounter_id.clone(),
            note_id: run.note_id.clone(),
            success: true,
            summary: format!("error message sequence {sequence}"),
            metadata_json: Some(
                serde_json::json!({
                    "guideRunId": run.id,
                    "planSessionId": plan_session_id,
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
    Ok(())
}

async fn active_run_rows(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    plan_session_id: &str,
    client_id: &str,
) -> PlanResult<Vec<ActiveRunClaimRow>> {
    Ok(sqlx::query_as(
        r#"
SELECT claim.guide_run_id, claim.plan_session_id, claim.source_thread_id,
       claim.source_turn_id, claim.provider, claim.model, claim.prompt_sha256,
       claim.claimed_at_ms, COUNT(read.id) AS context_read_count
FROM workspace_planning_turn_claims AS claim
JOIN workspace_guide_runs AS run ON run.id = claim.guide_run_id
LEFT JOIN workspace_planning_context_reads AS read
  ON read.guide_run_id = claim.guide_run_id
WHERE claim.plan_session_id = ? AND claim.client_id = ? AND run.status = 'running'
GROUP BY claim.guide_run_id, claim.plan_session_id, claim.source_thread_id,
         claim.source_turn_id, claim.provider, claim.model, claim.prompt_sha256,
         claim.claimed_at_ms
ORDER BY claim.claimed_at_ms ASC, claim.guide_run_id ASC
        "#,
    )
    .bind(plan_session_id)
    .bind(client_id)
    .fetch_all(&mut **tx)
    .await?)
}

async fn active_runs_from_rows(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    rows: Vec<ActiveRunClaimRow>,
) -> PlanResult<Vec<crate::WorkspacePlanActiveRun>> {
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        let run = crate::runtime::workspace_guides::run_by_id(tx, &row.guide_run_id)
            .await?
            .ok_or_else(|| {
                validation(format!(
                    "active workspace planning guide run `{}` was not found",
                    row.guide_run_id
                ))
            })?
            .try_into_model(false)?;
        result.push(crate::WorkspacePlanActiveRun {
            run,
            plan_session_id: row.plan_session_id,
            source_thread_id: row.source_thread_id,
            source_turn_id: row.source_turn_id,
            provider: row.provider,
            model: row.model,
            prompt_sha256: row.prompt_sha256,
            context_read_count: u32::try_from(row.context_read_count)
                .map_err(|_| validation("active plan context read count exceeds u32 range"))?,
            claimed_at: epoch_millis_to_datetime(row.claimed_at_ms)?,
        });
    }
    Ok(result)
}

async fn pending_questions(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    plan_session_id: &str,
    client_id: &str,
) -> PlanResult<Vec<crate::WorkspacePlanMessage>> {
    let rows = sqlx::query_as::<_, WorkspacePlanMessageRow>(
        r#"
SELECT question.id, question.plan_session_id, question.client_id,
       question.guide_run_id, question.sequence, question.role, question.content,
       question.content_sha256, question.idempotency_key,
       question.source_checkpoint_id, question.source_checkpoint_revision,
       question.source_checkpoint_sha256, question.encounter_id, question.note_id,
       question.source_thread_id, question.source_turn_id, question.created_at_ms
FROM workspace_plan_messages AS question
JOIN workspace_plan_turn_completions AS completion
  ON completion.guide_run_id = question.guide_run_id
 AND completion.assistant_message_id = question.id
WHERE question.plan_session_id = ? AND question.client_id = ?
  AND question.role = 'question'
  AND NOT EXISTS (
      SELECT 1
      FROM workspace_plan_messages AS answer
      WHERE answer.plan_session_id = question.plan_session_id
        AND answer.role IN ('human', 'answer')
        AND answer.sequence > question.sequence
  )
ORDER BY question.sequence ASC
        "#,
    )
    .bind(plan_session_id)
    .bind(client_id)
    .fetch_all(&mut **tx)
    .await?;
    rows.into_iter()
        .map(|row| row.try_into_model(false).map_err(Into::into))
        .collect()
}

async fn current_revision(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    plan_session_id: &str,
    client_id: &str,
) -> PlanResult<Option<crate::WorkspacePlanRevision>> {
    let row = sqlx::query_as::<_, WorkspacePlanRevisionRow>(
        r#"
SELECT id, plan_session_id, client_id, guide_run_id, revision, plan_markdown,
       decisions_json, open_questions_json, content_sha256,
       evidence_manifest_json, evidence_manifest_sha256, evidence_read_count,
       idempotency_key, status, source_checkpoint_id, source_checkpoint_revision,
       source_checkpoint_sha256, encounter_id, note_id, source_thread_id,
       source_turn_id, created_at_ms, submitted_at_ms
FROM workspace_plan_revisions
WHERE plan_session_id = ? AND client_id = ? AND status = 'current'
        "#,
    )
    .bind(plan_session_id)
    .bind(client_id)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(|row| row.try_into_model(false))
        .transpose()
        .map_err(Into::into)
}
