use anyhow::Result;
use codex_app_server_protocol::WorkspaceDraftSessionListResponse;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::workspace_drafts::create_checkpoint;
use super::workspace_drafts::create_client;
use super::workspace_drafts::draft;
use super::workspace_drafts::request;
use super::workspace_drafts::request_error;
use super::workspace_drafts::server;

#[tokio::test]
async fn workspace_draft_creation_key_maps_to_one_replayable_session() -> Result<()> {
    let (_codex_home, mut server) = server().await?;
    let client_id = create_client(&mut server, "Keyed Draft Patient").await?;
    let first = create_checkpoint(
        &mut server,
        json!({
            "sessionCreationKey": " first-save ",
            "clientId": client_id,
            "noteId": "draft-note",
            "baseNoteRevision": 1,
            "draft": draft("First"),
            "trigger": "focusChange",
            "actor": "Clinician Example"
        }),
    )
    .await?;
    let replay = create_checkpoint(
        &mut server,
        json!({
            "sessionCreationKey": "first-save",
            "clientId": client_id,
            "noteId": "draft-note",
            "baseNoteRevision": 1,
            "draft": draft("First"),
            "trigger": "focusChange",
            "actor": "Clinician Example"
        }),
    )
    .await?;
    let changed = create_checkpoint(
        &mut server,
        json!({
            "sessionCreationKey": "first-save",
            "clientId": client_id,
            "noteId": "draft-note",
            "baseNoteRevision": 1,
            "draft": draft("Second"),
            "trigger": "focusChange",
            "actor": "Clinician Example"
        }),
    )
    .await?;

    assert!(replay.replayed);
    assert_eq!(replay.checkpoint.id, first.checkpoint.id);
    assert_eq!(replay.checkpoint.session_id, first.checkpoint.session_id);
    assert_eq!(changed.checkpoint.session_id, first.checkpoint.session_id);
    assert_eq!(changed.checkpoint.revision, 2);

    let sessions: WorkspaceDraftSessionListResponse = request(
        &mut server,
        "workspace/draft/session/list",
        json!({"clientId": client_id, "includeClosed": true}),
    )
    .await?;
    assert_eq!(sessions.data.len(), 1);
    assert!(
        !serde_json::to_string(&sessions)?.contains("sessionCreationKey"),
        "creation keys must remain private storage metadata"
    );
    Ok(())
}

#[tokio::test]
async fn workspace_draft_creation_key_validates_combinations_and_byte_limit() -> Result<()> {
    let (_codex_home, mut server) = server().await?;
    let client_id = create_client(&mut server, "Creation Key Validation Patient").await?;
    let legacy = create_checkpoint(
        &mut server,
        json!({
            "clientId": client_id,
            "draft": draft("Legacy"),
            "trigger": "manual",
            "actor": "Legacy client"
        }),
    )
    .await?;

    let both = request_error(
        &mut server,
        "workspace/draft/checkpoint/create",
        json!({
            "sessionId": legacy.checkpoint.session_id,
            "sessionCreationKey": "first-save",
            "clientId": client_id,
            "draft": draft("Both"),
            "trigger": "manual",
            "actor": "Clinician Example"
        }),
    )
    .await?;
    assert!(both.error.message.contains("not both"));

    let empty = request_error(
        &mut server,
        "workspace/draft/checkpoint/create",
        json!({
            "sessionCreationKey": "   ",
            "clientId": client_id,
            "draft": draft("Empty"),
            "trigger": "manual",
            "actor": "Clinician Example"
        }),
    )
    .await?;
    assert!(empty.error.message.contains("must not be empty"));

    let oversized = request_error(
        &mut server,
        "workspace/draft/checkpoint/create",
        json!({
            "sessionCreationKey": "x".repeat(257),
            "clientId": client_id,
            "draft": draft("Oversized"),
            "trigger": "manual",
            "actor": "Clinician Example"
        }),
    )
    .await?;
    assert!(
        oversized
            .error
            .message
            .contains("must not exceed 256 bytes")
    );
    Ok(())
}
