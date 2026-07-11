use crate::StateRuntime;
use crate::runtime::test_support::unique_temp_dir;
use pretty_assertions::assert_eq;

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

async fn create_session(
    runtime: &StateRuntime,
    client: &crate::WorkspaceClient,
    title: &str,
) -> crate::WorkspaceDraftCheckpoint {
    runtime
        .workspace()
        .create_draft_checkpoint(crate::WorkspaceDraftCheckpointCreate {
            client_id: client.id.clone(),
            draft_json: serde_json::json!({
                "schemaVersion": 1,
                "note": {"title": title},
            })
            .to_string(),
            trigger: "focus_change".to_string(),
            actor: "Clinician Example".to_string(),
            ..Default::default()
        })
        .await
        .expect("checkpoint should save")
}

fn global_filter(cursor: Option<(i64, String)>, limit: u32) -> crate::WorkspaceDraftSessionFilter {
    crate::WorkspaceDraftSessionFilter {
        scope: crate::WorkspaceDraftSessionScope::AllActiveClients,
        include_closed: false,
        cursor_updated_at_ms: cursor.as_ref().map(|(updated_at, _)| *updated_at),
        cursor_id: cursor.map(|(_, id)| id),
        limit: Some(limit),
    }
}

#[tokio::test]
async fn global_draft_sessions_paginate_exactly_across_active_patients() {
    let runtime = StateRuntime::init(unique_temp_dir(), "test-provider".to_string())
        .await
        .expect("state db should initialize");
    let first_client = create_client(&runtime, "First Synthetic Patient").await;
    let second_client = create_client(&runtime, "Second Synthetic Patient").await;
    let closed_client = create_client(&runtime, "Closed Synthetic Patient").await;
    let archived_client = create_client(&runtime, "Archived Synthetic Patient").await;

    let first = create_session(&runtime, &first_client, "First active draft").await;
    let second = create_session(&runtime, &second_client, "Second active draft").await;
    let closed = create_session(&runtime, &closed_client, "Closed draft").await;
    let archived = create_session(&runtime, &archived_client, "Archived patient draft").await;
    runtime
        .workspace()
        .close_draft_session(crate::WorkspaceDraftSessionClose {
            session_id: closed.session_id,
            client_id: closed_client.id,
            status: crate::WorkspaceDraftSessionTerminalStatus::Closed,
            expected_current_checkpoint_id: Some(closed.id),
            expected_current_checkpoint_revision: Some(closed.revision),
            expected_current_checkpoint_sha256: Some(closed.content_sha256),
            actor: "Clinician Example".to_string(),
            reason: "Canonical chart saved".to_string(),
        })
        .await
        .expect("session should close");
    assert!(
        runtime
            .workspace()
            .archive_client(&archived_client.id)
            .await
            .expect("patient should archive")
    );

    const SHARED_UPDATED_AT_MS: i64 = 1_700_000_000_042;
    sqlx::query("UPDATE workspace_draft_sessions SET updated_at_ms = ? WHERE id IN (?, ?)")
        .bind(SHARED_UPDATED_AT_MS)
        .bind(&first.session_id)
        .bind(&second.session_id)
        .execute(runtime.workspace().pool.as_ref())
        .await
        .expect("active session times should align");

    let first_page = runtime
        .workspace()
        .list_draft_sessions(global_filter(None, /*limit*/ 1))
        .await
        .expect("first global page should list");
    assert_eq!(first_page.len(), 1);
    let cursor_session = &first_page[0].session;
    assert_eq!(
        cursor_session.updated_at.timestamp_millis(),
        SHARED_UPDATED_AT_MS
    );
    let second_page = runtime
        .workspace()
        .list_draft_sessions(global_filter(
            Some((
                cursor_session.updated_at.timestamp_millis(),
                cursor_session.id.clone(),
            )),
            /*limit*/ 1,
        ))
        .await
        .expect("second global page should list");
    assert_eq!(second_page.len(), 1);
    let third_page = runtime
        .workspace()
        .list_draft_sessions(global_filter(
            Some((
                second_page[0].session.updated_at.timestamp_millis(),
                second_page[0].session.id.clone(),
            )),
            /*limit*/ 1,
        ))
        .await
        .expect("final global page should list");
    assert_eq!(third_page, Vec::new());

    let mut listed_ids = vec![
        first_page[0].session.id.clone(),
        second_page[0].session.id.clone(),
    ];
    listed_ids.sort();
    let mut expected_ids = vec![first.session_id, second.session_id];
    expected_ids.sort();
    assert_eq!(listed_ids, expected_ids);
    assert!(!listed_ids.contains(&archived.session_id));
}

#[tokio::test]
async fn global_draft_sessions_reject_closed_history() {
    let runtime = StateRuntime::init(unique_temp_dir(), "test-provider".to_string())
        .await
        .expect("state db should initialize");
    let error = runtime
        .workspace()
        .list_draft_sessions(crate::WorkspaceDraftSessionFilter {
            scope: crate::WorkspaceDraftSessionScope::AllActiveClients,
            include_closed: true,
            cursor_updated_at_ms: None,
            cursor_id: None,
            limit: Some(1),
        })
        .await
        .expect_err("global closed-history listing should fail");
    assert!(error.to_string().contains("cannot include closed"));
}
