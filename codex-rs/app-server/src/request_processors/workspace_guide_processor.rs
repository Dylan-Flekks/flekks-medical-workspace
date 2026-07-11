use super::*;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_app_server_protocol::WorkspaceGuideRun;
use codex_app_server_protocol::WorkspaceGuideRunFinishOutcome;
use codex_app_server_protocol::WorkspaceGuideRunFinishParams;
use codex_app_server_protocol::WorkspaceGuideRunFinishResponse;
use codex_app_server_protocol::WorkspaceGuideRunListParams;
use codex_app_server_protocol::WorkspaceGuideRunListResponse;
use codex_app_server_protocol::WorkspaceGuideRunStartParams;
use codex_app_server_protocol::WorkspaceGuideRunStartResponse;
use codex_app_server_protocol::WorkspaceGuideRunStatus;
use codex_protocol::config_types::ModelToolMode;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

const DEFAULT_GUIDE_PAGE_LIMIT: u32 = 50;
const MAX_GUIDE_PAGE_LIMIT: u32 = 100;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GuideRunCursor {
    created_at_ms: i64,
    id: String,
}

impl WorkspaceRequestProcessor {
    pub(crate) async fn guide_run_start(
        &self,
        params: WorkspaceGuideRunStartParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let request_json = serde_json::to_string(&params.request).map_err(|error| {
            guide_validation_error(format!("workspace guide request is invalid: {error}"))
        })?;
        let run = self
            .state_db()?
            .workspace()
            .start_guide_run(codex_state::WorkspaceGuideRunStart {
                client_id: params.client_id,
                session_id: params.session_id,
                source_checkpoint_id: params.source_checkpoint_id,
                source_checkpoint_revision: params.source_checkpoint_revision,
                source_checkpoint_sha256: params.source_checkpoint_sha256,
                request_json,
                idempotency_key: params.idempotency_key,
                trigger: params.trigger,
                actor: params.actor,
                provider: params.provider,
                model: params.model,
            })
            .await
            .map_err(|error| guide_operation_error("start workspace guide run", error))?;
        let replayed = run.replayed;
        Ok(Some(
            WorkspaceGuideRunStartResponse {
                run: api_guide_run(run)?,
                replayed,
            }
            .into(),
        ))
    }

    pub(crate) async fn guide_run_finish(
        &self,
        params: WorkspaceGuideRunFinishParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let outcome = match params.outcome {
            WorkspaceGuideRunFinishOutcome::Completed { result } => {
                let result_json = serde_json::to_string(&result).map_err(|error| {
                    guide_validation_error(format!(
                        "workspace guide completion result is invalid: {error}"
                    ))
                })?;
                codex_state::WorkspaceGuideRunOutcome::Completed { result_json }
            }
            WorkspaceGuideRunFinishOutcome::Failed { error_summary } => {
                codex_state::WorkspaceGuideRunOutcome::Failed { error_summary }
            }
            WorkspaceGuideRunFinishOutcome::Canceled { reason } => {
                codex_state::WorkspaceGuideRunOutcome::Canceled { reason }
            }
        };
        let run = self
            .state_db()?
            .workspace()
            .finish_guide_run(codex_state::WorkspaceGuideRunFinish {
                run_id: params.run_id,
                client_id: params.client_id,
                session_id: params.session_id,
                source_checkpoint_id: params.source_checkpoint_id,
                source_checkpoint_revision: params.source_checkpoint_revision,
                source_checkpoint_sha256: params.source_checkpoint_sha256,
                request_envelope_sha256: params.request_envelope_sha256,
                source_thread_id: empty_to_none(params.source_thread_id),
                source_turn_id: empty_to_none(params.source_turn_id),
                outcome,
                actor: params.actor,
            })
            .await
            .map_err(|error| guide_operation_error("finish workspace guide run", error))?;
        let replayed = run.replayed;
        Ok(Some(
            WorkspaceGuideRunFinishResponse {
                run: api_guide_run(run)?,
                replayed,
            }
            .into(),
        ))
    }

    pub(crate) async fn guide_run_list(
        &self,
        params: WorkspaceGuideRunListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let client_id = required_guide_text("clientId", params.client_id)?;
        let session_id = params
            .session_id
            .map(|value| required_guide_text("sessionId", value))
            .transpose()?;
        let cursor = params
            .cursor
            .map(|cursor| {
                let bytes = URL_SAFE_NO_PAD.decode(&cursor).map_err(|error| {
                    guide_validation_error(format!("invalid workspace guide run cursor: {error}"))
                })?;
                let cursor: GuideRunCursor = serde_json::from_slice(&bytes).map_err(|error| {
                    guide_validation_error(format!("invalid workspace guide run cursor: {error}"))
                })?;
                if cursor.id.trim().is_empty() {
                    return Err(guide_validation_error(
                        "workspace guide run cursor id must not be empty",
                    ));
                }
                if cursor.created_at_ms < 0 {
                    return Err(guide_validation_error(
                        "workspace guide run cursor created time must not be negative",
                    ));
                }
                Ok(cursor)
            })
            .transpose()?;
        let limit = params
            .limit
            .unwrap_or(DEFAULT_GUIDE_PAGE_LIMIT)
            .clamp(1, MAX_GUIDE_PAGE_LIMIT);
        let mut runs = self
            .state_db()?
            .workspace()
            .list_guide_runs(codex_state::WorkspaceGuideRunFilter {
                client_id,
                session_id,
                cursor_created_at_ms: cursor.as_ref().map(|cursor| cursor.created_at_ms),
                cursor_id: cursor.map(|cursor| cursor.id),
                limit: Some(limit + 1),
            })
            .await
            .map_err(|error| guide_operation_error("list workspace guide runs", error))?;
        let has_more = runs.len() > limit as usize;
        runs.truncate(limit as usize);
        let next_cursor = if has_more {
            let last = runs.last().ok_or_else(|| {
                guide_internal_error("workspace guide run page ended unexpectedly")
            })?;
            let cursor = GuideRunCursor {
                created_at_ms: last.created_at.timestamp_millis(),
                id: last.id.clone(),
            };
            let bytes = serde_json::to_vec(&cursor).map_err(|error| {
                guide_internal_error(format!(
                    "failed to encode workspace guide run cursor: {error}"
                ))
            })?;
            Some(URL_SAFE_NO_PAD.encode(bytes))
        } else {
            None
        };
        let data = runs
            .into_iter()
            .map(api_guide_run)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(
            WorkspaceGuideRunListResponse { data, next_cursor }.into(),
        ))
    }
}

fn api_guide_run(
    value: codex_state::WorkspaceGuideRun,
) -> Result<WorkspaceGuideRun, JSONRPCErrorError> {
    value.verify_envelope_integrity().map_err(|error| {
        guide_internal_error(format!(
            "workspace guide run `{}` failed envelope integrity verification: {error}",
            value.id
        ))
    })?;
    let request_envelope = stored_guide_envelope(
        &value.id,
        "request",
        Some(value.request_envelope_json.as_str()),
    )?
    .ok_or_else(|| guide_internal_error("workspace guide request envelope was missing"))?;
    let terminal_envelope = stored_guide_envelope(
        &value.id,
        "terminal",
        value.terminal_envelope_json.as_deref(),
    )?;
    let model_tool_mode = match value.model_tool_mode.as_str() {
        "disabled" => ModelToolMode::Disabled,
        mode => {
            return Err(guide_internal_error(format!(
                "workspace guide run `{}` stored unsupported model tool mode `{mode}`",
                value.id
            )));
        }
    };
    let status = match value.status {
        codex_state::WorkspaceGuideRunStatus::Running => WorkspaceGuideRunStatus::Running,
        codex_state::WorkspaceGuideRunStatus::Completed => WorkspaceGuideRunStatus::Completed,
        codex_state::WorkspaceGuideRunStatus::Failed => WorkspaceGuideRunStatus::Failed,
        codex_state::WorkspaceGuideRunStatus::Canceled => WorkspaceGuideRunStatus::Canceled,
    };
    Ok(WorkspaceGuideRun {
        id: value.id,
        client_id: value.client_id,
        session_id: value.session_id,
        source_checkpoint_id: value.source_checkpoint_id,
        source_checkpoint_revision: value.source_checkpoint_revision,
        source_checkpoint_sha256: value.source_checkpoint_sha256,
        encounter_id: value.encounter_id,
        note_id: value.note_id,
        request_schema_version: value.request_schema_version,
        request_envelope,
        request_envelope_sha256: value.request_envelope_sha256,
        idempotency_key: value.idempotency_key,
        trigger: value.trigger,
        actor: value.actor,
        provider: value.provider,
        model: value.model,
        model_tool_mode,
        status,
        source_thread_id: value.source_thread_id,
        source_turn_id: value.source_turn_id,
        terminal_envelope,
        terminal_envelope_sha256: value.terminal_envelope_sha256,
        created_at: value.created_at.timestamp(),
        updated_at: value.updated_at.timestamp(),
        terminal_at: value.terminal_at.map(|value| value.timestamp()),
        is_stale: value.is_stale,
    })
}

fn stored_guide_envelope(
    run_id: &str,
    label: &str,
    value: Option<&str>,
) -> Result<Option<serde_json::Value>, JSONRPCErrorError> {
    match value {
        Some(value) => {
            let envelope: serde_json::Value = serde_json::from_str(value).map_err(|error| {
                guide_internal_error(format!(
                    "workspace guide run `{run_id}` stored invalid {label} envelope: {error}"
                ))
            })?;
            if !envelope.is_object() {
                return Err(guide_internal_error(format!(
                    "workspace guide run `{run_id}` stored non-object {label} envelope"
                )));
            }
            Ok(Some(envelope))
        }
        None => Ok(None),
    }
}

fn required_guide_text(label: &str, value: String) -> Result<String, JSONRPCErrorError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(guide_validation_error(format!(
            "workspace guide {label} must not be empty"
        )));
    }
    Ok(value.to_string())
}

fn guide_operation_error(
    operation: &str,
    error: codex_state::WorkspaceGuideError,
) -> JSONRPCErrorError {
    let kind = error.kind();
    let message = error.to_string();
    match &error {
        codex_state::WorkspaceGuideError::Validation { .. }
        | codex_state::WorkspaceGuideError::StaleCheckpoint { .. }
        | codex_state::WorkspaceGuideError::IdempotencyConflict { .. }
        | codex_state::WorkspaceGuideError::ActiveRunConflict { .. }
        | codex_state::WorkspaceGuideError::TerminalConflict { .. } => {}
        codex_state::WorkspaceGuideError::Storage { .. } => {
            return internal_error(format!("failed to {operation}: {message}"));
        }
    }
    let mut rpc_error = invalid_request(message);
    rpc_error.data = Some(json!({"kind": kind}));
    rpc_error
}

fn guide_validation_error(message: impl Into<String>) -> JSONRPCErrorError {
    let mut error = invalid_request(message);
    error.data = Some(json!({"kind": "validation"}));
    error
}

fn guide_internal_error(message: impl Into<String>) -> JSONRPCErrorError {
    internal_error(message)
}
