use super::*;

#[test]
fn newer_edit_generation_postpones_idle_checkpoint() {
    let start = Instant::now();
    let mut coordinator = WorkspaceDraftCoordinator::default();
    coordinator.note_edit_at(start);
    assert!(!coordinator.idle_checkpoint_is_due_at(start));

    let second_edit = start + Duration::from_millis(800);
    coordinator.note_edit_at(second_edit);

    assert!(
        !coordinator
            .idle_checkpoint_is_due_at(start + CHECKPOINT_IDLE_DELAY + Duration::from_millis(1))
    );
    assert!(coordinator.idle_checkpoint_is_due_at(second_edit + CHECKPOINT_IDLE_DELAY));
}

#[test]
fn handoff_forces_exact_checkpoint_without_edits() {
    let coordinator = WorkspaceDraftCoordinator::default();

    assert!(!coordinator.should_checkpoint(WorkspaceDraftCheckpointTrigger::FocusChange));
    assert!(coordinator.should_checkpoint(WorkspaceDraftCheckpointTrigger::Handoff));
}

#[test]
fn newer_edit_cancels_a_stale_canonical_close_continuation() {
    let mut coordinator = WorkspaceDraftCoordinator {
        edit_generation: 2,
        saved_generation: 2,
        canonical_save_pending_close: true,
        ..WorkspaceDraftCoordinator::default()
    };

    coordinator.note_edit_at(Instant::now());

    assert!(!coordinator.canonical_save_pending_close());
    assert!(coordinator.has_uncheckpointed_edits());
}

#[test]
fn canonical_only_acknowledges_only_the_pre_save_generation() {
    let mut coordinator = WorkspaceDraftCoordinator::default();
    coordinator.note_edit();
    let pre_save_generation = coordinator.generation();

    coordinator.context_edit();
    coordinator.acknowledge_canonical_only_save_through(pre_save_generation);

    assert!(coordinator.has_uncheckpointed_edits());
    assert!(coordinator.has_uncheckpointed_context_edits());
    assert!(coordinator.pending_delay().is_some());
}

#[test]
fn submitted_context_is_distinct_from_checkpointed_context() {
    let mut coordinator = WorkspaceDraftCoordinator::default();
    coordinator.context_edit();

    assert!(coordinator.has_unsubmitted_context_edits());
    coordinator.mark_context_submitted();
    assert!(!coordinator.has_unsubmitted_context_edits());

    coordinator.context_edit();
    assert!(coordinator.has_unsubmitted_context_edits());
}

#[tokio::test]
async fn pending_checkpoint_timeout_keeps_generation_and_task_for_later_poll() {
    let task = tokio::spawn(async {
        std::future::pending::<Result<WorkspaceDraftCheckpointCreateResponse>>().await
    });
    let mut coordinator = WorkspaceDraftCoordinator {
        active_client_id: Some("client-1".to_string()),
        session_creation_key: Some("pending-first-session-key".to_string()),
        edit_generation: 3,
        saved_generation: 2,
        in_flight: Some(WorkspaceDraftCheckpointInFlight {
            client_id: "client-1".to_string(),
            generation: 3,
            context_generation: 0,
            task,
        }),
        ..WorkspaceDraftCoordinator::default()
    };

    let outcome = coordinator
        .poll_in_flight_checkpoint(Duration::from_millis(1))
        .await
        .expect("timeout should remain a tracked pending checkpoint");

    assert_eq!(outcome, WorkspaceDraftCheckpointOutcome::Pending);
    assert_eq!(coordinator.saved_generation, 2);
    assert_eq!(
        coordinator.session_creation_key.as_deref(),
        Some("pending-first-session-key")
    );
    assert!(coordinator.has_in_flight_checkpoint());
    coordinator
        .in_flight
        .take()
        .expect("pending task should remain owned")
        .task
        .abort();
}

#[tokio::test]
async fn first_client_checkpoint_failure_preserves_key_and_schedules_safe_retry() {
    let mut coordinator = WorkspaceDraftCoordinator {
        edit_generation: 3,
        saved_generation: 2,
        ..WorkspaceDraftCoordinator::default()
    };

    coordinator
        .bind_client_for_checkpoint("client-1")
        .expect("first saved patient id should bind without resetting edits");
    assert_eq!(coordinator.active_client_id.as_deref(), Some("client-1"));
    assert_eq!(coordinator.edit_generation, 3);
    assert_eq!(coordinator.saved_generation, 2);

    let task = tokio::spawn(async {
        Err(color_eyre::eyre::eyre!(
            "simulated checkpoint persistence failure"
        ))
    });
    coordinator.in_flight = Some(WorkspaceDraftCheckpointInFlight {
        client_id: "client-1".to_string(),
        generation: 3,
        context_generation: 0,
        task,
    });
    coordinator.session_creation_key = Some("stable-first-session-key".to_string());

    let _error = coordinator
        .poll_in_flight_checkpoint(Duration::from_secs(1))
        .await
        .expect_err("unknown first-session persistence must remain retryable");

    assert_eq!(coordinator.edit_generation, 3);
    assert_eq!(coordinator.saved_generation, 2);
    assert!(coordinator.has_uncheckpointed_edits());
    assert!(coordinator.should_checkpoint(WorkspaceDraftCheckpointTrigger::ExplicitSave));
    assert!(coordinator.should_checkpoint(WorkspaceDraftCheckpointTrigger::Close));
    assert_eq!(
        coordinator.session_creation_key.as_deref(),
        Some("stable-first-session-key")
    );
    assert!(coordinator.pending_delay().is_some());
}

#[tokio::test]
async fn in_flight_checkpoint_blocks_scope_change_and_clear_without_detaching_task() {
    let task = tokio::spawn(async {
        std::future::pending::<Result<WorkspaceDraftCheckpointCreateResponse>>().await
    });
    let mut coordinator = WorkspaceDraftCoordinator {
        active_client_id: Some("client-1".to_string()),
        edit_generation: 4,
        saved_generation: 3,
        in_flight: Some(WorkspaceDraftCheckpointInFlight {
            client_id: "client-1".to_string(),
            generation: 4,
            context_generation: 0,
            task,
        }),
        ..WorkspaceDraftCoordinator::default()
    };

    assert!(!coordinator.prepare_client_scope("client-2"));
    assert!(!coordinator.try_clear());
    assert_eq!(coordinator.active_client_id.as_deref(), Some("client-1"));
    assert_eq!(coordinator.edit_generation, 4);
    assert_eq!(coordinator.saved_generation, 3);
    assert!(coordinator.has_in_flight_checkpoint());

    coordinator
        .in_flight
        .take()
        .expect("blocked transition must retain owned task")
        .task
        .abort();
}

#[tokio::test]
async fn canonical_saved_pending_checkpoint_remains_owned_and_retry_scheduled() {
    let task = tokio::spawn(async {
        std::future::pending::<Result<WorkspaceDraftCheckpointCreateResponse>>().await
    });
    let mut coordinator = WorkspaceDraftCoordinator {
        active_client_id: Some("client-1".to_string()),
        edit_generation: 2,
        saved_generation: 1,
        in_flight: Some(WorkspaceDraftCheckpointInFlight {
            client_id: "client-1".to_string(),
            generation: 2,
            context_generation: 0,
            task,
        }),
        canonical_save_pending_close: true,
        ..WorkspaceDraftCoordinator::default()
    };

    assert_eq!(
        coordinator
            .poll_in_flight_checkpoint(Duration::ZERO)
            .await
            .expect("zero wait should leave task pending"),
        WorkspaceDraftCheckpointOutcome::Pending
    );
    assert!(coordinator.canonical_save_pending_close());
    assert!(coordinator.has_in_flight_checkpoint());
    assert_eq!(coordinator.pending_delay(), Some(CHECKPOINT_POLL_DELAY));
    assert!(!coordinator.try_clear());

    coordinator
        .in_flight
        .take()
        .expect("pending continuation must retain owned task")
        .task
        .abort();
}

#[test]
fn session_creation_key_is_stable_until_scope_reset() {
    let mut coordinator = WorkspaceDraftCoordinator::default();
    coordinator.reset_for_client("client-1");

    let (first_session_id, first_key) = coordinator.checkpoint_session_identity();
    let (retry_session_id, retry_key) = coordinator.checkpoint_session_identity();

    assert_eq!(first_session_id, None);
    assert_eq!(retry_session_id, None);
    assert_eq!(retry_key, first_key);
    let first_key = first_key.expect("first request should receive a creation key");
    assert!(!first_key.is_empty());
    assert!(coordinator.should_checkpoint(WorkspaceDraftCheckpointTrigger::FocusChange));
    assert!(!coordinator.can_clear_dashboard());
    assert!(!coordinator.prepare_client_scope("client-2"));
    coordinator.debounce_deadline = Some(Instant::now());
    assert!(coordinator.idle_checkpoint_is_due());

    coordinator.reset_for_client("client-2");
    let (next_session_id, next_key) = coordinator.checkpoint_session_identity();
    assert_eq!(next_session_id, None);
    let next_key = next_key.expect("new scope should receive a creation key");
    assert!(!next_key.is_empty());
    assert_ne!(next_key, first_key);

    coordinator.session_id = Some("session-2".to_string());
    coordinator.session_creation_key = None;
    assert_eq!(
        coordinator.checkpoint_session_identity(),
        (Some("session-2".to_string()), None)
    );
}

#[test]
fn canonical_close_maps_the_exact_last_confirmed_checkpoint_cas() {
    let checkpoint = WorkspaceDraftCheckpoint {
        id: "checkpoint-7".to_string(),
        session_id: "session-1".to_string(),
        client_id: "client-1".to_string(),
        encounter_id: Some("encounter-1".to_string()),
        note_id: Some("note-1".to_string()),
        base_note_revision: Some(4),
        schema_version: 1,
        revision: 7,
        draft: serde_json::json!({"schemaVersion": 1}),
        content_sha256: "a".repeat(64),
        trigger: "explicit_save".to_string(),
        actor: "test clinician".to_string(),
        created_at: 1,
    };
    let coordinator = WorkspaceDraftCoordinator {
        active_client_id: Some("client-1".to_string()),
        session_id: Some("session-1".to_string()),
        last_confirmed_checkpoint: Some(checkpoint.clone()),
        ..WorkspaceDraftCoordinator::default()
    };

    let params = coordinator
        .canonical_close_params()
        .expect("confirmed checkpoint identity should map")
        .expect("active session should produce close params");

    assert_eq!(params.session_id, checkpoint.session_id);
    assert_eq!(params.client_id, checkpoint.client_id);
    assert_eq!(
        params.expected_current_checkpoint_id.as_deref(),
        Some(checkpoint.id.as_str())
    );
    assert_eq!(
        params.expected_current_checkpoint_revision,
        Some(checkpoint.revision)
    );
    assert_eq!(
        params.expected_current_checkpoint_sha256.as_deref(),
        Some(checkpoint.content_sha256.as_str())
    );

    let missing_checkpoint = WorkspaceDraftCoordinator {
        active_client_id: Some("client-1".to_string()),
        session_id: Some("session-1".to_string()),
        ..WorkspaceDraftCoordinator::default()
    };
    assert!(missing_checkpoint.canonical_close_params().is_err());
}
