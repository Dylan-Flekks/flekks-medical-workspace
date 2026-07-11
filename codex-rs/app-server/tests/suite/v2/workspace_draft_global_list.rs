use anyhow::Result;
use codex_app_server_protocol::WorkspaceDraftCheckpointCreateResponse;
use codex_app_server_protocol::WorkspaceDraftSessionCloseResponse;
use codex_app_server_protocol::WorkspaceDraftSessionListResponse;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

use super::workspace_drafts::create_checkpoint;
use super::workspace_drafts::create_client;
use super::workspace_drafts::draft;
use super::workspace_drafts::request;
use super::workspace_drafts::request_error;
use super::workspace_drafts::server;

async fn create_session(
    server: &mut app_test_support::TestAppServer,
    client_id: &str,
    title: &str,
) -> Result<WorkspaceDraftCheckpointCreateResponse> {
    create_checkpoint(
        server,
        json!({
            "clientId": client_id,
            "draft": draft(title),
            "trigger": "focusChange",
            "actor": "Clinician Example"
        }),
    )
    .await
}

#[tokio::test]
async fn global_active_drafts_are_explicit_bounded_and_paginated_across_patients() -> Result<()> {
    let (_codex_home, mut server) = server().await?;
    let first_client = create_client(&mut server, "First Synthetic Patient").await?;
    let second_client = create_client(&mut server, "Second Synthetic Patient").await?;
    let closed_client = create_client(&mut server, "Closed Synthetic Patient").await?;
    let archived_client = create_client(&mut server, "Archived Synthetic Patient").await?;
    let first = create_session(&mut server, &first_client, "First active draft").await?;
    let second = create_session(&mut server, &second_client, "Second active draft").await?;
    let closed = create_session(&mut server, &closed_client, "Closed draft").await?;
    let _archived = create_session(&mut server, &archived_client, "Archived draft").await?;

    let _: WorkspaceDraftSessionCloseResponse = request(
        &mut server,
        "workspace/draft/session/close",
        json!({
            "sessionId": closed.checkpoint.session_id,
            "clientId": closed_client,
            "status": "closed",
            "expectedCurrentCheckpointId": closed.checkpoint.id,
            "expectedCurrentCheckpointRevision": closed.checkpoint.revision,
            "expectedCurrentCheckpointSha256": closed.checkpoint.content_sha256,
            "actor": "Clinician Example",
            "reason": "Canonical chart saved"
        }),
    )
    .await?;
    let _: Value = request(
        &mut server,
        "workspace/client/archive",
        json!({"clientId": archived_client}),
    )
    .await?;

    let patient_scoped: WorkspaceDraftSessionListResponse = request(
        &mut server,
        "workspace/draft/session/list",
        json!({"clientId": first_client}),
    )
    .await?;
    assert_eq!(patient_scoped.data.len(), 1);
    assert_eq!(patient_scoped.data[0].id, first.checkpoint.session_id);

    let first_page: WorkspaceDraftSessionListResponse = request(
        &mut server,
        "workspace/draft/session/list",
        json!({"allClients": true, "limit": 100}),
    )
    .await?;
    assert_eq!(first_page.data.len(), 1);
    let first_cursor = first_page
        .next_cursor
        .expect("global page cap should leave a cursor");
    let second_page: WorkspaceDraftSessionListResponse = request(
        &mut server,
        "workspace/draft/session/list",
        json!({"allClients": true, "cursor": first_cursor, "limit": 100}),
    )
    .await?;
    assert_eq!(second_page.data.len(), 1);
    let mut listed_ids = vec![
        first_page.data[0].id.clone(),
        second_page.data[0].id.clone(),
    ];
    listed_ids.sort();
    let mut expected_ids = vec![first.checkpoint.session_id, second.checkpoint.session_id];
    expected_ids.sort();
    assert_eq!(listed_ids, expected_ids);
    assert_eq!(second_page.next_cursor, None);

    let neither = request_error(&mut server, "workspace/draft/session/list", json!({})).await?;
    assert!(
        neither
            .error
            .message
            .contains("requires clientId or allClients")
    );
    let both = request_error(
        &mut server,
        "workspace/draft/session/list",
        json!({"clientId": first_client, "allClients": true}),
    )
    .await?;
    assert!(both.error.message.contains("not both"));
    let global_history = request_error(
        &mut server,
        "workspace/draft/session/list",
        json!({"allClients": true, "includeClosed": true}),
    )
    .await?;
    assert!(
        global_history
            .error
            .message
            .contains("cannot include closed")
    );
    Ok(())
}
