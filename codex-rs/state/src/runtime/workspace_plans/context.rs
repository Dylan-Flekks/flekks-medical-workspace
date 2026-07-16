use super::PlanResult;
use super::idempotency;
use super::required;
use super::stale;
use super::validate_bound_thread;
use super::validation;
use crate::model::WorkspacePlanningContextReadRow;
use crate::model::WorkspacePlanningTurnClaimRow;
use crate::model::datetime_to_epoch_millis;
use crate::runtime::workspace::WorkspaceStore;
use crate::runtime::workspace::insert_audit_event;
use crate::runtime::workspace_policy::WorkspacePolicyRequirementError;
use crate::runtime::workspace_policy::require_synthetic_workspace;
use chrono::Utc;
use sha2::Digest;
use sha2::Sha256;
use uuid::Uuid;

const MAX_PROMPT_BYTES: usize = 128 * 1024;

macro_rules! context_read_query {
    ($suffix:literal) => {
        concat!(
            "SELECT id, guide_run_id, plan_session_id, client_id, category, max_records, ",
            "response_json, response_sha256, accessed_at_ms ",
            "FROM workspace_planning_context_reads ",
            $suffix
        )
    };
}

impl WorkspaceStore {
    pub async fn claim_planning_guide_turn(
        &self,
        input: crate::WorkspacePlanningGuideTurnClaimRequest,
    ) -> PlanResult<crate::WorkspacePlanningGuideExecutionBinding> {
        let guide_run_id = required("turn claim guide run id", &input.guide_run_id)?;
        let session_id = required("turn claim plan session id", &input.plan_session_id)?;
        let client_id = required("turn claim client id", &input.client_id)?;
        let checkpoint_id = required(
            "turn claim source checkpoint id",
            &input.source_checkpoint_id,
        )?;
        let checkpoint_sha256 = required(
            "turn claim source checkpoint SHA-256",
            &input.source_checkpoint_sha256,
        )?;
        let thread_id = required("turn claim source thread id", &input.source_thread_id)?;
        let turn_id = required("turn claim source turn id", &input.source_turn_id)?;
        let provider = required("turn claim provider", &input.provider)?;
        let model = required("turn claim model", &input.model)?;
        let prompt = required("turn claim prompt", &input.prompt)?;
        if prompt.len() > MAX_PROMPT_BYTES {
            return Err(validation(format!(
                "workspace plan prompt exceeds the {MAX_PROMPT_BYTES} byte limit"
            )));
        }
        let prompt_sha256 = format!("{:x}", Sha256::digest(prompt.as_bytes()));
        let execution_token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let execution_token_sha256 = format!("{:x}", Sha256::digest(execution_token.as_bytes()));
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        require_synthetic_workspace(&mut tx)
            .await
            .map_err(plan_policy_error)?;
        let session = self
            .require_active_plan_session(&mut tx, session_id, client_id)
            .await?;
        validate_bound_thread(&session, thread_id)?;
        let run = self
            .plan_run_binding(&mut tx, guide_run_id, client_id)
            .await?;
        if run.status != "running" {
            return Err(validation(format!(
                "workspace planning guide run `{guide_run_id}` is `{}` and cannot claim a turn",
                run.status
            )));
        }
        if run.source_checkpoint_id != checkpoint_id
            || run.source_checkpoint_revision != input.source_checkpoint_revision
            || run.source_checkpoint_sha256 != checkpoint_sha256
        {
            return Err(validation(
                "workspace planning turn claim does not match the guide checkpoint identity",
            ));
        }
        if run.is_stale != 0 {
            return Err(stale(
                "workspace planning source checkpoint changed before turn claim",
            ));
        }
        if run.provider != provider || run.model != model {
            return Err(validation(
                "workspace planning turn provider or model does not match the guide run",
            ));
        }
        if run.source_thread_id.is_some() || run.source_turn_id.is_some() {
            return Err(validation(format!(
                "workspace planning guide run `{guide_run_id}` was already claimed"
            )));
        }
        let claim_exists = sqlx::query_scalar::<_, i64>(
            "SELECT 1 FROM workspace_planning_turn_claims WHERE guide_run_id = ?",
        )
        .bind(guide_run_id)
        .fetch_optional(&mut *tx)
        .await?
        .is_some();
        if claim_exists {
            return Err(validation(format!(
                "workspace planning guide run `{guide_run_id}` already has a durable turn claim"
            )));
        }
        let now_ms = datetime_to_epoch_millis(Utc::now());
        sqlx::query(
            r#"
INSERT INTO workspace_planning_turn_claims (
    guide_run_id, plan_session_id, client_id, source_checkpoint_id,
    source_checkpoint_revision, source_checkpoint_sha256, source_thread_id,
    source_turn_id, provider, model, prompt_sha256, execution_token_sha256,
    claimed_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(guide_run_id)
        .bind(session_id)
        .bind(client_id)
        .bind(checkpoint_id)
        .bind(input.source_checkpoint_revision)
        .bind(checkpoint_sha256)
        .bind(thread_id)
        .bind(turn_id)
        .bind(provider)
        .bind(model)
        .bind(&prompt_sha256)
        .bind(&execution_token_sha256)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;
        let claimed = sqlx::query(
            "UPDATE workspace_guide_runs SET source_thread_id = ?, source_turn_id = ?, updated_at_ms = ? WHERE id = ? AND status = 'running' AND source_thread_id IS NULL AND source_turn_id IS NULL",
        )
        .bind(thread_id)
        .bind(turn_id)
        .bind(now_ms)
        .bind(guide_run_id)
        .execute(&mut *tx)
        .await?;
        if claimed.rows_affected() != 1 {
            return Err(validation(format!(
                "workspace planning guide run `{guide_run_id}` claim raced"
            )));
        }
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "guide_run".to_string(),
                entity_id: guide_run_id.to_string(),
                action: "planning_turn_claimed".to_string(),
                actor: "workspace planner".to_string(),
                actor_kind: "agent".to_string(),
                source: "workspace_plan".to_string(),
                client_id: Some(client_id.to_string()),
                encounter_id: run.encounter_id,
                note_id: run.note_id,
                source_thread_id: Some(thread_id.to_string()),
                source_turn_id: Some(turn_id.to_string()),
                success: true,
                summary: "restricted patient planning turn claimed".to_string(),
                metadata_json: Some(
                    serde_json::json!({
                        "planSessionId": session_id,
                        "promptSha256": prompt_sha256,
                        "sourceCheckpointId": checkpoint_id,
                    })
                    .to_string(),
                ),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        tx.commit().await?;
        Ok(crate::WorkspacePlanningGuideExecutionBinding {
            guide_run_id: guide_run_id.to_string(),
            plan_session_id: session_id.to_string(),
            client_id: client_id.to_string(),
            source_checkpoint_id: checkpoint_id.to_string(),
            source_checkpoint_revision: input.source_checkpoint_revision,
            source_checkpoint_sha256: checkpoint_sha256.to_string(),
            source_thread_id: thread_id.to_string(),
            source_turn_id: turn_id.to_string(),
            provider: provider.to_string(),
            model: model.to_string(),
            prompt_sha256,
            execution_token,
        })
    }

    pub async fn read_authorized_planning_context(
        &self,
        input: crate::WorkspacePlanningContextReadRequest,
    ) -> PlanResult<crate::WorkspacePlanningContextRead> {
        let execution = normalized_execution(input.execution)?;
        let category = required("context category", &input.category)?;
        if !matches!(
            category,
            "visit_history" | "progress_notes" | "patient_chart" | "selected_context"
        ) {
            return Err(validation(format!(
                "unsupported workspace planning context category `{category}`"
            )));
        }
        let key = required("context read idempotency key", &input.idempotency_key)?;
        let max_records = input.max_records.unwrap_or(20).clamp(1, 50);
        let token_sha256 = format!("{:x}", Sha256::digest(execution.execution_token.as_bytes()));
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        require_synthetic_workspace(&mut tx)
            .await
            .map_err(plan_policy_error)?;
        let claim = planning_claim(&mut tx, &execution.guide_run_id)
            .await?
            .ok_or_else(|| {
                validation(format!(
                    "workspace planning guide run `{}` has no durable turn claim",
                    execution.guide_run_id
                ))
            })?;
        validate_execution_claim(&execution, &claim, &token_sha256)?;
        let session = self
            .require_active_plan_session(&mut tx, &execution.plan_session_id, &execution.client_id)
            .await?;
        validate_bound_thread(&session, &execution.source_thread_id)?;
        let run = self
            .plan_run_binding(&mut tx, &execution.guide_run_id, &execution.client_id)
            .await?;
        if run.status != "running"
            || run.source_thread_id.as_deref() != Some(execution.source_thread_id.as_str())
            || run.source_turn_id.as_deref() != Some(execution.source_turn_id.as_str())
        {
            return Err(validation(
                "workspace planning context read does not match the active claimed turn",
            ));
        }
        if run.is_stale != 0 {
            return Err(stale(
                "workspace planning source checkpoint changed before context read",
            ));
        }
        if let Some(existing) = context_read_by_key(&mut tx, &execution.guide_run_id, key).await? {
            if existing.category != category || existing.max_records != i64::from(max_records) {
                return Err(idempotency(format!(
                    "workspace planning context read key `{key}` was reused with different content"
                )));
            }
            tx.rollback().await?;
            return Ok(existing.try_into_model(true)?);
        }

        let sources =
            super::context_sources::read_category(&mut tx, &execution, category, max_records)
                .await?;
        let response_json = serde_json::to_string(&sources)?;
        let response_sha256 = format!("{:x}", Sha256::digest(response_json.as_bytes()));
        let id = Uuid::new_v4().to_string();
        let now_ms = datetime_to_epoch_millis(Utc::now());
        sqlx::query(
            r#"
INSERT INTO workspace_planning_context_reads (
    id, guide_run_id, plan_session_id, client_id, idempotency_key, category,
    max_records, result_count, response_json, response_sha256,
    source_checkpoint_id, source_checkpoint_revision, source_checkpoint_sha256,
    source_thread_id, source_turn_id, prompt_sha256, execution_token_sha256,
    accessed_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&execution.guide_run_id)
        .bind(&execution.plan_session_id)
        .bind(&execution.client_id)
        .bind(key)
        .bind(category)
        .bind(i64::from(max_records))
        .bind(i64::try_from(sources.len()).map_err(|_| {
            validation("workspace planning context result count exceeds SQLite range")
        })?)
        .bind(&response_json)
        .bind(&response_sha256)
        .bind(&execution.source_checkpoint_id)
        .bind(execution.source_checkpoint_revision)
        .bind(&execution.source_checkpoint_sha256)
        .bind(&execution.source_thread_id)
        .bind(&execution.source_turn_id)
        .bind(&execution.prompt_sha256)
        .bind(&token_sha256)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;
        insert_audit_event(
            &mut tx,
            crate::WorkspaceAuditEventCreate {
                entity_type: "planning_context_read".to_string(),
                entity_id: id.clone(),
                action: "read".to_string(),
                actor: "workspace planner".to_string(),
                actor_kind: "agent".to_string(),
                source: "workspace_plan".to_string(),
                client_id: Some(execution.client_id.clone()),
                source_thread_id: Some(execution.source_thread_id.clone()),
                source_turn_id: Some(execution.source_turn_id.clone()),
                success: true,
                summary: format!(
                    "authorized {category} read returned {} records",
                    sources.len()
                ),
                metadata_json: Some(
                    serde_json::json!({
                        "guideRunId": execution.guide_run_id,
                        "planSessionId": execution.plan_session_id,
                        "responseSha256": response_sha256,
                        "sourceCheckpointId": execution.source_checkpoint_id,
                    })
                    .to_string(),
                ),
                ..Default::default()
            },
            now_ms,
        )
        .await?;
        let row = context_read_by_id(&mut tx, &id)
            .await?
            .ok_or_else(|| validation("inserted workspace planning context read was not found"))?;
        tx.commit().await?;
        Ok(row.try_into_model(false)?)
    }
}

pub(super) fn normalized_execution(
    mut execution: crate::WorkspacePlanningGuideExecutionBinding,
) -> PlanResult<crate::WorkspacePlanningGuideExecutionBinding> {
    execution.guide_run_id =
        required("execution guide run id", &execution.guide_run_id)?.to_string();
    execution.plan_session_id =
        required("execution plan session id", &execution.plan_session_id)?.to_string();
    execution.client_id = required("execution client id", &execution.client_id)?.to_string();
    execution.source_checkpoint_id = required(
        "execution source checkpoint id",
        &execution.source_checkpoint_id,
    )?
    .to_string();
    execution.source_checkpoint_sha256 = required(
        "execution source checkpoint SHA-256",
        &execution.source_checkpoint_sha256,
    )?
    .to_string();
    execution.source_thread_id =
        required("execution source thread id", &execution.source_thread_id)?.to_string();
    execution.source_turn_id =
        required("execution source turn id", &execution.source_turn_id)?.to_string();
    execution.provider = required("execution provider", &execution.provider)?.to_string();
    execution.model = required("execution model", &execution.model)?.to_string();
    execution.prompt_sha256 =
        required("execution prompt SHA-256", &execution.prompt_sha256)?.to_string();
    execution.execution_token =
        required("execution token", &execution.execution_token)?.to_string();
    Ok(execution)
}

pub(super) fn validate_execution_claim(
    execution: &crate::WorkspacePlanningGuideExecutionBinding,
    claim: &WorkspacePlanningTurnClaimRow,
    token_sha256: &str,
) -> PlanResult<()> {
    if claim.guide_run_id != execution.guide_run_id
        || claim.plan_session_id != execution.plan_session_id
        || claim.client_id != execution.client_id
        || claim.source_checkpoint_id != execution.source_checkpoint_id
        || claim.source_checkpoint_revision != execution.source_checkpoint_revision
        || claim.source_checkpoint_sha256 != execution.source_checkpoint_sha256
        || claim.source_thread_id != execution.source_thread_id
        || claim.source_turn_id != execution.source_turn_id
        || claim.provider != execution.provider
        || claim.model != execution.model
        || claim.prompt_sha256 != execution.prompt_sha256
        || claim.execution_token_sha256 != token_sha256
    {
        return Err(validation(
            "workspace planning context read does not match the claimed execution identity",
        ));
    }
    Ok(())
}

pub(super) async fn planning_claim(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    guide_run_id: &str,
) -> PlanResult<Option<WorkspacePlanningTurnClaimRow>> {
    Ok(sqlx::query_as(
        r#"
SELECT guide_run_id, plan_session_id, client_id, source_checkpoint_id,
       source_checkpoint_revision, source_checkpoint_sha256, source_thread_id,
       source_turn_id, provider, model, prompt_sha256, execution_token_sha256
FROM workspace_planning_turn_claims
WHERE guide_run_id = ?
        "#,
    )
    .bind(guide_run_id)
    .fetch_optional(&mut **tx)
    .await?)
}

async fn context_read_by_key(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    guide_run_id: &str,
    key: &str,
) -> PlanResult<Option<WorkspacePlanningContextReadRow>> {
    Ok(
        sqlx::query_as::<_, WorkspacePlanningContextReadRow>(context_read_query!(
            "WHERE guide_run_id = ? AND idempotency_key = ?"
        ))
        .bind(guide_run_id)
        .bind(key)
        .fetch_optional(&mut **tx)
        .await?,
    )
}

async fn context_read_by_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: &str,
) -> PlanResult<Option<WorkspacePlanningContextReadRow>> {
    Ok(
        sqlx::query_as::<_, WorkspacePlanningContextReadRow>(context_read_query!("WHERE id = ?"))
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?,
    )
}

pub(super) fn plan_policy_error(
    error: WorkspacePolicyRequirementError,
) -> crate::WorkspacePlanError {
    match error {
        WorkspacePolicyRequirementError::NotSynthetic => validation(error.to_string()),
        WorkspacePolicyRequirementError::Integrity(error) => error.into(),
    }
}
