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
            display_name: "Synthetic Context Plan Patient".to_string(),
            ..Default::default()
        })
        .await
        .expect("client should save");
    (runtime, client)
}

async fn save_checkpoint(
    runtime: &StateRuntime,
    client: &crate::WorkspaceClient,
    session: Option<&crate::WorkspaceDraftCheckpoint>,
    objective: &str,
) -> crate::WorkspaceDraftCheckpoint {
    runtime
        .workspace()
        .create_draft_checkpoint(crate::WorkspaceDraftCheckpointCreate {
            session_id: session.map(|checkpoint| checkpoint.session_id.clone()),
            client_id: client.id.clone(),
            expected_current_checkpoint_id: session.map(|checkpoint| checkpoint.id.clone()),
            expected_current_checkpoint_revision: session.map(|checkpoint| checkpoint.revision),
            expected_current_checkpoint_sha256: session
                .map(|checkpoint| checkpoint.content_sha256.clone()),
            draft_json: serde_json::json!({
                "schemaVersion": 2,
                "kind": "medicalWorkspaceWorkingDraft",
                "clientId": client.id,
                "objective": objective,
            })
            .to_string(),
            trigger: "packet_preview".to_string(),
            actor: "Clinician Example".to_string(),
            ..Default::default()
        })
        .await
        .expect("checkpoint should save")
}

fn packet_input(
    client: &crate::WorkspaceClient,
    checkpoint: &crate::WorkspaceDraftCheckpoint,
    readiness_json: String,
) -> crate::WorkspaceContextPacketCreate {
    let request = "Analyze the longitudinal context.";
    crate::WorkspaceContextPacketCreate {
        client_id: client.id.clone(),
        human_request: request.to_string(),
        selected_artifact_ids_json: "[]".to_string(),
        selected_derivative_ids_json: "[]".to_string(),
        selected_clip_ids_json: "[]".to_string(),
        context_envelope_json: serde_json::json!({
            "assemblyVersion": "context-plan-test-v1",
            "includeDocuments": false,
            "humanRequest": request,
            "ids": {
                "selectedArtifactIds": [],
                "selectedDerivativeIds": [],
                "selectedClipIds": [],
            },
            "safety": [
                "read-only context packet; do not mutate workspace records",
                "do not sign notes, submit claims, send payer communications, or overwrite saved data",
            ],
            "promptSnapshot": "Synthetic context plan test.",
        })
        .to_string(),
        workspace_profile: "medical".to_string(),
        plan_schema_version: Some(1),
        source_checkpoint_id: Some(checkpoint.id.clone()),
        source_checkpoint_sha256: Some(checkpoint.content_sha256.clone()),
        readiness_json,
        actor: "Clinician Example".to_string(),
        ..Default::default()
    }
}

fn acknowledged_readiness(checkpoint: &crate::WorkspaceDraftCheckpoint) -> String {
    serde_json::json!({
        "version": 1,
        "warnings": [{
            "code": "missing_referral",
            "message": "No reviewed referral is selected.",
        }],
        "acknowledgements": [{
            "warningCode": "missing_referral",
            "checkpointSha256": checkpoint.content_sha256,
            "reason": "Proceed with the currently reviewed record.",
        }],
        "legacy": false,
    })
    .to_string()
}

#[tokio::test]
async fn context_plan_metadata_is_normalized_hashed_listed_and_replayed() {
    let (runtime, client) = fixture().await;
    let checkpoint = save_checkpoint(&runtime, &client, None, "Initial plan").await;
    let packet = runtime
        .workspace()
        .prepare_context_packet(packet_input(
            &client,
            &checkpoint,
            acknowledged_readiness(&checkpoint),
        ))
        .await
        .expect("packet should prepare");

    assert_eq!(packet.workspace_profile, "medical");
    assert_eq!(packet.plan_schema_version, 1);
    assert_eq!(
        packet.source_checkpoint_id.as_deref(),
        Some(checkpoint.id.as_str())
    );
    assert_eq!(
        packet.source_checkpoint_sha256.as_deref(),
        Some(checkpoint.content_sha256.as_str())
    );
    assert_eq!(
        packet.context_envelope_sha256,
        format!(
            "{:x}",
            sha2::Sha256::digest(packet.context_envelope_json.as_bytes())
        )
    );
    let envelope: serde_json::Value =
        serde_json::from_str(&packet.context_envelope_json).expect("stored envelope should decode");
    assert_eq!(envelope["workspaceProfile"], "medical");
    assert_eq!(
        envelope["contextPlan"]["sourceCheckpoint"]["id"],
        checkpoint.id
    );

    let submitted = runtime
        .workspace()
        .submit_context_packet(crate::WorkspaceContextPacketLifecycleUpdate {
            packet_id: packet.id.clone(),
            client_id: client.id.clone(),
            expected_context_envelope_sha256: packet.context_envelope_sha256.clone(),
            actor: "Clinician Example".to_string(),
        })
        .await
        .expect("submission should validate")
        .expect("packet should exist");
    assert_eq!(submitted.status, "submitted");

    let replay = runtime
        .workspace()
        .get_context_packet_replay(crate::WorkspaceContextPacketReplayFilter {
            client_id: client.id,
            packet_id: packet.id,
            context_envelope_sha256: packet.context_envelope_sha256,
        })
        .await
        .expect("replay should load")
        .expect("submitted packet should replay");
    assert_eq!(replay.readiness_json, packet.readiness_json);
}

#[tokio::test]
async fn context_plan_submission_rejects_unacknowledged_and_stale_checkpoints() {
    let (runtime, client) = fixture().await;
    let checkpoint = save_checkpoint(&runtime, &client, None, "Initial plan").await;
    let unacknowledged = serde_json::json!({
        "version": 1,
        "warnings": [{
            "code": "missing_referral",
            "message": "No reviewed referral is selected.",
        }],
        "acknowledgements": [],
        "legacy": false,
    })
    .to_string();
    let packet = runtime
        .workspace()
        .prepare_context_packet(packet_input(&client, &checkpoint, unacknowledged))
        .await
        .expect("packet preview should preserve warnings");
    let error = runtime
        .workspace()
        .submit_context_packet(crate::WorkspaceContextPacketLifecycleUpdate {
            packet_id: packet.id,
            client_id: client.id.clone(),
            expected_context_envelope_sha256: packet.context_envelope_sha256,
            actor: "Clinician Example".to_string(),
        })
        .await
        .expect_err("unacknowledged warning should block submission")
        .to_string();
    assert!(error.contains("unacknowledged readiness warnings"));

    let acknowledged = runtime
        .workspace()
        .prepare_context_packet(packet_input(
            &client,
            &checkpoint,
            acknowledged_readiness(&checkpoint),
        ))
        .await
        .expect("acknowledged packet should preview");
    save_checkpoint(&runtime, &client, Some(&checkpoint), "Changed plan").await;
    let error = runtime
        .workspace()
        .submit_context_packet(crate::WorkspaceContextPacketLifecycleUpdate {
            packet_id: acknowledged.id,
            client_id: client.id,
            expected_context_envelope_sha256: acknowledged.context_envelope_sha256,
            actor: "Clinician Example".to_string(),
        })
        .await
        .expect_err("stale checkpoint should block submission")
        .to_string();
    assert!(error.contains("is stale"));
}

#[tokio::test]
async fn context_plan_creation_rejects_partial_metadata_and_self_declared_legacy_packets() {
    let (runtime, client) = fixture().await;
    let checkpoint = save_checkpoint(&runtime, &client, None, "Initial plan").await;

    let mut partial = packet_input(&client, &checkpoint, acknowledged_readiness(&checkpoint));
    partial.readiness_json.clear();
    let error = runtime
        .workspace()
        .prepare_context_packet(partial)
        .await
        .expect_err("partial Context Plan metadata should fail closed")
        .to_string();
    assert!(error.contains("must be provided together"));

    let self_declared_legacy = serde_json::json!({
        "version": 1,
        "warnings": [],
        "acknowledgements": [],
        "legacy": true,
    })
    .to_string();
    let error = runtime
        .workspace()
        .prepare_context_packet(packet_input(&client, &checkpoint, self_declared_legacy))
        .await
        .expect_err("new Context Plans must not opt into legacy validation")
        .to_string();
    assert!(error.contains("legacy readiness is reserved"));
}
