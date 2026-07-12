use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::request_processors) enum ThreadShutdownResult {
    Complete,
    SubmitFailed,
    TimedOut,
}

pub(in crate::request_processors) async fn wait_for_thread_shutdown(
    thread: &Arc<CodexThread>,
) -> ThreadShutdownResult {
    match tokio::time::timeout(Duration::from_secs(10), thread.shutdown_and_wait()).await {
        Ok(Ok(())) => ThreadShutdownResult::Complete,
        Ok(Err(_)) => ThreadShutdownResult::SubmitFailed,
        Err(_) => ThreadShutdownResult::TimedOut,
    }
}

pub(in crate::request_processors) async fn remove_isolated_threads_for_owner_disconnect(
    thread_manager: Arc<ThreadManager>,
    outgoing: Arc<OutgoingMessageSender>,
    pending_thread_unloads: Arc<Mutex<HashSet<ThreadId>>>,
    thread_state_manager: ThreadStateManager,
    thread_watch_manager: ThreadWatchManager,
    thread_ids: Vec<ThreadId>,
) {
    // First make every owned thread undiscoverable. No bounded shutdown wait may
    // leave a later thread reachable after its owner connection has closed.
    let removed_threads = futures::future::join_all(thread_ids.iter().map(|thread_id| {
        let thread_manager = Arc::clone(&thread_manager);
        async move { (*thread_id, thread_manager.remove_thread(thread_id).await) }
    }))
    .await;
    let shutdown_tasks = removed_threads
        .into_iter()
        .filter_map(|(thread_id, thread)| {
            thread.map(|thread| {
                tokio::spawn(async move { (thread_id, wait_for_thread_shutdown(&thread).await) })
            })
        })
        .collect::<Vec<_>>();

    for thread_id in &thread_ids {
        outgoing
            .cancel_requests_for_thread(*thread_id, /*error*/ None)
            .await;
        thread_state_manager.remove_thread_state(*thread_id).await;
        thread_watch_manager
            .remove_thread(&thread_id.to_string())
            .await;
        pending_thread_unloads.lock().await.remove(thread_id);
    }

    for shutdown in futures::future::join_all(shutdown_tasks).await {
        match shutdown {
            Ok((_, ThreadShutdownResult::Complete)) => {}
            Ok((thread_id, ThreadShutdownResult::SubmitFailed)) => {
                warn!("failed to submit Shutdown to isolated thread {thread_id}");
            }
            Ok((thread_id, ThreadShutdownResult::TimedOut)) => {
                warn!("isolated thread {thread_id} shutdown timed out after owner disconnect");
            }
            Err(error) => warn!("isolated thread shutdown task failed: {error}"),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::request_processors) async fn unload_thread_without_subscribers(
    thread_manager: Arc<ThreadManager>,
    outgoing: Arc<OutgoingMessageSender>,
    pending_thread_unloads: Arc<Mutex<HashSet<ThreadId>>>,
    thread_state_manager: ThreadStateManager,
    thread_watch_manager: ThreadWatchManager,
    thread_id: ThreadId,
    thread: Arc<CodexThread>,
    closed_notification_connection: Option<ConnectionId>,
) {
    info!("thread {thread_id} has no subscribers and is idle; shutting down");

    // An isolated thread must become undiscoverable before its ownership state
    // is removed. Keep the Arc for bounded shutdown, but remove the manager
    // entry first so no direct API can race into an ownerless live thread.
    let removed_before_shutdown = if closed_notification_connection.is_some() {
        thread_manager.remove_thread(&thread_id).await.is_some()
    } else {
        false
    };

    // Any pending app-server -> client requests for this thread can no longer be
    // answered; cancel their callbacks before shutdown/unload.
    outgoing
        .cancel_requests_for_thread(thread_id, /*error*/ None)
        .await;
    thread_state_manager.remove_thread_state(thread_id).await;

    tokio::spawn(async move {
        let shutdown_result = wait_for_thread_shutdown(&thread).await;
        match shutdown_result {
            ThreadShutdownResult::Complete => {}
            ThreadShutdownResult::SubmitFailed => {
                warn!("failed to submit Shutdown to thread {thread_id}");
            }
            ThreadShutdownResult::TimedOut if closed_notification_connection.is_some() => {
                warn!("isolated thread {thread_id} shutdown timed out; removing it fail-closed");
            }
            ThreadShutdownResult::TimedOut => {
                warn!("thread {thread_id} shutdown timed out; leaving thread loaded");
            }
        }

        let fail_closed_isolated = closed_notification_connection.is_some();
        if shutdown_result == ThreadShutdownResult::Complete || fail_closed_isolated {
            let removed =
                removed_before_shutdown || thread_manager.remove_thread(&thread_id).await.is_some();
            if !removed {
                info!("thread {thread_id} was already removed before teardown finalized");
            }
            thread_watch_manager
                .remove_thread(&thread_id.to_string())
                .await;
            if removed {
                let notification = ThreadClosedNotification {
                    thread_id: thread_id.to_string(),
                };
                match closed_notification_connection {
                    Some(connection_id) => {
                        outgoing
                            .send_server_notification_to_connections(
                                &[connection_id],
                                ServerNotification::ThreadClosed(notification),
                            )
                            .await;
                    }
                    None => {
                        outgoing
                            .send_server_notification(ServerNotification::ThreadClosed(
                                notification,
                            ))
                            .await;
                    }
                }
            }
        }
        pending_thread_unloads.lock().await.remove(&thread_id);
    });
}
