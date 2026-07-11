use super::*;
use crate::StateRuntime;
use crate::runtime::test_support::unique_temp_dir;
use pretty_assertions::assert_eq;

async fn fixture() -> (
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
            display_name: "Synthetic Guide Patient".to_string(),
            ..Default::default()
        })
        .await
        .expect("client should save");
    let checkpoint = checkpoint(&runtime, &client, None, "First").await;
    (runtime, client, checkpoint)
}

async fn checkpoint(
    runtime: &StateRuntime,
    client: &crate::WorkspaceClient,
    session_id: Option<String>,
    title: &str,
) -> crate::WorkspaceDraftCheckpoint {
    runtime
        .workspace()
        .create_draft_checkpoint(crate::WorkspaceDraftCheckpointCreate {
            session_id,
            client_id: client.id.clone(),
            note_id: Some("draft-note".to_string()),
            base_note_revision: Some(1),
            draft_json: format!(r#"{{"schemaVersion":1,"note":{{"title":{title:?}}}}}"#),
            trigger: "focus_change".to_string(),
            actor: "Clinician Example".to_string(),
            ..Default::default()
        })
        .await
        .expect("checkpoint should save")
}

fn start(
    client: &crate::WorkspaceClient,
    checkpoint: &crate::WorkspaceDraftCheckpoint,
    key: &str,
    request_json: &str,
) -> crate::WorkspaceGuideRunStart {
    crate::WorkspaceGuideRunStart {
        client_id: client.id.clone(),
        session_id: checkpoint.session_id.clone(),
        source_checkpoint_id: checkpoint.id.clone(),
        source_checkpoint_revision: checkpoint.revision,
        source_checkpoint_sha256: checkpoint.content_sha256.clone(),
        request_json: request_json.to_string(),
        idempotency_key: key.to_string(),
        trigger: "focus_change".to_string(),
        actor: "Clinician Example".to_string(),
        provider: "test-provider".to_string(),
        model: "test-model".to_string(),
    }
}

fn finish(
    run: &crate::WorkspaceGuideRun,
    outcome: crate::WorkspaceGuideRunOutcome,
) -> crate::WorkspaceGuideRunFinish {
    crate::WorkspaceGuideRunFinish {
        run_id: run.id.clone(),
        client_id: run.client_id.clone(),
        session_id: run.session_id.clone(),
        source_checkpoint_id: run.source_checkpoint_id.clone(),
        source_checkpoint_revision: run.source_checkpoint_revision,
        source_checkpoint_sha256: run.source_checkpoint_sha256.clone(),
        request_envelope_sha256: run.request_envelope_sha256.clone(),
        source_thread_id: Some("guide-thread".to_string()),
        source_turn_id: Some("guide-turn".to_string()),
        outcome,
        actor: "Workspace Guide".to_string(),
    }
}

async fn begin(
    runtime: &StateRuntime,
    input: crate::WorkspaceGuideRunStart,
) -> crate::WorkspaceGuideRun {
    runtime.workspace().start_guide_run(input).await.unwrap()
}

async fn end(
    runtime: &StateRuntime,
    input: crate::WorkspaceGuideRunFinish,
) -> crate::WorkspaceGuideRun {
    runtime.workspace().finish_guide_run(input).await.unwrap()
}

async fn begin_error(runtime: &StateRuntime, input: crate::WorkspaceGuideRunStart) -> String {
    runtime
        .workspace()
        .start_guide_run(input)
        .await
        .unwrap_err()
        .to_string()
}

#[tokio::test]
async fn workspace_guide_run_lifecycle_is_bounded_exact_stale_and_noncanonical() {
    let (runtime, client, first) = fixture().await;
    let input = start(
        &client,
        &first,
        "guide-key",
        r#"{"focus":"noteBody","text":"synthetic draft"}"#,
    );
    let run = begin(&runtime, input.clone()).await;
    let envelope: Value = serde_json::from_str(&run.request_envelope_json).unwrap();
    assert_eq!(envelope["guideRunId"], run.id);
    assert_eq!(envelope["sourceCheckpoint"]["id"], first.id);
    assert_eq!(envelope["sourceCheckpoint"]["revision"], 1);
    assert_eq!(envelope["safety"]["modelToolMode"], "disabled");
    assert_eq!(run.status, crate::WorkspaceGuideRunStatus::Running);
    assert_eq!(
        (
            run.source_thread_id.as_deref(),
            run.source_turn_id.as_deref()
        ),
        (None, None)
    );
    assert_eq!(
        run.request_envelope_sha256,
        format!("{:x}", Sha256::digest(run.request_envelope_json.as_bytes()))
    );

    let replay = begin(&runtime, input.clone()).await;
    assert!(replay.replayed);
    assert_eq!(replay.id, run.id);
    let mut changed = input.clone();
    changed.request_json = r#"{"focus":"different"}"#.to_string();
    assert!(
        begin_error(&runtime, changed)
            .await
            .contains("different content")
    );
    let mut changed = input.clone();
    changed.model = "different-model".to_string();
    assert!(
        begin_error(&runtime, changed)
            .await
            .contains("different content")
    );
    assert!(
        begin_error(&runtime, start(&client, &first, "other-key", r#"{"x":1}"#))
            .await
            .contains("already has active run")
    );
    assert!(
        begin_error(
            &runtime,
            start(
                &client,
                &first,
                "path-key",
                r#"{"sourcePath":"/tmp/private"}"#
            ),
        )
        .await
        .contains("path-bearing key")
    );
    let mut oversized = input;
    oversized.request_json = serde_json::json!({"text": "x".repeat(MAX_REQUEST_BYTES)}).to_string();
    assert!(begin_error(&runtime, oversized).await.contains("exceeds"));
    let second = checkpoint(&runtime, &client, Some(first.session_id.clone()), "Second").await;
    let completion = finish(
        &run,
        crate::WorkspaceGuideRunOutcome::Completed {
            result_json: r#"{"schemaVersion":1,"summary":"Old"}"#.to_string(),
        },
    );
    let old_run = end(&runtime, completion.clone()).await;
    assert!(old_run.is_stale);
    assert_eq!(
        old_run.terminal_envelope_sha256,
        Some(format!(
            "{:x}",
            Sha256::digest(old_run.terminal_envelope_json.as_ref().unwrap().as_bytes())
        ))
    );
    assert_eq!(
        (
            old_run.source_thread_id.as_deref(),
            old_run.source_turn_id.as_deref()
        ),
        (Some("guide-thread"), Some("guide-turn"))
    );
    assert!(end(&runtime, completion.clone()).await.replayed);
    let mut changed = completion;
    changed.source_turn_id = Some("different-turn".to_string());
    assert!(
        runtime
            .workspace()
            .finish_guide_run(changed)
            .await
            .unwrap_err()
            .to_string()
            .contains("different terminal content")
    );
    assert!(
        begin_error(
            &runtime,
            start(&client, &first, "stale", r#"{"revision":1}"#),
        )
        .await
        .contains("no longer current")
    );

    for (key, outcome, without_source) in [
        (
            "failed",
            crate::WorkspaceGuideRunOutcome::Failed {
                error_summary: "synthetic provider failure".to_string(),
            },
            false,
        ),
        (
            "canceled",
            crate::WorkspaceGuideRunOutcome::Canceled {
                reason: "superseded by local input".to_string(),
            },
            true,
        ),
    ] {
        let run = begin(&runtime, start(&client, &second, key, r#"{"revision":2}"#)).await;
        if without_source {
            let mut invalid = finish(
                &run,
                crate::WorkspaceGuideRunOutcome::Completed {
                    result_json: r#"{"schemaVersion":1}"#.to_string(),
                },
            );
            invalid.source_thread_id = None;
            invalid.source_turn_id = None;
            assert!(runtime.workspace().finish_guide_run(invalid).await.is_err());
        }
        let mut terminal = finish(&run, outcome);
        if without_source {
            terminal.source_thread_id = None;
            terminal.source_turn_id = None;
        }
        end(&runtime, terminal).await;
    }

    let history = runtime
        .workspace()
        .list_guide_runs(crate::WorkspaceGuideRunFilter {
            client_id: client.id,
            session_id: Some(second.session_id),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(history.len(), 3);
    assert_eq!(
        history
            .iter()
            .map(|run| (run.status, run.is_stale))
            .collect::<Vec<_>>(),
        vec![
            (crate::WorkspaceGuideRunStatus::Canceled, false),
            (crate::WorkspaceGuideRunStatus::Failed, false),
            (crate::WorkspaceGuideRunStatus::Completed, true),
        ]
    );

    let canonical_writes: i64 = sqlx::query_scalar(
        "SELECT (SELECT COUNT(*) FROM workspace_notes) + (SELECT COUNT(*) FROM workspace_context_packets) + (SELECT COUNT(*) FROM workspace_agent_results) + (SELECT COUNT(*) FROM workspace_note_proposals)",
    )
    .fetch_one(runtime.workspace().pool.as_ref())
    .await
    .unwrap();
    assert_eq!(canonical_writes, 0);
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workspace_audit_events WHERE entity_type = 'guide_run'",
    )
    .fetch_one(runtime.workspace().pool.as_ref())
    .await
    .unwrap();
    assert_eq!(audit_count, 6);
}
