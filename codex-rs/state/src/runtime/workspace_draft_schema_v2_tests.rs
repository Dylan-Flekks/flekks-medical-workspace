use crate::StateRuntime;
use crate::runtime::test_support::unique_temp_dir;
use pretty_assertions::assert_eq;
use serde_json::Value;
use sha2::Digest;

async fn fixture() -> (std::sync::Arc<StateRuntime>, crate::WorkspaceClient) {
    let runtime = StateRuntime::init(unique_temp_dir(), "test-provider".to_string())
        .await
        .expect("state db should initialize");
    let client = runtime
        .workspace()
        .upsert_client(crate::WorkspaceClientUpsert {
            display_name: "Schema Snapshot Patient".to_string(),
            ..Default::default()
        })
        .await
        .expect("client should save");
    (runtime, client)
}

fn input(
    client: &crate::WorkspaceClient,
    session_id: Option<String>,
    draft: &Value,
) -> crate::WorkspaceDraftCheckpointCreate {
    crate::WorkspaceDraftCheckpointCreate {
        session_id,
        client_id: client.id.clone(),
        note_id: Some("draft-note".to_string()),
        base_note_revision: Some(1),
        draft_json: serde_json::to_string_pretty(draft).expect("draft should encode"),
        trigger: "focus_change".to_string(),
        actor: "Clinician Example".to_string(),
        ..Default::default()
    }
}

fn v1_snapshot() -> Value {
    serde_json::json!({
        "schemaVersion": 1,
        "patient": {"displayName": "Schema Snapshot Patient"},
        "note": {"title": "Daily note", "body": "Patient-entered draft"},
    })
}

fn v2_snapshot(request: &str) -> Value {
    serde_json::json!({
        "schemaVersion": 2,
        "patient": {"displayName": "Schema Snapshot Patient"},
        "note": {"title": "Daily note", "body": "Patient-entered draft"},
        "agentRequest": {"body": request, "status": "draft"},
        "contextSelection": {
            "includeVisitHistory": true,
            "includeProgressNotes": true,
            "selectedSourceIds": ["source-note-1"],
        },
    })
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

#[tokio::test]
async fn v1_remains_compatible_and_v2_is_normalized_replayed_and_revisioned() {
    let (runtime, client) = fixture().await;

    let v1 = save(&runtime, input(&client, None, &v1_snapshot())).await;
    assert_eq!(v1.schema_version, 1);
    assert_eq!(v1.revision, 1);
    assert_eq!(
        v1.content_sha256,
        format!("{:x}", sha2::Sha256::digest(v1.draft_json.as_bytes()))
    );

    let first_value = v2_snapshot("Prepare a similar daily-note template.");
    let first = save(&runtime, input(&client, None, &first_value)).await;
    assert_eq!(first.schema_version, 2);
    assert_eq!(first.revision, 1);
    assert_eq!(
        first.draft_json,
        serde_json::to_string(&first_value).expect("normalized draft should encode")
    );
    assert_eq!(
        first.content_sha256,
        format!("{:x}", sha2::Sha256::digest(first.draft_json.as_bytes()))
    );

    let replay = save(
        &runtime,
        input(&client, Some(first.session_id.clone()), &first_value),
    )
    .await;
    assert!(replay.replayed);
    assert_eq!(replay.id, first.id);
    assert_eq!(replay.content_sha256, first.content_sha256);

    let changed_value = v2_snapshot("Prepare the template and flag missing objective measures.");
    let changed = save(
        &runtime,
        input(&client, Some(first.session_id.clone()), &changed_value),
    )
    .await;
    assert!(!changed.replayed);
    assert_eq!(changed.session_id, first.session_id);
    assert_eq!(changed.schema_version, 2);
    assert_eq!(changed.revision, 2);
    assert_ne!(changed.content_sha256, first.content_sha256);

    let side_effect_count: i64 = sqlx::query_scalar(
        r#"
SELECT
    (SELECT COUNT(*) FROM workspace_notes)
  + (SELECT COUNT(*) FROM workspace_context_packets)
  + (SELECT COUNT(*) FROM workspace_agent_runs)
  + (SELECT COUNT(*) FROM workspace_agent_results)
        "#,
    )
    .fetch_one(runtime.workspace().pool.as_ref())
    .await
    .expect("clinical and agent side effects should count");
    assert_eq!(side_effect_count, 0);
}

#[tokio::test]
async fn draft_snapshot_rejects_unsupported_versions_and_keeps_the_one_mib_bound() {
    let (runtime, client) = fixture().await;

    for unsupported in [0, 3] {
        let value = serde_json::json!({"schemaVersion": unsupported});
        let error = checkpoint_error(&runtime, input(&client, None, &value)).await;
        assert!(error.contains(&format!("schemaVersion {unsupported}")));
    }

    let oversized = serde_json::json!({
        "schemaVersion": 2,
        "agentRequest": {"body": "x".repeat(1024 * 1024)},
    });
    let error = checkpoint_error(&runtime, input(&client, None, &oversized)).await;
    assert!(error.contains("1048576 byte normalized limit"));
}
