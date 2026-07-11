use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::WorkspaceClientUpsertResponse;
use codex_app_server_protocol::WorkspaceDraftCheckpointCreateResponse;
use codex_app_server_protocol::WorkspaceDraftCheckpointListResponse;
use codex_app_server_protocol::WorkspaceDraftSessionCloseResponse;
use codex_app_server_protocol::WorkspaceDraftSessionListResponse;
use codex_app_server_protocol::WorkspaceDraftSessionStatus;
use pretty_assertions::assert_eq;
use serde::de::DeserializeOwned;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

use super::workspace_chart_commit::DEFAULT_READ_TIMEOUT;
use super::workspace_chart_commit::create_config_toml;

#[tokio::test]
async fn workspace_drafts_recover_exact_replayed_checkpoint_and_paginate() -> Result<()> {
    let (_codex_home, mut server) = server().await?;
    let client_id = create_client(&mut server, "Synthetic Draft Patient").await?;

    let first = create_checkpoint(
        &mut server,
        json!({
            "clientId": client_id,
            "noteId": "draft-note",
            "baseNoteRevision": 1,
            "draft": draft("First"),
            "trigger": "focusChange",
            "actor": "Clinician Example"
        }),
    )
    .await?;
    assert!(!first.replayed);
    assert_eq!(first.checkpoint.revision, 1);

    let second = create_checkpoint(
        &mut server,
        checkpoint_params(&client_id, &first.checkpoint.session_id, draft("Second")),
    )
    .await?;
    assert_eq!(second.checkpoint.revision, 2);

    let replay = create_checkpoint(
        &mut server,
        checkpoint_params(&client_id, &first.checkpoint.session_id, draft("First")),
    )
    .await?;
    assert!(replay.replayed);
    assert_eq!(replay.checkpoint.id, first.checkpoint.id);

    let sessions: WorkspaceDraftSessionListResponse = request(
        &mut server,
        "workspace/draft/session/list",
        json!({"clientId": client_id}),
    )
    .await?;
    assert_eq!(sessions.data.len(), 1);
    assert_eq!(sessions.data[0].current_revision, 1);
    assert_eq!(sessions.data[0].current_checkpoint.id, first.checkpoint.id);
    assert_eq!(sessions.data[0].current_checkpoint.draft, draft("First"));

    let first_page: WorkspaceDraftCheckpointListResponse = request(
        &mut server,
        "workspace/draft/checkpoint/list",
        json!({
            "clientId": client_id,
            "sessionId": first.checkpoint.session_id,
            "limit": 1
        }),
    )
    .await?;
    assert_eq!(first_page.data[0].revision, 2);
    let checkpoint_cursor = first_page
        .next_cursor
        .clone()
        .expect("first checkpoint page should continue");
    assert_ne!(checkpoint_cursor, "2");
    let second_page: WorkspaceDraftCheckpointListResponse = request(
        &mut server,
        "workspace/draft/checkpoint/list",
        json!({
            "clientId": client_id,
            "sessionId": first.checkpoint.session_id,
            "cursor": checkpoint_cursor,
            "limit": 1
        }),
    )
    .await?;
    assert_eq!(second_page.data[0].revision, 1);
    assert_eq!(second_page.next_cursor, None);

    let other_session = create_checkpoint(
        &mut server,
        json!({
            "clientId": client_id,
            "draft": draft("Other session"),
            "trigger": "manual",
            "actor": "Clinician Example"
        }),
    )
    .await?;
    let session_page: WorkspaceDraftSessionListResponse = request(
        &mut server,
        "workspace/draft/session/list",
        json!({"clientId": client_id, "limit": 1}),
    )
    .await?;
    assert_eq!(session_page.data.len(), 1);
    let session_cursor = session_page
        .next_cursor
        .clone()
        .expect("first session page should continue");
    assert!(!session_cursor.starts_with('{'));
    let remaining: WorkspaceDraftSessionListResponse = request(
        &mut server,
        "workspace/draft/session/list",
        json!({
            "clientId": client_id,
            "cursor": session_cursor,
            "limit": 1
        }),
    )
    .await?;
    let mut session_ids = vec![
        session_page.data[0].id.clone(),
        remaining.data[0].id.clone(),
    ];
    session_ids.sort();
    let mut expected_ids = vec![
        first.checkpoint.session_id,
        other_session.checkpoint.session_id,
    ];
    expected_ids.sort();
    assert_eq!(session_ids, expected_ids);
    Ok(())
}

#[tokio::test]
async fn workspace_drafts_close_and_validation_are_client_scoped() -> Result<()> {
    let (_codex_home, mut server) = server().await?;
    let client_id = create_client(&mut server, "Synthetic Draft Patient").await?;
    let other_client_id = create_client(&mut server, "Other Synthetic Patient").await?;
    let checkpoint = create_checkpoint(
        &mut server,
        json!({
            "clientId": client_id,
            "draft": draft("Discard me"),
            "trigger": "manual",
            "actor": "Clinician Example"
        }),
    )
    .await?;

    let wrong_client = request_error(
        &mut server,
        "workspace/draft/session/close",
        json!({
            "sessionId": checkpoint.checkpoint.session_id,
            "clientId": other_client_id,
            "status": "discarded",
            "actor": "Clinician Example",
            "reason": "Wrong patient"
        }),
    )
    .await?;
    assert!(wrong_client.error.message.contains("belongs to client"));

    let cross_client_create = request_error(
        &mut server,
        "workspace/draft/checkpoint/create",
        json!({
            "sessionId": checkpoint.checkpoint.session_id,
            "clientId": other_client_id,
            "draft": draft("Wrong patient"),
            "trigger": "manual",
            "actor": "Clinician Example"
        }),
    )
    .await?;
    assert!(
        cross_client_create
            .error
            .message
            .contains("belongs to client")
    );

    let close_params = json!({
        "sessionId": checkpoint.checkpoint.session_id,
        "clientId": client_id,
        "status": "discarded",
        "actor": "Clinician Example",
        "reason": "Dismiss recovered draft"
    });
    let discarded: WorkspaceDraftSessionCloseResponse = request(
        &mut server,
        "workspace/draft/session/close",
        close_params.clone(),
    )
    .await?;
    assert_eq!(
        discarded.session.status,
        WorkspaceDraftSessionStatus::Discarded
    );
    assert_eq!(
        discarded.session.current_checkpoint.id,
        checkpoint.checkpoint.id
    );
    let replayed_close: WorkspaceDraftSessionCloseResponse =
        request(&mut server, "workspace/draft/session/close", close_params).await?;
    assert_eq!(replayed_close, discarded);

    let active: WorkspaceDraftSessionListResponse = request(
        &mut server,
        "workspace/draft/session/list",
        json!({"clientId": client_id}),
    )
    .await?;
    assert_eq!(active.data, Vec::new());
    let archived: WorkspaceDraftSessionListResponse = request(
        &mut server,
        "workspace/draft/session/list",
        json!({"clientId": client_id, "includeClosed": true}),
    )
    .await?;
    assert_eq!(archived.data[0], discarded.session);

    let invalid_schema = request_error(
        &mut server,
        "workspace/draft/checkpoint/create",
        json!({
            "clientId": client_id,
            "draft": {"schemaVersion": 2},
            "trigger": "manual",
            "actor": "Clinician Example"
        }),
    )
    .await?;
    assert!(
        invalid_schema
            .error
            .message
            .contains("schemaVersion must be 1")
    );
    let negative_revision = request_error(
        &mut server,
        "workspace/draft/checkpoint/create",
        json!({
            "clientId": client_id,
            "baseNoteRevision": -1,
            "draft": draft("Invalid revision"),
            "trigger": "manual",
            "actor": "Clinician Example"
        }),
    )
    .await?;
    assert!(
        negative_revision
            .error
            .message
            .contains("must not be negative")
    );
    let invalid_cursor = request_error(
        &mut server,
        "workspace/draft/checkpoint/list",
        json!({
            "clientId": client_id,
            "sessionId": checkpoint.checkpoint.session_id,
            "cursor": "not-a-revision"
        }),
    )
    .await?;
    assert!(invalid_cursor.error.message.contains("checkpoint cursor"));
    let invalid_session_cursor = request_error(
        &mut server,
        "workspace/draft/session/list",
        json!({"clientId": client_id, "cursor": "not-a-cursor"}),
    )
    .await?;
    assert!(
        invalid_session_cursor
            .error
            .message
            .contains("session cursor")
    );
    Ok(())
}

async fn server() -> Result<(TempDir, TestAppServer)> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, server.initialize()).await??;
    Ok((codex_home, server))
}

async fn create_client(server: &mut TestAppServer, display_name: &str) -> Result<String> {
    let response: WorkspaceClientUpsertResponse = request(
        server,
        "workspace/client/upsert",
        json!({"displayName": display_name, "summary": ""}),
    )
    .await?;
    Ok(response.client.id)
}

async fn create_checkpoint(
    server: &mut TestAppServer,
    params: Value,
) -> Result<WorkspaceDraftCheckpointCreateResponse> {
    request(server, "workspace/draft/checkpoint/create", params).await
}

fn checkpoint_params(client_id: &str, session_id: &str, draft: Value) -> Value {
    json!({
        "sessionId": session_id,
        "clientId": client_id,
        "noteId": "draft-note",
        "baseNoteRevision": 1,
        "draft": draft,
        "trigger": "focusChange",
        "actor": "Clinician Example"
    })
}

fn draft(title: &str) -> Value {
    json!({"schemaVersion": 1, "note": {"title": title}})
}

async fn request<T: DeserializeOwned>(
    server: &mut TestAppServer,
    method: &str,
    params: Value,
) -> Result<T> {
    let request_id = server.send_raw_request(method, Some(params)).await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn request_error(
    server: &mut TestAppServer,
    method: &str,
    params: Value,
) -> Result<JSONRPCError> {
    let request_id = server.send_raw_request(method, Some(params)).await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await?
}
