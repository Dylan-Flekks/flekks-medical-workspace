use anyhow::Result;
use codex_app_server_protocol::WorkspaceAgentRunListResponse;
use codex_app_server_protocol::WorkspaceContextPacketListResponse;
use codex_app_server_protocol::WorkspaceNoteListResponse;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use sha2::Digest;

use super::workspace_drafts::create_checkpoint;
use super::workspace_drafts::create_client;
use super::workspace_drafts::request;
use super::workspace_drafts::request_error;
use super::workspace_drafts::server;

fn v1_snapshot() -> Value {
    json!({
        "schemaVersion": 1,
        "patient": {"displayName": "Schema Snapshot Patient"},
        "note": {"title": "Daily note", "body": "Human-authored draft"},
    })
}

fn v2_snapshot(request: &str) -> Value {
    json!({
        "schemaVersion": 2,
        "patient": {"displayName": "Schema Snapshot Patient"},
        "note": {"title": "Daily note", "body": "Human-authored draft"},
        "agentRequest": {"body": request, "status": "draft"},
        "contextSelection": {
            "includeVisitHistory": true,
            "includeProgressNotes": true,
            "selectedSourceIds": ["source-note-1"],
        },
    })
}

#[tokio::test]
async fn workspace_draft_v1_remains_compatible_and_v2_replays_and_revisions() -> Result<()> {
    let (_codex_home, mut server) = server().await?;
    let client_id = create_client(&mut server, "Schema Snapshot Patient").await?;

    let v1 = create_checkpoint(
        &mut server,
        json!({
            "clientId": client_id,
            "draft": v1_snapshot(),
            "trigger": "manual",
            "actor": "Clinician Example"
        }),
    )
    .await?;
    assert_eq!(v1.checkpoint.schema_version, 1);
    assert_eq!(v1.checkpoint.revision, 1);

    let first_value = v2_snapshot("Prepare a similar daily-note template.");
    let first = create_checkpoint(
        &mut server,
        json!({
            "sessionCreationKey": "schema-v2-first-save",
            "clientId": client_id,
            "noteId": "draft-note",
            "baseNoteRevision": 1,
            "draft": first_value,
            "trigger": "focusChange",
            "actor": "Clinician Example"
        }),
    )
    .await?;
    assert!(!first.replayed);
    assert_eq!(first.checkpoint.schema_version, 2);
    assert_eq!(first.checkpoint.revision, 1);
    assert_eq!(first.checkpoint.draft, first_value);
    assert_eq!(
        first.checkpoint.content_sha256,
        format!(
            "{:x}",
            sha2::Sha256::digest(serde_json::to_string(&first_value)?.as_bytes())
        )
    );

    let replay = create_checkpoint(
        &mut server,
        json!({
            "sessionCreationKey": "schema-v2-first-save",
            "clientId": client_id,
            "noteId": "draft-note",
            "baseNoteRevision": 1,
            "draft": first_value,
            "trigger": "focusChange",
            "actor": "Clinician Example"
        }),
    )
    .await?;
    assert!(replay.replayed);
    assert_eq!(replay.checkpoint.id, first.checkpoint.id);
    assert_eq!(replay.checkpoint.session_id, first.checkpoint.session_id);

    let changed_value = v2_snapshot("Prepare the template and flag missing objective measures.");
    let changed = create_checkpoint(
        &mut server,
        json!({
            "sessionCreationKey": "schema-v2-first-save",
            "clientId": client_id,
            "noteId": "draft-note",
            "baseNoteRevision": 1,
            "draft": changed_value,
            "trigger": "focusChange",
            "actor": "Clinician Example"
        }),
    )
    .await?;
    assert!(!changed.replayed);
    assert_eq!(changed.checkpoint.session_id, first.checkpoint.session_id);
    assert_eq!(changed.checkpoint.schema_version, 2);
    assert_eq!(changed.checkpoint.revision, 2);

    let notes: WorkspaceNoteListResponse = request(
        &mut server,
        "workspace/note/list",
        json!({"clientId": client_id}),
    )
    .await?;
    let packets: WorkspaceContextPacketListResponse = request(
        &mut server,
        "workspace/context/packet/list",
        json!({"clientId": client_id}),
    )
    .await?;
    let runs: WorkspaceAgentRunListResponse = request(
        &mut server,
        "workspace/agent/run/list",
        json!({"clientId": client_id}),
    )
    .await?;
    assert!(
        notes.notes.is_empty(),
        "checkpointing must not write a note"
    );
    assert!(
        packets.packets.is_empty(),
        "checkpointing must not submit a context packet"
    );
    assert!(
        runs.runs.is_empty(),
        "checkpointing must not launch a model"
    );
    Ok(())
}

#[tokio::test]
async fn workspace_draft_rejects_unsupported_versions_and_bounds_v2_size() -> Result<()> {
    let (_codex_home, mut server) = server().await?;
    let client_id = create_client(&mut server, "Schema Validation Patient").await?;

    for unsupported in [0, 3] {
        let error = request_error(
            &mut server,
            "workspace/draft/checkpoint/create",
            json!({
                "clientId": client_id,
                "draft": {"schemaVersion": unsupported},
                "trigger": "manual",
                "actor": "Clinician Example"
            }),
        )
        .await?;
        assert!(error.error.message.contains("schemaVersion must be 1 or 2"));
    }

    let oversized = request_error(
        &mut server,
        "workspace/draft/checkpoint/create",
        json!({
            "clientId": client_id,
            "draft": {
                "schemaVersion": 2,
                "agentRequest": {"body": "x".repeat(1024 * 1024)}
            },
            "trigger": "manual",
            "actor": "Clinician Example"
        }),
    )
    .await?;
    assert!(
        oversized
            .error
            .message
            .contains("1048576 byte normalized limit")
    );
    Ok(())
}
