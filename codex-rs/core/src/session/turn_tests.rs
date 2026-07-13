use super::*;
use codex_extension_api::ExtensionData;
use codex_extension_api::TurnItemContributor;
use codex_protocol::ResponseItemId;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_rollout_trace::ThreadStartedTraceMetadata;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

struct RewriteAgentMessageContributor;

impl TurnItemContributor for RewriteAgentMessageContributor {
    fn contribute<'a>(
        &'a self,
        _thread_store: &'a ExtensionData,
        _turn_store: &'a ExtensionData,
        item: &'a mut TurnItem,
    ) -> codex_extension_api::ExtensionFuture<'a, Result<(), String>> {
        Box::pin(async move {
            if let TurnItem::AgentMessage(agent_message) = item {
                agent_message.content = vec![AgentMessageContent::Text {
                    text: "plan contributed assistant text".to_string(),
                }];
            }
            Ok(())
        })
    }
}

fn assistant_output_text(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: Some(ResponseItemId::with_suffix("msg", "1")),
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    }
}

#[tokio::test]
async fn plan_mode_uses_contributed_turn_item_for_last_agent_message() {
    let (mut session, turn_context) = crate::session::tests::make_session_and_context().await;
    let mut builder = codex_extension_api::ExtensionRegistryBuilder::new();
    builder.turn_item_contributor(Arc::new(RewriteAgentMessageContributor));
    session.services.extensions = Arc::new(builder.build());
    let turn_store = ExtensionData::new(turn_context.sub_id.clone());
    let mut state = PlanModeStreamState::new(&turn_context.sub_id);
    let mut last_agent_message = None;
    let item = assistant_output_text("original assistant text");

    let handled = handle_assistant_item_done_in_plan_mode(
        &session,
        &turn_context,
        &turn_store,
        &item,
        &mut state,
        /*previously_active_item*/ None,
        &mut last_agent_message,
    )
    .await;

    assert!(handled);
    assert_eq!(
        last_agent_message.as_deref(),
        Some("plan contributed assistant text")
    );
}

fn attach_test_trace(session: &mut Session, root: &Path) -> anyhow::Result<()> {
    session.services.rollout_thread_trace =
        codex_rollout_trace::ThreadTraceContext::start_root_in_root_for_test(
            root,
            ThreadStartedTraceMetadata {
                thread_id: session.thread_id.to_string(),
                agent_path: "/root".to_string(),
                task_name: None,
                nickname: None,
                agent_role: None,
                session_source: SessionSource::Exec,
                cwd: PathBuf::from("/workspace"),
                rollout_path: None,
                model: "gpt-test".to_string(),
                provider_name: "test-provider".to_string(),
                approval_policy: "never".to_string(),
                sandbox_policy: "danger-full-access".to_string(),
            },
        )?;
    Ok(())
}

fn read_trace_tree(path: &Path) -> anyhow::Result<String> {
    let mut text = String::new();
    for entry in fs::read_dir(path)? {
        let path = entry?.path();
        if path.is_dir() {
            text.push_str(&read_trace_tree(&path)?);
        } else {
            text.push_str(&String::from_utf8_lossy(&fs::read(path)?));
        }
    }
    Ok(text)
}

#[tokio::test]
async fn workspace_context_only_disables_inference_and_protocol_trace_payloads()
-> anyhow::Result<()> {
    const TRACE_MARKER: &str = "synthetic-medical-answer-must-not-enter-rollout-trace";
    let (mut session, mut turn_context) = crate::session::tests::make_session_and_context().await;
    let trace_root = TempDir::new()?;
    attach_test_trace(&mut session, trace_root.path())?;

    assert!(inference_trace_context_for_turn(&session, &turn_context).is_enabled());
    turn_context.model_tool_mode = ModelToolMode::WorkspaceContextOnly;
    assert!(!inference_trace_context_for_turn(&session, &turn_context).is_enabled());

    session
        .send_event(
            &turn_context,
            EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id: turn_context.sub_id.clone(),
                last_agent_message: Some(TRACE_MARKER.to_string()),
                error: None,
                started_at: None,
                completed_at: None,
                duration_ms: None,
                time_to_first_token_ms: None,
            }),
        )
        .await;

    let trace_text = read_trace_tree(trace_root.path())?;
    assert!(!trace_text.contains(TRACE_MARKER));
    assert!(!trace_text.contains("turn_complete"));

    Ok(())
}
