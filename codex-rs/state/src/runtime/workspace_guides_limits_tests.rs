use super::*;
use crate::StateRuntime;
use crate::runtime::test_support::unique_temp_dir;
use pretty_assertions::assert_eq;
use serde_json::json;

const FIXED_RUN_ID: &str = "00000000-0000-4000-8000-000000000000";

fn start_input() -> crate::WorkspaceGuideRunStart {
    crate::WorkspaceGuideRunStart {
        client_id: "00000000-0000-4000-8000-000000000001".to_string(),
        session_id: "00000000-0000-4000-8000-000000000002".to_string(),
        source_checkpoint_id: "00000000-0000-4000-8000-000000000003".to_string(),
        source_checkpoint_revision: 1,
        source_checkpoint_sha256: "a".repeat(64),
        request_json: r#"{"focus":"noteBody"}"#.to_string(),
        idempotency_key: "guide-key".to_string(),
        trigger: "focusChange".to_string(),
        actor: "Clinician Example".to_string(),
        provider: "test-provider".to_string(),
        model: "test-model".to_string(),
    }
}

fn finish_input() -> crate::WorkspaceGuideRunFinish {
    crate::WorkspaceGuideRunFinish {
        run_id: FIXED_RUN_ID.to_string(),
        client_id: "00000000-0000-4000-8000-000000000001".to_string(),
        session_id: "00000000-0000-4000-8000-000000000002".to_string(),
        source_checkpoint_id: "00000000-0000-4000-8000-000000000003".to_string(),
        source_checkpoint_revision: 1,
        source_checkpoint_sha256: "a".repeat(64),
        request_envelope_sha256: "b".repeat(64),
        source_thread_id: Some("guide-thread".to_string()),
        source_turn_id: Some("guide-turn".to_string()),
        outcome: crate::WorkspaceGuideRunOutcome::Completed {
            result_json: r#"{"schemaVersion":1}"#.to_string(),
        },
        actor: "Workspace Guide".to_string(),
    }
}

#[test]
fn workspace_guide_envelopes_accept_exact_limits_and_reject_one_byte_over() {
    let input = start_input();
    let (empty_request, _) = request_envelope(FIXED_RUN_ID, &input, json!({"text": ""}))
        .expect("empty request should fit");
    let request_fill = envelopes::MAX_REQUEST_BYTES - empty_request.len();
    let (request, _) = request_envelope(
        FIXED_RUN_ID,
        &input,
        json!({"text": "x".repeat(request_fill)}),
    )
    .expect("request at byte limit should fit");
    assert_eq!(request.len(), envelopes::MAX_REQUEST_BYTES);
    let error = request_envelope(
        FIXED_RUN_ID,
        &input,
        json!({"text": "x".repeat(request_fill + 1)}),
    )
    .expect_err("request over byte limit should fail");
    assert_eq!(error.kind(), "validation");

    let empty_outcome = crate::WorkspaceGuideRunOutcome::Completed {
        result_json: r#"{"schemaVersion":1,"text":""}"#.to_string(),
    };
    let (_, empty_terminal, _) =
        terminal_envelope(&empty_outcome).expect("empty terminal should fit");
    let terminal_fill = envelopes::MAX_TERMINAL_BYTES - empty_terminal.len();
    let exact_outcome = crate::WorkspaceGuideRunOutcome::Completed {
        result_json: json!({"schemaVersion": 1, "text": "x".repeat(terminal_fill)}).to_string(),
    };
    let (_, terminal, _) =
        terminal_envelope(&exact_outcome).expect("terminal at byte limit should fit");
    assert_eq!(terminal.len(), envelopes::MAX_TERMINAL_BYTES);
    let oversized_outcome = crate::WorkspaceGuideRunOutcome::Completed {
        result_json: json!({"schemaVersion": 1, "text": "x".repeat(terminal_fill + 1)}).to_string(),
    };
    let error =
        terminal_envelope(&oversized_outcome).expect_err("terminal over byte limit should fail");
    assert_eq!(error.kind(), "validation");
}

#[test]
fn workspace_guide_scalar_limits_are_byte_exact() {
    for (field, max_bytes) in [
        ("idempotency", input_validation::MAX_IDEMPOTENCY_KEY_BYTES),
        ("trigger", input_validation::MAX_TRIGGER_BYTES),
        ("actor", input_validation::MAX_ACTOR_BYTES),
        ("provider", input_validation::MAX_PROVIDER_BYTES),
        ("model", input_validation::MAX_MODEL_BYTES),
    ] {
        let mut exact = start_input();
        set_start_field(&mut exact, field, "x".repeat(max_bytes));
        validate_start(&exact).expect("start scalar at byte limit should fit");

        let mut oversized = start_input();
        set_start_field(&mut oversized, field, "x".repeat(max_bytes + 1));
        let error = validate_start(&oversized).expect_err("oversized start scalar should fail");
        assert_eq!(error.kind(), "validation");
        assert!(error.to_string().contains("byte limit"));
    }

    let mut exact = finish_input();
    exact.actor = "x".repeat(input_validation::MAX_ACTOR_BYTES);
    exact.source_thread_id = Some("x".repeat(input_validation::MAX_SOURCE_PROVENANCE_ID_BYTES));
    exact.source_turn_id = Some("y".repeat(input_validation::MAX_SOURCE_PROVENANCE_ID_BYTES));
    validate_finish_metadata(&exact).expect("finish scalars at byte limits should fit");

    for field in ["actor", "thread", "turn"] {
        let mut oversized = finish_input();
        match field {
            "actor" => oversized.actor = "x".repeat(input_validation::MAX_ACTOR_BYTES + 1),
            "thread" => {
                oversized.source_thread_id =
                    Some("x".repeat(input_validation::MAX_SOURCE_PROVENANCE_ID_BYTES + 1));
            }
            "turn" => {
                oversized.source_turn_id =
                    Some("x".repeat(input_validation::MAX_SOURCE_PROVENANCE_ID_BYTES + 1));
            }
            _ => unreachable!(),
        }
        let error =
            validate_finish_metadata(&oversized).expect_err("oversized finish scalar should fail");
        assert_eq!(error.kind(), "validation");
        assert!(error.to_string().contains("byte limit"));
    }
}

#[test]
fn workspace_guide_finish_rejects_one_sided_provenance() {
    for (thread_id, turn_id) in [
        (Some("guide-thread".to_string()), None),
        (None, Some("guide-turn".to_string())),
    ] {
        let mut input = finish_input();
        input.source_thread_id = thread_id;
        input.source_turn_id = turn_id;
        let error =
            validate_finish_metadata(&input).expect_err("one-sided source provenance should fail");
        assert_eq!(error.kind(), "validation");
        assert!(error.to_string().contains("must be supplied together"));
    }
}

#[tokio::test]
async fn workspace_guide_start_rejects_wrong_checkpoint_revision_and_hash() {
    let (runtime, client, checkpoint) = persisted_fixture().await;
    let input = persisted_start(&client, &checkpoint);

    let mut wrong_revision = input.clone();
    wrong_revision.source_checkpoint_revision += 1;
    let error = runtime
        .workspace()
        .start_guide_run(wrong_revision)
        .await
        .expect_err("wrong checkpoint revision should fail");
    assert_eq!(error.kind(), "validation");

    let mut wrong_hash = input;
    wrong_hash.source_checkpoint_sha256 = "0".repeat(64);
    let error = runtime
        .workspace()
        .start_guide_run(wrong_hash)
        .await
        .expect_err("wrong checkpoint hash should fail");
    assert_eq!(error.kind(), "validation");
}

#[tokio::test]
async fn workspace_guide_reads_fail_closed_on_envelope_hash_corruption() {
    let (runtime, client, checkpoint) = persisted_fixture().await;
    let run = runtime
        .workspace()
        .start_guide_run(persisted_start(&client, &checkpoint))
        .await
        .expect("guide run should start");
    sqlx::query("UPDATE workspace_guide_runs SET request_envelope_json = '{}' WHERE id = ?")
        .bind(&run.id)
        .execute(runtime.workspace().pool.as_ref())
        .await
        .expect("request envelope corruption fixture should apply");
    let error = runtime
        .workspace()
        .list_guide_runs(crate::WorkspaceGuideRunFilter {
            client_id: client.id,
            ..Default::default()
        })
        .await
        .expect_err("request envelope hash mismatch should fail closed");
    assert_eq!(error.kind(), "storage");
    assert!(
        error
            .to_string()
            .contains("request envelope SHA-256 mismatch")
    );

    let (runtime, client, checkpoint) = persisted_fixture().await;
    let run = runtime
        .workspace()
        .start_guide_run(persisted_start(&client, &checkpoint))
        .await
        .expect("guide run should start");
    let finished = runtime
        .workspace()
        .finish_guide_run(crate::WorkspaceGuideRunFinish {
            run_id: run.id,
            client_id: run.client_id,
            session_id: run.session_id,
            source_checkpoint_id: run.source_checkpoint_id,
            source_checkpoint_revision: run.source_checkpoint_revision,
            source_checkpoint_sha256: run.source_checkpoint_sha256,
            request_envelope_sha256: run.request_envelope_sha256,
            source_thread_id: None,
            source_turn_id: None,
            outcome: crate::WorkspaceGuideRunOutcome::Canceled {
                reason: "synthetic cancellation".to_string(),
            },
            actor: "Workspace Guide".to_string(),
        })
        .await
        .expect("guide run should finish");
    sqlx::query(
        "UPDATE workspace_guide_runs SET terminal_envelope_json = '{\"schemaVersion\":1,\"type\":\"canceled\",\"reason\":\"corrupt\"}' WHERE id = ?",
    )
    .bind(&finished.id)
    .execute(runtime.workspace().pool.as_ref())
    .await
    .expect("terminal envelope corruption fixture should apply");
    let error = runtime
        .workspace()
        .list_guide_runs(crate::WorkspaceGuideRunFilter {
            client_id: client.id,
            ..Default::default()
        })
        .await
        .expect_err("terminal envelope hash mismatch should fail closed");
    assert_eq!(error.kind(), "storage");
    assert!(
        error
            .to_string()
            .contains("terminal envelope SHA-256 mismatch")
    );
}

fn set_start_field(input: &mut crate::WorkspaceGuideRunStart, field: &str, value: String) {
    match field {
        "idempotency" => input.idempotency_key = value,
        "trigger" => input.trigger = value,
        "actor" => input.actor = value,
        "provider" => input.provider = value,
        "model" => input.model = value,
        _ => unreachable!(),
    }
}

async fn persisted_fixture() -> (
    std::sync::Arc<StateRuntime>,
    crate::WorkspaceClient,
    crate::WorkspaceDraftCheckpoint,
) {
    let runtime = StateRuntime::init(unique_temp_dir(), "test-provider".to_string())
        .await
        .expect("state db should initialize");
    let client = runtime
        .workspace()
        .upsert_client(crate::WorkspaceClientUpsert {
            display_name: "Synthetic Guide Limit Patient".to_string(),
            ..Default::default()
        })
        .await
        .expect("client should save");
    let checkpoint = runtime
        .workspace()
        .create_draft_checkpoint(crate::WorkspaceDraftCheckpointCreate {
            client_id: client.id.clone(),
            draft_json: r#"{"schemaVersion":1,"note":{"title":"Synthetic"}}"#.to_string(),
            trigger: "focusChange".to_string(),
            actor: "Clinician Example".to_string(),
            ..Default::default()
        })
        .await
        .expect("checkpoint should save");
    (runtime, client, checkpoint)
}

fn persisted_start(
    client: &crate::WorkspaceClient,
    checkpoint: &crate::WorkspaceDraftCheckpoint,
) -> crate::WorkspaceGuideRunStart {
    crate::WorkspaceGuideRunStart {
        client_id: client.id.clone(),
        session_id: checkpoint.session_id.clone(),
        source_checkpoint_id: checkpoint.id.clone(),
        source_checkpoint_revision: checkpoint.revision,
        source_checkpoint_sha256: checkpoint.content_sha256.clone(),
        request_json: r#"{"focus":"noteBody"}"#.to_string(),
        idempotency_key: "guide-limit-key".to_string(),
        trigger: "focusChange".to_string(),
        actor: "Clinician Example".to_string(),
        provider: "test-provider".to_string(),
        model: "test-model".to_string(),
    }
}
