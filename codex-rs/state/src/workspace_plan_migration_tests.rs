use std::borrow::Cow;

use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePoolOptions;

use super::WORKSPACE_MIGRATOR;

fn migrator_through(version: i64) -> Migrator {
    Migrator {
        migrations: Cow::Owned(
            WORKSPACE_MIGRATOR
                .migrations
                .iter()
                .filter(|migration| migration.version <= version)
                .cloned()
                .collect(),
        ),
        ignore_missing: WORKSPACE_MIGRATOR.ignore_missing,
        locking: WORKSPACE_MIGRATOR.locking,
        table_name: WORKSPACE_MIGRATOR.table_name.clone(),
        create_schemas: WORKSPACE_MIGRATOR.create_schemas.clone(),
        no_tx: WORKSPACE_MIGRATOR.no_tx,
    }
}

async fn assert_sql_error(pool: &sqlx::SqlitePool, statement: &'static str, expected: &str) {
    let error = sqlx::query(statement)
        .execute(pool)
        .await
        .expect_err("statement should be rejected by an audit-integrity trigger");
    let message = error.to_string();
    assert!(
        message.contains(expected),
        "expected `{expected}` in SQLite error, got `{message}`"
    );
}

#[tokio::test]
async fn planning_migration_preserves_guide_rows_and_expands_tool_mode_check() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("memory database");
    migrator_through(22)
        .run(&pool)
        .await
        .expect("legacy workspace migrations");
    sqlx::query(
        "INSERT INTO workspace_clients (id, display_name, summary, created_at_ms, updated_at_ms) VALUES ('client', 'Synthetic', '', 1, 1)",
    )
    .execute(&pool)
    .await
    .expect("legacy client");
    sqlx::query(
        "INSERT INTO workspace_draft_sessions (id, client_id, status, current_revision, created_by, created_at_ms, updated_at_ms) VALUES ('draft-session', 'client', 'active', 1, 'human', 1, 1)",
    )
    .execute(&pool)
    .await
    .expect("legacy draft session");
    let hash = "a".repeat(64);
    sqlx::query(
        "INSERT INTO workspace_draft_checkpoints (id, session_id, client_id, schema_version, revision, draft_json, content_sha256, trigger, actor, created_at_ms) VALUES ('checkpoint', 'draft-session', 'client', 1, 1, '{}', ?, 'manual', 'human', 1)",
    )
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("legacy checkpoint");
    sqlx::query(
        "INSERT INTO workspace_guide_runs (id, client_id, session_id, source_checkpoint_id, source_checkpoint_revision, source_checkpoint_sha256, request_schema_version, request_envelope_json, request_envelope_sha256, idempotency_key, trigger, actor, provider, model, model_tool_mode, status, created_at_ms, updated_at_ms) VALUES ('guide', 'client', 'draft-session', 'checkpoint', 1, ?, 1, '{}', ?, 'key', 'manual', 'human', 'provider', 'model', 'disabled', 'running', 1, 1)",
    )
    .bind(&hash)
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("legacy guide row");
    sqlx::query(
        "INSERT INTO workspace_context_packets (id, client_id, human_request, status, created_at_ms, sent_at_ms, updated_at_ms) VALUES ('legacy-packet', 'client', 'Legacy request', 'prepared', 1, 1, 1)",
    )
    .execute(&pool)
    .await
    .expect("legacy packet row");
    sqlx::query(
        "INSERT INTO workspace_agent_runs (id, packet_id, client_id, context_envelope_sha256, run_kind, idempotency_key, status, started_at_ms, created_at_ms, updated_at_ms) VALUES ('legacy-agent-run', 'legacy-packet', 'client', '', 'manual_import', 'legacy-run', 'running', 1, 1, 1)",
    )
    .execute(&pool)
    .await
    .expect("legacy agent run row");

    WORKSPACE_MIGRATOR
        .run(&pool)
        .await
        .expect("planning migration");
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT model_tool_mode FROM workspace_guide_runs WHERE id = 'guide'",
        )
        .fetch_one(&pool)
        .await
        .expect("preserved guide"),
        "disabled"
    );
    assert!(
        sqlx::query(
            "UPDATE workspace_guide_runs SET model_tool_mode = 'workspace_planning_only' WHERE id = 'guide'",
        )
        .execute(&pool)
        .await
        .is_err(),
        "a persisted guide run cannot change execution mode"
    );
    sqlx::query(
        r#"
INSERT INTO workspace_guide_runs (
    id, client_id, session_id, source_checkpoint_id, source_checkpoint_revision,
    source_checkpoint_sha256, request_schema_version, request_envelope_json,
    request_envelope_sha256, idempotency_key, trigger, actor, provider, model,
    model_tool_mode, status, source_thread_id, source_turn_id,
    terminal_envelope_json, terminal_envelope_sha256, created_at_ms, updated_at_ms,
    terminal_at_ms
) VALUES (
    'planning-guide', 'client', 'draft-session', 'checkpoint', 1, ?, 1, '{}', ?,
    'planning-key', 'manual', 'human', 'provider', 'model',
    'workspace_planning_only', 'completed', 'planning-thread', 'planning-turn',
    '{}', ?, 2, 2, 2
)
        "#,
    )
    .bind(&hash)
    .bind(&hash)
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("planning-only mode should satisfy migrated check");
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT model_tool_mode FROM workspace_guide_runs WHERE id = 'planning-guide'",
        )
        .fetch_one(&pool)
        .await
        .expect("planning guide mode"),
        "workspace_planning_only"
    );
    assert!(
        sqlx::query(
            r#"
INSERT INTO workspace_guide_runs (
    id, client_id, session_id, source_checkpoint_id, source_checkpoint_revision,
    source_checkpoint_sha256, request_schema_version, request_envelope_json,
    request_envelope_sha256, idempotency_key, trigger, actor, provider, model,
    model_tool_mode, status, source_thread_id, source_turn_id,
    terminal_envelope_json, terminal_envelope_sha256, created_at_ms, updated_at_ms,
    terminal_at_ms
) VALUES (
    'invalid-guide', 'client', 'draft-session', 'checkpoint', 1, ?, 1, '{}', ?,
    'invalid-key', 'manual', 'human', 'provider', 'model', 'unrestricted',
    'completed', 'invalid-thread', 'invalid-turn', '{}', ?, 2, 2, 2
)
            "#,
        )
        .bind(&hash)
        .bind(&hash)
        .bind(&hash)
        .execute(&pool)
        .await
        .is_err(),
        "unrestricted mode must remain outside the storage allowlist"
    );
    let legacy_packet_binding = sqlx::query_as::<_, (
        Option<String>,
        Option<String>,
        Option<String>,
    )>(
        "SELECT workspace_plan_revision_id, workspace_plan_content_sha256, workspace_plan_evidence_manifest_sha256 FROM workspace_context_packets WHERE id = 'legacy-packet'",
    )
    .fetch_one(&pool)
    .await
    .expect("legacy packet binding columns");
    assert_eq!(legacy_packet_binding, (None, None, None));
    let legacy_run_binding = sqlx::query_as::<_, (
        Option<String>,
        Option<String>,
        Option<String>,
    )>(
        "SELECT workspace_plan_revision_id, workspace_plan_content_sha256, workspace_plan_evidence_manifest_sha256 FROM workspace_agent_runs WHERE id = 'legacy-agent-run'",
    )
    .fetch_one(&pool)
    .await
    .expect("legacy run binding columns");
    assert_eq!(legacy_run_binding, (None, None, None));

    sqlx::query(
        "INSERT INTO workspace_plan_sessions (id, client_id, status, latest_revision, created_by, created_at_ms, updated_at_ms) VALUES ('plan-session', 'client', 'active', 1, 'human', 2, 2)",
    )
    .execute(&pool)
    .await
    .expect("plan session");
    sqlx::query(
        r#"
INSERT INTO workspace_plan_revisions (
    id, plan_session_id, client_id, guide_run_id, revision, plan_markdown,
    decisions_json, open_questions_json, content_sha256, evidence_manifest_json,
    evidence_manifest_sha256, evidence_read_count, idempotency_key, status,
    source_checkpoint_id, source_checkpoint_revision, source_checkpoint_sha256,
    source_thread_id, source_turn_id, created_at_ms
) VALUES (
    'plan-revision', 'plan-session', 'client', 'guide', 1, '# Plan',
    '[]', '[]', ?, '[{}]', ?, 1, 'plan-key', 'current',
    'checkpoint', 1, ?, 'thread', 'turn', 2
)
        "#,
    )
    .bind(&hash)
    .bind(&hash)
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("plan revision");
    sqlx::query(
        r#"
INSERT INTO workspace_context_packets (
    id, client_id, human_request, status, source_checkpoint_id,
    source_checkpoint_sha256, workspace_plan_revision_id,
    workspace_plan_content_sha256, workspace_plan_evidence_manifest_sha256,
    created_at_ms, sent_at_ms, updated_at_ms
) VALUES (
    'bound-packet', 'client', 'Bound request', 'prepared', 'checkpoint', ?,
    'plan-revision', ?, ?, 2, 2, 2
)
        "#,
    )
    .bind(&hash)
    .bind(&hash)
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("complete packet binding");
    assert!(
        sqlx::query(
            "INSERT INTO workspace_context_packets (id, client_id, human_request, status, workspace_plan_revision_id, created_at_ms, sent_at_ms, updated_at_ms) VALUES ('partial-packet', 'client', 'Partial', 'prepared', 'plan-revision', 3, 3, 3)",
        )
        .execute(&pool)
        .await
        .is_err()
    );
    assert!(
        sqlx::query(
            "UPDATE workspace_context_packets SET workspace_plan_content_sha256 = ? WHERE id = 'bound-packet'",
        )
        .bind("b".repeat(64))
        .execute(&pool)
        .await
        .is_err()
    );
    assert!(
        sqlx::query(
            "INSERT INTO workspace_agent_runs (id, packet_id, client_id, context_envelope_sha256, run_kind, idempotency_key, status, started_at_ms, created_at_ms, updated_at_ms) VALUES ('mismatched-run', 'bound-packet', 'client', '', 'manual_import', 'mismatch', 'running', 3, 3, 3)",
        )
        .execute(&pool)
        .await
        .is_err()
    );
    sqlx::query(
        r#"
INSERT INTO workspace_agent_runs (
    id, packet_id, client_id, context_envelope_sha256,
    workspace_plan_revision_id, workspace_plan_content_sha256,
    workspace_plan_evidence_manifest_sha256, run_kind, idempotency_key,
    status, started_at_ms, created_at_ms, updated_at_ms
) VALUES (
    'bound-run', 'bound-packet', 'client', '', 'plan-revision', ?, ?,
    'agent', 'bound', 'running', 3, 3, 3
)
        "#,
    )
    .bind(&hash)
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("run inherits complete packet binding");

    sqlx::query(
        "UPDATE workspace_plan_revisions SET status = 'submitted', submitted_at_ms = 4 WHERE id = 'plan-revision'",
    )
    .execute(&pool)
    .await
    .expect("plan revision submission transition");
    sqlx::query(
        "UPDATE workspace_context_packets SET status = 'submitted' WHERE id = 'bound-packet'",
    )
    .execute(&pool)
    .await
    .expect("bound packet submission transition");
    assert!(
        sqlx::query(
            r#"
INSERT INTO workspace_plan_submission_receipts (
    plan_revision_id, packet_id, agent_run_id, plan_session_id, client_id,
    plan_content_sha256, evidence_manifest_sha256, submitted_by, submitted_at_ms
) VALUES (
    'plan-revision', 'bound-packet', 'bound-run', 'plan-session', 'client',
    ?, ?, 'human', 4
)
            "#,
        )
        .bind("b".repeat(64))
        .bind(&hash)
        .execute(&pool)
        .await
        .is_err()
    );
    sqlx::query(
        r#"
INSERT INTO workspace_plan_submission_receipts (
    plan_revision_id, packet_id, agent_run_id, plan_session_id, client_id,
    plan_content_sha256, evidence_manifest_sha256, submitted_by, submitted_at_ms
) VALUES (
    'plan-revision', 'bound-packet', 'bound-run', 'plan-session', 'client',
    ?, ?, 'human', 4
)
        "#,
    )
    .bind(&hash)
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("exact plan submission receipt");
    assert!(
        sqlx::query(
            "UPDATE workspace_plan_submission_receipts SET packet_id = 'legacy-packet' WHERE plan_revision_id = 'plan-revision'",
        )
        .execute(&pool)
        .await
        .is_err()
    );
    assert!(
        sqlx::query(
            "DELETE FROM workspace_plan_submission_receipts WHERE plan_revision_id = 'plan-revision'",
        )
        .execute(&pool)
        .await
        .is_err()
    );
}

#[tokio::test]
async fn plan_audit_rows_are_append_only_and_lifecycles_are_forward_only() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("memory database");
    WORKSPACE_MIGRATOR
        .run(&pool)
        .await
        .expect("workspace migrations");

    sqlx::query(
        "UPDATE workspace_data_policy SET data_classification = 'synthetic', classified_at_ms = 1, classified_by = 'audit-test' WHERE singleton_id = 1",
    )
    .execute(&pool)
    .await
    .expect("synthetic policy");
    sqlx::query(
        "INSERT INTO workspace_clients (id, display_name, summary, created_at_ms, updated_at_ms) VALUES ('client', 'Synthetic', '', 1, 1)",
    )
    .execute(&pool)
    .await
    .expect("client");
    sqlx::query(
        "INSERT INTO workspace_draft_sessions (id, client_id, status, current_revision, created_by, created_at_ms, updated_at_ms) VALUES ('draft-session', 'client', 'active', 1, 'human', 1, 1)",
    )
    .execute(&pool)
    .await
    .expect("draft session");
    let hash = "a".repeat(64);
    sqlx::query(
        "INSERT INTO workspace_draft_checkpoints (id, session_id, client_id, schema_version, revision, draft_json, content_sha256, trigger, actor, created_at_ms) VALUES ('checkpoint', 'draft-session', 'client', 1, 1, '{}', ?, 'manual', 'human', 1)",
    )
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("checkpoint");
    sqlx::query(
        r#"
INSERT INTO workspace_guide_runs (
    id, client_id, session_id, source_checkpoint_id, source_checkpoint_revision,
    source_checkpoint_sha256, request_schema_version, request_envelope_json,
    request_envelope_sha256, idempotency_key, trigger, actor, provider, model,
    model_tool_mode, status, created_at_ms, updated_at_ms
) VALUES (
    'guide', 'client', 'draft-session', 'checkpoint', 1, ?, 1, '{}', ?,
    'guide-key', 'manual', 'human', 'provider', 'model',
    'workspace_planning_only', 'running', 1, 1
)
        "#,
    )
    .bind(&hash)
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("guide run");
    sqlx::query(
        "INSERT INTO workspace_plan_sessions (id, client_id, status, latest_revision, created_by, created_at_ms, updated_at_ms) VALUES ('plan-session', 'client', 'active', 0, 'human', 1, 1)",
    )
    .execute(&pool)
    .await
    .expect("plan session");

    sqlx::query(
        "UPDATE workspace_plan_sessions SET source_thread_id = 'thread', updated_at_ms = 2 WHERE id = 'plan-session'",
    )
    .execute(&pool)
    .await
    .expect("one-time session thread binding");
    sqlx::query(
        "UPDATE workspace_guide_runs SET source_thread_id = 'thread', source_turn_id = 'turn', updated_at_ms = 2 WHERE id = 'guide'",
    )
    .execute(&pool)
    .await
    .expect("one-time run source binding");
    sqlx::query(
        "UPDATE workspace_plan_sessions SET latest_revision = 1, updated_at_ms = 3 WHERE id = 'plan-session'",
    )
    .execute(&pool)
    .await
    .expect("first session revision");
    sqlx::query(
        r#"
INSERT INTO workspace_plan_revisions (
    id, plan_session_id, client_id, guide_run_id, revision, plan_markdown,
    decisions_json, open_questions_json, content_sha256, evidence_manifest_json,
    evidence_manifest_sha256, evidence_read_count, idempotency_key, status,
    source_checkpoint_id, source_checkpoint_revision, source_checkpoint_sha256,
    source_thread_id, source_turn_id, created_at_ms
) VALUES (
    'revision-1', 'plan-session', 'client', 'guide', 1, '# Plan 1',
    '[]', '[]', ?, '[{}]', ?, 1, 'revision-key-1', 'current',
    'checkpoint', 1, ?, 'thread', 'turn', 3
)
        "#,
    )
    .bind(&hash)
    .bind(&hash)
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("first revision");
    sqlx::query("UPDATE workspace_plan_revisions SET status = 'outdated' WHERE id = 'revision-1'")
        .execute(&pool)
        .await
        .expect("current revision can become outdated");
    sqlx::query(
        "UPDATE workspace_plan_sessions SET latest_revision = 2, updated_at_ms = 4 WHERE id = 'plan-session'",
    )
    .execute(&pool)
    .await
    .expect("second session revision");
    sqlx::query(
        r#"
INSERT INTO workspace_plan_revisions (
    id, plan_session_id, client_id, guide_run_id, revision, plan_markdown,
    decisions_json, open_questions_json, content_sha256, evidence_manifest_json,
    evidence_manifest_sha256, evidence_read_count, idempotency_key, status,
    source_checkpoint_id, source_checkpoint_revision, source_checkpoint_sha256,
    source_thread_id, source_turn_id, created_at_ms
) VALUES (
    'revision-2', 'plan-session', 'client', 'guide', 2, '# Plan 2',
    '[]', '[]', ?, '[{}]', ?, 1, 'revision-key-2', 'current',
    'checkpoint', 1, ?, 'thread', 'turn', 4
)
        "#,
    )
    .bind(&hash)
    .bind(&hash)
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("second revision");
    sqlx::query(
        "UPDATE workspace_plan_revisions SET status = 'submitted', submitted_at_ms = 5 WHERE id = 'revision-2'",
    )
    .execute(&pool)
    .await
    .expect("current revision can become submitted");

    sqlx::query(
        r#"
INSERT INTO workspace_plan_messages (
    id, plan_session_id, client_id, guide_run_id, sequence, role, content,
    content_sha256, idempotency_key, source_checkpoint_id,
    source_checkpoint_revision, source_checkpoint_sha256, source_thread_id,
    source_turn_id, created_at_ms
) VALUES (
    'message', 'plan-session', 'client', 'guide', 1, 'assistant',
    'Assistant message', ?, 'message-key', 'checkpoint', 1, ?, 'thread', 'turn', 4
)
        "#,
    )
    .bind(&hash)
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("plan message");
    sqlx::query(
        r#"
INSERT INTO workspace_planning_turn_claims (
    guide_run_id, plan_session_id, client_id, source_checkpoint_id,
    source_checkpoint_revision, source_checkpoint_sha256, source_thread_id,
    source_turn_id, provider, model, prompt_sha256, execution_token_sha256,
    claimed_at_ms
) VALUES (
    'guide', 'plan-session', 'client', 'checkpoint', 1, ?, 'thread', 'turn',
    'provider', 'model', ?, ?, 4
)
        "#,
    )
    .bind(&hash)
    .bind(&hash)
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("planning claim");
    sqlx::query(
        r#"
INSERT INTO workspace_planning_context_reads (
    id, guide_run_id, plan_session_id, client_id, idempotency_key, category,
    max_records, result_count, response_json, response_sha256,
    source_checkpoint_id, source_checkpoint_revision, source_checkpoint_sha256,
    source_thread_id, source_turn_id, prompt_sha256, execution_token_sha256,
    accessed_at_ms
) VALUES (
    'read', 'guide', 'plan-session', 'client', 'read-key', 'patient_chart',
    1, 0, '[]', ?, 'checkpoint', 1, ?, 'thread', 'turn', ?, ?, 4
)
        "#,
    )
    .bind(&hash)
    .bind(&hash)
    .bind(&hash)
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("context read");
    sqlx::query(
        "INSERT INTO workspace_plan_turn_evidence (guide_run_id, ordinal, context_read_id, category, response_sha256, source_content_sha256_json) VALUES ('guide', 0, 'read', 'patient_chart', ?, '[]')",
    )
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("turn evidence");
    sqlx::query(
        "UPDATE workspace_guide_runs SET status = 'completed', terminal_envelope_json = '{}', terminal_envelope_sha256 = ?, terminal_at_ms = 5, updated_at_ms = 5 WHERE id = 'guide'",
    )
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("running guide can complete");
    sqlx::query(
        r#"
INSERT INTO workspace_plan_turn_completions (
    guide_run_id, plan_session_id, client_id, idempotency_key,
    assistant_message_id, plan_revision_id, completion_input_sha256,
    evidence_manifest_json, evidence_manifest_sha256, evidence_read_count,
    terminal_envelope_json, terminal_envelope_sha256, source_checkpoint_id,
    source_checkpoint_revision, source_checkpoint_sha256, source_thread_id,
    source_turn_id, provider, model, prompt_sha256, execution_token_sha256,
    completed_at_ms
) VALUES (
    'guide', 'plan-session', 'client', 'completion-key', 'message', 'revision-2',
    ?, '[{}]', ?, 1, '{}', ?, 'checkpoint', 1, ?, 'thread', 'turn',
    'provider', 'model', ?, ?, 5
)
        "#,
    )
    .bind(&hash)
    .bind(&hash)
    .bind(&hash)
    .bind(&hash)
    .bind(&hash)
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("turn completion");
    for (id, key) in [
        ("proposal-accepted", "proposal-key-1"),
        ("proposal-outdated", "proposal-key-2"),
    ] {
        sqlx::query(
            r#"
INSERT INTO workspace_plan_proposals (
    id, plan_session_id, plan_revision_id, client_id, guide_run_id,
    proposal_kind, payload_json, payload_sha256, summary, rationale,
    idempotency_key, status, source_checkpoint_id, source_checkpoint_revision,
    source_checkpoint_sha256, source_thread_id, source_turn_id, created_at_ms
) VALUES (
    ?, 'plan-session', 'revision-2', 'client', 'guide', 'task_draft',
    '{"kind":"task_draft"}', ?, 'Summary', 'Rationale', ?, 'pending',
    'checkpoint', 1, ?, 'thread', 'turn', 5
)
            "#,
        )
        .bind(id)
        .bind(&hash)
        .bind(key)
        .bind(&hash)
        .execute(&pool)
        .await
        .expect("plan proposal");
    }

    sqlx::query(
        "UPDATE workspace_plan_proposals SET status = 'accepted', resolved_at_ms = 6, resolved_by = 'clinician' WHERE id = 'proposal-accepted'",
    )
    .execute(&pool)
    .await
    .expect("pending proposal can be accepted");
    sqlx::query(
        "UPDATE workspace_plan_proposals SET status = 'outdated' WHERE id = 'proposal-outdated'",
    )
    .execute(&pool)
    .await
    .expect("pending proposal can become outdated");

    assert_sql_error(
        &pool,
        "UPDATE workspace_plan_messages SET content = 'rewritten' WHERE id = 'message'",
        "workspace plan message is immutable",
    )
    .await;
    assert_sql_error(
        &pool,
        "DELETE FROM workspace_plan_messages WHERE id = 'message'",
        "workspace plan message cannot be deleted",
    )
    .await;
    assert_sql_error(
        &pool,
        "UPDATE workspace_planning_turn_claims SET model = 'other' WHERE guide_run_id = 'guide'",
        "workspace planning turn claim is immutable",
    )
    .await;
    assert_sql_error(
        &pool,
        "DELETE FROM workspace_planning_turn_claims WHERE guide_run_id = 'guide'",
        "workspace planning turn claim cannot be deleted",
    )
    .await;
    assert_sql_error(
        &pool,
        "UPDATE workspace_planning_context_reads SET result_count = 1 WHERE id = 'read'",
        "workspace planning context read is immutable",
    )
    .await;
    assert_sql_error(
        &pool,
        "DELETE FROM workspace_planning_context_reads WHERE id = 'read'",
        "workspace planning context read cannot be deleted",
    )
    .await;
    assert_sql_error(
        &pool,
        "UPDATE workspace_plan_turn_evidence SET category = 'visit_history' WHERE guide_run_id = 'guide' AND ordinal = 0",
        "workspace plan turn evidence is immutable",
    )
    .await;
    assert_sql_error(
        &pool,
        "DELETE FROM workspace_plan_turn_evidence WHERE guide_run_id = 'guide' AND ordinal = 0",
        "workspace plan turn evidence cannot be deleted",
    )
    .await;
    assert_sql_error(
        &pool,
        "UPDATE workspace_plan_turn_completions SET model = 'other' WHERE guide_run_id = 'guide'",
        "workspace plan turn completion is immutable",
    )
    .await;
    assert_sql_error(
        &pool,
        "DELETE FROM workspace_plan_turn_completions WHERE guide_run_id = 'guide'",
        "workspace plan turn completion cannot be deleted",
    )
    .await;
    assert_sql_error(
        &pool,
        "UPDATE workspace_plan_revisions SET plan_markdown = '# Rewritten' WHERE id = 'revision-1'",
        "workspace plan revision content is immutable",
    )
    .await;
    assert_sql_error(
        &pool,
        "UPDATE workspace_plan_revisions SET status = 'current', submitted_at_ms = NULL WHERE id = 'revision-2'",
        "workspace plan revision lifecycle is forward-only",
    )
    .await;
    assert_sql_error(
        &pool,
        "DELETE FROM workspace_plan_revisions WHERE id = 'revision-1'",
        "workspace plan revision cannot be deleted",
    )
    .await;
    assert_sql_error(
        &pool,
        "UPDATE workspace_plan_proposals SET payload_json = '{}' WHERE id = 'proposal-accepted'",
        "workspace plan proposal content is immutable",
    )
    .await;
    assert_sql_error(
        &pool,
        "UPDATE workspace_plan_proposals SET status = 'pending', resolved_at_ms = NULL, resolved_by = NULL WHERE id = 'proposal-accepted'",
        "workspace plan proposal lifecycle is forward-only",
    )
    .await;
    assert_sql_error(
        &pool,
        "DELETE FROM workspace_plan_proposals WHERE id = 'proposal-accepted'",
        "workspace plan proposal cannot be deleted",
    )
    .await;
    assert_sql_error(
        &pool,
        "UPDATE workspace_guide_runs SET provider = 'other' WHERE id = 'guide'",
        "workspace guide run identity is immutable",
    )
    .await;
    assert_sql_error(
        &pool,
        "UPDATE workspace_guide_runs SET source_thread_id = 'other-thread', source_turn_id = 'other-turn' WHERE id = 'guide'",
        "workspace guide run source binding is permanent",
    )
    .await;
    assert_sql_error(
        &pool,
        "UPDATE workspace_guide_runs SET status = 'running', terminal_envelope_json = NULL, terminal_envelope_sha256 = NULL, terminal_at_ms = NULL WHERE id = 'guide'",
        "workspace guide run lifecycle is forward-only",
    )
    .await;
    assert_sql_error(
        &pool,
        "DELETE FROM workspace_guide_runs WHERE id = 'guide'",
        "workspace guide run cannot be deleted",
    )
    .await;
    assert_sql_error(
        &pool,
        "UPDATE workspace_plan_sessions SET source_thread_id = 'other-thread' WHERE id = 'plan-session'",
        "workspace plan session thread binding is permanent",
    )
    .await;
    assert_sql_error(
        &pool,
        "UPDATE workspace_plan_sessions SET latest_revision = 4 WHERE id = 'plan-session'",
        "workspace plan session revision must advance by one",
    )
    .await;
    sqlx::query(
        "UPDATE workspace_plan_sessions SET status = 'closed', closed_at_ms = 7, updated_at_ms = 7 WHERE id = 'plan-session'",
    )
    .execute(&pool)
    .await
    .expect("active plan session can close");
    assert_sql_error(
        &pool,
        "UPDATE workspace_plan_sessions SET status = 'active', closed_at_ms = NULL WHERE id = 'plan-session'",
        "workspace plan session lifecycle is forward-only",
    )
    .await;
    assert_sql_error(
        &pool,
        "DELETE FROM workspace_plan_sessions WHERE id = 'plan-session'",
        "workspace plan session cannot be deleted",
    )
    .await;

    assert_eq!(
        sqlx::query_as::<_, (String, String, String)>(
            "SELECT (SELECT status FROM workspace_plan_sessions WHERE id = 'plan-session'), (SELECT status FROM workspace_plan_revisions WHERE id = 'revision-2'), (SELECT status FROM workspace_plan_proposals WHERE id = 'proposal-accepted')",
        )
        .fetch_one(&pool)
        .await
        .expect("terminal audit states"),
        (
            "closed".to_string(),
            "submitted".to_string(),
            "accepted".to_string(),
        )
    );
}
