use std::borrow::Cow;
use std::sync::Arc;

use codex_protocol::config_types::ModelToolMode;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::UserInput;
use codex_tools::ToolExecutor;

use crate::function_tool::FunctionCallError;
use crate::session::turn_context::TurnContext;
use crate::tools::ToolRouter;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::WorkspaceContextReadHandler;
use crate::tools::handlers::workspace_context_spec::WORKSPACE_CONTEXT_READ_TOOL_NAME;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolRegistry;

/// Builds the complete runtime and model-visible tool surface for a restricted medical-context
/// turn. Keep this independent of the normal tool planner so newly added core, MCP, extension,
/// plugin, or dynamic tools cannot enter the restricted surface by default.
#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkspaceContextRunBinding(codex_state::WorkspaceAgentExecutionBinding);

const REDACTED_TOOL_LOG_PAYLOAD: &str = r#"{"payload_redacted":true}"#;

/// Prevent the immutable medical-run capability and any clinical identifiers in tool arguments
/// from entering generic logs or telemetry. The complete payload still reaches the bound handler
/// and the model-visible function output remains available inside the restricted turn.
pub(crate) fn tool_log_payload(mode: ModelToolMode, payload: &ToolPayload) -> Cow<'_, str> {
    if mode == ModelToolMode::WorkspaceContextOnly {
        Cow::Borrowed(REDACTED_TOOL_LOG_PAYLOAD)
    } else {
        payload.log_payload()
    }
}

/// Split a restricted handler error into an observability-safe error and the original error that
/// must still be returned to the model. Normal turns keep their existing error path unchanged.
pub(crate) fn tool_error_for_logging(
    mode: ModelToolMode,
    error: FunctionCallError,
) -> (FunctionCallError, Option<FunctionCallError>) {
    if mode == ModelToolMode::WorkspaceContextOnly {
        (
            FunctionCallError::RespondToModel(
                "workspace_context_read failed; clinical and run details redacted".to_string(),
            ),
            Some(error),
        )
    } else {
        (error, None)
    }
}

pub(crate) async fn claim_run(
    state_db: crate::StateDbHandle,
    turn_context: &TurnContext,
    thread_id: String,
    turn_id: String,
    run_id: String,
    items: &[UserInput],
) -> CodexResult<codex_state::WorkspaceAgentExecutionBinding> {
    if turn_context.model_tool_mode != ModelToolMode::WorkspaceContextOnly {
        return Err(CodexErr::InvalidRequest(
            "workspace context run binding requires workspaceContextOnly mode".to_string(),
        ));
    }
    let [
        UserInput::Text {
            text,
            text_elements,
        },
    ] = items
    else {
        return Err(CodexErr::InvalidRequest(
            "workspaceContextOnly requires exactly one plain-text generated handoff prompt"
                .to_string(),
        ));
    };
    if text.trim().is_empty() || !text_elements.is_empty() {
        return Err(CodexErr::InvalidRequest(
            "workspaceContextOnly requires exactly one plain-text generated handoff prompt"
                .to_string(),
        ));
    }
    state_db
        .workspace()
        .claim_agent_turn(codex_state::WorkspaceAgentTurnClaim {
            execution: codex_state::WorkspaceAgentExecutionBinding {
                run_id,
                source_thread_id: thread_id,
                source_turn_id: turn_id,
                provider: turn_context.config.model_provider_id.clone(),
                model: turn_context.model_info.slug.clone(),
            },
            prompt: text.clone(),
        })
        .await
        .map_err(|_| {
            CodexErr::InvalidRequest(
                "workspaceContextOnly turn does not match its stored run contract".to_string(),
            )
        })
}

pub(crate) fn bind_run(
    turn_context: &TurnContext,
    execution: codex_state::WorkspaceAgentExecutionBinding,
) -> CodexResult<()> {
    if turn_context
        .extension_data
        .insert(WorkspaceContextRunBinding(execution))
        .is_some()
    {
        return Err(CodexErr::InvalidRequest(
            "workspace context run binding was already set for this turn".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn build_router(turn_context: &TurnContext) -> CodexResult<Arc<ToolRouter>> {
    let execution = turn_context
        .extension_data
        .get::<WorkspaceContextRunBinding>()
        .map(|binding| binding.0.clone())
        .ok_or_else(|| {
            CodexErr::InvalidRequest(
                "workspaceContextOnly turn is missing its immutable run binding".to_string(),
            )
        })?;
    let handler = Arc::new(WorkspaceContextReadHandler::bound_to_execution(execution));
    let spec = handler.spec();
    let registry = ToolRegistry::from_tools([handler as Arc<dyn CoreToolRuntime>]);
    Ok(Arc::new(ToolRouter::from_parts(registry, vec![spec])))
}

pub(crate) fn filter_prompt_input(
    mode: ModelToolMode,
    turn_id: &str,
    input: Vec<ResponseItem>,
) -> Vec<ResponseItem> {
    if mode != ModelToolMode::WorkspaceContextOnly {
        return input;
    }
    input
        .into_iter()
        .filter(|item| item.turn_id() == Some(turn_id))
        .collect()
}

pub(crate) fn parse_run_id(items: &[UserInput]) -> CodexResult<String> {
    let run_ids = items
        .iter()
        .filter_map(|item| match item {
            UserInput::Text { text, .. } => Some(text.as_str()),
            UserInput::Image { .. }
            | UserInput::LocalImage { .. }
            | UserInput::Skill { .. }
            | UserInput::Mention { .. } => None,
            _ => None,
        })
        .flat_map(str::lines)
        .filter_map(|line| line.trim().strip_prefix("- run_id:"))
        .map(str::trim)
        .filter(|run_id| !run_id.is_empty())
        .collect::<Vec<_>>();
    let [run_id] = run_ids.as_slice() else {
        return Err(CodexErr::InvalidRequest(
            "workspaceContextOnly requires exactly one non-empty `- run_id: ...` line in the submitted text input"
                .to_string(),
        ));
    };
    Ok((*run_id).to_string())
}

pub(crate) fn validate_model_output(mode: ModelToolMode, item: &ResponseItem) -> CodexResult<()> {
    let is_tool_like = matches!(
        item,
        ResponseItem::AdditionalTools { .. }
            | ResponseItem::LocalShellCall { .. }
            | ResponseItem::FunctionCall { .. }
            | ResponseItem::ToolSearchCall { .. }
            | ResponseItem::FunctionCallOutput { .. }
            | ResponseItem::CustomToolCall { .. }
            | ResponseItem::CustomToolCallOutput { .. }
            | ResponseItem::ToolSearchOutput { .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::ImageGenerationCall { .. }
            | ResponseItem::CompactionTrigger {}
            | ResponseItem::Other
    );

    match mode {
        ModelToolMode::Default => Ok(()),
        ModelToolMode::Disabled if is_tool_like => Err(CodexErr::InvalidRequest(
            "model returned a tool item while model tool mode is disabled".to_string(),
        )),
        ModelToolMode::Disabled => Ok(()),
        ModelToolMode::WorkspaceContextOnly => match item {
            ResponseItem::FunctionCall {
                name, namespace, ..
            } if name == WORKSPACE_CONTEXT_READ_TOOL_NAME && namespace.is_none() => Ok(()),
            ResponseItem::Message { role, .. } if role == "assistant" => Ok(()),
            ResponseItem::Reasoning { .. } => Ok(()),
            ResponseItem::AdditionalTools { .. }
            | ResponseItem::Message { .. }
            | ResponseItem::AgentMessage { .. }
            | ResponseItem::LocalShellCall { .. }
            | ResponseItem::FunctionCall { .. }
            | ResponseItem::ToolSearchCall { .. }
            | ResponseItem::FunctionCallOutput { .. }
            | ResponseItem::CustomToolCall { .. }
            | ResponseItem::CustomToolCallOutput { .. }
            | ResponseItem::ToolSearchOutput { .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::ImageGenerationCall { .. }
            | ResponseItem::Compaction { .. }
            | ResponseItem::ContextCompaction { .. }
            | ResponseItem::CompactionTrigger {}
            | ResponseItem::Other => Err(CodexErr::InvalidRequest(format!(
                "model returned a disallowed output item while model tool mode is workspaceContextOnly; only assistant messages, reasoning, and the non-namespaced {WORKSPACE_CONTEXT_READ_TOOL_NAME} function tool are allowed"
            ))),
        },
    }
}

#[cfg(test)]
#[path = "workspace_context_only_tests.rs"]
mod tests;
