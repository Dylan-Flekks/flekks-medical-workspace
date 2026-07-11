use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::ModelToolMode;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::WorkspaceAgentResultListResponse;
use codex_app_server_protocol::WorkspaceClientUpsertResponse;
use codex_app_server_protocol::WorkspaceDraftCheckpointCreateResponse;
use codex_app_server_protocol::WorkspaceEncounterUpsertResponse;
use codex_app_server_protocol::WorkspaceGuideRunFinishResponse;
use codex_app_server_protocol::WorkspaceGuideRunListResponse;
use codex_app_server_protocol::WorkspaceGuideRunStartResponse;
use codex_app_server_protocol::WorkspaceGuideRunStatus;
use codex_app_server_protocol::WorkspaceNote;
use codex_app_server_protocol::WorkspaceNoteGetResponse;
use codex_app_server_protocol::WorkspaceNoteProposalListResponse;
use codex_app_server_protocol::WorkspaceNoteUpsertResponse;
use pretty_assertions::assert_eq;
use serde::de::DeserializeOwned;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

use super::workspace_chart_commit::DEFAULT_READ_TIMEOUT;
use super::workspace_chart_commit::create_config_toml;

const GUIDE_SERVER_START_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) struct ChartScope {
    pub(super) client_id: String,
    pub(super) encounter_id: String,
    pub(super) note: WorkspaceNote,
}

#[tokio::test]
async fn workspace_guides_persist_exact_envelopes_finish_and_paginate_without_launching()
-> Result<()> {
    let (_codex_home, mut server) = server().await?;
    let scope = create_chart_scope(&mut server, "Synthetic Guide Patient").await?;
    let checkpoint = create_checkpoint(&mut server, &scope, None, "First draft").await?;
    let threads_before: ThreadListResponse = request(&mut server, "thread/list", json!({})).await?;

    let params = start_params(&scope.client_id, &checkpoint, "guide-key", guide_request());
    let started: WorkspaceGuideRunStartResponse =
        request(&mut server, "workspace/guide/run/start", params.clone()).await?;
    assert!(!started.replayed);
    assert_eq!(started.run.client_id, scope.client_id);
    assert_eq!(
        started.run.encounter_id.as_deref(),
        Some(scope.encounter_id.as_str())
    );
    assert_eq!(started.run.note_id.as_deref(), Some(scope.note.id.as_str()));
    assert_eq!(started.run.model_tool_mode, ModelToolMode::Disabled);
    assert_eq!(started.run.status, WorkspaceGuideRunStatus::Running);
    assert!(!started.run.is_stale);
    assert_eq!(started.run.source_thread_id, None);
    assert_eq!(started.run.source_turn_id, None);
    assert_eq!(started.run.terminal_envelope, None);
    assert_eq!(started.run.request_envelope["request"], guide_request());
    assert_eq!(
        started.run.request_envelope["sourceCheckpoint"]["id"],
        checkpoint.checkpoint.id
    );
    assert_eq!(started.run.request_envelope["safety"]["readOnly"], true);
    assert_eq!(
        started.run.request_envelope["safety"]["canonicalChartWrites"],
        false
    );
    assert_eq!(
        started.run.request_envelope["safety"]["modelToolMode"],
        "disabled"
    );

    let replayed: WorkspaceGuideRunStartResponse =
        request(&mut server, "workspace/guide/run/start", params).await?;
    assert!(replayed.replayed);
    assert_eq!(replayed.run.id, started.run.id);
    assert_eq!(replayed.run.request_envelope, started.run.request_envelope);

    let finished: WorkspaceGuideRunFinishResponse = request(
        &mut server,
        "workspace/guide/run/finish",
        finish_params(
            &started,
            Some(("guide-thread", "guide-turn")),
            json!({
                "type": "completed",
                "result": {"schemaVersion": 1, "hints": ["Review the selected fields"]}
            }),
        ),
    )
    .await?;
    assert!(!finished.replayed);
    assert_eq!(finished.run.status, WorkspaceGuideRunStatus::Completed);
    assert_eq!(
        finished.run.source_thread_id.as_deref(),
        Some("guide-thread")
    );
    assert_eq!(finished.run.source_turn_id.as_deref(), Some("guide-turn"));
    assert_eq!(
        finished.run.terminal_envelope.as_ref().unwrap()["type"],
        "completed"
    );

    let finish_replay: WorkspaceGuideRunFinishResponse = request(
        &mut server,
        "workspace/guide/run/finish",
        finish_params(
            &started,
            Some(("guide-thread", "guide-turn")),
            json!({
                "type": "completed",
                "result": {"schemaVersion": 1, "hints": ["Review the selected fields"]}
            }),
        ),
    )
    .await?;
    assert!(finish_replay.replayed);

    let next_checkpoint = create_checkpoint(
        &mut server,
        &scope,
        Some(&checkpoint.checkpoint.session_id),
        "Second draft",
    )
    .await?;
    let stale_replay: WorkspaceGuideRunStartResponse = request(
        &mut server,
        "workspace/guide/run/start",
        start_params(&scope.client_id, &checkpoint, "guide-key", guide_request()),
    )
    .await?;
    assert!(stale_replay.replayed);
    assert!(stale_replay.run.is_stale);
    let second: WorkspaceGuideRunStartResponse = request(
        &mut server,
        "workspace/guide/run/start",
        start_params(
            &scope.client_id,
            &next_checkpoint,
            "guide-key-2",
            json!({"focus": "title"}),
        ),
    )
    .await?;
    let _: WorkspaceGuideRunFinishResponse = request(
        &mut server,
        "workspace/guide/run/finish",
        finish_params(
            &second,
            None,
            json!({"type": "canceled", "reason": "superseded by local input"}),
        ),
    )
    .await?;

    let first_page: WorkspaceGuideRunListResponse = request(
        &mut server,
        "workspace/guide/run/list",
        json!({"clientId": scope.client_id, "limit": 1}),
    )
    .await?;
    let cursor = first_page
        .next_cursor
        .clone()
        .expect("first page should continue");
    assert!(
        cursor
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    );
    assert!(!cursor.contains('='));
    let second_page: WorkspaceGuideRunListResponse = request(
        &mut server,
        "workspace/guide/run/list",
        json!({"clientId": scope.client_id, "cursor": cursor, "limit": 1}),
    )
    .await?;
    assert_eq!(second_page.next_cursor, None);
    let mut page_ids = vec![
        first_page.data[0].id.clone(),
        second_page.data[0].id.clone(),
    ];
    page_ids.sort();
    let mut expected_ids = vec![started.run.id.clone(), second.run.id];
    expected_ids.sort();
    assert_eq!(page_ids, expected_ids);
    let historical = first_page
        .data
        .iter()
        .chain(&second_page.data)
        .find(|run| run.id == started.run.id)
        .expect("completed historical guide should be listed");
    assert!(historical.is_stale);

    for index in 2..101 {
        let extra: WorkspaceGuideRunStartResponse = request(
            &mut server,
            "workspace/guide/run/start",
            start_params(
                &scope.client_id,
                &next_checkpoint,
                &format!("guide-bulk-{index}"),
                json!({"paginationIndex": index}),
            ),
        )
        .await?;
        let _: WorkspaceGuideRunFinishResponse = request(
            &mut server,
            "workspace/guide/run/finish",
            finish_params(
                &extra,
                None,
                json!({"type": "canceled", "reason": "pagination fixture"}),
            ),
        )
        .await?;
    }
    let max_page: WorkspaceGuideRunListResponse = request(
        &mut server,
        "workspace/guide/run/list",
        json!({"clientId": scope.client_id, "limit": 100}),
    )
    .await?;
    assert_eq!(max_page.data.len(), 100);
    let max_cursor = max_page
        .next_cursor
        .expect("one run should remain after a max-size page");
    let max_tail: WorkspaceGuideRunListResponse = request(
        &mut server,
        "workspace/guide/run/list",
        json!({"clientId": scope.client_id, "cursor": max_cursor, "limit": 100}),
    )
    .await?;
    assert_eq!(max_tail.data.len(), 1);
    assert_eq!(max_tail.next_cursor, None);

    let note_after: WorkspaceNoteGetResponse = request(
        &mut server,
        "workspace/note/get",
        json!({"noteId": scope.note.id}),
    )
    .await?;
    assert_eq!(note_after.note, Some(scope.note.clone()));
    let results: WorkspaceAgentResultListResponse = request(
        &mut server,
        "workspace/agent/result/list",
        json!({"clientId": scope.client_id, "noteId": scope.note.id}),
    )
    .await?;
    assert_eq!(results.results, Vec::new());
    let proposals: WorkspaceNoteProposalListResponse = request(
        &mut server,
        "workspace/note/proposal/list",
        json!({"noteId": scope.note.id}),
    )
    .await?;
    assert_eq!(proposals.proposals, Vec::new());
    let threads_after: ThreadListResponse = request(&mut server, "thread/list", json!({})).await?;
    assert_eq!(threads_after.data, threads_before.data);
    Ok(())
}

pub(super) async fn server() -> Result<(TempDir, TestAppServer)> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(GUIDE_SERVER_START_TIMEOUT, server.initialize()).await??;
    Ok((codex_home, server))
}

pub(super) async fn create_chart_scope(
    server: &mut TestAppServer,
    name: &str,
) -> Result<ChartScope> {
    let client: WorkspaceClientUpsertResponse = request(
        server,
        "workspace/client/upsert",
        json!({"displayName": name, "summary": ""}),
    )
    .await?;
    let encounter: WorkspaceEncounterUpsertResponse = request(
        server,
        "workspace/encounter/upsert",
        json!({
            "clientId": client.client.id,
            "kind": "daily",
            "title": "Synthetic visit",
            "status": "open"
        }),
    )
    .await?;
    let note: WorkspaceNoteUpsertResponse = request(
        server,
        "workspace/note/upsert",
        json!({
            "clientId": client.client.id,
            "encounterId": encounter.encounter.id,
            "title": "Daily note",
            "kind": "progress",
            "body": "Canonical body",
            "status": "draft",
            "summary": null
        }),
    )
    .await?;
    Ok(ChartScope {
        client_id: client.client.id,
        encounter_id: encounter.encounter.id,
        note: note.note,
    })
}

pub(super) async fn create_checkpoint(
    server: &mut TestAppServer,
    scope: &ChartScope,
    session_id: Option<&str>,
    title: &str,
) -> Result<WorkspaceDraftCheckpointCreateResponse> {
    request(
        server,
        "workspace/draft/checkpoint/create",
        json!({
            "sessionId": session_id,
            "clientId": scope.client_id,
            "encounterId": scope.encounter_id,
            "noteId": scope.note.id,
            "baseNoteRevision": scope.note.current_revision,
            "draft": {"schemaVersion": 1, "note": {"title": title}},
            "trigger": "focusChange",
            "actor": "Clinician Example"
        }),
    )
    .await
}

pub(super) fn guide_request() -> Value {
    json!({"focus": "noteBody", "selectedFields": ["note.title", "note.body"]})
}

pub(super) fn start_params(
    client_id: &str,
    checkpoint: &WorkspaceDraftCheckpointCreateResponse,
    key: &str,
    request: Value,
) -> Value {
    json!({
        "clientId": client_id,
        "sessionId": checkpoint.checkpoint.session_id,
        "sourceCheckpointId": checkpoint.checkpoint.id,
        "sourceCheckpointRevision": checkpoint.checkpoint.revision,
        "sourceCheckpointSha256": checkpoint.checkpoint.content_sha256,
        "request": request,
        "idempotencyKey": key,
        "trigger": "focusChange",
        "actor": "Clinician Example",
        "provider": "test-provider",
        "model": "test-model"
    })
}

pub(super) fn finish_params(
    started: &WorkspaceGuideRunStartResponse,
    source: Option<(&str, &str)>,
    outcome: Value,
) -> Value {
    json!({
        "runId": started.run.id,
        "clientId": started.run.client_id,
        "sessionId": started.run.session_id,
        "sourceCheckpointId": started.run.source_checkpoint_id,
        "sourceCheckpointRevision": started.run.source_checkpoint_revision,
        "sourceCheckpointSha256": started.run.source_checkpoint_sha256,
        "requestEnvelopeSha256": started.run.request_envelope_sha256,
        "sourceThreadId": source.map(|(thread, _)| thread),
        "sourceTurnId": source.map(|(_, turn)| turn),
        "outcome": outcome,
        "actor": "Workspace Guide"
    })
}

pub(super) fn assert_error_kind(error: JSONRPCError, expected: &str) {
    assert_eq!(error.error.data, Some(json!({"kind": expected})));
}

pub(super) async fn request<T: DeserializeOwned>(
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

pub(super) async fn request_error(
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
