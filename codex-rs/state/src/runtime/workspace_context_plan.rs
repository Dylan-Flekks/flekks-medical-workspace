use super::*;
use crate::model::WorkspaceContextPacketRow;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeSet;

const WORKSPACE_PROFILE: &str = "medical";
const CONTEXT_PLAN_SCHEMA_VERSION: i64 = 1;
const MAX_READINESS_ITEMS: usize = 100;
const MAX_READINESS_TEXT_BYTES: usize = 4 * 1024;

pub(super) struct NormalizedContextPlanMetadata {
    pub(super) workspace_profile: String,
    pub(super) plan_schema_version: i64,
    pub(super) source_checkpoint_id: Option<String>,
    pub(super) source_checkpoint_sha256: Option<String>,
    pub(super) readiness_json: String,
    pub(super) context_envelope_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContextPlanReadiness {
    version: i64,
    #[serde(default)]
    warnings: Vec<ContextPlanWarning>,
    #[serde(default)]
    acknowledgements: Vec<ContextPlanAcknowledgement>,
    #[serde(default)]
    legacy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContextPlanWarning {
    code: String,
    message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContextPlanAcknowledgement {
    warning_code: String,
    checkpoint_sha256: String,
    reason: String,
}

pub(super) async fn normalize_context_plan_metadata(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    input: &crate::WorkspaceContextPacketCreate,
) -> anyhow::Result<NormalizedContextPlanMetadata> {
    let source_checkpoint_id = normalized_optional(&input.source_checkpoint_id);
    let source_checkpoint_sha256 = normalized_optional(&input.source_checkpoint_sha256);
    let metadata_presence = [
        !input.workspace_profile.trim().is_empty(),
        input.plan_schema_version.is_some(),
        source_checkpoint_id.is_some(),
        source_checkpoint_sha256.is_some(),
        !input.readiness_json.trim().is_empty(),
    ];
    let is_legacy = metadata_presence.iter().all(|present| !present);
    if !is_legacy && metadata_presence.iter().any(|present| !present) {
        anyhow::bail!(
            "workspace context plan profile, schema version, source checkpoint id/hash, and readiness must be provided together"
        );
    }
    let workspace_profile = input.workspace_profile.trim();
    let workspace_profile = if workspace_profile.is_empty() {
        WORKSPACE_PROFILE.to_string()
    } else {
        workspace_profile.to_string()
    };
    if workspace_profile != WORKSPACE_PROFILE {
        anyhow::bail!(
            "unsupported workspace context packet profile `{workspace_profile}`; expected `{WORKSPACE_PROFILE}`"
        );
    }
    let plan_schema_version = input
        .plan_schema_version
        .unwrap_or(CONTEXT_PLAN_SCHEMA_VERSION);
    if plan_schema_version != CONTEXT_PLAN_SCHEMA_VERSION {
        anyhow::bail!("unsupported workspace context plan schemaVersion {plan_schema_version}");
    }

    if let Some(source_checkpoint_sha256) = source_checkpoint_sha256.as_deref()
        && !is_lower_hex_sha256(source_checkpoint_sha256)
    {
        anyhow::bail!(
            "workspace context plan source checkpoint SHA-256 must be 64 lowercase hexadecimal characters"
        );
    }
    if let (Some(checkpoint_id), Some(checkpoint_sha256)) = (
        source_checkpoint_id.as_deref(),
        source_checkpoint_sha256.as_deref(),
    ) {
        validate_source_checkpoint(tx, input, checkpoint_id, checkpoint_sha256).await?;
    }

    let readiness = if input.readiness_json.trim().is_empty() {
        ContextPlanReadiness {
            version: 1,
            warnings: Vec::new(),
            acknowledgements: Vec::new(),
            legacy: is_legacy,
        }
    } else {
        serde_json::from_str(input.readiness_json.trim()).map_err(|error| {
            anyhow::anyhow!("workspace context plan readiness must be valid JSON: {error}")
        })?
    };
    if readiness.legacy != is_legacy {
        anyhow::bail!(
            "workspace context plan legacy readiness is reserved for packets with no Context Plan metadata"
        );
    }
    let readiness = normalize_readiness(readiness, source_checkpoint_sha256.as_deref())?;
    if !readiness.legacy && source_checkpoint_id.is_none() {
        anyhow::bail!("workspace context plan requires a source checkpoint");
    }
    let readiness_json = serde_json::to_string(&readiness)?;
    let context_envelope_json = bind_metadata_to_context_envelope(
        &input.context_envelope_json,
        &workspace_profile,
        plan_schema_version,
        source_checkpoint_id.as_deref(),
        source_checkpoint_sha256.as_deref(),
        &readiness,
    )?;

    Ok(NormalizedContextPlanMetadata {
        workspace_profile,
        plan_schema_version,
        source_checkpoint_id,
        source_checkpoint_sha256,
        readiness_json,
        context_envelope_json,
    })
}

pub(super) async fn validate_context_plan_for_submission(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    packet: &WorkspaceContextPacketRow,
) -> anyhow::Result<()> {
    if packet.workspace_profile != WORKSPACE_PROFILE {
        anyhow::bail!(
            "workspace context packet `{}` has unsupported profile `{}`",
            packet.id,
            packet.workspace_profile
        );
    }
    if packet.plan_schema_version != CONTEXT_PLAN_SCHEMA_VERSION {
        anyhow::bail!(
            "workspace context packet `{}` has unsupported plan schemaVersion {}",
            packet.id,
            packet.plan_schema_version
        );
    }
    let readiness: ContextPlanReadiness =
        serde_json::from_str(&packet.readiness_json).map_err(|error| {
            anyhow::anyhow!(
                "workspace context packet `{}` readiness is invalid: {error}",
                packet.id
            )
        })?;
    let readiness = normalize_readiness(readiness, packet.source_checkpoint_sha256.as_deref())?;
    if readiness.legacy {
        if packet.source_checkpoint_id.is_some() || packet.source_checkpoint_sha256.is_some() {
            anyhow::bail!(
                "workspace context packet `{}` has legacy readiness with Context Plan checkpoint metadata",
                packet.id
            );
        }
        return Ok(());
    }
    let checkpoint_id = packet.source_checkpoint_id.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "workspace context packet `{}` is missing its source checkpoint",
            packet.id
        )
    })?;
    let checkpoint_sha256 = packet.source_checkpoint_sha256.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "workspace context packet `{}` is missing its source checkpoint SHA-256",
            packet.id
        )
    })?;
    let input = crate::WorkspaceContextPacketCreate {
        client_id: packet.client_id.clone(),
        encounter_id: packet.encounter_id.clone(),
        note_id: packet.note_id.clone(),
        base_note_revision: packet.base_note_revision,
        ..Default::default()
    };
    validate_source_checkpoint(tx, &input, checkpoint_id, checkpoint_sha256).await?;
    let acknowledged = readiness
        .acknowledgements
        .iter()
        .map(|acknowledgement| acknowledgement.warning_code.as_str())
        .collect::<BTreeSet<_>>();
    let unacknowledged = readiness
        .warnings
        .iter()
        .filter(|warning| !acknowledged.contains(warning.code.as_str()))
        .map(|warning| warning.code.as_str())
        .collect::<Vec<_>>();
    if !unacknowledged.is_empty() {
        anyhow::bail!(
            "workspace context packet `{}` has unacknowledged readiness warnings: {}",
            packet.id,
            unacknowledged.join(", ")
        );
    }
    Ok(())
}

async fn validate_source_checkpoint(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    input: &crate::WorkspaceContextPacketCreate,
    checkpoint_id: &str,
    checkpoint_sha256: &str,
) -> anyhow::Result<()> {
    let row = sqlx::query(
        r#"
SELECT
    checkpoint.client_id,
    checkpoint.encounter_id,
    checkpoint.note_id,
    checkpoint.base_note_revision,
    checkpoint.schema_version,
    checkpoint.revision,
    checkpoint.content_sha256,
    session.current_revision
FROM workspace_draft_checkpoints AS checkpoint
JOIN workspace_draft_sessions AS session
  ON session.id = checkpoint.session_id
 AND session.client_id = checkpoint.client_id
WHERE checkpoint.id = ?
LIMIT 1
        "#,
    )
    .bind(checkpoint_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        anyhow::anyhow!("workspace context plan source checkpoint `{checkpoint_id}` was not found")
    })?;
    let client_id: String = row.try_get("client_id")?;
    let encounter_id: Option<String> = row.try_get("encounter_id")?;
    let note_id: Option<String> = row.try_get("note_id")?;
    let base_note_revision: Option<i64> = row.try_get("base_note_revision")?;
    let schema_version: i64 = row.try_get("schema_version")?;
    let revision: i64 = row.try_get("revision")?;
    let stored_sha256: String = row.try_get("content_sha256")?;
    let current_revision: i64 = row.try_get("current_revision")?;
    if client_id != input.client_id.trim()
        || encounter_id != normalized_optional(&input.encounter_id)
        || note_id != normalized_optional(&input.note_id)
        || base_note_revision != input.base_note_revision
    {
        anyhow::bail!(
            "workspace context plan source checkpoint `{checkpoint_id}` does not match the packet patient and note scope"
        );
    }
    if !matches!(schema_version, 1 | 2) {
        anyhow::bail!(
            "workspace context plan source checkpoint `{checkpoint_id}` uses unsupported schemaVersion {schema_version}"
        );
    }
    if stored_sha256 != checkpoint_sha256 {
        anyhow::bail!(
            "workspace context plan source checkpoint `{checkpoint_id}` SHA-256 does not match"
        );
    }
    if current_revision != revision {
        anyhow::bail!(
            "workspace context plan source checkpoint `{checkpoint_id}` is stale; the draft session has a newer checkpoint"
        );
    }
    Ok(())
}

fn normalize_readiness(
    mut readiness: ContextPlanReadiness,
    source_checkpoint_sha256: Option<&str>,
) -> anyhow::Result<ContextPlanReadiness> {
    if readiness.version != 1 {
        anyhow::bail!(
            "unsupported workspace context plan readiness version {}",
            readiness.version
        );
    }
    if readiness.warnings.len() > MAX_READINESS_ITEMS
        || readiness.acknowledgements.len() > MAX_READINESS_ITEMS
    {
        anyhow::bail!(
            "workspace context plan readiness exceeds the {MAX_READINESS_ITEMS} item limit"
        );
    }
    let mut warning_codes = BTreeSet::new();
    for warning in &mut readiness.warnings {
        warning.code = normalized_code("readiness warning", &warning.code)?;
        warning.message = bounded_text("readiness warning message", &warning.message)?;
        if !warning_codes.insert(warning.code.clone()) {
            anyhow::bail!("workspace context plan readiness warning codes must be unique");
        }
    }
    readiness
        .warnings
        .sort_by(|left, right| left.code.cmp(&right.code));
    let mut acknowledgement_codes = BTreeSet::new();
    for acknowledgement in &mut readiness.acknowledgements {
        acknowledgement.warning_code =
            normalized_code("acknowledgement warning", &acknowledgement.warning_code)?;
        if !warning_codes.contains(&acknowledgement.warning_code) {
            anyhow::bail!(
                "workspace context plan acknowledgement references unknown warning `{}`",
                acknowledgement.warning_code
            );
        }
        if !acknowledgement_codes.insert(acknowledgement.warning_code.clone()) {
            anyhow::bail!(
                "workspace context plan readiness warnings may each be acknowledged once"
            );
        }
        if !is_lower_hex_sha256(&acknowledgement.checkpoint_sha256) {
            anyhow::bail!(
                "workspace context plan acknowledgement checkpoint SHA-256 must be 64 lowercase hexadecimal characters"
            );
        }
        if source_checkpoint_sha256 != Some(acknowledgement.checkpoint_sha256.as_str()) {
            anyhow::bail!(
                "workspace context plan acknowledgement must bind the packet source checkpoint SHA-256"
            );
        }
        acknowledgement.reason = bounded_text("acknowledgement reason", &acknowledgement.reason)?;
    }
    readiness.acknowledgements.sort_by(|left, right| {
        left.warning_code
            .cmp(&right.warning_code)
            .then_with(|| left.reason.cmp(&right.reason))
    });
    Ok(readiness)
}

fn bind_metadata_to_context_envelope(
    context_envelope_json: &str,
    workspace_profile: &str,
    plan_schema_version: i64,
    source_checkpoint_id: Option<&str>,
    source_checkpoint_sha256: Option<&str>,
    readiness: &ContextPlanReadiness,
) -> anyhow::Result<String> {
    let mut envelope: serde_json::Value = serde_json::from_str(context_envelope_json.trim())
        .map_err(|error| {
            anyhow::anyhow!("workspace context packet envelope must be valid JSON: {error}")
        })?;
    let object = envelope.as_object_mut().ok_or_else(|| {
        anyhow::anyhow!("workspace context packet envelope must be a JSON object")
    })?;
    let source_checkpoint = match (source_checkpoint_id, source_checkpoint_sha256) {
        (Some(id), Some(content_sha256)) => serde_json::json!({
            "id": id,
            "contentSha256": content_sha256,
        }),
        (None, None) => serde_json::Value::Null,
        _ => unreachable!("source checkpoint identity was validated together"),
    };
    let expected_profile = serde_json::json!(workspace_profile);
    let expected_context_plan = serde_json::json!({
        "schemaVersion": plan_schema_version,
        "sourceCheckpoint": source_checkpoint,
        "readiness": readiness,
    });
    insert_or_verify_envelope_value(object, "workspaceProfile", expected_profile)?;
    insert_or_verify_envelope_value(object, "contextPlan", expected_context_plan)?;
    Ok(serde_json::to_string(&envelope)?)
}

fn insert_or_verify_envelope_value(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    expected: serde_json::Value,
) -> anyhow::Result<()> {
    if let Some(existing) = object.get(key)
        && existing != &expected
    {
        anyhow::bail!(
            "workspace context packet envelope {key} does not match normalized context plan metadata"
        );
    }
    object.insert(key.to_string(), expected);
    Ok(())
}

fn normalized_optional(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn normalized_code(label: &str, value: &str) -> anyhow::Result<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 64
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
    {
        anyhow::bail!(
            "workspace context plan {label} code must contain 1 to 64 lowercase letters, digits, underscores, or hyphens"
        );
    }
    Ok(value.to_string())
}

fn bounded_text(label: &str, value: &str) -> anyhow::Result<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > MAX_READINESS_TEXT_BYTES {
        anyhow::bail!(
            "workspace context plan {label} must contain 1 to {MAX_READINESS_TEXT_BYTES} bytes"
        );
    }
    Ok(value.to_string())
}

fn is_lower_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
