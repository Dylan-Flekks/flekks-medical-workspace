use crate::StateRuntime;
use crate::runtime::test_support::unique_temp_dir;
use pretty_assertions::assert_eq;
use std::time::Duration;

async fn fixture() -> (
    std::sync::Arc<StateRuntime>,
    crate::WorkspaceClient,
    crate::WorkspaceClient,
) {
    let runtime = StateRuntime::init(unique_temp_dir(), "test-provider".to_string())
        .await
        .expect("state db should initialize");
    let first = create_client(&runtime, "First Synthetic Patient").await;
    let second = create_client(&runtime, "Second Synthetic Patient").await;
    (runtime, first, second)
}

async fn create_client(runtime: &StateRuntime, name: &str) -> crate::WorkspaceClient {
    runtime
        .workspace()
        .upsert_client(crate::WorkspaceClientUpsert {
            display_name: name.to_string(),
            ..Default::default()
        })
        .await
        .expect("client should save")
}

fn keyed_input(
    client: &crate::WorkspaceClient,
    creation_key: &str,
    title: &str,
) -> crate::WorkspaceDraftCheckpointCreate {
    crate::WorkspaceDraftCheckpointCreate {
        session_creation_key: Some(creation_key.to_string()),
        client_id: client.id.clone(),
        note_id: Some("draft-note".to_string()),
        base_note_revision: Some(1),
        draft_json: serde_json::json!({
            "schemaVersion": 1,
            "note": {"title": title},
        })
        .to_string(),
        trigger: "focus_change".to_string(),
        actor: "Clinician Example".to_string(),
        ..Default::default()
    }
}

async fn save(
    runtime: &StateRuntime,
    input: crate::WorkspaceDraftCheckpointCreate,
) -> crate::WorkspaceDraftCheckpoint {
    runtime
        .workspace()
        .create_draft_checkpoint(input)
        .await
        .expect("checkpoint should save")
}

async fn session_count(runtime: &StateRuntime, client_id: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM workspace_draft_sessions WHERE client_id = ?")
        .bind(client_id)
        .fetch_one(runtime.workspace().pool.as_ref())
        .await
        .expect("session count should load")
}

fn close_input(
    client: &crate::WorkspaceClient,
    checkpoint: &crate::WorkspaceDraftCheckpoint,
) -> crate::WorkspaceDraftSessionClose {
    crate::WorkspaceDraftSessionClose {
        session_id: checkpoint.session_id.clone(),
        client_id: client.id.clone(),
        status: crate::WorkspaceDraftSessionTerminalStatus::Closed,
        expected_current_checkpoint_id: Some(checkpoint.id.clone()),
        expected_current_checkpoint_revision: Some(checkpoint.revision),
        expected_current_checkpoint_sha256: Some(checkpoint.content_sha256.clone()),
        actor: "Clinician Example".to_string(),
        reason: "Canonical chart saved".to_string(),
    }
}

#[tokio::test]
async fn same_creation_key_replays_one_session_after_simulated_response_loss() {
    let (runtime, client, _) = fixture().await;
    let first = save(&runtime, keyed_input(&client, " first-save ", "Daily note")).await;

    // Simulate a committed first response that the caller never observed by retrying only
    // with the durable creation key, not the returned session id.
    let replay = save(&runtime, keyed_input(&client, "first-save", "Daily note")).await;

    assert!(replay.replayed);
    assert_eq!(replay.id, first.id);
    assert_eq!(replay.session_id, first.session_id);
    assert_eq!(replay.revision, 1);
    assert_eq!(session_count(&runtime, &client.id).await, 1);
    let checkpoint_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM workspace_draft_checkpoints WHERE session_id = ?")
            .bind(first.session_id)
            .fetch_one(runtime.workspace().pool.as_ref())
            .await
            .expect("checkpoint count should load");
    assert_eq!(checkpoint_count, 1);
}

#[tokio::test]
async fn changed_keyed_draft_appends_and_terminal_rejection_releases_transaction_lock() {
    let (runtime, client, _) = fixture().await;
    let first = save(&runtime, keyed_input(&client, "first-save", "First")).await;
    let second = save(&runtime, keyed_input(&client, "first-save", "Second")).await;

    assert_eq!(second.session_id, first.session_id);
    assert_eq!(second.revision, 2);
    runtime
        .workspace()
        .close_draft_session(close_input(&client, &second))
        .await
        .expect("keyed session should close");

    let error = runtime
        .workspace()
        .create_draft_checkpoint(keyed_input(&client, "first-save", "Third"))
        .await
        .expect_err("terminal keyed session should reject checkpoints");
    assert!(error.to_string().contains("cannot checkpoint"));

    let legacy = crate::WorkspaceDraftCheckpointCreate {
        session_creation_key: None,
        ..keyed_input(&client, "unused", "Legacy after rollback")
    };
    tokio::time::timeout(
        Duration::from_secs(1),
        runtime.workspace().create_draft_checkpoint(legacy),
    )
    .await
    .expect("terminal rejection must not retain the write lock")
    .expect("legacy checkpoint should save after rollback");
}

#[tokio::test]
async fn creation_key_validation_preserves_legacy_random_sessions() {
    let (runtime, client, _) = fixture().await;
    let mut both = keyed_input(&client, "first-save", "Both");
    both.session_id = Some("existing-session".to_string());
    assert!(
        runtime
            .workspace()
            .create_draft_checkpoint(both)
            .await
            .expect_err("session id and creation key should conflict")
            .to_string()
            .contains("not both")
    );

    for invalid_key in ["   ".to_string(), "x".repeat(257)] {
        let error = runtime
            .workspace()
            .create_draft_checkpoint(keyed_input(&client, &invalid_key, "Invalid key"))
            .await
            .expect_err("invalid creation key should fail");
        assert!(
            error.to_string().contains("must not be empty")
                || error.to_string().contains("must not exceed 256 bytes")
        );
    }

    save(&runtime, keyed_input(&client, &"x".repeat(256), "Max key")).await;
    save(
        &runtime,
        keyed_input(&client, &"é".repeat(128), "Max multibyte key"),
    )
    .await;
    let multibyte_error = runtime
        .workspace()
        .create_draft_checkpoint(keyed_input(
            &client,
            &format!("{}x", "é".repeat(128)),
            "Oversized multibyte key",
        ))
        .await
        .expect_err("257-byte multibyte creation key should fail");
    assert!(
        multibyte_error
            .to_string()
            .contains("must not exceed 256 bytes")
    );
    let legacy_input = crate::WorkspaceDraftCheckpointCreate {
        session_creation_key: None,
        ..keyed_input(&client, "unused", "Legacy")
    };
    let first_legacy = save(&runtime, legacy_input.clone()).await;
    let second_legacy = save(&runtime, legacy_input).await;
    assert_ne!(first_legacy.session_id, second_legacy.session_id);
}

#[tokio::test]
async fn same_creation_key_is_isolated_by_client() {
    let (runtime, first_client, second_client) = fixture().await;
    let first = save(
        &runtime,
        keyed_input(&first_client, "shared-device-key", "First patient"),
    )
    .await;
    let second = save(
        &runtime,
        keyed_input(&second_client, "shared-device-key", "Second patient"),
    )
    .await;

    assert_ne!(first.session_id, second.session_id);
    assert_eq!(session_count(&runtime, &first_client.id).await, 1);
    assert_eq!(session_count(&runtime, &second_client.id).await, 1);
}

#[tokio::test]
async fn creation_key_remains_private_after_session_audit() {
    let (runtime, client, _) = fixture().await;
    let key = "private-first-save-key";
    let checkpoint = save(&runtime, keyed_input(&client, key, "Private key")).await;
    runtime
        .workspace()
        .close_draft_session(close_input(&client, &checkpoint))
        .await
        .expect("keyed session should close");

    let audit_json: Vec<String> = sqlx::query_scalar(
        r#"
SELECT json_object(
    'id', id, 'entityType', entity_type, 'entityId', entity_id, 'action', action,
    'actor', actor, 'actorKind', actor_kind, 'source', source, 'clientId', client_id,
    'encounterId', encounter_id, 'noteId', note_id, 'documentId', document_id,
    'sourceThreadId', source_thread_id, 'sourceTurnId', source_turn_id,
    'success', success, 'summary', summary, 'metadataJson', metadata_json
)
FROM workspace_audit_events
WHERE client_id = ?
        "#,
    )
    .bind(&client.id)
    .fetch_all(runtime.workspace().pool.as_ref())
    .await
    .expect("draft session audits should load");
    assert!(!audit_json.join("\n").contains(key));
}

#[tokio::test]
async fn concurrent_same_key_creates_exactly_one_session() {
    let (runtime, client, _) = fixture().await;
    let first_store = runtime.workspace().clone();
    let second_store = runtime.workspace().clone();
    let input = keyed_input(&client, "concurrent-first-save", "Concurrent");
    let (first, second) = tokio::join!(
        first_store.create_draft_checkpoint(input.clone()),
        second_store.create_draft_checkpoint(input),
    );
    let first = first.expect("first concurrent checkpoint should save");
    let second = second.expect("second concurrent checkpoint should replay");

    assert_eq!(first.session_id, second.session_id);
    assert_eq!(first.id, second.id);
    assert_ne!(first.replayed, second.replayed);
    assert_eq!(session_count(&runtime, &client.id).await, 1);
}
