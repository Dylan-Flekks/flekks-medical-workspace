use crate::StateRuntime;
use crate::runtime::test_support::unique_temp_dir;

async fn fixture() -> (
    std::sync::Arc<StateRuntime>,
    crate::WorkspaceClient,
    crate::WorkspaceNote,
    crate::WorkspaceDraftCheckpoint,
) {
    let runtime = StateRuntime::init(unique_temp_dir(), "test-provider".to_string())
        .await
        .expect("state db should initialize");
    let client = runtime
        .workspace()
        .upsert_client(crate::WorkspaceClientUpsert {
            display_name: "Synthetic Packet Patient".to_string(),
            ..Default::default()
        })
        .await
        .expect("client should save");
    let note = runtime
        .workspace()
        .upsert_note(crate::WorkspaceNoteUpsert {
            client_id: client.id.clone(),
            title: "Synthetic daily note".to_string(),
            kind: "daily".to_string(),
            body: "Synthetic note body".to_string(),
            status: "draft".to_string(),
            actor: "Clinician Example".to_string(),
            ..Default::default()
        })
        .await
        .expect("note should save");
    let checkpoint = checkpoint(&runtime, &client, &note, None, "First").await;
    (runtime, client, note, checkpoint)
}

async fn checkpoint(
    runtime: &StateRuntime,
    client: &crate::WorkspaceClient,
    note: &crate::WorkspaceNote,
    session_id: Option<String>,
    title: &str,
) -> crate::WorkspaceDraftCheckpoint {
    runtime
        .workspace()
        .create_draft_checkpoint(crate::WorkspaceDraftCheckpointCreate {
            session_id,
            client_id: client.id.clone(),
            note_id: Some(note.id.clone()),
            base_note_revision: Some(note.current_revision),
            draft_json: serde_json::json!({
                "schemaVersion": 1,
                "note": {"title": title, "body": "Synthetic working draft"},
            })
            .to_string(),
            trigger: "ctrl_g_review".to_string(),
            actor: "Clinician Example".to_string(),
            ..Default::default()
        })
        .await
        .expect("checkpoint should save")
}

fn packet(
    client: &crate::WorkspaceClient,
    note: &crate::WorkspaceNote,
    checkpoint: &crate::WorkspaceDraftCheckpoint,
    request: &str,
) -> crate::WorkspaceContextPacketCreate {
    let source_checkpoint = serde_json::json!({
        "clientId": client.id,
        "sessionId": checkpoint.session_id,
        "id": checkpoint.id,
        "revision": checkpoint.revision,
        "contentSha256": checkpoint.content_sha256,
        "encounterId": null,
        "noteId": note.id,
        "baseNoteRevision": note.current_revision,
    });
    let context_envelope_json = serde_json::json!({
        "assemblyVersion": "checkpoint-test-v1",
        "sourceMode": "ctrl_g_handoff",
        "sourceCheckpoint": source_checkpoint,
        "includeDocuments": false,
        "humanRequest": request,
        "ids": {
            "selectedArtifactIds": [],
            "selectedDerivativeIds": [],
            "selectedClipIds": [],
        },
        "safety": [
            "read-only context packet; do not mutate workspace records",
            "do not sign notes, submit claims, send payer communications, or overwrite saved data"
        ],
        "promptSnapshot": "Synthetic reviewed checkpoint packet.",
    })
    .to_string();
    crate::WorkspaceContextPacketCreate {
        client_id: client.id.clone(),
        note_id: Some(note.id.clone()),
        source_draft_session_id: Some(checkpoint.session_id.clone()),
        source_draft_checkpoint_id: Some(checkpoint.id.clone()),
        source_draft_checkpoint_revision: Some(checkpoint.revision),
        source_draft_checkpoint_sha256: Some(checkpoint.content_sha256.clone()),
        human_request: request.to_string(),
        selected_artifact_ids_json: "[]".to_string(),
        selected_derivative_ids_json: "[]".to_string(),
        selected_clip_ids_json: "[]".to_string(),
        artifact_summary: "SENSITIVE_ARTIFACT_SUMMARY".to_string(),
        derivative_summary: "SENSITIVE_DERIVATIVE_SUMMARY".to_string(),
        clip_summary: "SENSITIVE_CLIP_SUMMARY".to_string(),
        chart_context_summary: "SENSITIVE_CHART_SUMMARY".to_string(),
        context_envelope_json,
        base_note_revision: Some(note.current_revision),
        status: "prepared".to_string(),
        actor: "Clinician Example".to_string(),
        ..Default::default()
    }
}

#[tokio::test]
async fn context_packet_checkpoint_binding_is_exact_current_idempotent_and_scoped() {
    let (runtime, client, note, first) = fixture().await;
    let input = packet(&client, &note, &first, "Review the synthetic daily note.");
    let created = runtime
        .workspace()
        .create_context_packet(input.clone())
        .await
        .expect("checkpoint-bound packet should save");
    assert_eq!(
        created.source_draft_session_id.as_deref(),
        Some(first.session_id.as_str())
    );
    assert_eq!(
        created.source_draft_checkpoint_id.as_deref(),
        Some(first.id.as_str())
    );
    assert_eq!(
        created.source_draft_checkpoint_revision,
        Some(first.revision)
    );
    assert_eq!(
        created.source_draft_checkpoint_sha256.as_deref(),
        Some(first.content_sha256.as_str())
    );

    let replay = runtime
        .workspace()
        .create_context_packet(input.clone())
        .await
        .expect("exact retry should replay");
    assert_eq!(replay.id, created.id);
    let packet_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_context_packets WHERE source_draft_checkpoint_id = ?",
    )
    .bind(&first.id)
    .fetch_one(runtime.workspace().pool.as_ref())
    .await
    .unwrap();
    assert_eq!(packet_count, 1);

    let mut changed = packet(&client, &note, &first, "Use a different reviewed request.");
    let changed_error = runtime
        .workspace()
        .create_context_packet(changed.clone())
        .await
        .unwrap_err();
    assert!(changed_error.to_string().contains("different content"));

    let other_client = runtime
        .workspace()
        .upsert_client(crate::WorkspaceClientUpsert {
            display_name: "Other Synthetic Patient".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    changed.client_id = other_client.id;
    let cross_client_error = runtime
        .workspace()
        .create_context_packet(changed)
        .await
        .unwrap_err();
    assert!(
        cross_client_error
            .to_string()
            .contains("not found for client")
    );

    let other_note = runtime
        .workspace()
        .upsert_note(crate::WorkspaceNoteUpsert {
            client_id: client.id.clone(),
            title: "Other synthetic note".to_string(),
            kind: "daily".to_string(),
            body: "Other synthetic body".to_string(),
            status: "draft".to_string(),
            actor: "Clinician Example".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let note_mismatch = packet(&client, &other_note, &first, "Review the other note.");
    let note_error = runtime
        .workspace()
        .create_context_packet(note_mismatch)
        .await
        .unwrap_err();
    assert!(
        note_error
            .to_string()
            .contains("does not match packet scope")
    );

    let _second = checkpoint(
        &runtime,
        &client,
        &note,
        Some(first.session_id.clone()),
        "Second",
    )
    .await;
    let stale_error = runtime
        .workspace()
        .create_context_packet(packet(
            &client,
            &note,
            &first,
            "Review stale synthetic context.",
        ))
        .await
        .unwrap_err();
    assert!(stale_error.to_string().contains("no longer current"));

    let audit: (String, String) = sqlx::query_as(
        "SELECT summary, metadata_json FROM workspace_audit_events WHERE entity_type = 'context_packet' AND entity_id = ?",
    )
    .bind(&created.id)
    .fetch_one(runtime.workspace().pool.as_ref())
    .await
    .unwrap();
    assert_eq!(audit.0, "checkpoint-bound context packet prepared");
    assert!(audit.1.contains(&first.id));
    assert!(!audit.1.contains("SENSITIVE_"));
}
