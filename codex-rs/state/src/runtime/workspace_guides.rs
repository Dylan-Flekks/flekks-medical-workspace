use super::workspace::WorkspaceStore;
use super::workspace::insert_audit_event;
use super::workspace::validate_agent_visible_json;
use crate::model::WorkspaceGuideRunRow;
use crate::model::datetime_to_epoch_millis;
use chrono::Utc;
use serde_json::Value;
use sha2::Digest;
use sha2::Sha256;
use sqlx::Sqlite;
use uuid::Uuid;

const GUIDE_SCHEMA_VERSION: i64 = 1;
const MAX_REQUEST_BYTES: usize = 32 * 1024;
const MAX_TERMINAL_BYTES: usize = 16 * 1024;

macro_rules! guide_run_query {
    ($suffix:literal) => {
        concat!(
            "SELECT run.id, run.client_id, run.session_id, run.source_checkpoint_id, ",
            "run.source_checkpoint_revision, run.source_checkpoint_sha256, ",
            "run.request_schema_version, run.request_envelope_json, ",
            "run.request_envelope_sha256, run.idempotency_key, run.trigger, run.actor, ",
            "run.provider, run.model, run.status, run.source_thread_id, run.source_turn_id, ",
            "run.terminal_envelope_json, run.terminal_envelope_sha256, ",
            "run.created_at_ms, run.updated_at_ms, run.terminal_at_ms, ",
            "CASE WHEN current.id = run.source_checkpoint_id ",
            "AND current.revision = run.source_checkpoint_revision ",
            "AND current.content_sha256 = run.source_checkpoint_sha256 ",
            "THEN 0 ELSE 1 END AS is_stale ",
            "FROM workspace_guide_runs AS run ",
            "JOIN workspace_draft_sessions AS session ON session.id = run.session_id ",
            "JOIN workspace_draft_checkpoints AS current ",
            "ON current.session_id = session.id AND current.revision = session.current_revision",
            $suffix
        )
    };
}

impl WorkspaceStore {
    pub async fn start_guide_run(
        &self,
        input: crate::WorkspaceGuideRunStart,
    ) -> anyhow::Result<crate::WorkspaceGuideRun> {
        validate_start(&input)?;
        let request: Value = serde_json::from_str(input.request_json.trim()).map_err(|error| {
            anyhow::anyhow!("workspace guide request must be valid JSON: {error}")
        })?;
        if !request.is_object() {
            return validation("workspace guide request must be a JSON object");
        }
        validate_agent_visible_json("guide request", &request)?;

        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        if let Some(existing) =
            run_by_key(&mut tx, &input.session_id, &input.idempotency_key).await?
        {
            let (envelope, hash) = request_envelope(&existing.id, &input, request)?;
            if existing.client_id != input.client_id.trim()
                || existing.source_checkpoint_id != input.source_checkpoint_id.trim()
                || existing.source_checkpoint_revision != input.source_checkpoint_revision
                || existing.source_checkpoint_sha256 != input.source_checkpoint_sha256.trim()
                || existing.request_envelope_json != envelope
                || existing.request_envelope_sha256 != hash
                || existing.trigger != input.trigger.trim()
                || existing.actor != input.actor.trim()
                || existing.provider != input.provider.trim()
                || existing.model != input.model.trim()
            {
                return conflict(format!(
                    "workspace guide idempotency key `{}` was reused with different content",
                    input.idempotency_key.trim()
                ));
            }
            tx.rollback().await?;
            return existing.try_into_model(true);
        }

        let checkpoint = checkpoint_binding(&mut tx, input.source_checkpoint_id.trim())
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "workspace guide checkpoint `{}` was not found",
                    input.source_checkpoint_id.trim()
                )
            })?;
        validate_checkpoint(&input, &checkpoint)?;
        if let Some(active_id) = sqlx::query_scalar::<_, String>(
            "SELECT id FROM workspace_guide_runs WHERE session_id = ? AND status = 'running'",
        )
        .bind(input.session_id.trim())
        .fetch_optional(&mut *tx)
        .await?
        {
            return conflict(format!(
                "workspace guide session `{}` already has active run `{active_id}`",
                input.session_id.trim()
            ));
        }

        let id = Uuid::new_v4().to_string();
        let (envelope, envelope_hash) = request_envelope(&id, &input, request)?;
        let now_ms = datetime_to_epoch_millis(Utc::now());
        sqlx::query(
            r#"
INSERT INTO workspace_guide_runs (
    id, client_id, session_id, source_checkpoint_id, source_checkpoint_revision,
    source_checkpoint_sha256, request_schema_version, request_envelope_json,
    request_envelope_sha256, idempotency_key, trigger, actor, provider, model,
    model_tool_mode, status, created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'disabled', 'running', ?, ?)
            "#,
        )
        .bind(&id)
        .bind(input.client_id.trim())
        .bind(input.session_id.trim())
        .bind(input.source_checkpoint_id.trim())
        .bind(input.source_checkpoint_revision)
        .bind(input.source_checkpoint_sha256.trim())
        .bind(GUIDE_SCHEMA_VERSION)
        .bind(&envelope)
        .bind(&envelope_hash)
        .bind(input.idempotency_key.trim())
        .bind(input.trigger.trim())
        .bind(input.actor.trim())
        .bind(input.provider.trim())
        .bind(input.model.trim())
        .bind(now_ms)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "guide_run".to_string(),
                entity_id: id.clone(),
                action: "started".to_string(),
                actor: input.actor.trim().to_string(),
                actor_kind: "human".to_string(),
                source: "workspace_guide".to_string(),
                client_id: Some(input.client_id.trim().to_string()),
                success: true,
                summary: "workspace guide run started".to_string(),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        let row = run_by_id(&mut tx, &id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("inserted workspace guide run was not found"))?;
        tx.commit().await?;
        row.try_into_model(false)
    }

    pub async fn finish_guide_run(
        &self,
        input: crate::WorkspaceGuideRunFinish,
    ) -> anyhow::Result<crate::WorkspaceGuideRun> {
        required("run id", &input.run_id)?;
        required("actor", &input.actor)?;
        let thread_id = normalized_optional(input.source_thread_id.as_deref());
        let turn_id = normalized_optional(input.source_turn_id.as_deref());
        if thread_id.is_some() != turn_id.is_some() {
            return validation("workspace guide source thread and turn must be supplied together");
        }
        let (status, terminal, terminal_hash) = terminal_envelope(&input.outcome)?;
        if status == crate::WorkspaceGuideRunStatus::Completed && thread_id.is_none() {
            return validation("completed workspace guide runs require source thread and turn ids");
        }

        let now_ms = datetime_to_epoch_millis(Utc::now());
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let existing = run_by_id(&mut tx, input.run_id.trim())
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "workspace guide run `{}` was not found",
                    input.run_id.trim()
                )
            })?;
        validate_finish_identity(&existing, &input)?;
        if existing.status != crate::WorkspaceGuideRunStatus::Running.as_str() {
            if existing.status == status.as_str()
                && existing.source_thread_id.as_deref() == thread_id
                && existing.source_turn_id.as_deref() == turn_id
                && existing.terminal_envelope_json.as_deref() == Some(terminal.as_str())
                && existing.terminal_envelope_sha256.as_deref() == Some(terminal_hash.as_str())
            {
                tx.rollback().await?;
                return existing.try_into_model(true);
            }
            return conflict(format!(
                "workspace guide run `{}` already finished with different terminal content",
                existing.id
            ));
        }

        let updated = sqlx::query(
            "UPDATE workspace_guide_runs SET status = ?, source_thread_id = ?, source_turn_id = ?, terminal_envelope_json = ?, terminal_envelope_sha256 = ?, updated_at_ms = ?, terminal_at_ms = ? WHERE id = ? AND status = 'running'",
        )
        .bind(status.as_str())
        .bind(thread_id)
        .bind(turn_id)
        .bind(&terminal)
        .bind(&terminal_hash)
        .bind(now_ms)
        .bind(now_ms)
        .bind(&existing.id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            return conflict(format!(
                "workspace guide run `{}` terminal update raced",
                existing.id
            ));
        }
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "guide_run".to_string(),
                entity_id: existing.id.clone(),
                action: status.as_str().to_string(),
                actor: input.actor.trim().to_string(),
                actor_kind: "agent".to_string(),
                source: "workspace_guide".to_string(),
                client_id: Some(existing.client_id),
                source_thread_id: thread_id.map(str::to_string),
                source_turn_id: turn_id.map(str::to_string),
                success: status != crate::WorkspaceGuideRunStatus::Failed,
                summary: format!("workspace guide run {}", status.as_str()),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        let row = run_by_id(&mut tx, input.run_id.trim())
            .await?
            .ok_or_else(|| anyhow::anyhow!("finished workspace guide run was not found"))?;
        tx.commit().await?;
        row.try_into_model(false)
    }

    pub async fn list_guide_runs(
        &self,
        filter: crate::WorkspaceGuideRunFilter,
    ) -> anyhow::Result<Vec<crate::WorkspaceGuideRun>> {
        let client_id = required("client id", &filter.client_id)?;
        let session_id = normalized_optional(filter.session_id.as_deref());
        let rows = sqlx::query_as::<_, WorkspaceGuideRunRow>(guide_run_query!(
            " WHERE run.client_id = ? AND (? IS NULL OR run.session_id = ?) ORDER BY run.created_at_ms DESC, run.id DESC LIMIT ?"
        ))
            .bind(client_id)
            .bind(session_id)
            .bind(session_id)
            .bind(i64::from(filter.limit.unwrap_or(20).clamp(1, 100)))
            .fetch_all(self.pool.as_ref())
            .await?;
        rows.into_iter()
            .map(|row| row.try_into_model(false))
            .collect()
    }
}

#[derive(sqlx::FromRow)]
struct CheckpointBinding {
    client_id: String,
    session_id: String,
    revision: i64,
    content_sha256: String,
    session_status: String,
    current_id: String,
}

async fn checkpoint_binding(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<Option<CheckpointBinding>> {
    Ok(sqlx::query_as(
        r#"
SELECT checkpoint.client_id, checkpoint.session_id, checkpoint.revision,
       checkpoint.content_sha256, session.status AS session_status,
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

fn validate_checkpoint(
    input: &crate::WorkspaceGuideRunStart,
    checkpoint: &CheckpointBinding,
) -> anyhow::Result<()> {
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
        return conflict("workspace guide source checkpoint is no longer current");
    }
    Ok(())
}

fn validate_start(input: &crate::WorkspaceGuideRunStart) -> anyhow::Result<()> {
    required("client id", &input.client_id)?;
    required("session id", &input.session_id)?;
    required("source checkpoint id", &input.source_checkpoint_id)?;
    required("idempotency key", &input.idempotency_key)?;
    required("trigger", &input.trigger)?;
    required("actor", &input.actor)?;
    required("provider", &input.provider)?;
    required("model", &input.model)?;
    if input.source_checkpoint_revision < 1 {
        return validation("workspace guide source checkpoint revision must be positive");
    }
    Ok(())
}

fn validate_finish_identity(
    run: &WorkspaceGuideRunRow,
    input: &crate::WorkspaceGuideRunFinish,
) -> anyhow::Result<()> {
    if run.client_id != input.client_id.trim()
        || run.session_id != input.session_id.trim()
        || run.source_checkpoint_id != input.source_checkpoint_id.trim()
        || run.source_checkpoint_revision != input.source_checkpoint_revision
        || run.source_checkpoint_sha256 != input.source_checkpoint_sha256.trim()
        || run.request_envelope_sha256 != input.request_envelope_sha256.trim()
    {
        return validation("workspace guide finish identity does not match the persisted run");
    }
    Ok(())
}

fn request_envelope(
    run_id: &str,
    input: &crate::WorkspaceGuideRunStart,
    request: Value,
) -> anyhow::Result<(String, String)> {
    let value = serde_json::json!({
        "schemaVersion": GUIDE_SCHEMA_VERSION,
        "kind": "workspaceGuide",
        "guideRunId": run_id,
        "sourceCheckpoint": {
            "clientId": input.client_id.trim(),
            "sessionId": input.session_id.trim(),
            "id": input.source_checkpoint_id.trim(),
            "revision": input.source_checkpoint_revision,
            "contentSha256": input.source_checkpoint_sha256.trim(),
        },
        "safety": {
            "readOnly": true,
            "canonicalChartWrites": false,
            "modelToolMode": "disabled",
        },
        "request": request,
    });
    normalize_envelope("request", value, MAX_REQUEST_BYTES)
}

fn terminal_envelope(
    outcome: &crate::WorkspaceGuideRunOutcome,
) -> anyhow::Result<(crate::WorkspaceGuideRunStatus, String, String)> {
    let (status, value) = match outcome {
        crate::WorkspaceGuideRunOutcome::Completed { result_json } => {
            let result: Value = serde_json::from_str(result_json.trim()).map_err(|error| {
                anyhow::anyhow!("workspace guide result must be valid JSON: {error}")
            })?;
            if !result.is_object() || result.get("schemaVersion").and_then(Value::as_i64) != Some(1)
            {
                return validation("workspace guide result must be a schemaVersion 1 JSON object");
            }
            (
                crate::WorkspaceGuideRunStatus::Completed,
                serde_json::json!({"schemaVersion": 1, "type": "completed", "result": result}),
            )
        }
        crate::WorkspaceGuideRunOutcome::Failed { error_summary } => {
            required("failure summary", error_summary)?;
            (
                crate::WorkspaceGuideRunStatus::Failed,
                serde_json::json!({"schemaVersion": 1, "type": "failed", "errorSummary": error_summary.trim()}),
            )
        }
        crate::WorkspaceGuideRunOutcome::Canceled { reason } => {
            required("cancellation reason", reason)?;
            (
                crate::WorkspaceGuideRunStatus::Canceled,
                serde_json::json!({"schemaVersion": 1, "type": "canceled", "reason": reason.trim()}),
            )
        }
    };
    let (json, hash) = normalize_envelope("terminal", value, MAX_TERMINAL_BYTES)?;
    Ok((status, json, hash))
}

fn normalize_envelope(
    label: &str,
    value: Value,
    max_bytes: usize,
) -> anyhow::Result<(String, String)> {
    validate_agent_visible_json(&format!("guide {label} envelope"), &value)?;
    let json = serde_json::to_string(&value)?;
    if json.len() > max_bytes {
        return validation(format!(
            "workspace guide {label} envelope exceeds the {max_bytes} byte limit"
        ));
    }
    let hash = format!("{:x}", Sha256::digest(json.as_bytes()));
    Ok((json, hash))
}

async fn run_by_id(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<Option<WorkspaceGuideRunRow>> {
    Ok(sqlx::query_as(guide_run_query!(" WHERE run.id = ?"))
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?)
}

async fn run_by_key(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    session_id: &str,
    key: &str,
) -> anyhow::Result<Option<WorkspaceGuideRunRow>> {
    Ok(sqlx::query_as(guide_run_query!(
        " WHERE run.session_id = ? AND run.idempotency_key = ?"
    ))
    .bind(session_id.trim())
    .bind(key.trim())
    .fetch_optional(&mut **tx)
    .await?)
}

fn required<'a>(label: &str, value: &'a str) -> anyhow::Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        return validation(format!("workspace guide {label} must not be empty"));
    }
    Ok(value)
}

fn normalized_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn validation<T>(message: impl Into<String>) -> anyhow::Result<T> {
    Err(anyhow::anyhow!(message.into()))
}

fn conflict<T>(message: impl Into<String>) -> anyhow::Result<T> {
    Err(anyhow::anyhow!(message.into()))
}

#[cfg(test)]
#[path = "workspace_guides_tests.rs"]
mod tests;
