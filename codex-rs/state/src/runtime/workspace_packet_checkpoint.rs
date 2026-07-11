use crate::WorkspaceContextPacketCreate;
use crate::model::WorkspaceContextPacketRow;
use serde_json::Value;
use sqlx::Sqlite;

pub(super) struct PacketReplayFields<'a> {
    pub(super) base_note_revision: Option<i64>,
    pub(super) context_envelope_sha256: &'a str,
    pub(super) clinician_actor: &'a str,
    pub(super) authorized_scope_json: &'a str,
    pub(super) expected_output_kind: &'a str,
}

#[derive(sqlx::FromRow)]
struct DraftCheckpointBinding {
    session_id: String,
    client_id: String,
    encounter_id: Option<String>,
    note_id: Option<String>,
    base_note_revision: Option<i64>,
    revision: i64,
    content_sha256: String,
    session_status: String,
    current_checkpoint_id: String,
    current_revision: i64,
    current_sha256: String,
}

pub(super) async fn validate_source_and_find_replay(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    input: &WorkspaceContextPacketCreate,
    fields: PacketReplayFields<'_>,
) -> anyhow::Result<Option<WorkspaceContextPacketRow>> {
    let source = match source_tuple(input)? {
        Some(source) => source,
        None => return Ok(None),
    };
    let checkpoint = checkpoint_binding(tx, source.checkpoint_id)
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "workspace context packet draft checkpoint `{}` was not found",
                source.checkpoint_id
            )
        })?;
    validate_binding(input, &fields, source, &checkpoint)?;
    validate_envelope_source(input, &fields, source)?;

    let existing = packet_by_checkpoint(tx, source.checkpoint_id).await?;
    if let Some(existing) = existing {
        validate_exact_replay(input, &fields, &existing)?;
        return Ok(Some(existing));
    }
    Ok(None)
}

#[derive(Clone, Copy)]
struct DraftSource<'a> {
    session_id: &'a str,
    checkpoint_id: &'a str,
    revision: i64,
    sha256: &'a str,
}

fn source_tuple(input: &WorkspaceContextPacketCreate) -> anyhow::Result<Option<DraftSource<'_>>> {
    let supplied = [
        input.source_draft_session_id.is_some(),
        input.source_draft_checkpoint_id.is_some(),
        input.source_draft_checkpoint_revision.is_some(),
        input.source_draft_checkpoint_sha256.is_some(),
    ];
    let supplied_count = supplied.into_iter().filter(|supplied| *supplied).count();
    if supplied_count == 0 {
        return Ok(None);
    }
    if supplied_count != supplied.len() {
        anyhow::bail!(
            "workspace context packet draft source requires session, checkpoint, revision, and hash"
        );
    }
    let session_id = required(
        "draft session id",
        input.source_draft_session_id.as_deref().unwrap_or_default(),
    )?;
    let checkpoint_id = required(
        "draft checkpoint id",
        input
            .source_draft_checkpoint_id
            .as_deref()
            .unwrap_or_default(),
    )?;
    let revision = input.source_draft_checkpoint_revision.unwrap_or_default();
    if revision < 1 {
        anyhow::bail!("workspace context packet draft checkpoint revision must be positive");
    }
    let sha256 = required(
        "draft checkpoint hash",
        input
            .source_draft_checkpoint_sha256
            .as_deref()
            .unwrap_or_default(),
    )?;
    if sha256.len() != 64
        || !sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        anyhow::bail!("workspace context packet draft checkpoint hash must be lowercase SHA-256");
    }
    Ok(Some(DraftSource {
        session_id,
        checkpoint_id,
        revision,
        sha256,
    }))
}

fn validate_binding(
    input: &WorkspaceContextPacketCreate,
    fields: &PacketReplayFields<'_>,
    source: DraftSource<'_>,
    checkpoint: &DraftCheckpointBinding,
) -> anyhow::Result<()> {
    if checkpoint.session_id != source.session_id
        || checkpoint.client_id != input.client_id.trim()
        || normalized(checkpoint.encounter_id.as_deref())
            != normalized(input.encounter_id.as_deref())
        || normalized(checkpoint.note_id.as_deref()) != normalized(input.note_id.as_deref())
        || checkpoint.base_note_revision != fields.base_note_revision
        || checkpoint.revision != source.revision
        || checkpoint.content_sha256 != source.sha256
    {
        anyhow::bail!(
            "workspace context packet draft checkpoint identity does not match packet scope"
        );
    }
    if checkpoint.session_status != "active" {
        anyhow::bail!("workspace context packet requires an active draft session");
    }
    if checkpoint.current_checkpoint_id != source.checkpoint_id
        || checkpoint.current_revision != source.revision
        || checkpoint.current_sha256 != source.sha256
    {
        anyhow::bail!("workspace context packet draft checkpoint is no longer current");
    }
    Ok(())
}

fn validate_envelope_source(
    input: &WorkspaceContextPacketCreate,
    fields: &PacketReplayFields<'_>,
    source: DraftSource<'_>,
) -> anyhow::Result<()> {
    let envelope: Value = serde_json::from_str(input.context_envelope_json.trim())?;
    let actual = envelope
        .get("sourceCheckpoint")
        .ok_or_else(|| anyhow::anyhow!("workspace context packet sourceCheckpoint is required"))?;
    let expected = serde_json::json!({
        "clientId": input.client_id.trim(),
        "sessionId": source.session_id,
        "id": source.checkpoint_id,
        "revision": source.revision,
        "contentSha256": source.sha256,
        "encounterId": normalized(input.encounter_id.as_deref()),
        "noteId": normalized(input.note_id.as_deref()),
        "baseNoteRevision": fields.base_note_revision,
    });
    if actual != &expected {
        anyhow::bail!(
            "workspace context packet sourceCheckpoint does not match the reviewed draft"
        );
    }
    Ok(())
}

fn validate_exact_replay(
    input: &WorkspaceContextPacketCreate,
    fields: &PacketReplayFields<'_>,
    existing: &WorkspaceContextPacketRow,
) -> anyhow::Result<()> {
    let matches = existing.client_id == input.client_id
        && existing.encounter_id == input.encounter_id
        && existing.note_id == input.note_id
        && existing.source_draft_session_id.as_deref()
            == normalized(input.source_draft_session_id.as_deref())
        && existing.source_draft_checkpoint_id.as_deref()
            == normalized(input.source_draft_checkpoint_id.as_deref())
        && existing.source_draft_checkpoint_revision == input.source_draft_checkpoint_revision
        && existing.source_draft_checkpoint_sha256.as_deref()
            == normalized(input.source_draft_checkpoint_sha256.as_deref())
        && existing.human_request == input.human_request
        && existing.selected_artifact_ids_json == input.selected_artifact_ids_json
        && existing.selected_derivative_ids_json == input.selected_derivative_ids_json
        && existing.selected_clip_ids_json == input.selected_clip_ids_json
        && existing.artifact_summary == input.artifact_summary
        && existing.derivative_summary == input.derivative_summary
        && existing.clip_summary == input.clip_summary
        && existing.chart_context_summary == input.chart_context_summary
        && existing.context_envelope_json == input.context_envelope_json
        && existing.context_envelope_sha256 == fields.context_envelope_sha256
        && existing.clinician_actor == fields.clinician_actor
        && existing.base_note_revision == fields.base_note_revision
        && existing.authorized_scope_json == fields.authorized_scope_json
        && existing.expected_output_kind == fields.expected_output_kind;
    if !matches {
        anyhow::bail!(
            "workspace context packet draft checkpoint was already reviewed with different content"
        );
    }
    Ok(())
}

async fn checkpoint_binding(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    checkpoint_id: &str,
) -> anyhow::Result<Option<DraftCheckpointBinding>> {
    Ok(sqlx::query_as(
        r#"
SELECT checkpoint.session_id, checkpoint.client_id, checkpoint.encounter_id,
       checkpoint.note_id, checkpoint.base_note_revision, checkpoint.revision,
       checkpoint.content_sha256, session.status AS session_status,
       current.id AS current_checkpoint_id, current.revision AS current_revision,
       current.content_sha256 AS current_sha256
FROM workspace_draft_checkpoints AS checkpoint
JOIN workspace_draft_sessions AS session
  ON session.id = checkpoint.session_id AND session.client_id = checkpoint.client_id
JOIN workspace_draft_checkpoints AS current
  ON current.session_id = session.id AND current.client_id = session.client_id
 AND current.revision = session.current_revision
WHERE checkpoint.id = ?
        "#,
    )
    .bind(checkpoint_id)
    .fetch_optional(&mut **tx)
    .await?)
}

async fn packet_by_checkpoint(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    checkpoint_id: &str,
) -> anyhow::Result<Option<WorkspaceContextPacketRow>> {
    let row = sqlx::query(
        r#"
SELECT id, client_id, encounter_id, note_id,
       source_draft_session_id, source_draft_checkpoint_id,
       source_draft_checkpoint_revision, source_draft_checkpoint_sha256,
       human_request, selected_artifact_ids_json, selected_derivative_ids_json,
       selected_clip_ids_json, artifact_summary, derivative_summary, clip_summary,
       chart_context_summary, context_envelope_json, context_envelope_sha256,
       clinician_actor, base_note_revision, authorized_scope_json,
       expected_output_kind, status, created_at_ms, sent_at_ms, submitted_at_ms,
       canceled_at_ms, updated_at_ms
FROM workspace_context_packets
WHERE source_draft_checkpoint_id = ?
LIMIT 1
        "#,
    )
    .bind(checkpoint_id)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(|row| WorkspaceContextPacketRow::try_from_row(&row))
        .transpose()
}

fn required<'a>(label: &str, value: &'a str) -> anyhow::Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        anyhow::bail!("workspace context packet {label} must not be empty");
    }
    Ok(value)
}

fn normalized(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
#[path = "workspace_packet_checkpoint_tests.rs"]
mod tests;
