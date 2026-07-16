use serde_json::Value;
use sha2::Digest;
use sha2::Sha256;
use sqlx::Sqlite;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorkspacePlanRevisionBinding {
    pub(super) revision_id: String,
    pub(super) content_sha256: String,
    pub(super) evidence_manifest_sha256: String,
}

pub(super) struct WorkspacePlanBindingContext<'a> {
    pub(super) client_id: &'a str,
    pub(super) encounter_id: Option<&'a str>,
    pub(super) note_id: Option<&'a str>,
    pub(super) source_checkpoint_id: Option<&'a str>,
    pub(super) source_checkpoint_sha256: Option<&'a str>,
    pub(super) context_envelope_json: &'a str,
}

#[derive(sqlx::FromRow)]
struct PlanRevisionBindingRow {
    id: String,
    client_id: String,
    status: String,
    plan_markdown: String,
    decisions_json: String,
    open_questions_json: String,
    content_sha256: String,
    evidence_manifest_json: String,
    evidence_manifest_sha256: String,
    evidence_read_count: i64,
    source_checkpoint_id: String,
    source_checkpoint_revision: i64,
    source_checkpoint_sha256: String,
    encounter_id: Option<String>,
    note_id: Option<String>,
    checkpoint_revision: i64,
    checkpoint_sha256: String,
    checkpoint_current_revision: i64,
}

pub(super) fn normalize_plan_revision_binding(
    revision_id: Option<&str>,
    content_sha256: Option<&str>,
    evidence_manifest_sha256: Option<&str>,
) -> anyhow::Result<Option<WorkspacePlanRevisionBinding>> {
    let revision_id = normalized_optional(revision_id);
    let content_sha256 = normalized_optional(content_sha256);
    let evidence_manifest_sha256 = normalized_optional(evidence_manifest_sha256);
    match (revision_id, content_sha256, evidence_manifest_sha256) {
        (None, None, None) => Ok(None),
        (Some(revision_id), Some(content_sha256), Some(evidence_manifest_sha256)) => {
            if !is_lower_hex_sha256(&content_sha256)
                || !is_lower_hex_sha256(&evidence_manifest_sha256)
            {
                anyhow::bail!(
                    "workspace plan binding hashes must be 64 lowercase hexadecimal characters"
                );
            }
            Ok(Some(WorkspacePlanRevisionBinding {
                revision_id,
                content_sha256,
                evidence_manifest_sha256,
            }))
        }
        _ => anyhow::bail!(
            "workspace plan revision id, content hash, and evidence manifest hash must be provided together"
        ),
    }
}

pub(super) async fn validate_plan_revision_binding(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    binding: &WorkspacePlanRevisionBinding,
    context: WorkspacePlanBindingContext<'_>,
) -> anyhow::Result<()> {
    let checkpoint_id = context
        .source_checkpoint_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("workspace plan binding requires a source checkpoint id"))?;
    let checkpoint_sha256 = context
        .source_checkpoint_sha256
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("workspace plan binding requires a source checkpoint SHA-256")
        })?;
    let row = sqlx::query_as::<_, PlanRevisionBindingRow>(
        r#"
SELECT
    revision.id,
    revision.client_id,
    revision.status,
    revision.plan_markdown,
    revision.decisions_json,
    revision.open_questions_json,
    revision.content_sha256,
    revision.evidence_manifest_json,
    revision.evidence_manifest_sha256,
    revision.evidence_read_count,
    revision.source_checkpoint_id,
    revision.source_checkpoint_revision,
    revision.source_checkpoint_sha256,
    revision.encounter_id,
    revision.note_id,
    checkpoint.revision AS checkpoint_revision,
    checkpoint.content_sha256 AS checkpoint_sha256,
    session.current_revision AS checkpoint_current_revision
FROM workspace_plan_revisions AS revision
JOIN workspace_draft_checkpoints AS checkpoint
  ON checkpoint.id = revision.source_checkpoint_id
 AND checkpoint.client_id = revision.client_id
JOIN workspace_draft_sessions AS session
  ON session.id = checkpoint.session_id
 AND session.client_id = checkpoint.client_id
WHERE revision.id = ?
LIMIT 1
        "#,
    )
    .bind(&binding.revision_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        anyhow::anyhow!(
            "workspace plan revision `{}` was not found",
            binding.revision_id
        )
    })?;

    if row.id != binding.revision_id
        || row.client_id != context.client_id.trim()
        || row.encounter_id.as_deref() != normalized_optional_ref(context.encounter_id)
        || row.note_id.as_deref() != normalized_optional_ref(context.note_id)
    {
        anyhow::bail!(
            "workspace plan revision `{}` does not match the packet patient, encounter, and note scope",
            binding.revision_id
        );
    }
    if row.source_checkpoint_id != checkpoint_id
        || row.source_checkpoint_sha256 != checkpoint_sha256
        || row.source_checkpoint_revision != row.checkpoint_revision
        || row.source_checkpoint_sha256 != row.checkpoint_sha256
    {
        anyhow::bail!(
            "workspace plan revision `{}` does not match the packet source checkpoint",
            binding.revision_id
        );
    }
    if row.content_sha256 != binding.content_sha256
        || row.evidence_manifest_sha256 != binding.evidence_manifest_sha256
    {
        anyhow::bail!(
            "workspace plan revision `{}` hashes do not match the packet binding",
            binding.revision_id
        );
    }
    validate_stored_revision_hashes(&row)?;
    validate_envelope_revision_binding(&row, context.context_envelope_json)?;
    match row.status.as_str() {
        "current" if row.checkpoint_current_revision == row.source_checkpoint_revision => Ok(()),
        "current" => anyhow::bail!(
            "workspace plan revision `{}` is stale because its source checkpoint is no longer current",
            binding.revision_id
        ),
        // An already-submitted revision is an exact, immutable replay. Its persisted scope and all
        // three caller-bound hashes were checked above, so it remains valid after later chart work.
        "submitted" => Ok(()),
        status => anyhow::bail!(
            "workspace plan revision `{}` is `{status}` and cannot bind a context packet",
            binding.revision_id
        ),
    }
}

pub(super) async fn require_submitted_plan_revision_receipt(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    binding: &WorkspacePlanRevisionBinding,
    packet_id: &str,
    agent_run_id: &str,
    client_id: &str,
) -> anyhow::Result<()> {
    let receipt_matches = sqlx::query_scalar::<_, i64>(
        r#"
SELECT 1
FROM workspace_plan_submission_receipts AS receipt
JOIN workspace_plan_revisions AS revision
  ON revision.id = receipt.plan_revision_id
WHERE receipt.plan_revision_id = ?
  AND receipt.packet_id = ?
  AND receipt.agent_run_id = ?
  AND receipt.client_id = ?
  AND receipt.plan_session_id = revision.plan_session_id
  AND receipt.plan_content_sha256 = ?
  AND receipt.evidence_manifest_sha256 = ?
  AND receipt.submitted_at_ms = revision.submitted_at_ms
  AND revision.client_id = receipt.client_id
  AND revision.content_sha256 = receipt.plan_content_sha256
  AND revision.evidence_manifest_sha256 = receipt.evidence_manifest_sha256
  AND revision.status = 'submitted'
LIMIT 1
        "#,
    )
    .bind(&binding.revision_id)
    .bind(packet_id)
    .bind(agent_run_id)
    .bind(client_id)
    .bind(&binding.content_sha256)
    .bind(&binding.evidence_manifest_sha256)
    .fetch_optional(&mut **tx)
    .await?
    .is_some();
    if !receipt_matches {
        anyhow::bail!(
            "workspace plan revision `{}` has no durable submission receipt for context packet `{packet_id}` and agent run `{agent_run_id}`",
            binding.revision_id
        );
    }
    Ok(())
}

fn validate_envelope_revision_binding(
    row: &PlanRevisionBindingRow,
    context_envelope_json: &str,
) -> anyhow::Result<()> {
    let envelope: Value = serde_json::from_str(context_envelope_json).map_err(|error| {
        anyhow::anyhow!(
            "workspace context packet envelope must be valid JSON for plan binding: {error}"
        )
    })?;
    let receipt_matches = envelope
        .pointer("/workspacePlanRevision/id")
        .and_then(Value::as_str)
        == Some(row.id.as_str())
        && envelope
            .pointer("/workspacePlanRevision/contentSha256")
            .and_then(Value::as_str)
            == Some(row.content_sha256.as_str())
        && envelope
            .pointer("/workspacePlanRevision/evidenceManifestSha256")
            .and_then(Value::as_str)
            == Some(row.evidence_manifest_sha256.as_str());
    if !receipt_matches {
        anyhow::bail!(
            "workspace context packet envelope plan revision receipt does not match persisted revision `{}`",
            row.id
        );
    }
    if envelope
        .get("workspacePlanMarkdown")
        .and_then(Value::as_str)
        != Some(row.plan_markdown.as_str())
    {
        anyhow::bail!(
            "workspace context packet envelope plan markdown does not match persisted revision `{}`",
            row.id
        );
    }
    Ok(())
}

fn validate_stored_revision_hashes(row: &PlanRevisionBindingRow) -> anyhow::Result<()> {
    let decisions: Value = serde_json::from_str(&row.decisions_json)?;
    let open_questions: Value = serde_json::from_str(&row.open_questions_json)?;
    if !open_questions
        .as_array()
        .is_some_and(std::vec::Vec::is_empty)
    {
        anyhow::bail!(
            "workspace plan revision `{}` is not decision-complete because it has open questions",
            row.id
        );
    }
    let content_json = serde_json::to_string(&serde_json::json!({
        "planMarkdown": row.plan_markdown,
        "decisions": decisions,
        "openQuestions": open_questions,
    }))?;
    if sha256(content_json.as_bytes()) != row.content_sha256 {
        anyhow::bail!(
            "workspace plan revision `{}` failed its content hash check",
            row.id
        );
    }
    let evidence: Value = serde_json::from_str(&row.evidence_manifest_json)?;
    let evidence_items = evidence.as_array().ok_or_else(|| {
        anyhow::anyhow!(
            "workspace plan revision `{}` evidence manifest is not an array",
            row.id
        )
    })?;
    let evidence_count = evidence_items.len();
    if i64::try_from(evidence_count)? != row.evidence_read_count
        || sha256(row.evidence_manifest_json.as_bytes()) != row.evidence_manifest_sha256
    {
        anyhow::bail!(
            "workspace plan revision `{}` failed its evidence manifest hash check",
            row.id
        );
    }
    for required_category in ["patient_chart", "selected_context"] {
        if !evidence_items
            .iter()
            .any(|item| item.get("category").and_then(Value::as_str) == Some(required_category))
        {
            anyhow::bail!(
                "workspace plan revision `{}` evidence manifest is missing required `{required_category}` context",
                row.id
            );
        }
    }
    Ok(())
}

fn normalized_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn normalized_optional_ref(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn is_lower_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
