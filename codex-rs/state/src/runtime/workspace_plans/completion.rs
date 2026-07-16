use std::collections::BTreeSet;

use chrono::Utc;
use sha2::Digest;
use sha2::Sha256;
use sqlx::Sqlite;
use uuid::Uuid;

use super::MAX_MESSAGE_BYTES;
use super::MAX_PLAN_BYTES;
use super::PlanResult;
use super::idempotency;
use super::not_found;
use super::required;
use super::stale;
use super::terminal_conflict;
use super::validate_bound_thread;
use super::validation;
use crate::model::WorkspacePlanTurnCompletionRow;
use crate::model::WorkspacePlanningContextSource;
use crate::model::datetime_to_epoch_millis;
use crate::runtime::workspace::WorkspaceStore;
use crate::runtime::workspace::insert_audit_event;
use crate::runtime::workspace_policy::require_synthetic_workspace;

const MAX_EVIDENCE_READS: usize = 64;
const MAX_IDEMPOTENCY_KEY_BYTES: usize = 512;

macro_rules! completion_query {
    ($suffix:literal) => {
        concat!(
            "SELECT guide_run_id, plan_session_id, client_id, idempotency_key, ",
            "assistant_message_id, plan_revision_id, completion_input_sha256, ",
            "evidence_manifest_json, evidence_manifest_sha256, evidence_read_count, ",
            "terminal_envelope_json, terminal_envelope_sha256, source_checkpoint_id, ",
            "source_checkpoint_revision, source_checkpoint_sha256, source_thread_id, ",
            "source_turn_id, provider, model, prompt_sha256, execution_token_sha256, ",
            "completed_at_ms FROM workspace_plan_turn_completions ",
            $suffix
        )
    };
}

#[derive(sqlx::FromRow)]
struct EvidenceReadRow {
    id: String,
    guide_run_id: String,
    plan_session_id: String,
    client_id: String,
    category: String,
    response_json: String,
    response_sha256: String,
    source_checkpoint_id: String,
    source_checkpoint_revision: i64,
    source_checkpoint_sha256: String,
    source_thread_id: String,
    source_turn_id: String,
    prompt_sha256: String,
    execution_token_sha256: String,
}

struct NormalizedPlan {
    plan_markdown: String,
    decisions_json: String,
    open_questions_json: String,
    content_sha256: String,
}

struct CompletionInputHash<'a> {
    execution: &'a crate::WorkspacePlanningGuideExecutionBinding,
    role: crate::WorkspacePlanMessageRole,
    message: &'a str,
    plan: Option<&'a NormalizedPlan>,
    evidence_read_ids: &'a [String],
    key: &'a str,
    actor: &'a str,
    token_sha256: &'a str,
}

impl WorkspaceStore {
    /// Lists only planning turns whose exact assistant completion committed atomically for this
    /// patient session and dedicated thread. Rollout markers alone are never authorization to
    /// reintroduce clinical history into a later planning prompt.
    pub async fn list_completed_plan_turn_ids(
        &self,
        plan_session_id: &str,
        client_id: &str,
        source_thread_id: &str,
    ) -> PlanResult<Vec<String>> {
        let plan_session_id = required("completed turn plan session id", plan_session_id)?;
        let client_id = required("completed turn client id", client_id)?;
        let source_thread_id = required("completed turn source thread id", source_thread_id)?;
        let turn_ids = sqlx::query_scalar::<_, String>(
            r#"
SELECT source_turn_id
FROM workspace_plan_turn_completions
WHERE plan_session_id = ? AND client_id = ? AND source_thread_id = ?
ORDER BY completed_at_ms ASC, source_turn_id ASC
            "#,
        )
        .bind(plan_session_id)
        .bind(client_id)
        .bind(source_thread_id)
        .fetch_all(self.pool.as_ref())
        .await?;
        Ok(turn_ids)
    }

    pub async fn get_plan_turn_completion(
        &self,
        guide_run_id: &str,
        plan_session_id: &str,
        client_id: &str,
    ) -> PlanResult<Option<crate::WorkspacePlanTurnCompletion>> {
        let guide_run_id = required("turn completion guide run id", guide_run_id)?;
        let plan_session_id = required("turn completion plan session id", plan_session_id)?;
        let client_id = required("turn completion client id", client_id)?;
        let mut tx = self.pool.begin().await?;
        let Some(row) = completion_by_run(&mut tx, guide_run_id).await? else {
            tx.rollback().await?;
            return Ok(None);
        };
        if row.plan_session_id != plan_session_id || row.client_id != client_id {
            return Err(validation(
                "workspace plan completion lookup identity does not match the persisted completion",
            ));
        }
        let result = completion_result(&mut tx, row, false).await?;
        tx.rollback().await?;
        Ok(Some(result))
    }

    /// Atomically completes one restricted planning turn.
    ///
    /// The transaction terminalizes the guide and appends its assistant message together. When a
    /// plan artifact is present it also creates the next current revision and binds that revision
    /// to an ordered manifest of immutable, capability-authorized context reads.
    pub async fn complete_plan_turn(
        &self,
        input: crate::WorkspacePlanTurnComplete,
    ) -> PlanResult<crate::WorkspacePlanTurnCompletion> {
        let execution = super::context::normalized_execution(input.execution)?;
        let message = required(
            "turn completion assistant message",
            &input.assistant_message,
        )?;
        let key = required("turn completion idempotency key", &input.idempotency_key)?;
        let actor = required("turn completion actor", &input.actor)?;
        if message.len() > MAX_MESSAGE_BYTES {
            return Err(validation(format!(
                "workspace plan message exceeds the {MAX_MESSAGE_BYTES} byte limit"
            )));
        }
        if key.len() > MAX_IDEMPOTENCY_KEY_BYTES {
            return Err(validation(format!(
                "workspace plan completion idempotency key exceeds the {MAX_IDEMPOTENCY_KEY_BYTES} byte limit"
            )));
        }
        if !matches!(
            input.assistant_message_role,
            crate::WorkspacePlanMessageRole::Assistant
                | crate::WorkspacePlanMessageRole::Question
                | crate::WorkspacePlanMessageRole::Error
        ) {
            return Err(validation(
                "workspace plan turn completion requires an assistant, question, or error message",
            ));
        }
        if input.plan.is_some()
            && input.assistant_message_role != crate::WorkspacePlanMessageRole::Assistant
        {
            return Err(validation(
                "a published workspace plan requires an assistant message",
            ));
        }
        let plan = input.plan.map(normalize_plan).transpose()?;
        let evidence_read_ids = normalize_evidence_ids(input.evidence_read_ids)?;
        let token_sha256 = sha256(execution.execution_token.as_bytes());
        let completion_input_sha256 = completion_input_sha256(CompletionInputHash {
            execution: &execution,
            role: input.assistant_message_role,
            message,
            plan: plan.as_ref(),
            evidence_read_ids: &evidence_read_ids,
            key,
            actor,
            token_sha256: &token_sha256,
        })?;

        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        require_synthetic_workspace(&mut tx)
            .await
            .map_err(super::context::plan_policy_error)?;

        if let Some(existing) = completion_by_run(&mut tx, &execution.guide_run_id).await? {
            ensure_exact_replay(
                &existing,
                &execution,
                key,
                &completion_input_sha256,
                &token_sha256,
            )?;
            let result = completion_result(&mut tx, existing, true).await?;
            tx.rollback().await?;
            return Ok(result);
        }
        if let Some(existing) = completion_by_key(&mut tx, &execution.plan_session_id, key).await? {
            return Err(idempotency(format!(
                "workspace plan completion key `{key}` already belongs to guide run `{}`",
                existing.guide_run_id
            )));
        }

        let claim = super::context::planning_claim(&mut tx, &execution.guide_run_id)
            .await?
            .ok_or_else(|| {
                validation(format!(
                    "workspace planning guide run `{}` has no durable turn claim",
                    execution.guide_run_id
                ))
            })?;
        super::context::validate_execution_claim(&execution, &claim, &token_sha256)?;
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
            return Err(terminal_conflict(format!(
                "workspace planning guide run `{}` is not the active claimed turn",
                execution.guide_run_id
            )));
        }
        if run.is_stale != 0 {
            return Err(stale(
                "workspace planning source checkpoint changed before turn completion",
            ));
        }
        if run.source_checkpoint_id != execution.source_checkpoint_id
            || run.source_checkpoint_revision != execution.source_checkpoint_revision
            || run.source_checkpoint_sha256 != execution.source_checkpoint_sha256
            || run.provider != execution.provider
            || run.model != execution.model
        {
            return Err(validation(
                "workspace planning completion does not match the guide identity",
            ));
        }

        let evidence_manifest =
            load_evidence_manifest(&mut tx, &execution, &token_sha256, &evidence_read_ids).await?;
        if plan.is_some() {
            validate_publish_evidence(&evidence_manifest)?;
        }
        let evidence_manifest_json = serde_json::to_string(&evidence_manifest)?;
        let evidence_manifest_sha256 = sha256(evidence_manifest_json.as_bytes());
        let evidence_read_count = i64::try_from(evidence_manifest.len())
            .map_err(|_| validation("workspace plan evidence count exceeds SQLite range"))?;

        let now_ms = datetime_to_epoch_millis(Utc::now());
        let message_id = Uuid::new_v4().to_string();
        let message_sha256 = sha256(message.as_bytes());
        let sequence = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM workspace_plan_messages WHERE plan_session_id = ?",
        )
        .bind(&execution.plan_session_id)
        .fetch_one(&mut *tx)
        .await?;
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
        .bind(&message_id)
        .bind(&execution.plan_session_id)
        .bind(&execution.client_id)
        .bind(&execution.guide_run_id)
        .bind(sequence)
        .bind(input.assistant_message_role.as_str())
        .bind(message)
        .bind(&message_sha256)
        .bind(key)
        .bind(&execution.source_checkpoint_id)
        .bind(execution.source_checkpoint_revision)
        .bind(&execution.source_checkpoint_sha256)
        .bind(&run.encounter_id)
        .bind(&run.note_id)
        .bind(&execution.source_thread_id)
        .bind(&execution.source_turn_id)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;

        for item in &evidence_manifest {
            sqlx::query(
                r#"
INSERT INTO workspace_plan_turn_evidence (
    guide_run_id, ordinal, context_read_id, category, response_sha256,
    source_content_sha256_json
) VALUES (?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&execution.guide_run_id)
            .bind(i64::from(item.ordinal))
            .bind(&item.context_read_id)
            .bind(&item.category)
            .bind(&item.response_sha256)
            .bind(serde_json::to_string(&item.source_content_sha256)?)
            .execute(&mut *tx)
            .await?;
        }

        let revision_id = if let Some(plan) = plan.as_ref() {
            sqlx::query(
                "UPDATE workspace_plan_revisions SET status = 'outdated' WHERE plan_session_id = ? AND status = 'current'",
            )
            .bind(&execution.plan_session_id)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "UPDATE workspace_plan_proposals SET status = 'outdated' WHERE plan_session_id = ? AND status = 'pending'",
            )
            .bind(&execution.plan_session_id)
            .execute(&mut *tx)
            .await?;
            let revision = session.latest_revision + 1;
            let revision_id = Uuid::new_v4().to_string();
            sqlx::query(
                r#"
INSERT INTO workspace_plan_revisions (
    id, plan_session_id, client_id, guide_run_id, revision, plan_markdown,
    decisions_json, open_questions_json, content_sha256, evidence_manifest_json,
    evidence_manifest_sha256, evidence_read_count, idempotency_key, status,
    source_checkpoint_id, source_checkpoint_revision, source_checkpoint_sha256,
    encounter_id, note_id, source_thread_id, source_turn_id, created_at_ms,
    submitted_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'current', ?, ?, ?, ?, ?, ?, ?, ?, NULL)
                "#,
            )
            .bind(&revision_id)
            .bind(&execution.plan_session_id)
            .bind(&execution.client_id)
            .bind(&execution.guide_run_id)
            .bind(revision)
            .bind(&plan.plan_markdown)
            .bind(&plan.decisions_json)
            .bind(&plan.open_questions_json)
            .bind(&plan.content_sha256)
            .bind(&evidence_manifest_json)
            .bind(&evidence_manifest_sha256)
            .bind(evidence_read_count)
            .bind(key)
            .bind(&execution.source_checkpoint_id)
            .bind(execution.source_checkpoint_revision)
            .bind(&execution.source_checkpoint_sha256)
            .bind(&run.encounter_id)
            .bind(&run.note_id)
            .bind(&execution.source_thread_id)
            .bind(&execution.source_turn_id)
            .bind(now_ms)
            .execute(&mut *tx)
            .await?;
            let updated = sqlx::query(
                "UPDATE workspace_plan_sessions SET latest_revision = ?, updated_at_ms = ? WHERE id = ? AND status = 'active' AND latest_revision = ?",
            )
            .bind(revision)
            .bind(now_ms)
            .bind(&execution.plan_session_id)
            .bind(session.latest_revision)
            .execute(&mut *tx)
            .await?;
            if updated.rows_affected() != 1 {
                return Err(stale(format!(
                    "workspace plan session `{}` revision changed concurrently",
                    execution.plan_session_id
                )));
            }
            Some(revision_id)
        } else {
            sqlx::query(
                "UPDATE workspace_plan_sessions SET updated_at_ms = ? WHERE id = ? AND status = 'active'",
            )
            .bind(now_ms)
            .bind(&execution.plan_session_id)
            .execute(&mut *tx)
            .await?;
            None
        };

        let revision_receipt =
            if let (Some(revision_id), Some(plan)) = (revision_id.as_deref(), plan.as_ref()) {
                Some(serde_json::json!({
                    "id": revision_id,
                    "contentSha256": plan.content_sha256,
                }))
            } else {
                None
            };
        let terminal_envelope_json = serde_json::to_string(&serde_json::json!({
            "schemaVersion": 1,
            "type": "workspacePlanTurnCompleted",
            "assistantMessageId": message_id,
            "assistantMessageSha256": message_sha256,
            "planRevision": revision_receipt,
            "evidenceManifestSha256": evidence_manifest_sha256,
            "evidenceReadCount": evidence_read_count,
        }))?;
        let terminal_envelope_sha256 = sha256(terminal_envelope_json.as_bytes());
        let updated = sqlx::query(
            "UPDATE workspace_guide_runs SET status = 'completed', terminal_envelope_json = ?, terminal_envelope_sha256 = ?, updated_at_ms = ?, terminal_at_ms = ? WHERE id = ? AND status = 'running' AND source_thread_id = ? AND source_turn_id = ?",
        )
        .bind(&terminal_envelope_json)
        .bind(&terminal_envelope_sha256)
        .bind(now_ms)
        .bind(now_ms)
        .bind(&execution.guide_run_id)
        .bind(&execution.source_thread_id)
        .bind(&execution.source_turn_id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(terminal_conflict(format!(
                "workspace planning guide run `{}` terminal update raced",
                execution.guide_run_id
            )));
        }

        sqlx::query(
            r#"
INSERT INTO workspace_plan_turn_completions (
    guide_run_id, plan_session_id, client_id, idempotency_key,
    assistant_message_id, plan_revision_id, completion_input_sha256,
    evidence_manifest_json, evidence_manifest_sha256, evidence_read_count,
    terminal_envelope_json, terminal_envelope_sha256, source_checkpoint_id,
    source_checkpoint_revision, source_checkpoint_sha256, source_thread_id,
    source_turn_id, provider, model, prompt_sha256, execution_token_sha256,
    completed_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&execution.guide_run_id)
        .bind(&execution.plan_session_id)
        .bind(&execution.client_id)
        .bind(key)
        .bind(&message_id)
        .bind(&revision_id)
        .bind(&completion_input_sha256)
        .bind(&evidence_manifest_json)
        .bind(&evidence_manifest_sha256)
        .bind(evidence_read_count)
        .bind(&terminal_envelope_json)
        .bind(&terminal_envelope_sha256)
        .bind(&execution.source_checkpoint_id)
        .bind(execution.source_checkpoint_revision)
        .bind(&execution.source_checkpoint_sha256)
        .bind(&execution.source_thread_id)
        .bind(&execution.source_turn_id)
        .bind(&execution.provider)
        .bind(&execution.model)
        .bind(&execution.prompt_sha256)
        .bind(&token_sha256)
        .bind(now_ms)
        .execute(&mut *tx)
        .await?;

        insert_completion_audit(
            &mut tx,
            &execution,
            actor,
            &message_id,
            revision_id.as_deref(),
            &evidence_manifest_sha256,
            now_ms,
        )
        .await?;
        let row = completion_by_run(&mut tx, &execution.guide_run_id)
            .await?
            .ok_or_else(|| not_found("inserted workspace plan completion was not found"))?;
        let result = completion_result(&mut tx, row, false).await?;
        tx.commit().await?;
        Ok(result)
    }
}

fn normalize_plan(input: crate::WorkspacePlanArtifact) -> PlanResult<NormalizedPlan> {
    let plan_markdown = required("revision plan", &input.plan_markdown)?.to_string();
    let decisions = normalized_string_array("decisions", &input.decisions_json)?;
    let open_questions = normalized_string_array("open questions", &input.open_questions_json)?;
    if !open_questions.is_empty() {
        return Err(validation(
            "workspace plan revision cannot be published while open questions remain",
        ));
    }
    let content_json = serde_json::to_string(&serde_json::json!({
        "planMarkdown": &plan_markdown,
        "decisions": &decisions,
        "openQuestions": &open_questions,
    }))?;
    if content_json.len() > MAX_PLAN_BYTES {
        return Err(validation(format!(
            "workspace plan revision exceeds the {MAX_PLAN_BYTES} byte limit"
        )));
    }
    let decisions_json = serde_json::to_string(&decisions)?;
    let open_questions_json = serde_json::to_string(&open_questions)?;
    Ok(NormalizedPlan {
        plan_markdown,
        decisions_json,
        open_questions_json,
        content_sha256: sha256(content_json.as_bytes()),
    })
}

fn normalized_string_array(label: &str, raw: &str) -> PlanResult<Vec<String>> {
    serde_json::from_str::<Vec<String>>(raw.trim()).map_err(|_| {
        validation(format!(
            "workspace plan revision {label} must be a JSON array of strings"
        ))
    })
}

fn normalize_evidence_ids(ids: Vec<String>) -> PlanResult<Vec<String>> {
    if ids.len() > MAX_EVIDENCE_READS {
        return Err(validation(format!(
            "workspace plan completion accepts at most {MAX_EVIDENCE_READS} evidence reads"
        )));
    }
    let mut seen = BTreeSet::new();
    ids.into_iter()
        .map(|id| {
            let id = required("evidence context read id", &id)?.to_string();
            if !seen.insert(id.clone()) {
                return Err(validation(format!(
                    "workspace plan evidence read `{id}` was supplied more than once"
                )));
            }
            Ok(id)
        })
        .collect()
}

fn completion_input_sha256(input: CompletionInputHash<'_>) -> PlanResult<String> {
    let plan = input.plan.map(|plan| {
        serde_json::json!({
            "planMarkdown": plan.plan_markdown,
            "decisionsJson": plan.decisions_json,
            "openQuestionsJson": plan.open_questions_json,
            "contentSha256": plan.content_sha256,
        })
    });
    let json = serde_json::to_string(&serde_json::json!({
        "schemaVersion": 1,
        "guideRunId": input.execution.guide_run_id,
        "planSessionId": input.execution.plan_session_id,
        "clientId": input.execution.client_id,
        "sourceCheckpointId": input.execution.source_checkpoint_id,
        "sourceCheckpointRevision": input.execution.source_checkpoint_revision,
        "sourceCheckpointSha256": input.execution.source_checkpoint_sha256,
        "sourceThreadId": input.execution.source_thread_id,
        "sourceTurnId": input.execution.source_turn_id,
        "provider": input.execution.provider,
        "model": input.execution.model,
        "promptSha256": input.execution.prompt_sha256,
        "executionTokenSha256": input.token_sha256,
        "assistantMessageRole": input.role.as_str(),
        "assistantMessage": input.message,
        "plan": plan,
        "evidenceReadIds": input.evidence_read_ids,
        "idempotencyKey": input.key,
        "actor": input.actor,
    }))?;
    Ok(sha256(json.as_bytes()))
}

async fn load_evidence_manifest(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    execution: &crate::WorkspacePlanningGuideExecutionBinding,
    token_sha256: &str,
    read_ids: &[String],
) -> PlanResult<Vec<crate::WorkspacePlanEvidenceRead>> {
    let mut manifest = Vec::with_capacity(read_ids.len());
    for (ordinal, read_id) in read_ids.iter().enumerate() {
        let row = evidence_read_by_id(tx, read_id).await?.ok_or_else(|| {
            not_found(format!(
                "workspace planning context read `{read_id}` was not found"
            ))
        })?;
        if row.guide_run_id != execution.guide_run_id
            || row.plan_session_id != execution.plan_session_id
            || row.client_id != execution.client_id
            || row.source_checkpoint_id != execution.source_checkpoint_id
            || row.source_checkpoint_revision != execution.source_checkpoint_revision
            || row.source_checkpoint_sha256 != execution.source_checkpoint_sha256
            || row.source_thread_id != execution.source_thread_id
            || row.source_turn_id != execution.source_turn_id
            || row.prompt_sha256 != execution.prompt_sha256
            || row.execution_token_sha256 != token_sha256
        {
            return Err(validation(format!(
                "workspace planning context read `{read_id}` does not belong to the exact claimed turn"
            )));
        }
        if sha256(row.response_json.as_bytes()) != row.response_sha256 {
            return Err(validation(format!(
                "workspace planning context read `{read_id}` failed its content hash check"
            )));
        }
        let sources: Vec<WorkspacePlanningContextSource> =
            serde_json::from_str(&row.response_json)?;
        let source_content_sha256 = sources
            .into_iter()
            .map(|source| source.content_sha256)
            .collect();
        manifest.push(crate::WorkspacePlanEvidenceRead {
            ordinal: u32::try_from(ordinal)
                .map_err(|_| validation("workspace plan evidence ordinal exceeds u32 range"))?,
            context_read_id: row.id,
            category: row.category,
            response_sha256: row.response_sha256,
            source_content_sha256,
        });
    }
    Ok(manifest)
}

fn validate_publish_evidence(manifest: &[crate::WorkspacePlanEvidenceRead]) -> PlanResult<()> {
    if manifest.is_empty() {
        return Err(validation(
            "a published workspace plan requires at least one immutable context read",
        ));
    }
    let categories: BTreeSet<&str> = manifest.iter().map(|item| item.category.as_str()).collect();
    for required_category in ["patient_chart", "selected_context"] {
        if !categories.contains(required_category) {
            return Err(validation(format!(
                "a published workspace plan requires `{required_category}` evidence"
            )));
        }
    }
    Ok(())
}

async fn evidence_read_by_id(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    read_id: &str,
) -> PlanResult<Option<EvidenceReadRow>> {
    Ok(sqlx::query_as(
        r#"
SELECT id, guide_run_id, plan_session_id, client_id, category, response_json,
       response_sha256, source_checkpoint_id, source_checkpoint_revision,
       source_checkpoint_sha256, source_thread_id, source_turn_id, prompt_sha256,
       execution_token_sha256
FROM workspace_planning_context_reads
WHERE id = ?
        "#,
    )
    .bind(read_id)
    .fetch_optional(&mut **tx)
    .await?)
}

fn ensure_exact_replay(
    existing: &WorkspacePlanTurnCompletionRow,
    execution: &crate::WorkspacePlanningGuideExecutionBinding,
    key: &str,
    input_sha256: &str,
    token_sha256: &str,
) -> PlanResult<()> {
    if existing.plan_session_id != execution.plan_session_id
        || existing.client_id != execution.client_id
        || existing.idempotency_key != key
        || existing.completion_input_sha256 != input_sha256
        || existing.source_checkpoint_id != execution.source_checkpoint_id
        || existing.source_checkpoint_revision != execution.source_checkpoint_revision
        || existing.source_checkpoint_sha256 != execution.source_checkpoint_sha256
        || existing.source_thread_id != execution.source_thread_id
        || existing.source_turn_id != execution.source_turn_id
        || existing.provider != execution.provider
        || existing.model != execution.model
        || existing.prompt_sha256 != execution.prompt_sha256
        || existing.execution_token_sha256 != token_sha256
    {
        return Err(terminal_conflict(format!(
            "workspace planning guide run `{}` already completed with different terminal content",
            existing.guide_run_id
        )));
    }
    Ok(())
}

async fn completion_result(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    row: WorkspacePlanTurnCompletionRow,
    replayed: bool,
) -> PlanResult<crate::WorkspacePlanTurnCompletion> {
    let verified = super::completion_integrity::verify_completion(tx, &row, replayed).await?;
    let receipt = row.try_into_receipt(replayed)?;
    let evidence_manifest_sha256 = receipt.evidence_manifest_sha256.clone();
    Ok(crate::WorkspacePlanTurnCompletion {
        run: verified.run,
        assistant_message: verified.assistant_message,
        revision: verified.revision,
        evidence_manifest: verified.evidence_manifest,
        evidence_manifest_sha256,
        receipt,
        replayed,
    })
}

pub(super) async fn completion_by_run(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    guide_run_id: &str,
) -> PlanResult<Option<WorkspacePlanTurnCompletionRow>> {
    Ok(
        sqlx::query_as::<_, WorkspacePlanTurnCompletionRow>(completion_query!(
            "WHERE guide_run_id = ?"
        ))
        .bind(guide_run_id)
        .fetch_optional(&mut **tx)
        .await?,
    )
}

pub(super) async fn last_completion_for_session(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    plan_session_id: &str,
    client_id: &str,
) -> PlanResult<Option<WorkspacePlanTurnCompletionRow>> {
    Ok(
        sqlx::query_as::<_, WorkspacePlanTurnCompletionRow>(completion_query!(
            "WHERE plan_session_id = ? AND client_id = ? ORDER BY completed_at_ms DESC, guide_run_id DESC LIMIT 1"
        ))
        .bind(plan_session_id)
        .bind(client_id)
        .fetch_optional(&mut **tx)
        .await?,
    )
}

async fn completion_by_key(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    plan_session_id: &str,
    key: &str,
) -> PlanResult<Option<WorkspacePlanTurnCompletionRow>> {
    Ok(
        sqlx::query_as::<_, WorkspacePlanTurnCompletionRow>(completion_query!(
            "WHERE plan_session_id = ? AND idempotency_key = ?"
        ))
        .bind(plan_session_id)
        .bind(key)
        .fetch_optional(&mut **tx)
        .await?,
    )
}

async fn insert_completion_audit(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    execution: &crate::WorkspacePlanningGuideExecutionBinding,
    actor: &str,
    message_id: &str,
    revision_id: Option<&str>,
    evidence_manifest_sha256: &str,
    now_ms: i64,
) -> PlanResult<()> {
    insert_audit_event(
        tx,
        crate::WorkspaceAuditEventCreate {
            entity_type: "plan_turn_completion".to_string(),
            entity_id: execution.guide_run_id.clone(),
            action: "completed".to_string(),
            actor: actor.to_string(),
            actor_kind: "agent".to_string(),
            source: "workspace_plan".to_string(),
            client_id: Some(execution.client_id.clone()),
            source_thread_id: Some(execution.source_thread_id.clone()),
            source_turn_id: Some(execution.source_turn_id.clone()),
            success: true,
            summary: if revision_id.is_some() {
                "assistant turn completed and evidence-bound plan revision published".to_string()
            } else {
                "assistant conversational turn completed".to_string()
            },
            metadata_json: Some(
                serde_json::json!({
                    "assistantMessageId": message_id,
                    "evidenceManifestSha256": evidence_manifest_sha256,
                    "guideRunId": execution.guide_run_id,
                    "planRevisionId": revision_id,
                    "planSessionId": execution.plan_session_id,
                    "sourceCheckpointId": execution.source_checkpoint_id,
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

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
