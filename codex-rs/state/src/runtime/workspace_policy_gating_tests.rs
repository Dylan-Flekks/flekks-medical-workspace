use crate::StateRuntime;
use crate::runtime::test_support::unique_temp_dir;
use pretty_assertions::assert_eq;
use sqlx::QueryBuilder;
use sqlx::Sqlite;

async fn runtime() -> std::sync::Arc<StateRuntime> {
    StateRuntime::init(unique_temp_dir(), "test-provider".to_string())
        .await
        .expect("state db should initialize")
}

#[tokio::test]
async fn unclassified_agent_start_leaves_prepared_packet_and_audits_unchanged() {
    let runtime = runtime().await;
    let client = runtime
        .workspace()
        .upsert_client(crate::WorkspaceClientUpsert {
            display_name: "Synthetic Policy Patient".to_string(),
            ..Default::default()
        })
        .await
        .expect("client should save");
    let note = runtime
        .workspace()
        .upsert_note(crate::WorkspaceNoteUpsert {
            client_id: client.id.clone(),
            title: "Daily note".to_string(),
            kind: "daily".to_string(),
            body: "Synthetic draft".to_string(),
            status: "draft".to_string(),
            actor: "Clinician Example".to_string(),
            ..Default::default()
        })
        .await
        .expect("note should save");
    let packet = runtime
        .workspace()
        .prepare_context_packet(crate::WorkspaceContextPacketCreate {
            client_id: client.id.clone(),
            note_id: Some(note.id.clone()),
            human_request: "Generate a synthetic template".to_string(),
            selected_artifact_ids_json: "[]".to_string(),
            selected_derivative_ids_json: "[]".to_string(),
            selected_clip_ids_json: "[]".to_string(),
            artifact_summary: "0 selected files".to_string(),
            derivative_summary: "0 reviewed text items".to_string(),
            clip_summary: "0 selected clips".to_string(),
            chart_context_summary: "synthetic daily note".to_string(),
            context_envelope_json: serde_json::json!({
                "assemblyVersion": "synthetic-policy-test-v1",
                "sourceMode": "agent_request",
                "includeDocuments": false,
                "humanRequest": "Generate a synthetic template",
                "ids": {
                    "selectedArtifactIds": [],
                    "selectedDerivativeIds": [],
                    "selectedClipIds": [],
                },
                "safety": [
                    "read-only context packet; do not mutate workspace records",
                    "do not sign notes, submit claims, send payer communications, or overwrite saved data",
                ],
                "promptSnapshot": "Synthetic policy packet without filesystem paths.",
            })
            .to_string(),
            base_note_revision: Some(note.current_revision),
            authorized_scope_json: r#"{"version":1,"categories":["active_note"]}"#
                .to_string(),
            expected_output_kind: "template_proposal".to_string(),
            actor: "Clinician Example".to_string(),
            ..Default::default()
        })
        .await
        .expect("packet should prepare");
    let audits_before = count_rows(&runtime, "workspace_audit_events").await;

    let error = runtime
        .workspace()
        .start_agent_run(crate::WorkspaceAgentRunStart {
            packet_id: packet.id.clone(),
            expected_client_id: client.id.clone(),
            expected_context_envelope_sha256: packet.context_envelope_sha256.clone(),
            run_kind: "agent".to_string(),
            idempotency_key: "policy-rejection".to_string(),
            provider: "test-provider".to_string(),
            model: "test-model".to_string(),
            actor: "Clinician Example".to_string(),
            ..Default::default()
        })
        .await
        .expect_err("unclassified agent start must fail");
    assert!(error.to_string().contains("explicit synthetic"));
    assert_eq!(count_rows(&runtime, "workspace_agent_runs").await, 0);
    assert_eq!(
        count_rows(&runtime, "workspace_audit_events").await,
        audits_before
    );
    let stored = runtime
        .workspace()
        .list_context_packets(crate::WorkspaceContextPacketFilter {
            client_id: client.id,
            note_id: Some(note.id),
            limit: Some(10),
        })
        .await
        .expect("packet should list");
    assert_eq!(stored, vec![packet]);
}

#[tokio::test]
async fn policy_gate_precedes_guide_replay_and_preserves_run_and_audit_rows() {
    let runtime = runtime().await;
    runtime
        .workspace()
        .provision_synthetic_workspace("guide replay fixture")
        .await
        .expect("fixture should provision");
    let client = runtime
        .workspace()
        .upsert_client(crate::WorkspaceClientUpsert {
            display_name: "Synthetic Guide Patient".to_string(),
            ..Default::default()
        })
        .await
        .expect("client should save");
    let checkpoint = runtime
        .workspace()
        .create_draft_checkpoint(crate::WorkspaceDraftCheckpointCreate {
            client_id: client.id.clone(),
            draft_json: r#"{"schemaVersion":1,"note":{"title":"Daily"}}"#.to_string(),
            trigger: "focusChange".to_string(),
            actor: "Clinician Example".to_string(),
            ..Default::default()
        })
        .await
        .expect("checkpoint should save");
    let input = crate::WorkspaceGuideRunStart {
        client_id: client.id,
        session_id: checkpoint.session_id.clone(),
        source_checkpoint_id: checkpoint.id.clone(),
        source_checkpoint_revision: checkpoint.revision,
        source_checkpoint_sha256: checkpoint.content_sha256,
        request_json: r#"{"focus":"noteBody"}"#.to_string(),
        idempotency_key: "guide-policy-replay".to_string(),
        trigger: "focusChange".to_string(),
        actor: "Clinician Example".to_string(),
        provider: "test-provider".to_string(),
        model: "test-model".to_string(),
    };
    runtime
        .workspace()
        .start_guide_run(input.clone())
        .await
        .expect("classified guide run should start");
    let runs_before = count_rows(&runtime, "workspace_guide_runs").await;
    let audits_before = count_rows(&runtime, "workspace_audit_events").await;
    let mut corrupt_connection = runtime
        .workspace()
        .pool
        .acquire()
        .await
        .expect("corrupt policy fixture connection");
    sqlx::query("DROP TRIGGER workspace_data_policy_restrict_update")
        .execute(&mut *corrupt_connection)
        .await
        .expect("drop update guard fixture");
    sqlx::query("PRAGMA ignore_check_constraints = ON")
        .execute(&mut *corrupt_connection)
        .await
        .expect("corrupt policy fixture mode");
    sqlx::query("UPDATE workspace_data_policy SET data_classification = 'unclassified', classified_at_ms = NULL, classified_by = NULL")
        .execute(&mut *corrupt_connection)
        .await
        .expect("downgrade fixture should apply after dropping guard");

    let error = runtime
        .workspace()
        .start_guide_run(input)
        .await
        .expect_err("unclassified guide replay must fail");
    assert_eq!(error.kind(), "validation");
    assert!(error.to_string().contains("explicit synthetic"));
    assert_eq!(
        count_rows(&runtime, "workspace_guide_runs").await,
        runs_before
    );
    assert_eq!(
        count_rows(&runtime, "workspace_audit_events").await,
        audits_before
    );
}

async fn count_rows(runtime: &StateRuntime, table: &str) -> i64 {
    let mut query = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM ");
    query.push(table);
    query
        .build_query_scalar()
        .fetch_one(runtime.workspace().pool.as_ref())
        .await
        .unwrap_or_else(|error| panic!("count {table}: {error}"))
}
