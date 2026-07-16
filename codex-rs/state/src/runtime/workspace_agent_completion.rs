use super::workspace_agent_queries::workspace_agent_result_row_by_id;
use super::workspace_agent_queries::workspace_agent_run_row_by_id;
use super::workspace_agent_queries::workspace_context_packet_row_by_id;
use super::workspace_plan_binding::normalize_plan_revision_binding;
use super::workspace_plan_binding::require_submitted_plan_revision_receipt;
use super::*;
use crate::model::WorkspaceAgentTurnCompletionRow;
use sha2::Digest;
use sha2::Sha256;
use uuid::Uuid;

use super::workspace::WorkspaceStore;
use super::workspace::insert_audit_event;
use super::workspace_policy::require_synthetic_workspace;

const MAX_MASTER_AGENT_BODY_BYTES: usize = 256 * 1024;
const MAX_COMPLETION_ID_BYTES: usize = 512;
const MAX_COMPLETION_KEY_BYTES: usize = 1024;

impl WorkspaceStore {
    /// Reconciles interrupted master-agent handoffs on startup.
    ///
    /// A receipt-bound unclaimed run is deliberately preserved so the exact submitted Plan can be
    /// resumed. Claimed runs are terminalized because their master turn was interrupted, and
    /// unclaimed agent runs without a submission receipt are pre-submit orphans that cannot be
    /// selected as the durable handoff.
    pub(super) async fn reconcile_orphaned_agent_turns(&self) -> anyhow::Result<u64> {
        let classification: Option<String> = sqlx::query_scalar(
            "SELECT data_classification FROM workspace_data_policy WHERE singleton_id = 1",
        )
        .fetch_optional(self.pool.as_ref())
        .await?;
        if classification.as_deref() != Some("synthetic") {
            return Ok(0);
        }
        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let rows = sqlx::query(
            r#"
SELECT id, client_id, note_id, source_thread_id, source_turn_id
FROM workspace_agent_runs AS run
WHERE run.status = 'running'
  AND run.run_kind = 'agent'
  AND NOT EXISTS (
      SELECT 1 FROM workspace_agent_turn_completions AS completion
      WHERE completion.run_id = run.id
  )
  AND (
      run.source_turn_id IS NOT NULL
      OR NOT EXISTS (
          SELECT 1 FROM workspace_plan_submission_receipts AS receipt
          WHERE receipt.agent_run_id = run.id
      )
  )
            "#,
        )
        .fetch_all(&mut *tx)
        .await?;
        for row in &rows {
            let run_id: String = row.try_get("id")?;
            let client_id: String = row.try_get("client_id")?;
            let note_id: Option<String> = row.try_get("note_id")?;
            let source_thread_id: Option<String> = row.try_get("source_thread_id")?;
            let source_turn_id: Option<String> = row.try_get("source_turn_id")?;
            let was_claimed = source_turn_id.is_some();
            let reason = if was_claimed {
                "recovered after an interrupted master-agent turn"
            } else {
                "recovered an unclaimed master-agent run without a Plan submission receipt"
            };
            let updated = sqlx::query(
                r#"
UPDATE workspace_agent_runs
SET status = 'canceled', error_summary = ?, completed_at_ms = ?, updated_at_ms = ?
WHERE id = ?
  AND status = 'running'
  AND (
      source_turn_id IS NOT NULL
      OR NOT EXISTS (
          SELECT 1 FROM workspace_plan_submission_receipts AS receipt
          WHERE receipt.agent_run_id = workspace_agent_runs.id
      )
  )
                "#,
            )
            .bind(reason)
            .bind(now_ms)
            .bind(now_ms)
            .bind(&run_id)
            .execute(&mut *tx)
            .await?;
            if updated.rows_affected() != 1 {
                anyhow::bail!(
                    "workspace startup recovery could not atomically reconcile agent run `{run_id}`"
                );
            }
            insert_audit_event(
                &mut tx,
                crate::WorkspaceAuditEventCreate {
                    entity_type: "agent_run".to_string(),
                    entity_id: run_id,
                    action: "canceled".to_string(),
                    actor: "workspace startup recovery".to_string(),
                    actor_kind: "system".to_string(),
                    source: "state_recovery".to_string(),
                    client_id: Some(client_id),
                    note_id,
                    source_thread_id,
                    source_turn_id,
                    success: true,
                    summary: if was_claimed {
                        "orphaned claimed master-agent run closed on startup".to_string()
                    } else {
                        "unclaimed master-agent run without a Plan submission receipt closed on startup"
                            .to_string()
                    },
                    ..Default::default()
                },
                now_ms,
            )
            .await?;
        }
        let reconciled = u64::try_from(rows.len())?;
        tx.commit().await?;
        Ok(reconciled)
    }

    /// Atomically saves the exact final response from a capability-bound master-agent turn.
    ///
    /// This is the only path that may attribute a result to the agent harness. Ordinary RPC
    /// callers may still create explicit manual imports, but cannot supply agent-authored text.
    pub async fn complete_agent_turn(
        &self,
        input: crate::WorkspaceAgentTurnComplete,
    ) -> anyhow::Result<crate::WorkspaceAgentTurnCompletion> {
        let execution = normalize_execution(input.execution)?;
        let assistant_message_id = required_bounded(
            "workspace agent assistant message id",
            &input.assistant_message_id,
            MAX_COMPLETION_ID_BYTES,
        )?;
        let idempotency_key = required_bounded(
            "workspace agent completion idempotency key",
            &input.idempotency_key,
            MAX_COMPLETION_KEY_BYTES,
        )?;
        if input.body.trim().is_empty() {
            anyhow::bail!("workspace agent completion body must not be empty");
        }
        if input.body.len() > MAX_MASTER_AGENT_BODY_BYTES {
            anyhow::bail!(
                "workspace agent completion body exceeds the {MAX_MASTER_AGENT_BODY_BYTES} byte limit"
            );
        }
        let body_sha256 = sha256(input.body.as_bytes());
        let completion_input_sha256 = completion_input_sha256(
            &execution,
            assistant_message_id,
            &body_sha256,
            idempotency_key,
        )?;

        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        require_synthetic_workspace(&mut tx).await?;
        if let Some(existing) = completion_by_run(&mut tx, &execution.run_id).await? {
            ensure_exact_replay(
                &existing,
                &execution,
                assistant_message_id,
                &body_sha256,
                &completion_input_sha256,
                idempotency_key,
            )?;
            let result = completion_result(&mut tx, existing, true).await?;
            tx.rollback().await?;
            return Ok(result);
        }
        if let Some(existing_run_id) = sqlx::query_scalar::<_, String>(
            "SELECT run_id FROM workspace_agent_turn_completions WHERE idempotency_key = ?",
        )
        .bind(idempotency_key)
        .fetch_optional(&mut *tx)
        .await?
        {
            anyhow::bail!(
                "workspace agent completion key `{idempotency_key}` already belongs to run `{existing_run_id}`"
            );
        }

        let run = workspace_agent_run_row_by_id(&mut tx, &execution.run_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("workspace agent run `{}` was not found", execution.run_id)
            })?;
        if run.run_kind != "agent" {
            anyhow::bail!(
                "workspace agent run `{}` is kind `{}` and cannot complete as model output",
                run.id,
                run.run_kind
            );
        }
        if run.status != "running" {
            anyhow::bail!(
                "workspace agent run `{}` is `{}` and cannot complete as model output",
                run.id,
                run.status
            );
        }
        validate_execution(&run, &execution)?;
        let packet = workspace_context_packet_row_by_id(&mut tx, &run.packet_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "workspace context packet `{}` for run `{}` was not found",
                    run.packet_id,
                    run.id
                )
            })?;
        if packet.status != "submitted" {
            anyhow::bail!(
                "workspace context packet `{}` is `{}` and cannot complete an agent turn",
                packet.id,
                packet.status
            );
        }
        if packet.client_id != run.client_id
            || packet.note_id != run.note_id
            || packet.context_envelope_sha256 != run.context_envelope_sha256
            || packet.workspace_plan_revision_id != run.workspace_plan_revision_id
            || packet.workspace_plan_content_sha256 != run.workspace_plan_content_sha256
            || packet.workspace_plan_evidence_manifest_sha256
                != run.workspace_plan_evidence_manifest_sha256
        {
            anyhow::bail!(
                "workspace agent run `{}` no longer matches its authoritative context packet `{}`",
                run.id,
                packet.id
            );
        }
        if let Some(binding) = normalize_plan_revision_binding(
            packet.workspace_plan_revision_id.as_deref(),
            packet.workspace_plan_content_sha256.as_deref(),
            packet.workspace_plan_evidence_manifest_sha256.as_deref(),
        )? {
            require_submitted_plan_revision_receipt(
                &mut tx,
                &binding,
                &packet.id,
                &run.id,
                &packet.client_id,
            )
            .await?;
        }
        let prompt_source_exists = sqlx::query_scalar::<_, i64>(
            r#"
SELECT 1
FROM workspace_agent_run_sources
WHERE run_id = ? AND source_entity_type = 'handoff_prompt' AND source_entity_id = ?
LIMIT 1
            "#,
        )
        .bind(&run.id)
        .bind(&execution.source_turn_id)
        .fetch_optional(&mut *tx)
        .await?
        .is_some();
        if !prompt_source_exists {
            anyhow::bail!(
                "workspace agent run `{}` does not have a durable claimed handoff prompt",
                run.id
            );
        }

        let result_id = Uuid::new_v4().to_string();
        let completed_at_ms = datetime_to_epoch_millis(Utc::now());
        let result_kind = if packet.expected_output_kind.trim().is_empty() {
            "recommendation"
        } else {
            packet.expected_output_kind.trim()
        };
        sqlx::query(
            r#"
INSERT INTO workspace_agent_results (
    id, packet_id, client_id, note_id, run_id, base_note_revision,
    packet_context_sha256, body, summary, result_kind,
    structured_changes_json, rationale_summary, status, created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, '[]', '', 'review_pending', ?, ?)
            "#,
        )
        .bind(&result_id)
        .bind(&run.packet_id)
        .bind(&run.client_id)
        .bind(&run.note_id)
        .bind(&run.id)
        .bind(run.base_note_revision)
        .bind(&run.context_envelope_sha256)
        .bind(&input.body)
        .bind("Master agent response ready for clinician review")
        .bind(result_kind)
        .bind(completed_at_ms)
        .bind(completed_at_ms)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
INSERT INTO workspace_agent_turn_completions (
    run_id, result_id, packet_id, client_id, source_thread_id, source_turn_id,
    provider, model, assistant_message_id, body_sha256, completion_input_sha256,
    idempotency_key, completed_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&run.id)
        .bind(&result_id)
        .bind(&packet.id)
        .bind(&run.client_id)
        .bind(&execution.source_thread_id)
        .bind(&execution.source_turn_id)
        .bind(&execution.provider)
        .bind(&execution.model)
        .bind(assistant_message_id)
        .bind(&body_sha256)
        .bind(&completion_input_sha256)
        .bind(idempotency_key)
        .bind(completed_at_ms)
        .execute(&mut *tx)
        .await?;
        let updated = sqlx::query(
            "UPDATE workspace_agent_runs SET status = 'completed', completed_at_ms = ?, updated_at_ms = ? WHERE id = ? AND status = 'running'",
        )
        .bind(completed_at_ms)
        .bind(completed_at_ms)
        .bind(&run.id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            anyhow::bail!("workspace agent run `{}` completion raced", run.id);
        }

        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "agent_result".to_string(),
                entity_id: result_id.clone(),
                action: "saved".to_string(),
                actor: "agent".to_string(),
                actor_kind: "agent".to_string(),
                source: "agent_harness".to_string(),
                client_id: Some(run.client_id.clone()),
                note_id: run.note_id.clone(),
                source_thread_id: Some(execution.source_thread_id.clone()),
                source_turn_id: Some(execution.source_turn_id.clone()),
                success: true,
                summary: "exact master-agent response committed for review".to_string(),
                metadata_json: Some(
                    serde_json::json!({
                        "assistantMessageId": assistant_message_id,
                        "bodySha256": body_sha256,
                        "completionInputSha256": completion_input_sha256,
                        "packetId": packet.id,
                        "runId": run.id,
                        "workspacePlanRevisionId": run.workspace_plan_revision_id,
                    })
                    .to_string(),
                ),
                ..Default::default()
            },
            completed_at_ms,
        )
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "agent_run".to_string(),
                entity_id: run.id.clone(),
                action: "completed".to_string(),
                actor: "agent".to_string(),
                actor_kind: "agent".to_string(),
                source: "agent_harness".to_string(),
                client_id: Some(run.client_id),
                note_id: run.note_id,
                source_thread_id: Some(execution.source_thread_id),
                source_turn_id: Some(execution.source_turn_id),
                success: true,
                summary: format!("result {result_id} saved atomically"),
                ..Default::default()
            },
            completed_at_ms,
        )
        .await?;

        let row = completion_by_run(&mut tx, &run.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("inserted agent turn completion was not found"))?;
        let result = completion_result(&mut tx, row, false).await?;
        tx.commit().await?;
        Ok(result)
    }
}

fn required_bounded<'a>(label: &str, value: &'a str, max: usize) -> anyhow::Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        anyhow::bail!("{label} must not be empty");
    }
    if value.len() > max {
        anyhow::bail!("{label} exceeds the {max} byte limit");
    }
    Ok(value)
}

fn normalize_execution(
    mut execution: crate::WorkspaceAgentExecutionBinding,
) -> anyhow::Result<crate::WorkspaceAgentExecutionBinding> {
    execution.run_id = required_bounded("workspace agent run id", &execution.run_id, 512)?.into();
    execution.source_thread_id = required_bounded(
        "workspace agent source thread id",
        &execution.source_thread_id,
        MAX_COMPLETION_ID_BYTES,
    )?
    .into();
    execution.source_turn_id = required_bounded(
        "workspace agent source turn id",
        &execution.source_turn_id,
        MAX_COMPLETION_ID_BYTES,
    )?
    .into();
    execution.provider =
        required_bounded("workspace agent provider", &execution.provider, 256)?.into();
    execution.model = required_bounded("workspace agent model", &execution.model, 256)?.into();
    Ok(execution)
}

fn validate_execution(
    run: &crate::model::WorkspaceAgentRunRow,
    execution: &crate::WorkspaceAgentExecutionBinding,
) -> anyhow::Result<()> {
    if run.id != execution.run_id
        || run.source_thread_id.as_deref() != Some(execution.source_thread_id.as_str())
        || run.source_turn_id.as_deref() != Some(execution.source_turn_id.as_str())
        || run.provider != execution.provider
        || run.model != execution.model
    {
        anyhow::bail!("workspace agent completion does not match the durably claimed run identity");
    }
    Ok(())
}

fn completion_input_sha256(
    execution: &crate::WorkspaceAgentExecutionBinding,
    assistant_message_id: &str,
    body_sha256: &str,
    idempotency_key: &str,
) -> anyhow::Result<String> {
    let json = serde_json::to_string(&serde_json::json!({
        "schemaVersion": 1,
        "runId": execution.run_id,
        "sourceThreadId": execution.source_thread_id,
        "sourceTurnId": execution.source_turn_id,
        "provider": execution.provider,
        "model": execution.model,
        "assistantMessageId": assistant_message_id,
        "bodySha256": body_sha256,
        "idempotencyKey": idempotency_key,
    }))?;
    Ok(sha256(json.as_bytes()))
}

fn ensure_exact_replay(
    existing: &WorkspaceAgentTurnCompletionRow,
    execution: &crate::WorkspaceAgentExecutionBinding,
    assistant_message_id: &str,
    body_sha256: &str,
    completion_input_sha256: &str,
    idempotency_key: &str,
) -> anyhow::Result<()> {
    if existing.run_id != execution.run_id
        || existing.source_thread_id != execution.source_thread_id
        || existing.source_turn_id != execution.source_turn_id
        || existing.provider != execution.provider
        || existing.model != execution.model
        || existing.assistant_message_id != assistant_message_id
        || existing.body_sha256 != body_sha256
        || existing.completion_input_sha256 != completion_input_sha256
        || existing.idempotency_key != idempotency_key
    {
        anyhow::bail!(
            "workspace agent run `{}` already has a different terminal completion",
            execution.run_id
        );
    }
    Ok(())
}

async fn completion_by_run(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    run_id: &str,
) -> anyhow::Result<Option<WorkspaceAgentTurnCompletionRow>> {
    Ok(sqlx::query_as(
        r#"
SELECT run_id, result_id, packet_id, client_id, source_thread_id, source_turn_id,
       provider, model, assistant_message_id, body_sha256, completion_input_sha256,
       idempotency_key, completed_at_ms
FROM workspace_agent_turn_completions
WHERE run_id = ?
        "#,
    )
    .bind(run_id)
    .fetch_optional(&mut **tx)
    .await?)
}

async fn completion_result(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    row: WorkspaceAgentTurnCompletionRow,
    replayed: bool,
) -> anyhow::Result<crate::WorkspaceAgentTurnCompletion> {
    let result_row = workspace_agent_result_row_by_id(tx, &row.result_id)
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "workspace agent completion result `{}` was not found",
                row.result_id
            )
        })?;
    if result_row.run_id.as_deref() != Some(row.run_id.as_str())
        || result_row.packet_id != row.packet_id
        || result_row.client_id != row.client_id
        || sha256(result_row.body.as_bytes()) != row.body_sha256
    {
        anyhow::bail!("workspace agent completion failed its persisted integrity checks");
    }
    let run_status: Option<String> =
        sqlx::query_scalar("SELECT status FROM workspace_agent_runs WHERE id = ?")
            .bind(&row.run_id)
            .fetch_optional(&mut **tx)
            .await?;
    if run_status.as_deref() != Some("completed") {
        anyhow::bail!("workspace agent completion does not own a completed run");
    }
    Ok(crate::WorkspaceAgentTurnCompletion {
        result: result_row.try_into()?,
        source_thread_id: row.source_thread_id,
        source_turn_id: row.source_turn_id,
        provider: row.provider,
        model: row.model,
        assistant_message_id: row.assistant_message_id,
        body_sha256: row.body_sha256,
        completion_input_sha256: row.completion_input_sha256,
        idempotency_key: row.idempotency_key,
        completed_at: epoch_millis_to_datetime(row.completed_at_ms)?,
        replayed,
    })
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
