-- SQLite validates trigger bodies while a referenced table is rebuilt. Recreate the policy
-- transition trigger after all planning tables exist so classification remains fail-closed.
DROP TRIGGER workspace_data_policy_restrict_update;

CREATE TABLE workspace_guide_runs_next (
    id TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    source_checkpoint_id TEXT NOT NULL,
    source_checkpoint_revision INTEGER NOT NULL,
    source_checkpoint_sha256 TEXT NOT NULL,
    request_schema_version INTEGER NOT NULL,
    request_envelope_json TEXT NOT NULL,
    request_envelope_sha256 TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    trigger TEXT NOT NULL,
    actor TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    model_tool_mode TEXT NOT NULL DEFAULT 'disabled',
    status TEXT NOT NULL DEFAULT 'running',
    source_thread_id TEXT,
    source_turn_id TEXT,
    terminal_envelope_json TEXT,
    terminal_envelope_sha256 TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    terminal_at_ms INTEGER,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE,
    FOREIGN KEY(session_id) REFERENCES workspace_draft_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY(source_checkpoint_id) REFERENCES workspace_draft_checkpoints(id) ON DELETE CASCADE,
    UNIQUE(session_id, idempotency_key),
    CHECK(source_checkpoint_revision > 0),
    CHECK(length(source_checkpoint_sha256) = 64
        AND source_checkpoint_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(request_schema_version = 1),
    CHECK(length(trim(request_envelope_json)) > 0 AND json_valid(request_envelope_json)),
    CHECK(length(request_envelope_sha256) = 64
        AND request_envelope_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(trim(idempotency_key)) > 0),
    CHECK(length(trim(trigger)) > 0),
    CHECK(length(trim(actor)) > 0),
    CHECK(length(trim(provider)) > 0),
    CHECK(length(trim(model)) > 0),
    CHECK(terminal_envelope_json IS NULL OR json_valid(terminal_envelope_json)),
    CHECK(terminal_envelope_sha256 IS NULL OR (length(terminal_envelope_sha256) = 64
        AND terminal_envelope_sha256 NOT GLOB '*[^0-9a-f]*')),
    CHECK(model_tool_mode IN ('disabled', 'workspace_planning_only')),
    CHECK(status IN ('running', 'completed', 'failed', 'canceled')),
    CHECK(
        (source_thread_id IS NULL AND source_turn_id IS NULL)
        OR (source_thread_id IS NOT NULL AND source_turn_id IS NOT NULL)
    ),
    CHECK(
        (status = 'running' AND terminal_envelope_json IS NULL AND terminal_envelope_sha256 IS NULL
            AND terminal_at_ms IS NULL)
        OR (status != 'running' AND terminal_envelope_json IS NOT NULL
            AND terminal_envelope_sha256 IS NOT NULL AND terminal_at_ms IS NOT NULL)
    ),
    CHECK(status != 'completed' OR source_thread_id IS NOT NULL)
);

INSERT INTO workspace_guide_runs_next (
    id, client_id, session_id, source_checkpoint_id, source_checkpoint_revision,
    source_checkpoint_sha256, request_schema_version, request_envelope_json,
    request_envelope_sha256, idempotency_key, trigger, actor, provider, model,
    model_tool_mode, status, source_thread_id, source_turn_id,
    terminal_envelope_json, terminal_envelope_sha256, created_at_ms, updated_at_ms,
    terminal_at_ms
)
SELECT
    id, client_id, session_id, source_checkpoint_id, source_checkpoint_revision,
    source_checkpoint_sha256, request_schema_version, request_envelope_json,
    request_envelope_sha256, idempotency_key, trigger, actor, provider, model,
    model_tool_mode, status, source_thread_id, source_turn_id,
    terminal_envelope_json, terminal_envelope_sha256, created_at_ms, updated_at_ms,
    terminal_at_ms
FROM workspace_guide_runs;

DROP TABLE workspace_guide_runs;
ALTER TABLE workspace_guide_runs_next RENAME TO workspace_guide_runs;

CREATE UNIQUE INDEX idx_workspace_guide_runs_one_active_session
ON workspace_guide_runs(session_id)
WHERE status = 'running';

CREATE INDEX idx_workspace_guide_runs_client_session_created
ON workspace_guide_runs(client_id, session_id, created_at_ms DESC, id DESC);

CREATE INDEX idx_workspace_guide_runs_checkpoint_created
ON workspace_guide_runs(source_checkpoint_id, created_at_ms DESC);

CREATE TABLE workspace_plan_sessions (
    id TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    source_thread_id TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    latest_revision INTEGER NOT NULL DEFAULT 0,
    created_by TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    closed_at_ms INTEGER,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE,
    CHECK(length(trim(created_by)) > 0),
    CHECK(status IN ('active', 'closed')),
    CHECK(latest_revision >= 0),
    CHECK(
        (status = 'active' AND closed_at_ms IS NULL)
        OR (status = 'closed' AND closed_at_ms IS NOT NULL)
    )
);

CREATE UNIQUE INDEX idx_workspace_plan_sessions_one_active_client
ON workspace_plan_sessions(client_id)
WHERE status = 'active';

CREATE UNIQUE INDEX idx_workspace_plan_sessions_source_thread
ON workspace_plan_sessions(source_thread_id)
WHERE source_thread_id IS NOT NULL;

CREATE INDEX idx_workspace_plan_sessions_client_updated
ON workspace_plan_sessions(client_id, updated_at_ms DESC, id DESC);

CREATE TABLE workspace_plan_messages (
    id TEXT PRIMARY KEY,
    plan_session_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
    guide_run_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    content_sha256 TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    source_checkpoint_id TEXT NOT NULL,
    source_checkpoint_revision INTEGER NOT NULL,
    source_checkpoint_sha256 TEXT NOT NULL,
    encounter_id TEXT,
    note_id TEXT,
    source_thread_id TEXT,
    source_turn_id TEXT,
    created_at_ms INTEGER NOT NULL,
    FOREIGN KEY(plan_session_id) REFERENCES workspace_plan_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE,
    FOREIGN KEY(guide_run_id) REFERENCES workspace_guide_runs(id) ON DELETE CASCADE,
    FOREIGN KEY(source_checkpoint_id) REFERENCES workspace_draft_checkpoints(id) ON DELETE CASCADE,
    UNIQUE(plan_session_id, sequence),
    UNIQUE(plan_session_id, idempotency_key),
    CHECK(sequence > 0),
    CHECK(role IN ('human', 'assistant', 'question', 'answer', 'error', 'system_status')),
    CHECK(length(content) > 0),
    CHECK(length(content_sha256) = 64 AND content_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(trim(idempotency_key)) > 0),
    CHECK(source_checkpoint_revision > 0),
    CHECK(length(source_checkpoint_sha256) = 64
        AND source_checkpoint_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(
        (source_thread_id IS NULL AND source_turn_id IS NULL)
        OR (source_thread_id IS NOT NULL AND source_turn_id IS NOT NULL)
    )
);

CREATE INDEX idx_workspace_plan_messages_session_sequence
ON workspace_plan_messages(plan_session_id, sequence);

CREATE INDEX idx_workspace_plan_messages_guide_run
ON workspace_plan_messages(guide_run_id, sequence);

CREATE TABLE workspace_planning_turn_claims (
    guide_run_id TEXT PRIMARY KEY,
    plan_session_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
    source_checkpoint_id TEXT NOT NULL,
    source_checkpoint_revision INTEGER NOT NULL,
    source_checkpoint_sha256 TEXT NOT NULL,
    source_thread_id TEXT NOT NULL,
    source_turn_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt_sha256 TEXT NOT NULL,
    execution_token_sha256 TEXT NOT NULL,
    claimed_at_ms INTEGER NOT NULL,
    FOREIGN KEY(guide_run_id) REFERENCES workspace_guide_runs(id) ON DELETE CASCADE,
    FOREIGN KEY(plan_session_id) REFERENCES workspace_plan_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE,
    FOREIGN KEY(source_checkpoint_id) REFERENCES workspace_draft_checkpoints(id) ON DELETE CASCADE,
    CHECK(source_checkpoint_revision > 0),
    CHECK(length(source_checkpoint_sha256) = 64
        AND source_checkpoint_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(trim(source_thread_id)) > 0),
    CHECK(length(trim(source_turn_id)) > 0),
    CHECK(length(trim(provider)) > 0),
    CHECK(length(trim(model)) > 0),
    CHECK(length(prompt_sha256) = 64 AND prompt_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(execution_token_sha256) = 64
        AND execution_token_sha256 NOT GLOB '*[^0-9a-f]*')
);

CREATE UNIQUE INDEX idx_workspace_planning_turn_claims_thread_turn
ON workspace_planning_turn_claims(source_thread_id, source_turn_id);

CREATE TABLE workspace_planning_context_reads (
    id TEXT PRIMARY KEY,
    guide_run_id TEXT NOT NULL,
    plan_session_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    category TEXT NOT NULL,
    max_records INTEGER NOT NULL,
    result_count INTEGER NOT NULL,
    response_json TEXT NOT NULL,
    response_sha256 TEXT NOT NULL,
    source_checkpoint_id TEXT NOT NULL,
    source_checkpoint_revision INTEGER NOT NULL,
    source_checkpoint_sha256 TEXT NOT NULL,
    source_thread_id TEXT NOT NULL,
    source_turn_id TEXT NOT NULL,
    prompt_sha256 TEXT NOT NULL,
    execution_token_sha256 TEXT NOT NULL,
    accessed_at_ms INTEGER NOT NULL,
    FOREIGN KEY(guide_run_id) REFERENCES workspace_planning_turn_claims(guide_run_id) ON DELETE CASCADE,
    FOREIGN KEY(plan_session_id) REFERENCES workspace_plan_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE,
    FOREIGN KEY(source_checkpoint_id) REFERENCES workspace_draft_checkpoints(id) ON DELETE CASCADE,
    UNIQUE(guide_run_id, idempotency_key),
    CHECK(length(trim(idempotency_key)) > 0),
    CHECK(category IN ('visit_history', 'progress_notes', 'patient_chart', 'selected_context')),
    CHECK(max_records BETWEEN 1 AND 50),
    CHECK(result_count BETWEEN 0 AND max_records),
    CHECK(json_valid(response_json) AND json_type(response_json) = 'array'),
    CHECK(length(response_sha256) = 64 AND response_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(source_checkpoint_revision > 0),
    CHECK(length(source_checkpoint_sha256) = 64
        AND source_checkpoint_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(trim(source_thread_id)) > 0),
    CHECK(length(trim(source_turn_id)) > 0),
    CHECK(length(prompt_sha256) = 64 AND prompt_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(execution_token_sha256) = 64
        AND execution_token_sha256 NOT GLOB '*[^0-9a-f]*')
);

CREATE INDEX idx_workspace_planning_context_reads_run_accessed
ON workspace_planning_context_reads(guide_run_id, accessed_at_ms, id);

CREATE TRIGGER workspace_planning_turn_claims_immutable
BEFORE UPDATE ON workspace_planning_turn_claims
BEGIN
    SELECT RAISE(ABORT, 'workspace planning turn claim is immutable');
END;

CREATE TRIGGER workspace_planning_context_reads_immutable
BEFORE UPDATE ON workspace_planning_context_reads
BEGIN
    SELECT RAISE(ABORT, 'workspace planning context read is immutable');
END;

CREATE TABLE workspace_plan_revisions (
    id TEXT PRIMARY KEY,
    plan_session_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
    guide_run_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    plan_markdown TEXT NOT NULL,
    decisions_json TEXT NOT NULL,
    open_questions_json TEXT NOT NULL,
    content_sha256 TEXT NOT NULL,
    evidence_manifest_json TEXT NOT NULL,
    evidence_manifest_sha256 TEXT NOT NULL,
    evidence_read_count INTEGER NOT NULL,
    idempotency_key TEXT NOT NULL,
    status TEXT NOT NULL,
    source_checkpoint_id TEXT NOT NULL,
    source_checkpoint_revision INTEGER NOT NULL,
    source_checkpoint_sha256 TEXT NOT NULL,
    encounter_id TEXT,
    note_id TEXT,
    source_thread_id TEXT NOT NULL,
    source_turn_id TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    submitted_at_ms INTEGER,
    FOREIGN KEY(plan_session_id) REFERENCES workspace_plan_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE,
    FOREIGN KEY(guide_run_id) REFERENCES workspace_guide_runs(id) ON DELETE CASCADE,
    FOREIGN KEY(source_checkpoint_id) REFERENCES workspace_draft_checkpoints(id) ON DELETE CASCADE,
    UNIQUE(plan_session_id, revision),
    UNIQUE(plan_session_id, idempotency_key),
    CHECK(revision > 0),
    CHECK(length(trim(plan_markdown)) > 0),
    CHECK(json_valid(decisions_json) AND json_type(decisions_json) = 'array'),
    CHECK(json_valid(open_questions_json) AND json_type(open_questions_json) = 'array'),
    CHECK(length(content_sha256) = 64 AND content_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(json_valid(evidence_manifest_json)
        AND json_type(evidence_manifest_json) = 'array'
        AND json_array_length(evidence_manifest_json) = evidence_read_count),
    CHECK(length(evidence_manifest_sha256) = 64
        AND evidence_manifest_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(evidence_read_count > 0),
    CHECK(length(trim(idempotency_key)) > 0),
    CHECK(status IN ('current', 'outdated', 'submitted')),
    CHECK(source_checkpoint_revision > 0),
    CHECK(length(source_checkpoint_sha256) = 64
        AND source_checkpoint_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(trim(source_thread_id)) > 0),
    CHECK(length(trim(source_turn_id)) > 0),
    CHECK(
        (status = 'submitted' AND submitted_at_ms IS NOT NULL)
        OR (status != 'submitted' AND submitted_at_ms IS NULL)
    )
);

CREATE UNIQUE INDEX idx_workspace_plan_revisions_one_current_session
ON workspace_plan_revisions(plan_session_id)
WHERE status = 'current';

CREATE INDEX idx_workspace_plan_revisions_session_revision
ON workspace_plan_revisions(plan_session_id, revision DESC);

CREATE TRIGGER workspace_plan_revisions_immutable
BEFORE UPDATE OF
    id, plan_session_id, client_id, guide_run_id, revision, plan_markdown,
    decisions_json, open_questions_json, content_sha256, evidence_manifest_json,
    evidence_manifest_sha256, evidence_read_count, idempotency_key,
    source_checkpoint_id, source_checkpoint_revision, source_checkpoint_sha256,
    encounter_id, note_id, source_thread_id, source_turn_id, created_at_ms
ON workspace_plan_revisions
BEGIN
    SELECT RAISE(ABORT, 'workspace plan revision content is immutable');
END;

CREATE TABLE workspace_plan_turn_completions (
    guide_run_id TEXT PRIMARY KEY,
    plan_session_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    assistant_message_id TEXT NOT NULL,
    plan_revision_id TEXT,
    completion_input_sha256 TEXT NOT NULL,
    evidence_manifest_json TEXT NOT NULL,
    evidence_manifest_sha256 TEXT NOT NULL,
    evidence_read_count INTEGER NOT NULL,
    terminal_envelope_json TEXT NOT NULL,
    terminal_envelope_sha256 TEXT NOT NULL,
    source_checkpoint_id TEXT NOT NULL,
    source_checkpoint_revision INTEGER NOT NULL,
    source_checkpoint_sha256 TEXT NOT NULL,
    source_thread_id TEXT NOT NULL,
    source_turn_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt_sha256 TEXT NOT NULL,
    execution_token_sha256 TEXT NOT NULL,
    completed_at_ms INTEGER NOT NULL,
    FOREIGN KEY(guide_run_id) REFERENCES workspace_guide_runs(id) ON DELETE CASCADE,
    FOREIGN KEY(plan_session_id) REFERENCES workspace_plan_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE,
    FOREIGN KEY(assistant_message_id) REFERENCES workspace_plan_messages(id) ON DELETE CASCADE,
    FOREIGN KEY(plan_revision_id) REFERENCES workspace_plan_revisions(id) ON DELETE CASCADE,
    FOREIGN KEY(source_checkpoint_id) REFERENCES workspace_draft_checkpoints(id) ON DELETE CASCADE,
    UNIQUE(plan_session_id, idempotency_key),
    CHECK(length(trim(idempotency_key)) > 0),
    CHECK(length(completion_input_sha256) = 64
        AND completion_input_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(json_valid(evidence_manifest_json)
        AND json_type(evidence_manifest_json) = 'array'
        AND json_array_length(evidence_manifest_json) = evidence_read_count),
    CHECK(length(evidence_manifest_sha256) = 64
        AND evidence_manifest_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(evidence_read_count >= 0),
    CHECK(plan_revision_id IS NULL OR evidence_read_count > 0),
    CHECK(json_valid(terminal_envelope_json)
        AND json_type(terminal_envelope_json) = 'object'),
    CHECK(length(terminal_envelope_sha256) = 64
        AND terminal_envelope_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(source_checkpoint_revision > 0),
    CHECK(length(source_checkpoint_sha256) = 64
        AND source_checkpoint_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(trim(source_thread_id)) > 0),
    CHECK(length(trim(source_turn_id)) > 0),
    CHECK(length(trim(provider)) > 0),
    CHECK(length(trim(model)) > 0),
    CHECK(length(prompt_sha256) = 64 AND prompt_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(execution_token_sha256) = 64
        AND execution_token_sha256 NOT GLOB '*[^0-9a-f]*')
);

CREATE INDEX idx_workspace_plan_turn_completions_session_completed
ON workspace_plan_turn_completions(plan_session_id, completed_at_ms DESC, guide_run_id DESC);

CREATE TABLE workspace_plan_turn_evidence (
    guide_run_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    context_read_id TEXT NOT NULL,
    category TEXT NOT NULL,
    response_sha256 TEXT NOT NULL,
    source_content_sha256_json TEXT NOT NULL,
    PRIMARY KEY(guide_run_id, ordinal),
    UNIQUE(guide_run_id, context_read_id),
    FOREIGN KEY(guide_run_id) REFERENCES workspace_guide_runs(id) ON DELETE CASCADE,
    FOREIGN KEY(context_read_id) REFERENCES workspace_planning_context_reads(id) ON DELETE RESTRICT,
    CHECK(ordinal >= 0),
    CHECK(category IN ('visit_history', 'progress_notes', 'patient_chart', 'selected_context')),
    CHECK(length(response_sha256) = 64 AND response_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(json_valid(source_content_sha256_json)
        AND json_type(source_content_sha256_json) = 'array')
);

CREATE TRIGGER workspace_plan_turn_completions_immutable
BEFORE UPDATE ON workspace_plan_turn_completions
BEGIN
    SELECT RAISE(ABORT, 'workspace plan turn completion is immutable');
END;

CREATE TRIGGER workspace_plan_turn_completions_evidence_count_insert
BEFORE INSERT ON workspace_plan_turn_completions
WHEN NEW.evidence_read_count != (
    SELECT COUNT(*) FROM workspace_plan_turn_evidence
    WHERE guide_run_id = NEW.guide_run_id
)
BEGIN
    SELECT RAISE(ABORT, 'workspace plan turn evidence count mismatch');
END;

CREATE TRIGGER workspace_plan_turn_evidence_immutable
BEFORE UPDATE ON workspace_plan_turn_evidence
BEGIN
    SELECT RAISE(ABORT, 'workspace plan turn evidence is immutable');
END;

CREATE TABLE workspace_plan_proposals (
    id TEXT PRIMARY KEY,
    plan_session_id TEXT NOT NULL,
    plan_revision_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
    guide_run_id TEXT NOT NULL,
    proposal_kind TEXT NOT NULL,
    target_note_id TEXT,
    base_revision INTEGER,
    payload_json TEXT NOT NULL,
    payload_sha256 TEXT NOT NULL,
    summary TEXT NOT NULL,
    rationale TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    status TEXT NOT NULL,
    source_checkpoint_id TEXT NOT NULL,
    source_checkpoint_revision INTEGER NOT NULL,
    source_checkpoint_sha256 TEXT NOT NULL,
    source_thread_id TEXT NOT NULL,
    source_turn_id TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    resolved_at_ms INTEGER,
    resolved_by TEXT,
    FOREIGN KEY(plan_session_id) REFERENCES workspace_plan_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY(plan_revision_id) REFERENCES workspace_plan_revisions(id) ON DELETE CASCADE,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE,
    FOREIGN KEY(guide_run_id) REFERENCES workspace_guide_runs(id) ON DELETE CASCADE,
    FOREIGN KEY(target_note_id) REFERENCES workspace_notes(id) ON DELETE CASCADE,
    FOREIGN KEY(source_checkpoint_id) REFERENCES workspace_draft_checkpoints(id) ON DELETE CASCADE,
    UNIQUE(plan_session_id, idempotency_key),
    CHECK(proposal_kind IN ('note_revision', 'note_addendum', 'task_draft')),
    CHECK(
        (proposal_kind IN ('note_revision', 'note_addendum')
            AND target_note_id IS NOT NULL AND base_revision > 0)
        OR (proposal_kind = 'task_draft' AND target_note_id IS NULL AND base_revision IS NULL)
    ),
    CHECK(json_valid(payload_json) AND json_type(payload_json) = 'object'),
    CHECK(length(payload_sha256) = 64 AND payload_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(trim(summary)) > 0),
    CHECK(length(trim(rationale)) > 0),
    CHECK(length(trim(idempotency_key)) > 0),
    CHECK(status IN ('pending', 'accepted', 'declined', 'outdated')),
    CHECK(source_checkpoint_revision > 0),
    CHECK(length(source_checkpoint_sha256) = 64
        AND source_checkpoint_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(trim(source_thread_id)) > 0),
    CHECK(length(trim(source_turn_id)) > 0),
    CHECK(
        (status IN ('pending', 'outdated') AND resolved_at_ms IS NULL AND resolved_by IS NULL)
        OR (status IN ('accepted', 'declined') AND resolved_at_ms IS NOT NULL
            AND length(trim(resolved_by)) > 0)
    )
);

CREATE INDEX idx_workspace_plan_proposals_session_status
ON workspace_plan_proposals(plan_session_id, status, created_at_ms DESC, id DESC);

CREATE INDEX idx_workspace_plan_proposals_revision_status
ON workspace_plan_proposals(plan_revision_id, status, created_at_ms DESC, id DESC);

CREATE TRIGGER workspace_plan_proposals_immutable
BEFORE UPDATE OF
    id, plan_session_id, plan_revision_id, client_id, guide_run_id,
    proposal_kind, target_note_id, base_revision, payload_json, payload_sha256,
    summary, rationale, idempotency_key, source_checkpoint_id,
    source_checkpoint_revision, source_checkpoint_sha256, source_thread_id,
    source_turn_id, created_at_ms
ON workspace_plan_proposals
BEGIN
    SELECT RAISE(ABORT, 'workspace plan proposal content is immutable');
END;

CREATE TRIGGER workspace_plan_revisions_outdate_after_checkpoint
AFTER UPDATE OF current_revision ON workspace_draft_sessions
WHEN OLD.current_revision != NEW.current_revision
BEGIN
    UPDATE workspace_plan_revisions
    SET status = 'outdated'
    WHERE client_id = NEW.client_id
      AND status = 'current'
      AND NOT EXISTS (
          SELECT 1
          FROM workspace_draft_checkpoints AS checkpoint
          WHERE checkpoint.session_id = NEW.id
            AND checkpoint.revision = NEW.current_revision
            AND checkpoint.id = workspace_plan_revisions.source_checkpoint_id
            AND checkpoint.content_sha256 = workspace_plan_revisions.source_checkpoint_sha256
      );

    UPDATE workspace_plan_proposals
    SET status = 'outdated'
    WHERE status = 'pending'
      AND plan_revision_id IN (
          SELECT id
          FROM workspace_plan_revisions
          WHERE client_id = NEW.client_id AND status = 'outdated'
      );
END;

CREATE TRIGGER workspace_plan_proposals_outdate_after_note_revision
AFTER UPDATE OF current_revision ON workspace_notes
WHEN OLD.current_revision != NEW.current_revision
BEGIN
    UPDATE workspace_plan_proposals
    SET status = 'outdated'
    WHERE target_note_id = NEW.id
      AND status = 'pending'
      AND base_revision != NEW.current_revision;
END;

CREATE TRIGGER workspace_data_policy_restrict_update
BEFORE UPDATE ON workspace_data_policy
WHEN NOT (
    OLD.singleton_id = 1
    AND OLD.schema_version = 1
    AND OLD.data_classification = 'unclassified'
    AND OLD.classified_at_ms IS NULL
    AND OLD.classified_by IS NULL
    AND NEW.singleton_id = 1
    AND NEW.schema_version = 1
    AND NEW.data_classification = 'synthetic'
    AND NEW.classified_at_ms IS NOT NULL
    AND NEW.classified_at_ms >= 0
    AND NEW.classified_by IS NOT NULL
    AND NEW.classified_by = trim(NEW.classified_by)
    AND length(NEW.classified_by) > 0
    AND length(CAST(NEW.classified_by AS BLOB)) <= 256
    AND NOT EXISTS (SELECT 1 FROM workspace_clients)
    AND NOT EXISTS (SELECT 1 FROM workspace_notes)
    AND NOT EXISTS (SELECT 1 FROM workspace_note_revisions)
    AND NOT EXISTS (SELECT 1 FROM workspace_note_proposals)
    AND NOT EXISTS (SELECT 1 FROM workspace_audit_events)
    AND NOT EXISTS (SELECT 1 FROM workspace_documents)
    AND NOT EXISTS (SELECT 1 FROM workspace_encounters)
    AND NOT EXISTS (SELECT 1 FROM workspace_note_signatures)
    AND NOT EXISTS (SELECT 1 FROM workspace_note_addenda)
    AND NOT EXISTS (SELECT 1 FROM workspace_tasks)
    AND NOT EXISTS (SELECT 1 FROM workspace_context_packets)
    AND NOT EXISTS (SELECT 1 FROM workspace_agent_results)
    AND NOT EXISTS (SELECT 1 FROM workspace_artifact_derivatives)
    AND NOT EXISTS (SELECT 1 FROM workspace_context_clips)
    AND NOT EXISTS (SELECT 1 FROM workspace_client_contacts)
    AND NOT EXISTS (SELECT 1 FROM workspace_client_coverages)
    AND NOT EXISTS (SELECT 1 FROM workspace_coverages)
    AND NOT EXISTS (SELECT 1 FROM workspace_coverage_card_verifications)
    AND NOT EXISTS (SELECT 1 FROM workspace_patient_safety_items)
    AND NOT EXISTS (SELECT 1 FROM workspace_agent_runs)
    AND NOT EXISTS (SELECT 1 FROM workspace_agent_run_sources)
    AND NOT EXISTS (SELECT 1 FROM workspace_note_proposal_decisions)
    AND NOT EXISTS (SELECT 1 FROM workspace_chart_commits)
    AND NOT EXISTS (SELECT 1 FROM workspace_draft_sessions)
    AND NOT EXISTS (SELECT 1 FROM workspace_draft_checkpoints)
    AND NOT EXISTS (SELECT 1 FROM workspace_guide_runs)
    AND NOT EXISTS (SELECT 1 FROM workspace_plan_sessions)
    AND NOT EXISTS (SELECT 1 FROM workspace_plan_messages)
    AND NOT EXISTS (SELECT 1 FROM workspace_planning_turn_claims)
    AND NOT EXISTS (SELECT 1 FROM workspace_planning_context_reads)
    AND NOT EXISTS (SELECT 1 FROM workspace_plan_revisions)
    AND NOT EXISTS (SELECT 1 FROM workspace_plan_proposals)
    AND NOT EXISTS (SELECT 1 FROM workspace_plan_turn_completions)
    AND NOT EXISTS (SELECT 1 FROM workspace_plan_turn_evidence)
)
BEGIN
    SELECT RAISE(ABORT, 'workspace data policy only permits unclassified to synthetic');
END;
