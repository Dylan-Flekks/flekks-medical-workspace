use super::workspace_chart_commit::DEFAULT_READ_TIMEOUT;
use super::workspace_chart_commit::create_config_toml;
use super::workspace_chart_commit::list_clients;
use super::workspace_chart_commit::send_client_update;
use super::workspace_chart_commit::send_commit;
use super::workspace_chart_commit::send_commit_error;
use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::WorkspaceArtifactDerivativeStatusUpdateResponse;
use codex_app_server_protocol::WorkspaceChartEntityKind;
use codex_app_server_protocol::WorkspaceContextClipStatusUpdateResponse;
use codex_app_server_protocol::WorkspaceTaskUpsertResponse;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

#[tokio::test]
async fn stale_client_and_task_versions_fail_and_client_id_payload_can_edit() -> Result<()> {
    let (_home, mut server) = initialized_server().await?;
    let created = send_commit(
        &mut server,
        json!({
            "idempotencyKey": "version-create",
            "actor": "clinician-1",
            "reason": "create patient and task",
            "client": {
                "displayName": "Version Patient",
                "summary": ""
            },
            "task": {
                "clientId": "",
                "title": "Initial task",
                "details": "Initial details",
                "kind": "follow_up",
                "status": "open",
                "priority": "normal"
            }
        }),
    )
    .await?;
    let created_task = created.task.clone().expect("created task");

    let concurrent_client = send_client_update(
        &mut server,
        json!({
            "id": created.client.id,
            "displayName": "Concurrent Patient",
            "summary": "concurrent update"
        }),
    )
    .await?
    .client;
    let concurrent_task = send_task_update(
        &mut server,
        json!({
            "id": created_task.id,
            "clientId": created.client.id,
            "title": "Concurrent task",
            "details": "Concurrent details",
            "kind": "follow_up",
            "status": "open",
            "priority": "high"
        }),
    )
    .await?
    .task;

    let stale_client = send_commit_error(
        &mut server,
        json!({
            "idempotencyKey": "version-stale-client",
            "actor": "clinician-1",
            "reason": "stale demographic edit",
            "expectedVersions": { "client": created.client.version },
            "client": {
                "id": created.client.id,
                "displayName": "Stale Patient",
                "summary": "stale update"
            }
        }),
    )
    .await?;
    assert_eq!(
        stale_client.error.data,
        Some(json!({
            "kind": "staleEntityVersion",
            "entityKind": "client",
            "entityId": created.client.id,
            "expectedVersion": created.client.version,
            "actualVersion": concurrent_client.version
        }))
    );

    let stale_task = send_commit_error(
        &mut server,
        json!({
            "idempotencyKey": "version-stale-task",
            "actor": "clinician-1",
            "reason": "stale task edit",
            "clientId": created.client.id,
            "expectedVersions": { "task": created_task.version },
            "task": {
                "id": created_task.id,
                "clientId": created.client.id,
                "title": "Stale task",
                "details": "Stale details",
                "kind": "follow_up",
                "status": "open",
                "priority": "normal"
            }
        }),
    )
    .await?;
    assert_eq!(
        stale_task.error.data,
        Some(json!({
            "kind": "staleEntityVersion",
            "entityKind": "task",
            "entityId": created_task.id,
            "expectedVersion": created_task.version,
            "actualVersion": concurrent_task.version
        }))
    );

    let edited = send_commit(
        &mut server,
        json!({
            "idempotencyKey": "version-current-client-edit",
            "actor": "clinician-1",
            "reason": "current demographic edit",
            "expectedVersions": { "client": concurrent_client.version },
            "client": {
                "id": concurrent_client.id,
                "displayName": "Chart Edited Patient",
                "summary": "edited through atomic commit"
            }
        }),
    )
    .await?;
    assert_eq!(
        edited.changed_entity_kinds,
        vec![WorkspaceChartEntityKind::Client]
    );
    assert_eq!(edited.client.id, concurrent_client.id);
    assert_eq!(edited.client.display_name, "Chart Edited Patient");
    assert_ne!(edited.client.version, concurrent_client.version);
    Ok(())
}

#[tokio::test]
async fn archived_derivative_and_clip_are_rejected_even_with_current_versions() -> Result<()> {
    let (_home, mut server) = initialized_server().await?;
    let created = send_commit(&mut server, full_graph_params()).await?;
    let safety = created.safety_item.expect("safety item");
    let encounter = created.encounter.expect("encounter");
    let document = created.document.expect("document");
    let derivative = created.artifact_derivative.expect("derivative");
    let clip = created.context_clip.expect("clip");
    let task = created.task.expect("task");
    for version in [
        created.client.version.as_str(),
        safety.version.as_str(),
        encounter.version.as_str(),
        document.version.as_str(),
        derivative.version.as_str(),
        clip.version.as_str(),
        task.version.as_str(),
    ] {
        assert!(!version.is_empty());
    }

    let archived_clip = update_clip_status(&mut server, &clip.id, "archived")
        .await?
        .clip
        .expect("archived clip");
    let clip_error = send_commit_error(
        &mut server,
        json!({
            "idempotencyKey": "archived-clip-edit",
            "actor": "clinician-1",
            "reason": "edit archived clip",
            "clientId": created.client.id,
            "expectedVersions": { "contextClip": archived_clip.version },
            "contextClip": {
                "id": archived_clip.id,
                "derivativeId": archived_clip.derivative_id,
                "documentId": archived_clip.document_id,
                "clientId": archived_clip.client_id,
                "encounterId": archived_clip.encounter_id,
                "noteId": archived_clip.note_id,
                "kind": archived_clip.kind,
                "title": archived_clip.title,
                "body": "Edited archived clip",
                "reviewStatus": "draft",
                "sourceMethod": archived_clip.source_method,
                "pageRange": archived_clip.page_range,
                "timestampRange": archived_clip.timestamp_range,
                "lineRange": archived_clip.line_range,
                "segmentLabel": archived_clip.segment_label,
                "tags": archived_clip.tags,
                "metadataJson": archived_clip.metadata_json
            }
        }),
    )
    .await?;
    assert_eq!(clip_error.error.data, Some(json!({ "kind": "validation" })));

    let archived_derivative = update_derivative_status(&mut server, &derivative.id, "archived")
        .await?
        .derivative
        .expect("archived derivative");
    let derivative_error = send_commit_error(
        &mut server,
        json!({
            "idempotencyKey": "archived-derivative-edit",
            "actor": "clinician-1",
            "reason": "edit archived derivative",
            "clientId": created.client.id,
            "expectedVersions": { "artifactDerivative": archived_derivative.version },
            "artifactDerivative": {
                "id": archived_derivative.id,
                "documentId": archived_derivative.document_id,
                "clientId": archived_derivative.client_id,
                "encounterId": archived_derivative.encounter_id,
                "noteId": archived_derivative.note_id,
                "kind": archived_derivative.kind,
                "title": archived_derivative.title,
                "body": "Edited archived derivative",
                "reviewStatus": "draft",
                "sourceMethod": archived_derivative.source_method,
                "pageRange": archived_derivative.page_range,
                "timestampRange": archived_derivative.timestamp_range,
                "segmentLabel": archived_derivative.segment_label,
                "tags": archived_derivative.tags,
                "metadataJson": archived_derivative.metadata_json
            }
        }),
    )
    .await?;
    assert_eq!(
        derivative_error.error.data,
        Some(json!({ "kind": "validation" }))
    );
    Ok(())
}

#[tokio::test]
async fn invalid_statuses_times_and_sizes_are_semantic_validation_errors() -> Result<()> {
    let (_home, mut server) = initialized_server().await?;
    let invalid_requests = [
        json!({
            "idempotencyKey": "invalid-encounter-status",
            "actor": "clinician-1",
            "reason": "invalid encounter status",
            "client": { "displayName": "Invalid One", "summary": "" },
            "encounter": {
                "clientId": "", "kind": "visit", "title": "Visit", "status": "unknown"
            }
        }),
        json!({
            "idempotencyKey": "invalid-encounter-time",
            "actor": "clinician-1",
            "reason": "invalid encounter time",
            "client": { "displayName": "Invalid Two", "summary": "" },
            "encounter": {
                "clientId": "", "kind": "visit", "title": "Visit", "status": "open",
                "startedAt": 200, "endedAt": 100
            }
        }),
        json!({
            "idempotencyKey": "invalid-note-status",
            "actor": "clinician-1",
            "reason": "invalid note status",
            "client": { "displayName": "Invalid Three", "summary": "" },
            "note": {
                "upsert": {
                    "clientId": "", "title": "Note", "kind": "progress",
                    "body": "Body", "status": "signed"
                }
            }
        }),
        json!({
            "idempotencyKey": "invalid-document-size",
            "actor": "clinician-1",
            "reason": "invalid document size",
            "client": { "displayName": "Invalid Four", "summary": "" },
            "document": document_json(-1)
        }),
        json!({
            "idempotencyKey": "invalid-derivative-status",
            "actor": "clinician-1",
            "reason": "invalid derivative status",
            "client": { "displayName": "Invalid Five", "summary": "" },
            "document": document_json(10),
            "artifactDerivative": {
                "documentId": "", "clientId": "", "kind": "text", "title": "Text",
                "body": "Body", "reviewStatus": "approved", "sourceMethod": "human_typed",
                "pageRange": "", "timestampRange": "", "segmentLabel": "", "tags": "",
                "metadataJson": "{}"
            }
        }),
    ];

    for params in invalid_requests {
        let error = send_commit_error(&mut server, params).await?;
        assert_eq!(error.error.code, -32600);
        assert_eq!(error.error.data, Some(json!({ "kind": "validation" })));
    }
    assert_eq!(list_clients(&mut server).await?.clients, Vec::new());
    Ok(())
}

async fn initialized_server() -> Result<(TempDir, TestAppServer)> {
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

async fn send_task_update(
    server: &mut TestAppServer,
    params: Value,
) -> Result<WorkspaceTaskUpsertResponse> {
    let request_id = server
        .send_raw_request("workspace/task/upsert", Some(params))
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn update_clip_status(
    server: &mut TestAppServer,
    clip_id: &str,
    review_status: &str,
) -> Result<WorkspaceContextClipStatusUpdateResponse> {
    let request_id = server
        .send_raw_request(
            "workspace/context/clip/status/update",
            Some(json!({ "clipId": clip_id, "reviewStatus": review_status })),
        )
        .await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn update_derivative_status(
    server: &mut TestAppServer,
    derivative_id: &str,
    review_status: &str,
) -> Result<WorkspaceArtifactDerivativeStatusUpdateResponse> {
    let request_id = server
        .send_raw_request(
            "workspace/artifact/derivative/status/update",
            Some(json!({
                "derivativeId": derivative_id,
                "reviewStatus": review_status
            })),
        )
        .await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

fn full_graph_params() -> Value {
    json!({
        "idempotencyKey": "full-version-graph",
        "actor": "clinician-1",
        "reason": "create full chart",
        "client": { "displayName": "Full Graph Patient", "summary": "" },
        "safetyItem": {
            "clientId": "", "category": "allergy", "name": "Latex", "notes": "Synthetic"
        },
        "encounter": {
            "clientId": "", "kind": "visit", "title": "Visit", "status": "open"
        },
        "document": document_json(10),
        "artifactDerivative": {
            "documentId": "", "clientId": "", "kind": "text", "title": "Reviewed text",
            "body": "Synthetic derivative", "reviewStatus": "draft",
            "sourceMethod": "human_typed", "pageRange": "1", "timestampRange": "",
            "segmentLabel": "", "tags": "", "metadataJson": "{}"
        },
        "contextClip": {
            "derivativeId": "", "documentId": "", "clientId": "", "kind": "excerpt",
            "title": "Selected clip", "body": "Synthetic clip", "reviewStatus": "draft",
            "sourceMethod": "human_selected", "pageRange": "1", "timestampRange": "",
            "lineRange": "1-2", "segmentLabel": "", "tags": "", "metadataJson": "{}"
        },
        "task": {
            "clientId": "", "title": "Follow up", "details": "Synthetic task",
            "kind": "follow_up", "status": "open", "priority": "normal"
        }
    })
}

fn document_json(file_size_bytes: i64) -> Value {
    json!({
        "clientId": "",
        "title": "Synthetic document",
        "kind": "image",
        "localPath": "/synthetic/document.png",
        "notes": "",
        "scope": "patient",
        "detectedKind": "image",
        "fileSizeBytes": file_size_bytes,
        "tags": "",
        "sourceLabel": "synthetic",
        "existenceStatus": "available",
        "metadataJson": "{}"
    })
}
