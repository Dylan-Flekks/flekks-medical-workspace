use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::WorkspaceChartCommitResponse;
use codex_app_server_protocol::WorkspaceChartEntityKind;
use codex_app_server_protocol::WorkspaceClientListResponse;
use codex_app_server_protocol::WorkspaceClientUpsertResponse;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::path::Path;
use tempfile::TempDir;
use tokio::time::timeout;

pub(super) const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test]
async fn chart_commit_routes_atomic_save_replay_noop_and_typed_errors() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, server.initialize()).await??;

    let initial_params = json!({
        "idempotencyKey": "chart-save-1",
        "actor": "clinician-1",
        "reason": "daily note save",
        "sourceThreadId": "thread-1",
        "sourceTurnId": "turn-1",
        "client": {
            "displayName": "Patient One",
            "summary": "",
            "primaryPhone": "555-old",
            "payerName": "Old Payer"
        },
        "encounter": {
            "clientId": "",
            "kind": "visit",
            "title": "Daily visit",
            "status": "open",
            "startedAt": 1700000000,
            "endedAt": 1700003600
        },
        "note": {
            "upsert": {
                "clientId": "",
                "title": "Daily note",
                "kind": "progress",
                "body": "Initial body",
                "status": "draft"
            }
        }
    });

    let committed = send_commit(&mut server, initial_params.clone()).await?;
    assert!(!committed.replayed);
    assert_eq!(
        committed.changed_entity_kinds,
        vec![
            WorkspaceChartEntityKind::Client,
            WorkspaceChartEntityKind::Encounter,
            WorkspaceChartEntityKind::Note,
        ]
    );
    let encounter = committed.encounter.clone().expect("committed encounter");
    let note = committed.note.clone().expect("committed note");
    assert_eq!(encounter.client_id, committed.client.id);
    assert_eq!(note.client_id, committed.client.id);
    assert_eq!(note.encounter_id.as_deref(), Some(encounter.id.as_str()));
    assert_eq!(committed.resulting_note_revision, Some(1));
    assert!(!committed.client.version.is_empty());
    assert!(!encounter.version.is_empty());

    let replayed = send_commit(&mut server, initial_params.clone()).await?;
    assert!(replayed.replayed);
    assert_eq!(replayed.commit_id, committed.commit_id);
    assert_eq!(replayed.note, committed.note);

    let mut conflicting_params = initial_params;
    conflicting_params["note"]["upsert"]["body"] = json!("Different body");
    let conflict = send_commit_error(&mut server, conflicting_params).await?;
    assert_eq!(
        conflict.error.data,
        Some(json!({
            "kind": "idempotencyConflict",
            "idempotencyKey": "chart-save-1"
        }))
    );

    let encounter_no_op = send_commit(
        &mut server,
        json!({
            "idempotencyKey": "chart-save-encounter-noop",
            "actor": "clinician-1",
            "reason": "verify encounter",
            "clientId": committed.client.id,
            "expectedVersions": {
                "encounter": encounter.version
            },
            "encounter": {
                "id": encounter.id,
                "clientId": encounter.client_id,
                "kind": encounter.kind,
                "title": encounter.title,
                "status": encounter.status,
                "startedAt": 1700000000,
                "endedAt": 1700003600
            }
        }),
    )
    .await?;
    assert_eq!(encounter_no_op.changed_entity_kinds, Vec::new());
    assert_eq!(encounter_no_op.encounter, Some(encounter.clone()));

    let concurrently_updated_client = send_client_update(
        &mut server,
        json!({
            "id": committed.client.id,
            "displayName": "Patient One",
            "summary": "",
            "primaryPhone": "555-new",
            "payerName": "New Payer",
            "planName": "Updated Plan"
        }),
    )
    .await?;
    assert_eq!(
        concurrently_updated_client.client.primary_phone.as_deref(),
        Some("555-new")
    );

    let identity_only_note_commit = send_commit(
        &mut server,
        json!({
            "idempotencyKey": "chart-save-identity-only",
            "actor": "clinician-1",
            "reason": "daily note correction",
            "clientId": committed.client.id,
            "note": {
                "upsert": {
                    "id": note.id,
                    "clientId": note.client_id,
                    "encounterId": note.encounter_id,
                    "title": "Daily note",
                    "kind": "progress",
                    "body": "Corrected body",
                    "status": "draft"
                },
                "expectedBaseRevision": 1
            }
        }),
    )
    .await?;
    assert_eq!(
        identity_only_note_commit.changed_entity_kinds,
        vec![WorkspaceChartEntityKind::Note]
    );
    assert_eq!(identity_only_note_commit.resulting_note_revision, Some(2));
    assert_eq!(
        identity_only_note_commit.client.primary_phone.as_deref(),
        Some("555-new")
    );
    assert_eq!(
        identity_only_note_commit.client.payer_name.as_deref(),
        Some("New Payer")
    );
    assert_eq!(
        identity_only_note_commit.client.plan_name.as_deref(),
        Some("Updated Plan")
    );
    let updated_note = identity_only_note_commit
        .note
        .clone()
        .expect("updated note");

    let stale = send_commit_error(
        &mut server,
        json!({
            "idempotencyKey": "chart-save-stale",
            "actor": "clinician-1",
            "reason": "daily note correction",
            "clientId": committed.client.id,
            "note": {
                "upsert": {
                    "id": updated_note.id,
                    "clientId": updated_note.client_id,
                    "encounterId": updated_note.encounter_id,
                    "title": "Daily note",
                    "kind": "progress",
                    "body": "Stale edit",
                    "status": "draft"
                },
                "expectedBaseRevision": 0
            }
        }),
    )
    .await?;
    assert_eq!(
        stale.error.data,
        Some(json!({
            "kind": "staleNoteRevision",
            "noteId": updated_note.id,
            "expectedRevision": 0,
            "actualRevision": 2
        }))
    );

    let ownership_error = send_commit_error(
        &mut server,
        json!({
            "idempotencyKey": "chart-save-wrong-owner",
            "actor": "clinician-1",
            "reason": "create follow-up task",
            "clientId": committed.client.id,
            "task": {
                "clientId": "different-client",
                "title": "Follow up",
                "details": "Call patient",
                "kind": "follow_up",
                "status": "open",
                "priority": "normal"
            }
        }),
    )
    .await?;
    assert_eq!(
        ownership_error.error.data,
        Some(json!({ "kind": "validation" }))
    );

    let mismatched_root_error = send_commit_error(
        &mut server,
        json!({
            "idempotencyKey": "chart-save-mismatched-root",
            "actor": "clinician-1",
            "reason": "demographic correction",
            "clientId": committed.client.id,
            "client": {
                "id": "different-client",
                "displayName": "Patient One",
                "summary": ""
            }
        }),
    )
    .await?;
    assert_eq!(
        mismatched_root_error.error.data,
        Some(json!({ "kind": "validation" }))
    );

    let no_op = send_commit(
        &mut server,
        json!({
            "idempotencyKey": "chart-save-noop",
            "actor": "clinician-1",
            "reason": "verify unchanged chart",
            "clientId": committed.client.id
        }),
    )
    .await?;
    assert_eq!(no_op.changed_entity_kinds, Vec::new());
    assert_eq!(no_op.client, concurrently_updated_client.client);

    Ok(())
}

#[tokio::test]
async fn chart_commit_rejects_blank_reason_with_typed_validation_data() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, server.initialize()).await??;

    let error = send_commit_error(
        &mut server,
        json!({
            "idempotencyKey": "chart-save-invalid",
            "actor": "clinician-1",
            "reason": "  ",
            "client": {
                "displayName": "Patient One",
                "summary": ""
            }
        }),
    )
    .await?;

    assert_eq!(error.error.data, Some(json!({ "kind": "validation" })));

    let malformed = send_commit_error(
        &mut server,
        json!({
            "idempotencyKey": "chart-save-malformed",
            "actor": "clinician-1",
            "clientId": "client-1"
        }),
    )
    .await?;
    assert_eq!(malformed.error.code, -32600);
    assert_eq!(malformed.error.data, None);
    Ok(())
}

#[tokio::test]
async fn chart_commit_late_child_validation_is_atomic() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, server.initialize()).await??;

    let error = send_commit_error(
        &mut server,
        json!({
            "idempotencyKey": "chart-save-invalid-task-link",
            "actor": "clinician-1",
            "reason": "new daily chart",
            "client": {
                "displayName": "Patient Never Persisted",
                "summary": ""
            },
            "encounter": {
                "clientId": "",
                "kind": "visit",
                "title": "Daily visit",
                "status": "open"
            },
            "note": {
                "upsert": {
                    "clientId": "",
                    "title": "Daily note",
                    "kind": "progress",
                    "body": "Should roll back",
                    "status": "draft"
                }
            },
            "task": {
                "clientId": "",
                "documentId": "missing-document",
                "title": "Follow up",
                "details": "Invalid late relation",
                "kind": "follow_up",
                "status": "open",
                "priority": "normal"
            }
        }),
    )
    .await?;
    assert_eq!(error.error.data, Some(json!({ "kind": "validation" })));

    let clients = list_clients(&mut server).await?;
    assert_eq!(clients.clients, Vec::new());
    Ok(())
}

pub(super) async fn send_commit(
    server: &mut TestAppServer,
    params: Value,
) -> Result<WorkspaceChartCommitResponse> {
    let request_id = server
        .send_raw_request("workspace/chart/commit", Some(params))
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

pub(super) async fn send_commit_error(
    server: &mut TestAppServer,
    params: Value,
) -> Result<JSONRPCError> {
    let request_id = server
        .send_raw_request("workspace/chart/commit", Some(params))
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await?
}

pub(super) async fn send_client_update(
    server: &mut TestAppServer,
    params: Value,
) -> Result<WorkspaceClientUpsertResponse> {
    let request_id = server
        .send_raw_request("workspace/client/upsert", Some(params))
        .await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

pub(super) async fn list_clients(
    server: &mut TestAppServer,
) -> Result<WorkspaceClientListResponse> {
    let request_id = server
        .send_raw_request("workspace/client/list", Some(json!({})))
        .await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

pub(super) fn create_config_toml(codex_home: &Path) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"
model_provider = "mock_provider"
suppress_unstable_features_warning = true

[features]
sqlite = true

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "http://127.0.0.1:9/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#,
    )
}
