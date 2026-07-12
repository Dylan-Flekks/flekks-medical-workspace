use anyhow::Result;
use app_test_support::ChatGptIdTokenClaims;
use app_test_support::TestAppServer;
use app_test_support::encode_id_token;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use app_test_support::write_mock_responses_config_toml_with_chatgpt_base_url;
use app_test_support::write_models_cache;
use codex_app_server_protocol::AdditionalContextEntry;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_protocol::config_types::ModelToolMode;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::HashMap;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::ResponseTemplate;

use super::analytics::mount_analytics_capture;
use super::analytics::wait_for_analytics_event;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;

fn isolated_thread_start_params() -> ThreadStartParams {
    ThreadStartParams {
        model: Some("gpt-5.4".to_string()),
        model_provider: Some("mock_provider".to_string()),
        model_tool_mode: Some(ModelToolMode::Isolated),
        ephemeral: Some(true),
        environments: Some(Vec::new()),
        runtime_workspace_roots: Some(Vec::new()),
        ..Default::default()
    }
}

fn isolated_output_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "answer": { "type": "string" }
        },
        "required": ["answer"],
        "additionalProperties": false
    })
}

async fn start_isolated_thread(mcp: &mut TestAppServer) -> Result<ThreadStartResponse> {
    let request_id = mcp
        .send_thread_start_request(isolated_thread_start_params())
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn isolated_start_and_turn_use_the_bounded_app_server_path() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", r#"{"answer":"ok"}"#),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml(
        codex_home.path(),
        &server.uri(),
        &BTreeMap::new(),
        /*auto_compact_limit*/ 1024,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "ambient compact instructions",
    )?;
    write_models_cache(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let ThreadStartResponse {
        thread,
        model_tool_mode,
        runtime_workspace_roots,
        instruction_sources,
        ..
    } = start_isolated_thread(&mut mcp).await?;
    assert_eq!(model_tool_mode, ModelToolMode::Isolated);
    assert_eq!(runtime_workspace_roots, Vec::new());
    assert_eq!(instruction_sources, Vec::new());

    let turn_request = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![UserInput::Text {
                text: "analyze the bounded packet".to_string(),
                text_elements: Vec::new(),
            }],
            output_schema: Some(isolated_output_schema()),
            ..Default::default()
        })
        .await?;
    let turn_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_request)),
    )
    .await??;
    let _: TurnStartResponse = to_response(turn_response)?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let request = response_mock.single_request();
    let body = request.body_json();
    assert_eq!(body["tools"], json!([]));
    assert_eq!(body["parallel_tool_calls"], json!(false));
    assert_eq!(body["input"].as_array().map(Vec::len), Some(1));
    assert_eq!(body["input"][0]["role"], json!("user"));
    assert_eq!(
        body["input"][0]["content"][0]["text"],
        json!("analyze the bounded packet")
    );
    let serialized = body.to_string();
    assert!(!serialized.contains("ambient compact instructions"));
    assert!(!serialized.contains(codex_home.path().to_string_lossy().as_ref()));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn isolated_app_server_path_emits_no_thread_or_turn_analytics() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-analytics"),
            responses::ev_assistant_message("msg-analytics", r#"{"answer":"ok"}"#),
            responses::ev_completed("resp-analytics"),
        ]),
    )
    .await;
    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml_with_chatgpt_base_url(
        codex_home.path(),
        &server.uri(),
        &server.uri(),
    )?;
    write_models_cache(codex_home.path())?;
    mount_analytics_capture(&server, codex_home.path()).await?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .without_managed_config()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let normal_start_request = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;
    let normal_start_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(normal_start_request)),
    )
    .await??;
    let ThreadStartResponse {
        thread: normal_thread,
        ..
    } = to_response(normal_start_response)?;
    let normal_event =
        wait_for_analytics_event(&server, DEFAULT_READ_TIMEOUT, "codex_thread_initialized").await?;
    assert_eq!(
        normal_event["event_params"]["thread_id"].as_str(),
        Some(normal_thread.id.as_str())
    );

    let ThreadStartResponse { thread, .. } = start_isolated_thread(&mut mcp).await?;
    let stale_turn_request = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: "00000000-0000-0000-0000-000000000099".to_string(),
            input: vec![UserInput::Text {
                text: "stale clinical packet sentinel".to_string(),
                text_elements: Vec::new(),
            }],
            output_schema: Some(isolated_output_schema()),
            ..Default::default()
        })
        .await?;
    let stale_turn_error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(stale_turn_request)),
    )
    .await??;
    assert!(stale_turn_error.error.message.contains("thread not found"));

    let turn_request = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "clinical packet sentinel".to_string(),
                text_elements: Vec::new(),
            }],
            output_schema: Some(isolated_output_schema()),
            ..Default::default()
        })
        .await?;
    let turn_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_request)),
    )
    .await??;
    let _: TurnStartResponse = to_response(turn_response)?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    let analytics_events = server
        .received_requests()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|request| request.url.path() == "/codex/analytics-events/events")
        .filter_map(|request| serde_json::from_slice::<serde_json::Value>(&request.body).ok())
        .flat_map(|payload| payload["events"].as_array().cloned().unwrap_or_default())
        .collect::<Vec<_>>();
    assert!(analytics_events.iter().all(|event| {
        event["event_params"]["thread_id"].as_str() != Some(thread.id.as_str())
            && event["event_params"]["session_id"].as_str() != Some(thread.session_id.as_str())
    }));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn isolated_unauthorized_does_not_request_external_auth_refresh() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_response_sequence(
        &server,
        vec![ResponseTemplate::new(401).set_body_json(json!({
            "error": {"message": "unauthorized"}
        }))],
    )
    .await;
    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml(
        codex_home.path(),
        &server.uri(),
        &BTreeMap::new(),
        /*auto_compact_limit*/ 1024,
        /*requires_openai_auth*/ Some(true),
        "mock_provider",
        "compact",
    )?;
    write_models_cache(codex_home.path())?;
    let access_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email("isolated@example.com")
            .plan_type("pro")
            .chatgpt_account_id("123e4567-e89b-42d3-a456-426614174099"),
    )?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;
    let login_request = mcp
        .send_chatgpt_auth_tokens_login_request(
            access_token.clone(),
            "123e4567-e89b-42d3-a456-426614174099".to_string(),
            Some("pro".to_string()),
        )
        .await?;
    let login_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(login_request)),
    )
    .await??;
    let _: LoginAccountResponse = to_response(login_response)?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("account/updated"),
    )
    .await??;

    let ThreadStartResponse { thread, .. } = start_isolated_thread(&mut mcp).await?;
    let turn_request = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![UserInput::Text {
                text: "bounded packet".to_string(),
                text_elements: Vec::new(),
            }],
            output_schema: Some(isolated_output_schema()),
            ..Default::default()
        })
        .await?;
    let turn_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_request)),
    )
    .await??;
    let _: TurnStartResponse = to_response(turn_response)?;

    assert!(
        timeout(
            std::time::Duration::from_millis(500),
            mcp.read_stream_until_request_message(),
        )
        .await
        .is_err(),
        "isolated 401 must not emit account/chatgptAuthTokens/refresh"
    );
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].header("authorization"),
        Some(format!("Bearer {access_token}"))
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn isolated_invalid_schema_does_not_consume_the_single_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", r#"{"answer":"ok"}"#),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml(
        codex_home.path(),
        &server.uri(),
        &BTreeMap::new(),
        /*auto_compact_limit*/ 1024,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )?;
    write_models_cache(codex_home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let ThreadStartResponse { thread, .. } = start_isolated_thread(&mut mcp).await?;
    let turn_params = || TurnStartParams {
        thread_id: thread.id.clone(),
        input: vec![UserInput::Text {
            text: "analyze once".to_string(),
            text_elements: Vec::new(),
        }],
        output_schema: Some(isolated_output_schema()),
        ..Default::default()
    };
    let invalid_request = mcp
        .send_turn_start_request(TurnStartParams {
            output_schema: Some(json!({
                "type": "array",
                "items": {"type": "string"}
            })),
            ..turn_params()
        })
        .await?;
    let invalid_error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(invalid_request)),
    )
    .await??;
    assert_eq!(invalid_error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert!(
        invalid_error
            .error
            .message
            .contains("schema root type must be object")
    );

    let first_request = mcp.send_turn_start_request(turn_params()).await?;
    let first_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(first_request)),
    )
    .await??;
    let _: TurnStartResponse = to_response(first_response)?;

    let second_request = mcp.send_turn_start_request(turn_params()).await?;
    let second_error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(second_request)),
    )
    .await??;
    assert_eq!(second_error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        second_error.error.message,
        "isolated model mode permits exactly one turn"
    );
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    response_mock.single_request();

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn isolated_start_and_turn_reject_ambient_overrides_before_inference() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml(
        codex_home.path(),
        &server.uri(),
        &BTreeMap::new(),
        /*auto_compact_limit*/ 1024,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )?;
    write_models_cache(codex_home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_request = mcp
        .send_thread_start_request(ThreadStartParams {
            cwd: Some(codex_home.path().display().to_string()),
            ..isolated_thread_start_params()
        })
        .await?;
    let start_error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(start_request)),
    )
    .await??;
    assert_eq!(start_error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert!(start_error.error.message.contains("`cwd`"));

    let ThreadStartResponse { thread, .. } = start_isolated_thread(&mut mcp).await?;
    let turn_request = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![UserInput::Text {
                text: "do not infer".to_string(),
                text_elements: Vec::new(),
            }],
            output_schema: Some(isolated_output_schema()),
            additional_context: Some(HashMap::<String, AdditionalContextEntry>::new()),
            ..Default::default()
        })
        .await?;
    let turn_error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(turn_request)),
    )
    .await??;
    assert_eq!(turn_error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert!(turn_error.error.message.contains("`additionalContext`"));

    let requests = server
        .received_requests()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|request| request.url.path().ends_with("/responses"))
        .count();
    assert_eq!(requests, 0);

    Ok(())
}
