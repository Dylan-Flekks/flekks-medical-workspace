use super::*;

pub(in crate::request_processors) async fn ensure_isolated_conversation_listener(
    listener_task_context: ListenerTaskContext,
    conversation_id: ThreadId,
    connection_id: ConnectionId,
    config_snapshot: &ThreadConfigSnapshot,
) -> Result<EnsureConversationListenerResult, JSONRPCErrorError> {
    if !config_snapshot.model_tool_mode.is_isolated() {
        return Err(internal_error(
            "isolated listener requires an isolated thread snapshot",
        ));
    }
    ensure_conversation_listener_with_setup(
        listener_task_context,
        conversation_id,
        connection_id,
        /*raw_events_enabled*/ false,
        ListenerSetup::Isolated {
            config_snapshot: Box::new(config_snapshot.clone()),
            owner_connection: connection_id,
        },
    )
    .await
}
