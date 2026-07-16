use super::*;
use crate::model::WorkspaceAgentResultRow;
use crate::model::WorkspaceAgentRunRow;
use crate::model::WorkspaceContextPacketRow;

pub(super) async fn validate_agent_source_ownership(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run: &WorkspaceAgentRunRow,
    source_type: &str,
    source_id: &str,
    source_revision: Option<i64>,
) -> anyhow::Result<()> {
    let owner: Option<String> = match source_type {
        "client" | "demographics" | "patient_summary" => {
            (source_id == run.client_id).then(|| run.client_id.clone())
        }
        "note" | "active_note" | "prior_note" => {
            sqlx::query_scalar("SELECT client_id FROM workspace_notes WHERE id = ?")
                .bind(source_id)
                .fetch_optional(&mut **tx)
                .await?
        }
        "note_revision" => {
            let revision = source_revision.ok_or_else(|| {
                anyhow::anyhow!("workspace note revision source requires a revision")
            })?;
            sqlx::query_scalar(
                "SELECT note.client_id FROM workspace_note_revisions AS revision JOIN workspace_notes AS note ON note.id = revision.note_id WHERE revision.note_id = ? AND revision.revision = ?",
            )
            .bind(source_id)
            .bind(revision)
            .fetch_optional(&mut **tx)
            .await?
        }
        "encounter" => {
            sqlx::query_scalar("SELECT client_id FROM workspace_encounters WHERE id = ?")
                .bind(source_id)
                .fetch_optional(&mut **tx)
                .await?
        }
        "document" => {
            sqlx::query_scalar("SELECT client_id FROM workspace_documents WHERE id = ?")
                .bind(source_id)
                .fetch_optional(&mut **tx)
                .await?
        }
        "task" => {
            sqlx::query_scalar("SELECT client_id FROM workspace_tasks WHERE id = ?")
                .bind(source_id)
                .fetch_optional(&mut **tx)
                .await?
        }
        "patient_safety_item" => {
            sqlx::query_scalar("SELECT client_id FROM workspace_patient_safety_items WHERE id = ?")
                .bind(source_id)
                .fetch_optional(&mut **tx)
                .await?
        }
        "artifact_derivative" => {
            sqlx::query_scalar("SELECT client_id FROM workspace_artifact_derivatives WHERE id = ?")
                .bind(source_id)
                .fetch_optional(&mut **tx)
                .await?
        }
        "context_clip" => {
            sqlx::query_scalar("SELECT client_id FROM workspace_context_clips WHERE id = ?")
                .bind(source_id)
                .fetch_optional(&mut **tx)
                .await?
        }
        "context_packet" => (source_id == run.packet_id).then(|| run.client_id.clone()),
        other => anyhow::bail!("unsupported workspace agent source type `{other}`"),
    };
    match owner {
        Some(owner) if owner == run.client_id => Ok(()),
        Some(owner) => anyhow::bail!(
            "workspace agent source `{source_type}:{source_id}` belongs to client `{owner}` not `{}`",
            run.client_id
        ),
        None => anyhow::bail!(
            "workspace agent source `{source_type}:{source_id}` was not found for client `{}`",
            run.client_id
        ),
    }
}

pub(super) async fn workspace_context_packet_row_by_id(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<Option<WorkspaceContextPacketRow>> {
    let row = sqlx::query(
        r#"
SELECT
    id, client_id, encounter_id, note_id, human_request,
    selected_artifact_ids_json, selected_derivative_ids_json, selected_clip_ids_json,
    artifact_summary, derivative_summary, clip_summary, chart_context_summary,
    context_envelope_json, context_envelope_sha256, clinician_actor,
    base_note_revision, authorized_scope_json, expected_output_kind,
    workspace_profile, plan_schema_version, source_checkpoint_id,
    source_checkpoint_sha256, readiness_json, workspace_plan_revision_id,
    workspace_plan_content_sha256, workspace_plan_evidence_manifest_sha256, status,
    created_at_ms, sent_at_ms, submitted_at_ms, canceled_at_ms, updated_at_ms
FROM workspace_context_packets
WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(|row| WorkspaceContextPacketRow::try_from_row(&row))
        .transpose()
}

pub(super) async fn workspace_agent_run_row_by_id(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<Option<WorkspaceAgentRunRow>> {
    let row = sqlx::query(
        r#"
SELECT
    id, packet_id, client_id, note_id, base_note_revision,
    context_envelope_sha256, workspace_plan_revision_id,
    workspace_plan_content_sha256, workspace_plan_evidence_manifest_sha256,
    run_kind, idempotency_key, provider, model,
    source_thread_id, source_turn_id, status, error_summary,
    started_at_ms, completed_at_ms, created_at_ms, updated_at_ms
FROM workspace_agent_runs
WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(|row| WorkspaceAgentRunRow::try_from_row(&row))
        .transpose()
}

pub(super) async fn workspace_agent_run_row_by_key(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    packet_id: &str,
    idempotency_key: &str,
) -> anyhow::Result<Option<WorkspaceAgentRunRow>> {
    let row = sqlx::query(
        r#"
SELECT
    id, packet_id, client_id, note_id, base_note_revision,
    context_envelope_sha256, workspace_plan_revision_id,
    workspace_plan_content_sha256, workspace_plan_evidence_manifest_sha256,
    run_kind, idempotency_key, provider, model,
    source_thread_id, source_turn_id, status, error_summary,
    started_at_ms, completed_at_ms, created_at_ms, updated_at_ms
FROM workspace_agent_runs
WHERE packet_id = ? AND idempotency_key = ?
        "#,
    )
    .bind(packet_id)
    .bind(idempotency_key)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(|row| WorkspaceAgentRunRow::try_from_row(&row))
        .transpose()
}

pub(super) async fn workspace_agent_result_row_by_id(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<Option<WorkspaceAgentResultRow>> {
    let row = sqlx::query(
        r#"
SELECT
    result.id, result.packet_id, result.client_id, result.note_id,
    result.run_id, result.base_note_revision,
    packet.context_envelope_sha256 AS context_envelope_sha256,
    COALESCE(NULLIF(result.packet_context_sha256, ''), packet.context_envelope_sha256)
        AS packet_context_sha256,
    result.body, result.summary, result.result_kind,
    result.structured_changes_json, result.rationale_summary,
    result.status, result.created_at_ms, result.updated_at_ms
FROM workspace_agent_results AS result
JOIN workspace_context_packets AS packet ON packet.id = result.packet_id
WHERE result.id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(|row| WorkspaceAgentResultRow::try_from_row(&row))
        .transpose()
}

pub(super) async fn workspace_agent_result_row_by_run(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    run_id: &str,
) -> anyhow::Result<Option<WorkspaceAgentResultRow>> {
    let row = sqlx::query(
        r#"
SELECT
    result.id, result.packet_id, result.client_id, result.note_id,
    result.run_id, result.base_note_revision,
    packet.context_envelope_sha256 AS context_envelope_sha256,
    COALESCE(NULLIF(result.packet_context_sha256, ''), packet.context_envelope_sha256)
        AS packet_context_sha256,
    result.body, result.summary, result.result_kind,
    result.structured_changes_json, result.rationale_summary,
    result.status, result.created_at_ms, result.updated_at_ms
FROM workspace_agent_results AS result
JOIN workspace_context_packets AS packet ON packet.id = result.packet_id
WHERE result.run_id = ?
        "#,
    )
    .bind(run_id)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(|row| WorkspaceAgentResultRow::try_from_row(&row))
        .transpose()
}

pub(super) async fn workspace_note_proposal_row_by_id(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<Option<sqlx::sqlite::SqliteRow>> {
    sqlx::query(
        r#"
SELECT
    id, note_id, base_revision, agent_result_id, proposed_body, summary,
    status, source_thread_id, source_turn_id, created_at_ms, resolved_at_ms
FROM workspace_note_proposals
WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(Into::into)
}

pub(super) async fn workspace_note_proposal_decision_row_by_id(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    id: &str,
) -> anyhow::Result<sqlx::sqlite::SqliteRow> {
    sqlx::query(
        r#"
SELECT
    id, proposal_id, agent_result_id, note_id, base_revision, decision_kind,
    change_id, applied_text, applied_text_sha256, resulting_note_revision,
    actor, reason, created_at_ms
FROM workspace_note_proposal_decisions
WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(&mut **tx)
    .await
    .map_err(Into::into)
}
