use crate::StateRuntime;
use crate::runtime::test_support::unique_temp_dir;
use pretty_assertions::assert_eq;
use sha2::Digest;

async fn fixture() -> (std::sync::Arc<StateRuntime>, crate::WorkspaceClient) {
    let runtime = StateRuntime::init(unique_temp_dir(), "test-provider".to_string())
        .await
        .expect("state db should initialize");
    let client = runtime
        .workspace()
        .upsert_client(crate::WorkspaceClientUpsert {
            display_name: "Synthetic Draft Patient".to_string(),
            ..Default::default()
        })
        .await
        .expect("client should save");
    (runtime, client)
}

fn input(
    client: &crate::WorkspaceClient,
    session_id: Option<String>,
    title: &str,
) -> crate::WorkspaceDraftCheckpointCreate {
    crate::WorkspaceDraftCheckpointCreate {
        session_id,
        client_id: client.id.clone(),
        note_id: Some("draft-note".to_string()),
        base_note_revision: Some(1),
        draft_json: format!(r#"{{ "note": {{ "title": {title:?} }}, "schemaVersion": 1 }}"#),
        trigger: "focus_change".to_string(),
        actor: "Clinician Example".to_string(),
        ..Default::default()
    }
}

async fn checkpoint_error(
    runtime: &StateRuntime,
    input: crate::WorkspaceDraftCheckpointCreate,
) -> String {
    runtime
        .workspace()
        .create_draft_checkpoint(input)
        .await
        .expect_err("checkpoint should fail")
        .to_string()
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

#[tokio::test]
async fn workspace_drafts_checkpoint_is_normalized_idempotent_and_revisioned() {
    let (runtime, client) = fixture().await;
    let first = save(&runtime, input(&client, None, "First")).await;
    assert!(!first.replayed);
    assert_eq!(first.revision, 1);
    assert_eq!(
        first.content_sha256,
        format!("{:x}", sha2::Sha256::digest(first.draft_json.as_bytes()))
    );

    let replay = save(
        &runtime,
        input(&client, Some(first.session_id.clone()), "First"),
    )
    .await;
    assert!(replay.replayed);
    assert_eq!(replay.id, first.id);
    let second = save(
        &runtime,
        input(&client, Some(first.session_id.clone()), "Second"),
    )
    .await;
    assert_eq!(second.revision, 2);

    save(
        &runtime,
        input(&client, Some(first.session_id.clone()), "First"),
    )
    .await;
    let current = runtime
        .workspace()
        .list_draft_checkpoints(crate::WorkspaceDraftCheckpointFilter {
            client_id: client.id.clone(),
            ..Default::default()
        })
        .await
        .expect("current checkpoint should list");
    assert_eq!(current[0].id, first.id);
    let history = runtime
        .workspace()
        .list_draft_checkpoints(crate::WorkspaceDraftCheckpointFilter {
            client_id: client.id.clone(),
            session_id: Some(first.session_id.clone()),
            ..Default::default()
        })
        .await
        .expect("full session history should list");
    assert_eq!(history.len(), 2);
    let first_history_page = runtime
        .workspace()
        .list_draft_checkpoints(crate::WorkspaceDraftCheckpointFilter {
            client_id: client.id.clone(),
            session_id: Some(first.session_id.clone()),
            limit: Some(1),
            ..Default::default()
        })
        .await
        .expect("first history page should list");
    assert_eq!(first_history_page[0].revision, 2);
    let second_history_page = runtime
        .workspace()
        .list_draft_checkpoints(crate::WorkspaceDraftCheckpointFilter {
            client_id: client.id.clone(),
            session_id: Some(first.session_id.clone()),
            cursor_before_revision: Some(first_history_page[0].revision),
            limit: Some(1),
        })
        .await
        .expect("second history page should list");
    assert_eq!(second_history_page[0].revision, 1);

    let sessions = runtime
        .workspace()
        .list_draft_sessions(crate::WorkspaceDraftSessionFilter {
            client_id: client.id.clone(),
            ..Default::default()
        })
        .await
        .expect("draft sessions should list");
    assert_eq!(sessions[0].session.current_revision, 1);
    assert_eq!(sessions[0].current_checkpoint.id, first.id);

    let mut mismatch = input(&client, Some(first.session_id), "First");
    mismatch.note_id = Some("different-note".to_string());
    let error = checkpoint_error(&runtime, mismatch).await;
    assert!(error.contains("different metadata"));
}

#[tokio::test]
async fn workspace_drafts_checkpoint_validates_client_schema_and_size() {
    let (runtime, client) = fixture().await;
    let mut missing_client = input(&client, None, "Missing");
    missing_client.client_id = "missing-client".to_string();
    assert!(
        checkpoint_error(&runtime, missing_client)
            .await
            .contains("not found or is archived")
    );

    let mut bad_schema = input(&client, None, "Bad schema");
    bad_schema.draft_json = r#"{"schemaVersion":2}"#.to_string();
    assert!(
        checkpoint_error(&runtime, bad_schema)
            .await
            .contains("schemaVersion 2")
    );

    let mut oversized = input(&client, None, "Large");
    oversized.draft_json = serde_json::json!({
        "schemaVersion": 1,
        "body": "x".repeat(1024 * 1024),
    })
    .to_string();
    assert!(
        checkpoint_error(&runtime, oversized)
            .await
            .contains("normalized limit")
    );
}

#[tokio::test]
async fn workspace_draft_session_cursor_is_stable_for_equal_timestamps() {
    let (runtime, client) = fixture().await;
    let first = save(&runtime, input(&client, None, "First session")).await;
    let second = save(&runtime, input(&client, None, "Second session")).await;
    const SHARED_UPDATED_AT_MS: i64 = 1_700_000_000_042;
    sqlx::query("UPDATE workspace_draft_sessions SET updated_at_ms = ?")
        .bind(SHARED_UPDATED_AT_MS)
        .execute(runtime.workspace().pool.as_ref())
        .await
        .expect("session times should align");

    let first_page = runtime
        .workspace()
        .list_draft_sessions(crate::WorkspaceDraftSessionFilter {
            client_id: client.id.clone(),
            limit: Some(1),
            ..Default::default()
        })
        .await
        .expect("first session page should list");
    let cursor = &first_page[0].session;
    assert_eq!(cursor.updated_at.timestamp_millis(), SHARED_UPDATED_AT_MS);
    let second_page = runtime
        .workspace()
        .list_draft_sessions(crate::WorkspaceDraftSessionFilter {
            client_id: client.id.clone(),
            cursor_updated_at_ms: Some(cursor.updated_at.timestamp_millis()),
            cursor_id: Some(cursor.id.clone()),
            limit: Some(1),
            ..Default::default()
        })
        .await
        .expect("second session page should list");

    let mut ids = vec![
        first_page[0].session.id.clone(),
        second_page[0].session.id.clone(),
    ];
    ids.sort();
    let mut expected = vec![first.session_id.clone(), second.session_id.clone()];
    expected.sort();
    assert_eq!(ids, expected);

    let other = runtime
        .workspace()
        .upsert_client(crate::WorkspaceClientUpsert {
            display_name: "Other Synthetic Patient".to_string(),
            ..Default::default()
        })
        .await
        .expect("other client should save");
    sqlx::query("UPDATE workspace_draft_checkpoints SET client_id = ? WHERE session_id = ?")
        .bind(other.id)
        .bind(&first.session_id)
        .execute(runtime.workspace().pool.as_ref())
        .await
        .expect("test checkpoint scope should change");
    let scoped = runtime
        .workspace()
        .list_draft_sessions(crate::WorkspaceDraftSessionFilter {
            client_id: client.id,
            ..Default::default()
        })
        .await
        .expect("patient-scoped sessions should list");
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].session.id, second.session_id);
}

#[tokio::test]
async fn workspace_drafts_discard_is_durable_idempotent_and_client_scoped() {
    let (runtime, client) = fixture().await;
    let other = runtime
        .workspace()
        .upsert_client(crate::WorkspaceClientUpsert {
            display_name: "Other Synthetic Patient".to_string(),
            ..Default::default()
        })
        .await
        .expect("other client should save");
    let checkpoint = save(&runtime, input(&client, None, "Discard me")).await;
    let close = crate::WorkspaceDraftSessionClose {
        session_id: checkpoint.session_id.clone(),
        client_id: client.id.clone(),
        status: crate::WorkspaceDraftSessionTerminalStatus::Discarded,
        actor: "Clinician Example".to_string(),
        reason: "Dismiss recovered draft.".to_string(),
    };
    let mut wrong_client = close.clone();
    wrong_client.client_id = other.id;
    assert!(
        runtime
            .workspace()
            .close_draft_session(wrong_client)
            .await
            .expect_err("cross-client discard should fail")
            .to_string()
            .contains("belongs to client")
    );
    sqlx::query(
        "CREATE TRIGGER ignore_draft_close BEFORE UPDATE ON workspace_draft_sessions BEGIN SELECT RAISE(IGNORE); END",
    )
    .execute(runtime.workspace().pool.as_ref())
    .await
    .expect("test trigger should install");
    let error = runtime
        .workspace()
        .close_draft_session(close.clone())
        .await
        .expect_err("ignored lifecycle write should fail closed");
    assert!(error.to_string().contains("changed concurrently"));
    sqlx::query("DROP TRIGGER ignore_draft_close")
        .execute(runtime.workspace().pool.as_ref())
        .await
        .expect("test trigger should drop");
    let discarded = runtime
        .workspace()
        .close_draft_session(close.clone())
        .await
        .expect("discard should persist");
    assert_eq!(discarded.session.status, "discarded");
    assert_eq!(
        runtime
            .workspace()
            .close_draft_session(close)
            .await
            .expect("same discard should replay"),
        discarded
    );
    assert!(
        runtime
            .workspace()
            .list_draft_sessions(crate::WorkspaceDraftSessionFilter {
                client_id: client.id.clone(),
                ..Default::default()
            })
            .await
            .expect("active sessions should list")
            .is_empty()
    );
    let rejected = input(&client, Some(checkpoint.session_id), "Newer");
    assert!(
        checkpoint_error(&runtime, rejected)
            .await
            .contains("cannot checkpoint")
    );
}
