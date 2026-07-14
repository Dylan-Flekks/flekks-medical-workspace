use anyhow::Context;
use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml_with_chatgpt_base_url;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_app_server_protocol::WorkspaceAgentRunStartResponse;
use codex_app_server_protocol::WorkspaceClientUpsertResponse;
use codex_app_server_protocol::WorkspaceContextPacketCreateResponse;
use codex_app_server_protocol::WorkspaceNoteUpsertResponse;
use codex_protocol::config_types::ModelToolMode;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde::de::DeserializeOwned;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

use super::workspace_chart_commit::DEFAULT_READ_TIMEOUT;

const CLASSIFICATION_ENV: &str = "FLEKKS_MEDICAL_WORKSPACE_DATA_CLASSIFICATION";
const DIRECT_CONTEXT_READ_DENIAL: &str = "workspace agent context is available only to the claimed workspaceContextOnly model turn through the workspace_context_read tool; run-id-only RPC reads are not authorized";

struct WorkspaceAgentFixture {
    _root: TempDir,
    server: TestAppServer,
}

struct StartedAgentRun {
    id: String,
    prompt: String,
}

#[tokio::test]
async fn unclaimed_agent_run_rejects_run_id_only_context_rpc() -> Result<()> {
    let model_server = responses::start_mock_server().await;
    let mut fixture = WorkspaceAgentFixture::start(&model_server.uri()).await?;
    let run = start_agent_run(
        &mut fixture.server,
        "thread-unclaimed",
        "mock_provider",
        "mock-model",
        "unclaimed-context-rpc",
    )
    .await?;

    assert_direct_context_read_denied(&mut fixture.server, &run.id).await?;
    assert!(
        model_server
            .received_requests()
            .await
            .context("mock model server should retain request history")?
            .is_empty()
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn claimed_wco_tool_read_succeeds_but_run_id_only_rpc_stays_denied() -> Result<()> {
    let call_id = "workspace-context-call";
    let model_server = responses::start_mock_server().await;
    let mut fixture = WorkspaceAgentFixture::start(&model_server.uri()).await?;

    let thread_request = fixture
        .server
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        fixture
            .server
            .read_stream_until_response_message(RequestId::Integer(thread_request)),
    )
    .await??;
    let ThreadStartResponse {
        thread,
        model,
        model_provider,
        ..
    } = to_response(thread_response)?;
    let run = start_agent_run(
        &mut fixture.server,
        &thread.id,
        &model_provider,
        &model,
        "bound-wco-context-read",
    )
    .await?;

    let response_mock = responses::mount_sse_sequence(
        &model_server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("response-tool-bound"),
                responses::ev_function_call(
                    call_id,
                    "workspace_context_read",
                    &json!({
                        "run_id": &run.id,
                        "category": "progress_notes",
                        "limit": 5,
                    })
                    .to_string(),
                ),
                responses::ev_completed("response-tool-bound"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("response-final-bound"),
                responses::ev_assistant_message("message-final-bound", "Bound context reviewed."),
                responses::ev_completed("response-final-bound"),
            ]),
        ],
    )
    .await;

    let turn_request = fixture
        .server
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![V2UserInput::Text {
                text: run.prompt,
                text_elements: Vec::new(),
            }],
            model_tool_mode: Some(ModelToolMode::WorkspaceContextOnly),
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        fixture
            .server
            .read_stream_until_response_message(RequestId::Integer(turn_request)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        fixture
            .server
            .read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    let output = requests[1].function_call_output(call_id);
    let output_text = output
        .get("output")
        .and_then(Value::as_str)
        .context("workspace context tool output should be JSON text")?;
    let output: Value = serde_json::from_str(output_text)?;
    assert_eq!(output["run_id"], json!(run.id));
    assert_eq!(output["category"], json!("progress_notes"));
    let sources = output["sources"]
        .as_array()
        .context("workspace context tool output should contain sources")?;
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0]["source_entity_type"], json!("note_revision"));

    assert_direct_context_read_denied(&mut fixture.server, &run.id).await?;
    Ok(())
}

impl WorkspaceAgentFixture {
    async fn start(model_server_uri: &str) -> Result<Self> {
        let root = TempDir::new()?;
        let codex_home = root.path().join("codex-home");
        let sqlite_home = root.path().join("medical-sqlite-home");
        std::fs::create_dir_all(&codex_home)?;
        std::fs::create_dir_all(&sqlite_home)?;
        write_mock_responses_config_toml_with_chatgpt_base_url(
            &codex_home,
            model_server_uri,
            model_server_uri,
        )?;
        let configured_sqlite_home = sqlite_home.to_string_lossy().into_owned();
        let sqlite_override = format!(
            "sqlite_home={}",
            serde_json::to_string(&configured_sqlite_home)?
        );
        let mut server = TestAppServer::builder()
            .with_codex_home(&codex_home)
            .with_env_overrides(&[
                (CLASSIFICATION_ENV, Some("synthetic")),
                (
                    codex_state::SQLITE_HOME_ENV,
                    Some(configured_sqlite_home.as_str()),
                ),
            ])
            .with_args(&["-c", sqlite_override.as_str()])
            .build()
            .await?;
        timeout(DEFAULT_READ_TIMEOUT, server.initialize()).await??;
        let _: Value = request(&mut server, "workspace/dataPolicy/provision", json!({})).await?;
        Ok(Self {
            _root: root,
            server,
        })
    }
}

async fn start_agent_run(
    server: &mut TestAppServer,
    thread_id: &str,
    provider: &str,
    model: &str,
    idempotency_key: &str,
) -> Result<StartedAgentRun> {
    let client: WorkspaceClientUpsertResponse = request(
        server,
        "workspace/client/upsert",
        json!({
            "displayName": "Synthetic Context Patient",
            "summary": "Synthetic app-server authorization fixture",
        }),
    )
    .await?;
    let note: WorkspaceNoteUpsertResponse = request(
        server,
        "workspace/note/upsert",
        json!({
            "id": null,
            "clientId": &client.client.id,
            "encounterId": null,
            "title": "Synthetic prior progress note",
            "kind": "progress",
            "body": "Synthetic prior progress content.",
            "status": "draft",
            "summary": "Synthetic prior progress note",
        }),
    )
    .await?;
    let human_request = "Review the submitted synthetic progress-note packet.";
    let context_envelope_json = json!({
        "assemblyVersion": "app-server-context-auth-test-v1",
        "sourceMode": "agent_request",
        "includeDocuments": false,
        "humanRequest": human_request,
        "ids": {
            "selectedArtifactIds": [],
            "selectedDerivativeIds": [],
            "selectedClipIds": [],
        },
        "patient": { "displayName": "Synthetic Context Patient" },
        "summaries": { "chartContextSummary": "Synthetic chart context" },
        "safety": [
            "read-only context packet; do not mutate workspace records",
            "do not sign notes, submit claims, send payer communications, or overwrite saved data",
        ],
        "promptSnapshot": "Synthetic packet without filesystem paths.",
    })
    .to_string();
    let packet: WorkspaceContextPacketCreateResponse = request(
        server,
        "workspace/context/packet/create",
        json!({
            "clientId": &client.client.id,
            "encounterId": null,
            "noteId": &note.note.id,
            "humanRequest": human_request,
            "selectedArtifactIdsJson": "[]",
            "selectedDerivativeIdsJson": "[]",
            "selectedClipIdsJson": "[]",
            "artifactSummary": "0 selected files",
            "derivativeSummary": "0 selected text items",
            "clipSummary": "0 selected clips",
            "chartContextSummary": "Synthetic chart context",
            "contextEnvelopeJson": &context_envelope_json,
            "clinicianActor": "Synthetic Clinician",
            "baseNoteRevision": note.note.current_revision,
            "authorizedScopeJson": json!({
                "version": 1,
                "categories": ["progress_notes"],
                "maxRecords": 5,
            }).to_string(),
            "expectedOutputKind": "note_proposal",
        }),
    )
    .await?;
    let run: WorkspaceAgentRunStartResponse = request(
        server,
        "workspace/agent/run/start",
        json!({
            "packetId": &packet.packet.id,
            "idempotencyKey": idempotency_key,
            "clientId": &packet.packet.client_id,
            "contextEnvelopeSha256": &packet.packet.context_envelope_sha256,
            "provider": provider,
            "model": model,
            "sourceThreadId": thread_id,
            "sourceTurnId": null,
        }),
    )
    .await?;
    let prompt = codex_state::render_workspace_agent_handoff_prompt(
        &codex_state::WorkspaceAgentHandoffPromptInput {
            packet_id: packet.packet.id,
            client_id: packet.packet.client_id,
            encounter_id: packet.packet.encounter_id,
            note_id: packet.packet.note_id,
            human_request: packet.packet.human_request,
            chart_context_summary: packet.packet.chart_context_summary,
            context_envelope_json: packet.packet.context_envelope_json,
            context_envelope_sha256: packet.packet.context_envelope_sha256,
            authorized_scope_json: packet.packet.authorized_scope_json,
        },
        Some(&run.run.id),
    );
    Ok(StartedAgentRun {
        id: run.run.id,
        prompt,
    })
}

async fn assert_direct_context_read_denied(server: &mut TestAppServer, run_id: &str) -> Result<()> {
    let error = request_error(
        server,
        "workspace/agent/run/context/read",
        json!({
            "runId": run_id,
            "category": "progressNotes",
            "limit": 5,
        }),
    )
    .await?;
    assert_eq!(error.error.message, DIRECT_CONTEXT_READ_DENIAL);
    Ok(())
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
