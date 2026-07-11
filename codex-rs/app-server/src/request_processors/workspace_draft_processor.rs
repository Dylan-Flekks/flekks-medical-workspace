use super::*;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_app_server_protocol::WorkspaceDraftCheckpoint;
use codex_app_server_protocol::WorkspaceDraftCheckpointCreateParams;
use codex_app_server_protocol::WorkspaceDraftCheckpointCreateResponse;
use codex_app_server_protocol::WorkspaceDraftCheckpointListParams;
use codex_app_server_protocol::WorkspaceDraftCheckpointListResponse;
use codex_app_server_protocol::WorkspaceDraftSession;
use codex_app_server_protocol::WorkspaceDraftSessionCloseParams;
use codex_app_server_protocol::WorkspaceDraftSessionCloseResponse;
use codex_app_server_protocol::WorkspaceDraftSessionCloseStatus;
use codex_app_server_protocol::WorkspaceDraftSessionListParams;
use codex_app_server_protocol::WorkspaceDraftSessionListResponse;
use codex_app_server_protocol::WorkspaceDraftSessionStatus;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;

const DEFAULT_DRAFT_PAGE_LIMIT: u32 = 50;
const MAX_DRAFT_PAGE_LIMIT: u32 = 100;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DraftSessionCursor {
    updated_at_ms: i64,
    id: String,
}

impl WorkspaceRequestProcessor {
    pub(crate) async fn draft_checkpoint_create(
        &self,
        params: WorkspaceDraftCheckpointCreateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let client_id = required_draft_text("clientId", params.client_id)?;
        let session_id = params
            .session_id
            .map(|value| required_draft_text("sessionId", value))
            .transpose()?;
        let trigger = required_draft_text("trigger", params.trigger)?;
        let actor = required_draft_text("actor", params.actor)?;
        if params
            .base_note_revision
            .is_some_and(|revision| revision < 0)
        {
            return Err(invalid_request(
                "workspace draft baseNoteRevision must not be negative",
            ));
        }
        if !params.draft.is_object() {
            return Err(invalid_request("workspace draft must be a JSON object"));
        }
        if params
            .draft
            .get("schemaVersion")
            .and_then(serde_json::Value::as_i64)
            != Some(1)
        {
            return Err(invalid_request("workspace draft schemaVersion must be 1"));
        }
        let draft_json = serde_json::to_string(&params.draft)
            .map_err(|error| invalid_request(format!("workspace draft is invalid: {error}")))?;
        let checkpoint = self
            .state_db()?
            .workspace()
            .create_draft_checkpoint(codex_state::WorkspaceDraftCheckpointCreate {
                session_id,
                client_id,
                encounter_id: empty_to_none(params.encounter_id),
                note_id: empty_to_none(params.note_id),
                base_note_revision: params.base_note_revision,
                draft_json,
                trigger,
                actor,
            })
            .await
            .map_err(|error| draft_operation_error("create checkpoint", error))?;
        let replayed = checkpoint.replayed;
        Ok(Some(
            WorkspaceDraftCheckpointCreateResponse {
                checkpoint: api_draft_checkpoint(checkpoint)?,
                replayed,
            }
            .into(),
        ))
    }

    pub(crate) async fn draft_checkpoint_list(
        &self,
        params: WorkspaceDraftCheckpointListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let client_id = required_draft_text("clientId", params.client_id)?;
        let session_id = required_draft_text("sessionId", params.session_id)?;
        let cursor_before_revision = params
            .cursor
            .map(|cursor| {
                let revision: i64 = decode_draft_cursor("checkpoint", &cursor)?;
                if revision < 1 {
                    return Err(invalid_request(
                        "workspace draft checkpoint cursor must be positive",
                    ));
                }
                Ok(revision)
            })
            .transpose()?;
        let limit = draft_page_limit(params.limit);
        let mut checkpoints = self
            .state_db()?
            .workspace()
            .list_draft_checkpoints(codex_state::WorkspaceDraftCheckpointFilter {
                client_id,
                session_id: Some(session_id),
                cursor_before_revision,
                limit: Some(limit + 1),
            })
            .await
            .map_err(|error| {
                internal_error(format!(
                    "failed to list workspace draft checkpoints: {error}"
                ))
            })?;
        let has_more = checkpoints.len() > limit as usize;
        checkpoints.truncate(limit as usize);
        let next_cursor = if has_more {
            let last = checkpoints.last().ok_or_else(|| {
                internal_error("workspace draft checkpoint page ended unexpectedly")
            })?;
            Some(encode_draft_cursor("checkpoint", &last.revision)?)
        } else {
            None
        };
        let data = checkpoints
            .into_iter()
            .map(api_draft_checkpoint)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(
            WorkspaceDraftCheckpointListResponse { data, next_cursor }.into(),
        ))
    }

    pub(crate) async fn draft_session_list(
        &self,
        params: WorkspaceDraftSessionListParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let client_id = required_draft_text("clientId", params.client_id)?;
        let cursor = params
            .cursor
            .map(|cursor| {
                let cursor: DraftSessionCursor = decode_draft_cursor("session", &cursor)?;
                if cursor.id.trim().is_empty() {
                    return Err(invalid_request(
                        "workspace draft session cursor id must not be empty",
                    ));
                }
                Ok(cursor)
            })
            .transpose()?;
        let limit = draft_page_limit(params.limit);
        let mut sessions = self
            .state_db()?
            .workspace()
            .list_draft_sessions(codex_state::WorkspaceDraftSessionFilter {
                client_id,
                include_closed: params.include_closed,
                cursor_updated_at_ms: cursor.as_ref().map(|cursor| cursor.updated_at_ms),
                cursor_id: cursor.map(|cursor| cursor.id),
                limit: Some(limit + 1),
            })
            .await
            .map_err(|error| {
                internal_error(format!("failed to list workspace draft sessions: {error}"))
            })?;
        let has_more = sessions.len() > limit as usize;
        sessions.truncate(limit as usize);
        let next_cursor = if has_more {
            let session = &sessions
                .last()
                .ok_or_else(|| internal_error("workspace draft session page ended unexpectedly"))?
                .session;
            Some(encode_draft_cursor(
                "session",
                &DraftSessionCursor {
                    updated_at_ms: session.updated_at.timestamp_millis(),
                    id: session.id.clone(),
                },
            )?)
        } else {
            None
        };
        let data = sessions
            .into_iter()
            .map(api_draft_session)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(
            WorkspaceDraftSessionListResponse { data, next_cursor }.into(),
        ))
    }

    pub(crate) async fn draft_session_close(
        &self,
        params: WorkspaceDraftSessionCloseParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let session_id = required_draft_text("sessionId", params.session_id)?;
        let client_id = required_draft_text("clientId", params.client_id)?;
        let actor = required_draft_text("actor", params.actor)?;
        let reason = required_draft_text("reason", params.reason)?;
        let status = match params.status {
            WorkspaceDraftSessionCloseStatus::Closed => {
                codex_state::WorkspaceDraftSessionTerminalStatus::Closed
            }
            WorkspaceDraftSessionCloseStatus::Discarded => {
                codex_state::WorkspaceDraftSessionTerminalStatus::Discarded
            }
        };
        let session = self
            .state_db()?
            .workspace()
            .close_draft_session(codex_state::WorkspaceDraftSessionClose {
                session_id,
                client_id,
                status,
                expected_current_checkpoint_id: params.expected_current_checkpoint_id,
                expected_current_checkpoint_revision: params.expected_current_checkpoint_revision,
                expected_current_checkpoint_sha256: params.expected_current_checkpoint_sha256,
                actor,
                reason,
            })
            .await
            .map_err(|error| draft_operation_error("close session", error))?;
        Ok(Some(
            WorkspaceDraftSessionCloseResponse {
                session: api_draft_session(session)?,
            }
            .into(),
        ))
    }
}

fn api_draft_session(
    value: codex_state::WorkspaceDraftSessionSnapshot,
) -> Result<WorkspaceDraftSession, JSONRPCErrorError> {
    let status = match value.session.status.as_str() {
        "active" => WorkspaceDraftSessionStatus::Active,
        "closed" => WorkspaceDraftSessionStatus::Closed,
        "discarded" => WorkspaceDraftSessionStatus::Discarded,
        status => {
            return Err(internal_error(format!(
                "unsupported workspace draft session status `{status}`"
            )));
        }
    };
    Ok(WorkspaceDraftSession {
        id: value.session.id,
        client_id: value.session.client_id,
        status,
        current_revision: value.session.current_revision,
        current_checkpoint: api_draft_checkpoint(value.current_checkpoint)?,
        created_by: value.session.created_by,
        created_at: value.session.created_at.timestamp(),
        updated_at: value.session.updated_at.timestamp(),
        closed_at: value.session.closed_at.map(|value| value.timestamp()),
    })
}

fn api_draft_checkpoint(
    value: codex_state::WorkspaceDraftCheckpoint,
) -> Result<WorkspaceDraftCheckpoint, JSONRPCErrorError> {
    let draft: serde_json::Value = serde_json::from_str(&value.draft_json).map_err(|error| {
        internal_error(format!(
            "stored workspace draft checkpoint `{}` is invalid: {error}",
            value.id
        ))
    })?;
    if !draft.is_object() {
        return Err(internal_error(format!(
            "stored workspace draft checkpoint `{}` is not a JSON object",
            value.id
        )));
    }
    Ok(WorkspaceDraftCheckpoint {
        id: value.id,
        session_id: value.session_id,
        client_id: value.client_id,
        encounter_id: value.encounter_id,
        note_id: value.note_id,
        base_note_revision: value.base_note_revision,
        schema_version: value.schema_version,
        revision: value.revision,
        draft,
        content_sha256: value.content_sha256,
        trigger: value.trigger,
        actor: value.actor,
        created_at: value.created_at.timestamp(),
    })
}

fn required_draft_text(label: &str, value: String) -> Result<String, JSONRPCErrorError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(invalid_request(format!(
            "workspace draft {label} must not be empty"
        )));
    }
    Ok(value.to_string())
}

fn draft_page_limit(limit: Option<u32>) -> u32 {
    limit
        .unwrap_or(DEFAULT_DRAFT_PAGE_LIMIT)
        .clamp(1, MAX_DRAFT_PAGE_LIMIT)
}

fn encode_draft_cursor(
    resource: &str,
    value: &impl Serialize,
) -> Result<String, JSONRPCErrorError> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        internal_error(format!(
            "failed to encode workspace draft {resource} cursor: {error}"
        ))
    })?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn decode_draft_cursor<T: DeserializeOwned>(
    resource: &str,
    cursor: &str,
) -> Result<T, JSONRPCErrorError> {
    let bytes = URL_SAFE_NO_PAD.decode(cursor).map_err(|error| {
        invalid_request(format!(
            "invalid workspace draft {resource} cursor: {error}"
        ))
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        invalid_request(format!(
            "invalid workspace draft {resource} cursor: {error}"
        ))
    })
}

fn draft_operation_error(
    operation: &str,
    error: codex_state::WorkspaceDraftError,
) -> JSONRPCErrorError {
    match error {
        codex_state::WorkspaceDraftError::Validation { message } => invalid_request(message),
        codex_state::WorkspaceDraftError::Storage { message } => {
            internal_error(format!("failed to {operation}: {message}"))
        }
    }
}
