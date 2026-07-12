//! Fail-closed validation for the bounded isolated-turn contract.
//!
//! This module intentionally does not make session boot or model context
//! sterile. Callers must not use `ModelToolMode::Isolated` for Workspace Guide
//! execution until those follow-up isolation guarantees are implemented.

mod schema;

use codex_extension_api::ExtensionDataInit;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::config_types::ModelToolMode;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::ContentItem;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadSource;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::user_input::UserInput;
use serde_json::Value;

use super::session::Session;
use crate::config::Config;
use crate::environment_selection::TurnEnvironmentSnapshot;

const MAX_ISOLATED_INPUT_BYTES: usize = 32 * 1024;
const MAX_ISOLATED_OUTPUT_BYTES: usize = 16 * 1024;
const MAX_ISOLATED_OUTPUT_SCHEMA_BYTES: usize = 16 * 1024;
const MAX_ISOLATED_OUTPUT_SCHEMA_DEPTH: usize = 32;

pub(super) struct IsolatedThreadCreation<'a> {
    pub(super) config: &'a Config,
    pub(super) initial_history: &'a InitialHistory,
    pub(super) session_source: &'a SessionSource,
    pub(super) forked_from_thread_id_present: bool,
    pub(super) parent_thread_id_present: bool,
    pub(super) thread_source: Option<&'a ThreadSource>,
    pub(super) dynamic_tools: &'a [DynamicToolSpec],
    pub(super) inherited_environments: Option<&'a TurnEnvironmentSnapshot>,
    pub(super) environment_selections: &'a [TurnEnvironmentSelection],
    pub(super) thread_extension_init: &'a ExtensionDataInit,
}

#[derive(Clone, Copy)]
pub(super) enum ModelOutputStage {
    Added,
    Completed,
}

#[derive(Default)]
pub(super) struct IsolatedOutputState {
    pending_assistant: Option<ResponseItem>,
}

pub(super) fn initial_model_tool_mode(
    initial_history: &InitialHistory,
    thread_extension_init: &ExtensionDataInit,
) -> ModelToolMode {
    thread_extension_init
        .get::<ModelToolMode>()
        .map(|mode| *mode)
        .or_else(|| initial_history.get_latest_model_tool_mode())
        .unwrap_or_default()
}

pub(super) fn validate_thread_creation(
    mode: ModelToolMode,
    input: IsolatedThreadCreation<'_>,
) -> CodexResult<()> {
    if let Some(previous_mode) = input.initial_history.get_latest_model_tool_mode() {
        validate_mode_transition(previous_mode, mode).map_err(CodexErr::InvalidRequest)?;
    }
    if !mode.is_isolated() {
        return Ok(());
    }

    if !input.config.ephemeral {
        return invalid_creation("the thread must be ephemeral");
    }
    if !matches!(input.initial_history, InitialHistory::New) {
        return invalid_creation("the thread must use fresh history");
    }
    if input.session_source.is_non_root_agent()
        || matches!(input.thread_source, Some(ThreadSource::Subagent))
    {
        return invalid_creation("subagent and internal session sources are not allowed");
    }
    if input.forked_from_thread_id_present || input.parent_thread_id_present {
        return invalid_creation("forked and parent threads are not allowed");
    }
    if !input.dynamic_tools.is_empty() {
        return invalid_creation("dynamic tools must be empty");
    }
    if input.inherited_environments.is_some() || !input.environment_selections.is_empty() {
        return invalid_creation("execution environments must be empty");
    }
    if !input.config.effective_workspace_roots().is_empty() {
        return invalid_creation("workspace roots must be empty");
    }
    if input
        .thread_extension_init
        .get::<Vec<SelectedCapabilityRoot>>()
        .is_some_and(|roots| !roots.is_empty())
    {
        return invalid_creation("selected capability roots must be empty");
    }

    Ok(())
}

pub(super) fn validate_mode_transition(
    current: ModelToolMode,
    requested: ModelToolMode,
) -> Result<(), String> {
    if current != requested && (current.is_isolated() || requested.is_isolated()) {
        return Err(
            "isolated model mode is immutable and can only be selected at thread creation"
                .to_string(),
        );
    }
    Ok(())
}

pub(super) async fn validate_submission(sess: &Session, op: &Op) -> Result<(), String> {
    if !sess.model_tool_mode_is_isolated().await {
        return Ok(());
    }

    match op {
        Op::Interrupt | Op::Shutdown => Ok(()),
        Op::UserInput {
            items,
            final_output_json_schema,
            responsesapi_client_metadata,
            additional_context,
            thread_settings,
        } => {
            if sess
                .active_turn
                .lock()
                .await
                .as_ref()
                .and_then(|turn| turn.task.as_ref())
                .is_some()
                || !sess.clone_history().await.raw_items().is_empty()
            {
                return Err("isolated model mode permits exactly one turn".to_string());
            }
            if !additional_context.is_empty() {
                return Err("isolated model mode does not accept additional context".to_string());
            }
            if responsesapi_client_metadata.is_some() {
                return Err(
                    "isolated model mode does not accept Responses API client metadata".to_string(),
                );
            }
            if thread_settings != &Default::default() {
                return Err(
                    "isolated model mode does not accept turn setting overrides".to_string()
                );
            }
            validate_single_text_input(items)?;
            let schema = final_output_json_schema.as_ref().ok_or_else(|| {
                "isolated model mode requires a strict object output schema".to_string()
            })?;
            schema::validate_strict_object_schema(schema)
        }
        _ => Err("submission is unavailable in isolated model mode".to_string()),
    }
}

pub(super) fn validate_model_output(
    mode: ModelToolMode,
    item: &ResponseItem,
    stage: ModelOutputStage,
    output_schema: Option<&Value>,
    state: &mut IsolatedOutputState,
) -> CodexResult<()> {
    if mode.is_isolated() {
        return validate_isolated_model_output(item, stage, output_schema, state);
    }
    if !mode.tools_disabled() {
        return Ok(());
    }

    let tool_like = matches!(
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
    if tool_like {
        return Err(CodexErr::InvalidRequest(
            "model returned a tool item while model tool mode is disabled".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn take_validated_assistant_output(
    mode: ModelToolMode,
    state: &mut IsolatedOutputState,
    end_turn: Option<bool>,
) -> CodexResult<Option<ResponseItem>> {
    if !mode.is_isolated() {
        return Ok(None);
    }
    if end_turn == Some(false) {
        return invalid_isolated_output(
            "model requested a follow-up response while model tool mode is isolated",
        );
    }
    state.pending_assistant.take().map(Some).ok_or_else(|| {
        CodexErr::InvalidRequest(
            "isolated model mode requires exactly one completed assistant message".to_string(),
        )
    })
}

impl Session {
    pub(crate) async fn model_tool_mode_is_isolated(&self) -> bool {
        self.state
            .lock()
            .await
            .session_configuration
            .model_tool_mode
            .is_isolated()
    }
}

fn validate_single_text_input(items: &[UserInput]) -> Result<(), String> {
    let [
        UserInput::Text {
            text,
            text_elements,
        },
    ] = items
    else {
        return Err("isolated model mode requires exactly one text input".to_string());
    };
    if text.trim().is_empty() {
        return Err("isolated model mode input must not be empty".to_string());
    }
    if text.len() > MAX_ISOLATED_INPUT_BYTES {
        return Err(format!(
            "isolated model mode input exceeds the {MAX_ISOLATED_INPUT_BYTES}-byte limit"
        ));
    }
    if !text_elements.is_empty() {
        return Err("isolated model mode text elements must be empty".to_string());
    }
    Ok(())
}

fn validate_isolated_model_output(
    item: &ResponseItem,
    stage: ModelOutputStage,
    output_schema: Option<&Value>,
    state: &mut IsolatedOutputState,
) -> CodexResult<()> {
    match item {
        ResponseItem::Reasoning { .. } => Ok(()),
        ResponseItem::Message {
            role,
            content,
            phase,
            internal_chat_message_metadata_passthrough,
            ..
        } if role == "assistant"
            && matches!(content.as_slice(), [ContentItem::OutputText { .. }])
            && matches!(phase.as_ref(), None | Some(MessagePhase::FinalAnswer))
            && internal_chat_message_metadata_passthrough.is_none() =>
        {
            if matches!(stage, ModelOutputStage::Completed) {
                if state.pending_assistant.is_some() {
                    return invalid_isolated_output(
                        "isolated model mode permits only one completed assistant message",
                    );
                }
                let text = bounded_output_text(content)?;
                let value = serde_json::from_str(&text).map_err(|error| {
                    CodexErr::InvalidRequest(format!(
                        "isolated assistant output is not valid JSON: {error}"
                    ))
                })?;
                let output_schema = output_schema.ok_or_else(|| {
                    CodexErr::InvalidRequest(
                        "isolated model output is missing its required schema".to_string(),
                    )
                })?;
                schema::validate_value(output_schema, &value).map_err(CodexErr::InvalidRequest)?;
                state.pending_assistant = Some(item.clone());
            }
            Ok(())
        }
        _ => invalid_isolated_output(
            "model returned a non-assistant item while model tool mode is isolated",
        ),
    }
}

fn bounded_output_text(content: &[ContentItem]) -> CodexResult<String> {
    let mut output_bytes = 0usize;
    for item in content {
        let ContentItem::OutputText { text } = item else {
            return invalid_isolated_output(
                "isolated assistant messages may contain only output text",
            );
        };
        output_bytes = output_bytes.checked_add(text.len()).ok_or_else(|| {
            CodexErr::InvalidRequest("isolated assistant output is too large".to_string())
        })?;
        if output_bytes > MAX_ISOLATED_OUTPUT_BYTES {
            return invalid_isolated_output(format!(
                "isolated assistant output exceeds the {MAX_ISOLATED_OUTPUT_BYTES}-byte limit"
            ));
        }
    }
    let mut output = String::with_capacity(output_bytes);
    for item in content {
        if let ContentItem::OutputText { text } = item {
            output.push_str(text);
        }
    }
    Ok(output)
}

fn invalid_creation(reason: &str) -> CodexResult<()> {
    Err(CodexErr::InvalidRequest(format!(
        "isolated model mode requires a fresh bounded thread: {reason}"
    )))
}

fn invalid_isolated_output<T>(message: impl Into<String>) -> CodexResult<T> {
    Err(CodexErr::InvalidRequest(message.into()))
}

#[cfg(test)]
#[path = "model_isolation_tests.rs"]
mod tests;
