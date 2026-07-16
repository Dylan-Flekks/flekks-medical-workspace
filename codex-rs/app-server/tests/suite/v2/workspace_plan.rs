use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::WorkspacePlanSnapshotGetResponse;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;

use super::workspace_chart_commit::DEFAULT_READ_TIMEOUT;
use super::workspace_chart_commit::create_config_toml;

#[tokio::test]
async fn workspace_plan_finish_rejects_completion_but_routes_recovery_outcomes() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, server.initialize()).await??;

    let completed = request_error(
        &mut server,
        finish_params(json!({
            "status": "completed",
            "result_json": r#"{"schemaVersion":1}"#,
        })),
    )
    .await?;
    assert_eq!(completed.error.code, -32600);
    assert!(completed.error.message.contains("completed"));

    for outcome in [
        json!({
            "status": "failed",
            "error_summary": "synthetic provider failure",
        }),
        json!({
            "status": "canceled",
            "reason": "synthetic recovery cancellation",
        }),
    ] {
        let recovery = request_error(&mut server, finish_params(outcome)).await?;
        assert!(
            recovery
                .error
                .message
                .contains("workspace guide run `missing-run` was not found"),
            "recovery outcome should reach state instead of the completion rejection: {recovery:?}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn workspace_plan_public_writes_cannot_forge_agent_provenance() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, server.initialize()).await??;

    let error_message_id = server
        .send_raw_request(
            "workspace/plan/message/append",
            Some(json!({
                "planSessionId": "session",
                "clientId": "client",
                "guideRunId": "run",
                "role": "error",
                "content": "caller-supplied planner error",
                "idempotencyKey": "forged-error",
                "sourceThreadId": "thread",
                "sourceTurnId": "turn",
            })),
        )
        .await?;
    let error_message = timeout(
        DEFAULT_READ_TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(error_message_id)),
    )
    .await??;
    assert_eq!(error_message.error.code, -32600);
    assert!(error_message.error.message.contains("unknown field `role`"));

    let proposal_id = server
        .send_raw_request(
            "workspace/plan/proposal/create",
            Some(json!({"summary": "caller-supplied agent proposal"})),
        )
        .await?;
    let proposal_error = timeout(
        DEFAULT_READ_TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(proposal_id)),
    )
    .await??;
    assert_eq!(proposal_error.error.code, -32600);
    assert!(
        proposal_error
            .error
            .message
            .contains("unknown variant `workspace/plan/proposal/create`")
    );

    Ok(())
}

#[tokio::test]
async fn workspace_plan_snapshot_returns_the_submission_receipt_collection() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, server.initialize()).await??;

    let request_id = server
        .send_raw_request(
            "workspace/plan/snapshot/get",
            Some(json!({
                "clientId": "client-without-a-plan-session",
                "planSessionId": null,
                "afterMessageSequence": null,
                "messageLimit": 20,
                "revisionLimit": 20,
                "proposalLimit": 20,
            })),
        )
        .await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let snapshot: WorkspacePlanSnapshotGetResponse = to_response(response)?;
    assert_eq!(snapshot.session, None);
    assert!(snapshot.messages.is_empty());
    assert!(snapshot.revisions.is_empty());
    assert!(snapshot.submission_receipts.is_empty());
    assert!(snapshot.proposals.is_empty());

    Ok(())
}

fn finish_params(outcome: Value) -> Value {
    json!({
        "runId": "missing-run",
        "clientId": "client",
        "draftSessionId": "draft-session",
        "sourceCheckpointId": "checkpoint",
        "sourceCheckpointRevision": 1,
        "sourceCheckpointSha256": "a".repeat(64),
        "requestEnvelopeSha256": "b".repeat(64),
        "sourceThreadId": null,
        "sourceTurnId": null,
        "outcome": outcome,
    })
}

async fn request_error(server: &mut TestAppServer, params: Value) -> Result<JSONRPCError> {
    let request_id = server
        .send_raw_request("workspace/plan/guideRun/finish", Some(params))
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await?
}
