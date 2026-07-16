use sha2::Digest;
use sha2::Sha256;
use sqlx::Sqlite;

use super::PlanResult;
use super::not_found;
use super::validation;
use crate::model::WorkspacePlanTurnCompletionRow;
use crate::model::WorkspacePlanningContextSource;

pub(super) struct VerifiedPlanCompletion {
    pub run: crate::WorkspaceGuideRun,
    pub assistant_message: crate::WorkspacePlanMessage,
    pub revision: Option<crate::WorkspacePlanRevision>,
    pub evidence_manifest: Vec<crate::WorkspacePlanEvidenceRead>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CompletionTerminalEnvelope {
    schema_version: u32,
    #[serde(rename = "type")]
    event_type: String,
    assistant_message_id: String,
    assistant_message_sha256: String,
    plan_revision: Option<CompletionTerminalRevision>,
    evidence_manifest_sha256: String,
    evidence_read_count: u32,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CompletionTerminalRevision {
    id: String,
    content_sha256: String,
}

#[derive(sqlx::FromRow)]
struct EvidenceManifestRow {
    ordinal: i64,
    context_read_id: String,
    category: String,
    response_sha256: String,
    source_content_sha256_json: String,
}

#[derive(sqlx::FromRow)]
struct EvidenceReadRow {
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

pub(super) async fn verify_completion(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    row: &WorkspacePlanTurnCompletionRow,
    replayed: bool,
) -> PlanResult<VerifiedPlanCompletion> {
    let evidence_manifest = row.evidence_manifest()?;
    let persisted_evidence = persisted_evidence_manifest(tx, row).await?;
    if evidence_manifest.len()
        != usize::try_from(row.evidence_read_count)
            .map_err(|_| validation("workspace plan evidence count is invalid"))?
        || sha256(row.evidence_manifest_json.as_bytes()) != row.evidence_manifest_sha256
        || sha256(row.terminal_envelope_json.as_bytes()) != row.terminal_envelope_sha256
        || persisted_evidence != evidence_manifest
    {
        return Err(validation(
            "workspace plan completion failed its persisted integrity checks",
        ));
    }
    let terminal: CompletionTerminalEnvelope = serde_json::from_str(&row.terminal_envelope_json)
        .map_err(|_| validation("workspace plan terminal envelope is invalid"))?;
    if terminal.schema_version != 1
        || terminal.event_type != "workspacePlanTurnCompleted"
        || terminal.assistant_message_id != row.assistant_message_id
        || terminal.evidence_manifest_sha256 != row.evidence_manifest_sha256
        || terminal.evidence_read_count
            != u32::try_from(row.evidence_read_count)
                .map_err(|_| validation("workspace plan evidence count is invalid"))?
    {
        return Err(validation(
            "workspace plan terminal envelope does not match its completion receipt",
        ));
    }

    let run = crate::runtime::workspace_guides::run_by_id(tx, &row.guide_run_id)
        .await?
        .ok_or_else(|| not_found("completed workspace planning guide run was not found"))?
        .try_into_model(replayed)?;
    if run.client_id != row.client_id
        || run.source_checkpoint_id != row.source_checkpoint_id
        || run.source_checkpoint_revision != row.source_checkpoint_revision
        || run.source_checkpoint_sha256 != row.source_checkpoint_sha256
        || run.provider != row.provider
        || run.model != row.model
        || run.model_tool_mode != crate::WorkspaceGuideModelToolMode::WorkspacePlanningOnly.as_str()
        || run.status != crate::WorkspaceGuideRunStatus::Completed
        || run.source_thread_id.as_deref() != Some(row.source_thread_id.as_str())
        || run.source_turn_id.as_deref() != Some(row.source_turn_id.as_str())
        || run.terminal_envelope_json.as_deref() != Some(row.terminal_envelope_json.as_str())
        || run.terminal_envelope_sha256.as_deref() != Some(row.terminal_envelope_sha256.as_str())
        || sha256(run.request_envelope_json.as_bytes()) != run.request_envelope_sha256
    {
        return Err(validation(
            "workspace plan completion does not match its terminal guide run",
        ));
    }

    let assistant_message_row = super::messages::message_by_id(tx, &row.assistant_message_id)
        .await?
        .ok_or_else(|| not_found("completed workspace plan message was not found"))?;
    if assistant_message_row.plan_session_id != row.plan_session_id
        || assistant_message_row.client_id != row.client_id
        || assistant_message_row.guide_run_id != row.guide_run_id
        || assistant_message_row.source_checkpoint_id != row.source_checkpoint_id
        || assistant_message_row.source_checkpoint_revision != row.source_checkpoint_revision
        || assistant_message_row.source_checkpoint_sha256 != row.source_checkpoint_sha256
        || assistant_message_row.source_thread_id.as_deref() != Some(row.source_thread_id.as_str())
        || assistant_message_row.source_turn_id.as_deref() != Some(row.source_turn_id.as_str())
        || sha256(assistant_message_row.content.as_bytes()) != assistant_message_row.content_sha256
        || terminal.assistant_message_sha256 != assistant_message_row.content_sha256
    {
        return Err(validation(
            "workspace plan completion does not match its assistant message",
        ));
    }
    let assistant_message = assistant_message_row.try_into_model(replayed)?;
    if !matches!(
        assistant_message.role,
        crate::WorkspacePlanMessageRole::Assistant
            | crate::WorkspacePlanMessageRole::Question
            | crate::WorkspacePlanMessageRole::Error
    ) {
        return Err(validation(
            "workspace plan completion references a non-assistant message",
        ));
    }

    let revision = match row.plan_revision_id.as_deref() {
        Some(revision_id) => {
            let revision_row = super::revisions::revision_by_id(tx, revision_id)
                .await?
                .ok_or_else(|| not_found("completed workspace plan revision was not found"))?;
            if revision_row.plan_session_id != row.plan_session_id
                || revision_row.client_id != row.client_id
                || revision_row.guide_run_id != row.guide_run_id
                || revision_row.source_checkpoint_id != row.source_checkpoint_id
                || revision_row.source_checkpoint_revision != row.source_checkpoint_revision
                || revision_row.source_checkpoint_sha256 != row.source_checkpoint_sha256
                || revision_row.source_thread_id != row.source_thread_id
                || revision_row.source_turn_id != row.source_turn_id
                || sha256(revision_row.evidence_manifest_json.as_bytes())
                    != revision_row.evidence_manifest_sha256
                || revision_row.evidence_manifest_json != row.evidence_manifest_json
                || revision_row.evidence_manifest_sha256 != row.evidence_manifest_sha256
                || revision_row.evidence_read_count != row.evidence_read_count
            {
                return Err(validation(
                    "workspace plan completion does not match its published revision",
                ));
            }
            Some(revision_row.try_into_model(replayed)?)
        }
        None => None,
    };
    let terminal_revision_matches = match (terminal.plan_revision.as_ref(), revision.as_ref()) {
        (None, None) => true,
        (Some(terminal_revision), Some(revision)) => {
            terminal_revision.id == revision.id
                && terminal_revision.content_sha256 == revision.content_sha256
        }
        (None, Some(_)) | (Some(_), None) => false,
    };
    if !terminal_revision_matches {
        return Err(validation(
            "workspace plan terminal envelope does not match its published revision",
        ));
    }

    Ok(VerifiedPlanCompletion {
        run,
        assistant_message,
        revision,
        evidence_manifest,
    })
}

async fn persisted_evidence_manifest(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    completion: &WorkspacePlanTurnCompletionRow,
) -> PlanResult<Vec<crate::WorkspacePlanEvidenceRead>> {
    let rows = sqlx::query_as::<_, EvidenceManifestRow>(
        r#"
SELECT ordinal, context_read_id, category, response_sha256,
       source_content_sha256_json
FROM workspace_plan_turn_evidence
WHERE guide_run_id = ?
ORDER BY ordinal ASC
        "#,
    )
    .bind(&completion.guide_run_id)
    .fetch_all(&mut **tx)
    .await?;
    let mut manifest = Vec::with_capacity(rows.len());
    for (expected_ordinal, row) in rows.into_iter().enumerate() {
        if row.ordinal
            != i64::try_from(expected_ordinal)
                .map_err(|_| validation("workspace plan evidence ordinal exceeds SQLite range"))?
        {
            return Err(validation(
                "workspace plan evidence ordinals are not contiguous",
            ));
        }
        let read = evidence_read_by_id(tx, &row.context_read_id)
            .await?
            .ok_or_else(|| not_found("workspace plan evidence context read was not found"))?;
        let source_content_sha256: Vec<String> =
            serde_json::from_str(&row.source_content_sha256_json)?;
        let sources: Vec<WorkspacePlanningContextSource> =
            serde_json::from_str(&read.response_json)?;
        let read_source_content_sha256 = sources
            .into_iter()
            .map(|source| source.content_sha256)
            .collect::<Vec<_>>();
        if read.guide_run_id != completion.guide_run_id
            || read.plan_session_id != completion.plan_session_id
            || read.client_id != completion.client_id
            || read.source_checkpoint_id != completion.source_checkpoint_id
            || read.source_checkpoint_revision != completion.source_checkpoint_revision
            || read.source_checkpoint_sha256 != completion.source_checkpoint_sha256
            || read.source_thread_id != completion.source_thread_id
            || read.source_turn_id != completion.source_turn_id
            || read.prompt_sha256 != completion.prompt_sha256
            || read.execution_token_sha256 != completion.execution_token_sha256
            || read.category != row.category
            || read.response_sha256 != row.response_sha256
            || sha256(read.response_json.as_bytes()) != read.response_sha256
            || source_content_sha256 != read_source_content_sha256
        {
            return Err(validation(
                "workspace plan evidence does not match its immutable context read",
            ));
        }
        manifest.push(crate::WorkspacePlanEvidenceRead {
            ordinal: u32::try_from(row.ordinal)
                .map_err(|_| validation("workspace plan evidence ordinal is invalid"))?,
            context_read_id: row.context_read_id,
            category: row.category,
            response_sha256: row.response_sha256,
            source_content_sha256,
        });
    }
    Ok(manifest)
}

async fn evidence_read_by_id(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    read_id: &str,
) -> PlanResult<Option<EvidenceReadRow>> {
    Ok(sqlx::query_as(
        r#"
SELECT guide_run_id, plan_session_id, client_id, category, response_json,
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

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
