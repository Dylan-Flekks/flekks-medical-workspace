use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;

use codex_protocol::config_types::ModelToolMode;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::UserInput;
use serde::Deserialize;

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
#[derive(Clone)]
pub(crate) enum WorkspaceContextRunBinding {
    Agent(codex_state::WorkspaceAgentExecutionBinding),
    Planning(WorkspacePlanningRunBinding),
}

/// Capability and ordered immutable evidence accumulated by one planning turn. The capability is
/// never serialized or exposed to the model; only successful planning-context read ids are kept.
#[derive(Clone)]
pub(crate) struct WorkspacePlanningRunBinding {
    pub(crate) execution: codex_state::WorkspacePlanningGuideExecutionBinding,
    pub(crate) evidence_read_ids: Arc<tokio::sync::Mutex<Vec<String>>>,
}

impl WorkspacePlanningRunBinding {
    fn new(execution: codex_state::WorkspacePlanningGuideExecutionBinding) -> Self {
        Self {
            execution,
            evidence_read_ids: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    pub(crate) async fn evidence_read_ids(&self) -> Vec<String> {
        self.evidence_read_ids.lock().await.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkspaceRunContract {
    Agent {
        run_id: String,
    },
    Planning {
        guide_run_id: String,
        plan_session_id: String,
        client_id: String,
        source_checkpoint_id: String,
        source_checkpoint_revision: i64,
        source_checkpoint_sha256: String,
    },
}

const REDACTED_TOOL_LOG_PAYLOAD: &str = r#"{"payload_redacted":true}"#;
const WORKSPACE_PLAN_ARTIFACT_OPEN: &str = "<workspace_plan_artifact>";
const WORKSPACE_PLAN_ARTIFACT_CLOSE: &str = "</workspace_plan_artifact>";
const MAX_WORKSPACE_PLAN_MESSAGE_BYTES: usize = 64 * 1024;
const MAX_WORKSPACE_PLAN_ARTIFACT_BYTES: usize = 128 * 1024;
const WORKSPACE_PLAN_COMPLETION_ACTOR: &str = "codex workspace planner";

/// Prevent the immutable medical-run capability and any clinical identifiers in tool arguments
/// from entering generic logs or telemetry. The complete payload still reaches the bound handler
/// and the model-visible function output remains available inside the restricted turn.
pub(crate) fn tool_log_payload(mode: ModelToolMode, payload: &ToolPayload) -> Cow<'_, str> {
    if mode.is_workspace_restricted() {
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
    if mode.is_workspace_restricted() {
        (
            FunctionCallError::RespondToModel(
                "restricted workspace tool failed; clinical and run details redacted".to_string(),
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
    contract: WorkspaceRunContract,
    items: &[UserInput],
) -> CodexResult<WorkspaceContextRunBinding> {
    if !turn_context.model_tool_mode.is_workspace_restricted() {
        return Err(CodexErr::InvalidRequest(
            "workspace context run binding requires a restricted workspace mode".to_string(),
        ));
    }
    let [
        UserInput::Text {
            text,
            text_elements,
        },
    ] = items
    else {
        return Err(CodexErr::InvalidRequest(format!(
            "{} requires exactly one plain-text generated handoff prompt",
            turn_context.model_tool_mode
        )));
    };
    if text.trim().is_empty() || !text_elements.is_empty() {
        return Err(CodexErr::InvalidRequest(format!(
            "{} requires exactly one plain-text generated handoff prompt",
            turn_context.model_tool_mode
        )));
    }
    let binding = match (turn_context.model_tool_mode, contract) {
        (ModelToolMode::WorkspaceContextOnly, WorkspaceRunContract::Agent { run_id }) => state_db
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
            .map(WorkspaceContextRunBinding::Agent),
        (
            ModelToolMode::WorkspacePlanningOnly,
            WorkspaceRunContract::Planning {
                guide_run_id,
                plan_session_id,
                client_id,
                source_checkpoint_id,
                source_checkpoint_revision,
                source_checkpoint_sha256,
            },
        ) => {
            let request = codex_state::WorkspacePlanningGuideTurnClaimRequest {
                guide_run_id,
                plan_session_id,
                client_id,
                source_checkpoint_id,
                source_checkpoint_revision,
                source_checkpoint_sha256,
                source_thread_id: thread_id,
                source_turn_id: turn_id,
                provider: turn_context.config.model_provider_id.clone(),
                model: turn_context.model_info.slug.clone(),
                prompt: text.clone(),
            };
            match tokio::spawn(async move {
                state_db
                    .workspace()
                    .claim_planning_guide_turn(request)
                    .await
            })
            .await
            {
                Ok(result) => result
                    .map(WorkspacePlanningRunBinding::new)
                    .map(WorkspaceContextRunBinding::Planning)
                    .map_err(anyhow::Error::from),
                Err(error) => Err(anyhow::Error::from(error)),
            }
        }
        _ => {
            return Err(CodexErr::InvalidRequest(
                "workspace run contract does not match the requested restricted mode".to_string(),
            ));
        }
    };
    binding.map_err(|_| {
        CodexErr::InvalidRequest(format!(
            "{} turn does not match its stored run contract",
            turn_context.model_tool_mode
        ))
    })
}

pub(crate) fn bind_run(
    turn_context: &TurnContext,
    execution: WorkspaceContextRunBinding,
) -> CodexResult<()> {
    if !turn_context.model_tool_mode.is_workspace_restricted() {
        return Err(CodexErr::InvalidRequest(
            "workspace context run binding requires a restricted workspace mode".to_string(),
        ));
    }
    if turn_context.extension_data.insert(execution).is_some() {
        return Err(CodexErr::InvalidRequest(
            "workspace context run binding was already set for this turn".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn planning_binding(
    turn_context: &TurnContext,
) -> CodexResult<WorkspacePlanningRunBinding> {
    if turn_context.model_tool_mode != ModelToolMode::WorkspacePlanningOnly {
        return Err(CodexErr::InvalidRequest(
            "workspace planning completion requires workspacePlanningOnly mode".to_string(),
        ));
    }
    match turn_context
        .extension_data
        .get::<WorkspaceContextRunBinding>()
        .map(|binding| binding.as_ref().clone())
    {
        Some(WorkspaceContextRunBinding::Planning(binding)) => Ok(binding),
        _ => Err(CodexErr::InvalidRequest(
            "workspacePlanningOnly turn is missing its immutable planning binding".to_string(),
        )),
    }
}

pub(crate) fn agent_binding(
    turn_context: &TurnContext,
) -> CodexResult<codex_state::WorkspaceAgentExecutionBinding> {
    if turn_context.model_tool_mode != ModelToolMode::WorkspaceContextOnly {
        return Err(CodexErr::InvalidRequest(
            "workspace agent completion requires workspaceContextOnly mode".to_string(),
        ));
    }
    match turn_context
        .extension_data
        .get::<WorkspaceContextRunBinding>()
        .map(|binding| binding.as_ref().clone())
    {
        Some(WorkspaceContextRunBinding::Agent(execution)) => Ok(execution),
        _ => Err(CodexErr::InvalidRequest(
            "workspaceContextOnly turn is missing its immutable agent binding".to_string(),
        )),
    }
}

/// Commit the exact final master-agent response before it can enter rollout history or reach a
/// client. This makes the database result, completion receipt, and terminal run status one atomic
/// boundary instead of allowing a UI or RPC caller to attribute arbitrary text to the agent.
pub(crate) async fn complete_agent_assistant_message(
    state_db: crate::StateDbHandle,
    turn_context: &TurnContext,
    item: ResponseItem,
) -> CodexResult<ResponseItem> {
    let execution = agent_binding(turn_context)?;
    let ResponseItem::Message {
        id,
        role,
        content,
        phase,
        ..
    } = &item
    else {
        return Err(CodexErr::InvalidRequest(
            "workspace agent completion requires one assistant message".to_string(),
        ));
    };
    if role != "assistant" {
        return Err(CodexErr::InvalidRequest(
            "workspace agent completion requires one assistant message".to_string(),
        ));
    }
    if matches!(
        phase,
        Some(codex_protocol::models::MessagePhase::Commentary)
    ) {
        return Err(CodexErr::InvalidRequest(
            "workspace agent completion requires a final assistant message".to_string(),
        ));
    }
    let assistant_message_id = id
        .as_ref()
        .map(codex_protocol::ResponseItemId::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| {
            CodexErr::InvalidRequest(
                "workspace agent completion requires a durable assistant message id".to_string(),
            )
        })?;
    let body = assistant_output_text(content)?;
    let idempotency_key = format!(
        "workspace-agent-turn:{}:{}",
        execution.run_id, execution.source_turn_id
    );
    state_db
        .workspace()
        .complete_agent_turn(codex_state::WorkspaceAgentTurnComplete {
            execution,
            assistant_message_id: assistant_message_id.to_string(),
            body,
            idempotency_key,
        })
        .await
        .map_err(|_| {
            CodexErr::InvalidRequest(
                "workspace medical agent response could not be committed".to_string(),
            )
        })?;
    Ok(item)
}

/// Persist one final planning response before it can enter rollout history or reach the client.
/// The capability-bearing execution binding remains process-local, while the state transaction
/// verifies every ordered evidence id and terminalizes the guide atomically.
pub(crate) async fn complete_planning_assistant_message(
    state_db: crate::StateDbHandle,
    turn_context: &TurnContext,
    mut item: ResponseItem,
) -> CodexResult<ResponseItem> {
    let binding = planning_binding(turn_context)?;
    let ResponseItem::Message { role, content, .. } = &mut item else {
        return Err(CodexErr::InvalidRequest(
            "workspace planning completion requires one assistant message".to_string(),
        ));
    };
    if role != "assistant" {
        return Err(CodexErr::InvalidRequest(
            "workspace planning completion requires one assistant message".to_string(),
        ));
    }
    let raw_message = assistant_output_text(content)?;
    let parsed = parse_workspace_plan_artifact(&raw_message)?;
    *content = vec![codex_protocol::models::ContentItem::OutputText {
        text: parsed.assistant_message.clone(),
    }];
    let evidence_read_ids = binding.evidence_read_ids().await;
    let plan = parsed.plan.map(|plan| codex_state::WorkspacePlanArtifact {
        plan_markdown: plan.plan_markdown,
        decisions_json: plan.decisions_json,
        open_questions_json: plan.open_questions_json,
    });
    let idempotency_key = format!(
        "workspace-planning-turn:{}:{}",
        binding.execution.guide_run_id, binding.execution.source_turn_id
    );
    state_db
        .workspace()
        .complete_plan_turn(codex_state::WorkspacePlanTurnComplete {
            execution: binding.execution,
            assistant_message_role: codex_state::WorkspacePlanMessageRole::Assistant,
            assistant_message: parsed.assistant_message,
            plan,
            evidence_read_ids,
            idempotency_key,
            actor: WORKSPACE_PLAN_COMPLETION_ACTOR.to_string(),
        })
        .await
        .map_err(|_| {
            CodexErr::InvalidRequest(
                "workspace medical planning response could not be committed".to_string(),
            )
        })?;
    Ok(item)
}

struct ParsedWorkspacePlanArtifact {
    assistant_message: String,
    plan: Option<ParsedPublishableWorkspacePlan>,
}

struct ParsedPublishableWorkspacePlan {
    plan_markdown: String,
    decisions_json: String,
    open_questions_json: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspacePlanArtifactJson {
    plan_markdown: String,
    decisions: Vec<String>,
    open_questions: Vec<String>,
}

fn assistant_output_text(content: &[codex_protocol::models::ContentItem]) -> CodexResult<String> {
    let mut text = String::new();
    for item in content {
        let codex_protocol::models::ContentItem::OutputText { text: part } = item else {
            return Err(CodexErr::InvalidRequest(
                "restricted workspace assistant messages must contain text only".to_string(),
            ));
        };
        text.push_str(part);
    }
    Ok(text)
}

fn parse_workspace_plan_artifact(message: &str) -> CodexResult<ParsedWorkspacePlanArtifact> {
    let opens = message
        .match_indices(WORKSPACE_PLAN_ARTIFACT_OPEN)
        .collect::<Vec<_>>();
    let closes = message
        .match_indices(WORKSPACE_PLAN_ARTIFACT_CLOSE)
        .collect::<Vec<_>>();
    if opens.is_empty() && closes.is_empty() {
        if message.contains("workspace_plan_artifact") {
            return Err(malformed_workspace_plan_artifact());
        }
        let assistant_message = normalized_bounded_text(
            message,
            MAX_WORKSPACE_PLAN_MESSAGE_BYTES,
            "workspace planning assistant message",
        )?;
        return Ok(ParsedWorkspacePlanArtifact {
            assistant_message,
            plan: None,
        });
    }
    let ([(open_start, _)], [(close_start, _)]) = (opens.as_slice(), closes.as_slice()) else {
        return Err(malformed_workspace_plan_artifact());
    };
    let artifact_start = open_start + WORKSPACE_PLAN_ARTIFACT_OPEN.len();
    if artifact_start > *close_start {
        return Err(malformed_workspace_plan_artifact());
    }
    let before = &message[..*open_start];
    let artifact = &message[artifact_start..*close_start];
    let after = &message[*close_start + WORKSPACE_PLAN_ARTIFACT_CLOSE.len()..];
    if before.contains("workspace_plan_artifact")
        || artifact.contains("workspace_plan_artifact")
        || after.contains("workspace_plan_artifact")
    {
        return Err(malformed_workspace_plan_artifact());
    }
    let assistant_message = normalized_bounded_text(
        &format!("{before}{after}"),
        MAX_WORKSPACE_PLAN_MESSAGE_BYTES,
        "workspace planning assistant message",
    )?;
    let artifact_json = normalized_bounded_text(
        artifact,
        MAX_WORKSPACE_PLAN_ARTIFACT_BYTES,
        "workspace plan artifact",
    )?;
    let artifact: WorkspacePlanArtifactJson =
        serde_json::from_str(&artifact_json).map_err(|_| invalid_workspace_plan_artifact_json())?;
    let plan_markdown = normalized_bounded_text(
        &artifact.plan_markdown,
        MAX_WORKSPACE_PLAN_ARTIFACT_BYTES,
        "workspace plan artifact planMarkdown",
    )?;
    if !artifact.open_questions.is_empty() {
        return Err(CodexErr::InvalidRequest(
            "workspace plan artifact cannot be published while openQuestions is non-empty"
                .to_string(),
        ));
    }
    let decisions_json = serde_json::to_string(&artifact.decisions)
        .map_err(|_| invalid_workspace_plan_artifact_json())?;
    let open_questions_json = serde_json::to_string(&artifact.open_questions)
        .map_err(|_| invalid_workspace_plan_artifact_json())?;
    Ok(ParsedWorkspacePlanArtifact {
        assistant_message,
        plan: Some(ParsedPublishableWorkspacePlan {
            plan_markdown,
            decisions_json,
            open_questions_json,
        }),
    })
}

fn normalized_bounded_text(value: &str, max_bytes: usize, label: &str) -> CodexResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CodexErr::InvalidRequest(format!(
            "{label} must not be empty"
        )));
    }
    if value.len() > max_bytes {
        return Err(CodexErr::InvalidRequest(format!(
            "{label} exceeds the {max_bytes} byte limit"
        )));
    }
    Ok(value.to_string())
}

fn malformed_workspace_plan_artifact() -> CodexErr {
    CodexErr::InvalidRequest(
        "workspace plan artifact markers must contain exactly one complete non-nested pair"
            .to_string(),
    )
}

fn invalid_workspace_plan_artifact_json() -> CodexErr {
    CodexErr::InvalidRequest(
        "workspace plan artifact must be strict JSON with exactly planMarkdown, decisions, and openQuestions; decisions and openQuestions must be string arrays"
            .to_string(),
    )
}

pub(crate) fn build_router(turn_context: &TurnContext) -> CodexResult<Arc<ToolRouter>> {
    let execution = turn_context
        .extension_data
        .get::<WorkspaceContextRunBinding>()
        .map(|binding| binding.as_ref().clone())
        .ok_or_else(|| {
            CodexErr::InvalidRequest(format!(
                "{} turn is missing its immutable run binding",
                turn_context.model_tool_mode
            ))
        })?;
    let reader = match (turn_context.model_tool_mode, execution) {
        (ModelToolMode::WorkspaceContextOnly, WorkspaceContextRunBinding::Agent(execution)) => {
            WorkspaceContextReadHandler::bound_to_execution(execution)
        }
        (ModelToolMode::WorkspacePlanningOnly, WorkspaceContextRunBinding::Planning(binding)) => {
            WorkspaceContextReadHandler::bound_to_planning_execution(
                binding.execution,
                binding.evidence_read_ids,
            )
        }
        _ => {
            return Err(CodexErr::InvalidRequest(
                "workspace run binding does not match the active restricted mode".to_string(),
            ));
        }
    };
    let reader = Arc::new(reader);
    let handlers = vec![reader as Arc<dyn CoreToolRuntime>];
    let specs = handlers.iter().map(|handler| handler.spec()).collect();
    let registry = ToolRegistry::from_tools(handlers);
    Ok(Arc::new(ToolRouter::from_parts(registry, specs)))
}

pub(crate) fn filter_prompt_input(
    mode: ModelToolMode,
    turn_id: &str,
    verified_planning_turn_ids: &HashSet<String>,
    input: Vec<ResponseItem>,
) -> Vec<ResponseItem> {
    match mode {
        ModelToolMode::WorkspaceContextOnly => input
            .into_iter()
            .filter(|item| item.turn_id() == Some(turn_id))
            .collect(),
        ModelToolMode::WorkspacePlanningOnly => input
            .into_iter()
            .filter(|item| {
                item.turn_id()
                    .is_some_and(|turn_id| verified_planning_turn_ids.contains(turn_id))
            })
            .collect(),
        ModelToolMode::Default | ModelToolMode::Disabled => input,
    }
}

pub(crate) fn parse_run_contract(
    mode: ModelToolMode,
    items: &[UserInput],
) -> CodexResult<WorkspaceRunContract> {
    let [UserInput::Text { text, .. }] = items else {
        return Err(CodexErr::InvalidRequest(format!(
            "{mode} requires exactly one plain-text generated handoff prompt"
        )));
    };
    let run_id = prompt_field(mode, text, "run_id")?;
    match mode {
        ModelToolMode::WorkspaceContextOnly => Ok(WorkspaceRunContract::Agent { run_id }),
        ModelToolMode::WorkspacePlanningOnly => {
            let source_checkpoint_revision = prompt_field(mode, text, "checkpoint_revision")?
                .parse::<i64>()
                .ok()
                .filter(|revision| *revision > 0)
                .ok_or_else(|| {
                    CodexErr::InvalidRequest(
                        "workspacePlanningOnly requires `- checkpoint_revision: ...` to contain one positive integer"
                            .to_string(),
                    )
                })?;
            Ok(WorkspaceRunContract::Planning {
                guide_run_id: run_id,
                plan_session_id: prompt_field(mode, text, "plan_session_id")?,
                client_id: prompt_field(mode, text, "patient_id")?,
                source_checkpoint_id: prompt_field(mode, text, "checkpoint_id")?,
                source_checkpoint_revision,
                source_checkpoint_sha256: prompt_field(mode, text, "checkpoint_sha256")?,
            })
        }
        ModelToolMode::Default | ModelToolMode::Disabled => Err(CodexErr::InvalidRequest(
            "workspace run contracts require a restricted workspace mode".to_string(),
        )),
    }
}

fn prompt_field(mode: ModelToolMode, text: &str, label: &str) -> CodexResult<String> {
    let prefix = format!("- {label}:");
    let values = text
        .lines()
        .filter_map(|line| line.trim().strip_prefix(&prefix))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let [value] = values.as_slice() else {
        return Err(CodexErr::InvalidRequest(format!(
            "{mode} requires exactly one non-empty `- {label}: ...` line in the submitted text input"
        )));
    };
    Ok((*value).to_string())
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
        ModelToolMode::WorkspacePlanningOnly => match item {
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
                "model returned a disallowed output item while model tool mode is workspacePlanningOnly; only assistant messages, reasoning, and the non-namespaced {WORKSPACE_CONTEXT_READ_TOOL_NAME} function tool are allowed"
            ))),
        },
    }
}

#[cfg(test)]
#[path = "workspace_context_only_tests.rs"]
mod tests;
