use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::WorkspaceClientUpsertResponse;
use codex_app_server_protocol::WorkspaceContextPacketCreateResponse;
use codex_app_server_protocol::WorkspaceContextPacketListResponse;
use codex_app_server_protocol::WorkspaceDraftCheckpoint;
use codex_app_server_protocol::WorkspaceDraftCheckpointCreateResponse;
use pretty_assertions::assert_eq;
use serde::de::DeserializeOwned;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

use super::workspace_chart_commit::DEFAULT_READ_TIMEOUT;
use super::workspace_chart_commit::create_config_toml;

#[tokio::test]
async fn context_packets_bind_exact_current_checkpoint_and_preserve_legacy_create() -> Result<()> {
    let (_codex_home, mut server) = server().await?;
    let client_id = create_client(&mut server).await?;
    let first = create_checkpoint(&mut server, &client_id, None, "First").await?;
    let bound_params = packet_params(&client_id, Some(&first));

    let created: WorkspaceContextPacketCreateResponse = request(
        &mut server,
        "workspace/context/packet/create",
        bound_params.clone(),
    )
    .await?;
    assert_eq!(
        (
            created.packet.source_draft_session_id.as_deref(),
            created.packet.source_draft_checkpoint_id.as_deref(),
            created.packet.source_draft_checkpoint_revision,
            created.packet.source_draft_checkpoint_sha256.as_deref(),
        ),
        (
            Some(first.session_id.as_str()),
            Some(first.id.as_str()),
            Some(first.revision),
            Some(first.content_sha256.as_str()),
        )
    );

    let replayed: WorkspaceContextPacketCreateResponse = request(
        &mut server,
        "workspace/context/packet/create",
        bound_params.clone(),
    )
    .await?;
    assert_eq!(replayed, created);

    let listed: WorkspaceContextPacketListResponse = request(
        &mut server,
        "workspace/context/packet/list",
        json!({"clientId": client_id}),
    )
    .await?;
    assert_eq!(listed.packets, vec![created.packet.clone()]);

    let mut partial = bound_params.clone();
    partial
        .as_object_mut()
        .expect("packet params should be an object")
        .remove("sourceDraftCheckpointSha256");
    let partial_error =
        request_error(&mut server, "workspace/context/packet/create", partial).await?;
    assert!(
        partial_error
            .error
            .message
            .contains("requires session, checkpoint, revision, and hash")
    );

    let mut mismatched = bound_params.clone();
    mismatched["sourceDraftCheckpointRevision"] = json!(first.revision + 1);
    let mismatch_error =
        request_error(&mut server, "workspace/context/packet/create", mismatched).await?;
    assert!(
        mismatch_error
            .error
            .message
            .contains("identity does not match packet scope")
    );

    create_checkpoint(
        &mut server,
        &client_id,
        Some(first.session_id.as_str()),
        "Second",
    )
    .await?;
    let stale_error =
        request_error(&mut server, "workspace/context/packet/create", bound_params).await?;
    assert!(
        stale_error
            .error
            .message
            .contains("draft checkpoint is no longer current")
    );

    let legacy: WorkspaceContextPacketCreateResponse = request(
        &mut server,
        "workspace/context/packet/create",
        packet_params(&client_id, None),
    )
    .await?;
    assert_eq!(
        (
            legacy.packet.source_draft_session_id,
            legacy.packet.source_draft_checkpoint_id,
            legacy.packet.source_draft_checkpoint_revision,
            legacy.packet.source_draft_checkpoint_sha256,
        ),
        (None, None, None, None)
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

async fn create_client(server: &mut TestAppServer) -> Result<String> {
    let response: WorkspaceClientUpsertResponse = request(
        server,
        "workspace/client/upsert",
        json!({
            "displayName": "Synthetic Packet Patient",
            "summary": ""
        }),
    )
    .await?;
    Ok(response.client.id)
}

async fn create_checkpoint(
    server: &mut TestAppServer,
    client_id: &str,
    session_id: Option<&str>,
    title: &str,
) -> Result<WorkspaceDraftCheckpoint> {
    let response: WorkspaceDraftCheckpointCreateResponse = request(
        server,
        "workspace/draft/checkpoint/create",
        json!({
            "sessionId": session_id,
            "clientId": client_id,
            "draft": {
                "schemaVersion": 1,
                "note": {"title": title, "body": "Synthetic working draft"}
            },
            "trigger": "ctrl_g_review",
            "actor": "Clinician Example"
        }),
    )
    .await?;
    Ok(response.checkpoint)
}

fn packet_params(client_id: &str, checkpoint: Option<&WorkspaceDraftCheckpoint>) -> Value {
    let human_request = "Review the synthetic daily note.";
    let mut envelope = json!({
        "assemblyVersion": "packet-api-test-v1",
        "includeDocuments": false,
        "humanRequest": human_request,
        "ids": {
            "selectedArtifactIds": [],
            "selectedDerivativeIds": [],
            "selectedClipIds": []
        },
        "safety": [
            "read-only context packet; do not mutate workspace records",
            "do not sign notes, submit claims, send payer communications, or overwrite saved data"
        ],
        "promptSnapshot": "Synthetic reviewed checkpoint packet."
    });
    if let Some(checkpoint) = checkpoint {
        envelope["sourceCheckpoint"] = json!({
            "clientId": client_id,
            "sessionId": checkpoint.session_id,
            "id": checkpoint.id,
            "revision": checkpoint.revision,
            "contentSha256": checkpoint.content_sha256,
            "encounterId": null,
            "noteId": null,
            "baseNoteRevision": null
        });
    }
    let mut params = json!({
        "clientId": client_id,
        "humanRequest": human_request,
        "selectedArtifactIdsJson": "[]",
        "selectedDerivativeIdsJson": "[]",
        "selectedClipIdsJson": "[]",
        "artifactSummary": "",
        "derivativeSummary": "",
        "clipSummary": "",
        "chartContextSummary": "",
        "contextEnvelopeJson": envelope.to_string(),
        "clinicianActor": "Clinician Example"
    });
    if let Some(checkpoint) = checkpoint {
        params["sourceDraftSessionId"] = json!(checkpoint.session_id);
        params["sourceDraftCheckpointId"] = json!(checkpoint.id);
        params["sourceDraftCheckpointRevision"] = json!(checkpoint.revision);
        params["sourceDraftCheckpointSha256"] = json!(checkpoint.content_sha256);
    }
    params
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
