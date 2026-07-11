use super::super::workspace::validate_agent_visible_json;
use super::errors::GuideResult;
use super::errors::required;
use super::errors::validation;
use serde_json::Value;
use sha2::Digest;
use sha2::Sha256;

pub(super) const GUIDE_SCHEMA_VERSION: i64 = 1;
pub(super) const MAX_REQUEST_BYTES: usize = 32 * 1024;
pub(super) const MAX_TERMINAL_BYTES: usize = 16 * 1024;

pub(super) fn request_envelope(
    run_id: &str,
    input: &crate::WorkspaceGuideRunStart,
    request: Value,
) -> GuideResult<(String, String)> {
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

pub(super) fn terminal_envelope(
    outcome: &crate::WorkspaceGuideRunOutcome,
) -> GuideResult<(crate::WorkspaceGuideRunStatus, String, String)> {
    let (status, value) = match outcome {
        crate::WorkspaceGuideRunOutcome::Completed { result_json } => {
            let result: Value = serde_json::from_str(result_json.trim()).map_err(|error| {
                crate::WorkspaceGuideError::Validation {
                    message: format!("workspace guide result must be valid JSON: {error}"),
                }
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
) -> GuideResult<(String, String)> {
    validate_agent_visible_json(&format!("guide {label} envelope"), &value).map_err(|error| {
        crate::WorkspaceGuideError::Validation {
            message: error.to_string(),
        }
    })?;
    let json = serde_json::to_string(&value)?;
    if json.len() > max_bytes {
        return validation(format!(
            "workspace guide {label} envelope exceeds the {max_bytes} byte limit"
        ));
    }
    let hash = format!("{:x}", Sha256::digest(json.as_bytes()));
    Ok((json, hash))
}
