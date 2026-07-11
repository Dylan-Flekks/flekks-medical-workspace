use anyhow::Result;
use codex_app_server_protocol::ModelToolMode;
use codex_app_server_protocol::WorkspaceGuideRunFinishResponse;
use codex_app_server_protocol::WorkspaceGuideRunListResponse;
use codex_app_server_protocol::WorkspaceGuideRunStartResponse;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::workspace_guides::assert_error_kind;
use super::workspace_guides::create_chart_scope;
use super::workspace_guides::create_checkpoint;
use super::workspace_guides::finish_params;
use super::workspace_guides::guide_request;
use super::workspace_guides::request;
use super::workspace_guides::request_error;
use super::workspace_guides::server;
use super::workspace_guides::start_params;

#[tokio::test]
async fn workspace_guides_fail_closed_with_typed_errors_and_patient_scope() -> Result<()> {
    let (_codex_home, mut server) = server().await?;
    let scope = create_chart_scope(&mut server, "Synthetic Guide Patient").await?;
    let other = create_chart_scope(&mut server, "Other Synthetic Patient").await?;
    let checkpoint = create_checkpoint(&mut server, &scope, None, "First draft").await?;

    assert_error_kind(
        request_error(
            &mut server,
            "workspace/guide/run/start",
            start_params(
                &other.client_id,
                &checkpoint,
                "wrong-patient",
                guide_request(),
            ),
        )
        .await?,
        "validation",
    );
    let mut wrong_revision = start_params(
        &scope.client_id,
        &checkpoint,
        "wrong-revision",
        guide_request(),
    );
    wrong_revision["sourceCheckpointRevision"] =
        json!(checkpoint.checkpoint.revision.saturating_add(1));
    assert_error_kind(
        request_error(&mut server, "workspace/guide/run/start", wrong_revision).await?,
        "validation",
    );
    let mut wrong_checkpoint_hash =
        start_params(&scope.client_id, &checkpoint, "wrong-hash", guide_request());
    wrong_checkpoint_hash["sourceCheckpointSha256"] = json!("0".repeat(64));
    assert_error_kind(
        request_error(
            &mut server,
            "workspace/guide/run/start",
            wrong_checkpoint_hash,
        )
        .await?,
        "validation",
    );
    assert_error_kind(
        request_error(
            &mut server,
            "workspace/guide/run/start",
            start_params(
                &scope.client_id,
                &checkpoint,
                "path-bearing",
                json!({"sourcePath": "/tmp/private"}),
            ),
        )
        .await?,
        "validation",
    );
    assert_error_kind(
        request_error(
            &mut server,
            "workspace/guide/run/start",
            start_params(&scope.client_id, &checkpoint, "not-object", json!(["hint"])),
        )
        .await?,
        "validation",
    );
    let mut params = start_params(&scope.client_id, &checkpoint, "guide-key", guide_request());
    params["modelToolMode"] = json!("default");
    let started: WorkspaceGuideRunStartResponse =
        request(&mut server, "workspace/guide/run/start", params).await?;
    assert_eq!(started.run.model_tool_mode, ModelToolMode::Disabled);
    assert_error_kind(
        request_error(
            &mut server,
            "workspace/guide/run/start",
            start_params(
                &scope.client_id,
                &checkpoint,
                "guide-key",
                json!({"focus": "different"}),
            ),
        )
        .await?,
        "idempotencyConflict",
    );
    assert_error_kind(
        request_error(
            &mut server,
            "workspace/guide/run/start",
            start_params(&scope.client_id, &checkpoint, "other-key", guide_request()),
        )
        .await?,
        "activeRunConflict",
    );
    assert_error_kind(
        request_error(
            &mut server,
            "workspace/guide/run/finish",
            finish_params(
                &started,
                None,
                json!({"type": "completed", "result": {"schemaVersion": 1}}),
            ),
        )
        .await?,
        "validation",
    );
    let mut one_sided_provenance = finish_params(
        &started,
        Some(("guide-thread", "guide-turn")),
        json!({"type": "completed", "result": {"schemaVersion": 1}}),
    );
    one_sided_provenance["sourceTurnId"] = json!(null);
    assert_error_kind(
        request_error(
            &mut server,
            "workspace/guide/run/finish",
            one_sided_provenance,
        )
        .await?,
        "validation",
    );
    let mut wrong_client = finish_params(
        &started,
        Some(("guide-thread", "guide-turn")),
        json!({"type": "completed", "result": {"schemaVersion": 1}}),
    );
    wrong_client["clientId"] = json!(other.client_id);
    assert_error_kind(
        request_error(&mut server, "workspace/guide/run/finish", wrong_client).await?,
        "validation",
    );
    assert_error_kind(
        request_error(
            &mut server,
            "workspace/guide/run/finish",
            finish_params(
                &started,
                Some(("guide-thread", "guide-turn")),
                json!({"type": "completed", "result": {"schemaVersion": 2}}),
            ),
        )
        .await?,
        "validation",
    );
    let mut wrong_hash = finish_params(
        &started,
        Some(("guide-thread", "guide-turn")),
        json!({"type": "completed", "result": {"schemaVersion": 1}}),
    );
    wrong_hash["requestEnvelopeSha256"] = json!("0".repeat(64));
    assert_error_kind(
        request_error(&mut server, "workspace/guide/run/finish", wrong_hash).await?,
        "validation",
    );
    assert_error_kind(
        request_error(
            &mut server,
            "workspace/guide/run/finish",
            finish_params(
                &started,
                Some(("guide-thread", "guide-turn")),
                json!({
                    "type": "completed",
                    "result": {"schemaVersion": 1, "localPath": "/tmp/private"}
                }),
            ),
        )
        .await?,
        "validation",
    );
    let _: WorkspaceGuideRunFinishResponse = request(
        &mut server,
        "workspace/guide/run/finish",
        finish_params(
            &started,
            Some(("guide-thread", "guide-turn")),
            json!({"type": "completed", "result": {"schemaVersion": 1}}),
        ),
    )
    .await?;
    assert_error_kind(
        request_error(
            &mut server,
            "workspace/guide/run/finish",
            finish_params(
                &started,
                Some(("guide-thread", "different-turn")),
                json!({"type": "completed", "result": {"schemaVersion": 1}}),
            ),
        )
        .await?,
        "terminalConflict",
    );

    let _new_checkpoint = create_checkpoint(
        &mut server,
        &scope,
        Some(&checkpoint.checkpoint.session_id),
        "Second draft",
    )
    .await?;
    assert_error_kind(
        request_error(
            &mut server,
            "workspace/guide/run/start",
            start_params(&scope.client_id, &checkpoint, "stale", guide_request()),
        )
        .await?,
        "staleCheckpoint",
    );
    assert_error_kind(
        request_error(
            &mut server,
            "workspace/guide/run/list",
            json!({"clientId": scope.client_id, "sessionId": " "}),
        )
        .await?,
        "validation",
    );
    assert_error_kind(
        request_error(
            &mut server,
            "workspace/guide/run/list",
            json!({"clientId": scope.client_id, "cursor": "not-a-cursor"}),
        )
        .await?,
        "validation",
    );
    let other_runs: WorkspaceGuideRunListResponse = request(
        &mut server,
        "workspace/guide/run/list",
        json!({"clientId": other.client_id}),
    )
    .await?;
    assert_eq!(other_runs.data, Vec::new());
    Ok(())
}
