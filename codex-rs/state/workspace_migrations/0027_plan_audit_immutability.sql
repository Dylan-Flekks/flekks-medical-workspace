-- Plan history is an audit surface. Rows that describe what the clinician or
-- restricted planner saw, said, claimed, or completed are append-only. The
-- handful of lifecycle rows remain mutable only along their documented
-- forward-only transitions.

CREATE TRIGGER workspace_plan_sessions_identity_immutable
BEFORE UPDATE OF id, client_id, created_by, created_at_ms
ON workspace_plan_sessions
BEGIN
    SELECT RAISE(ABORT, 'workspace plan session identity is immutable');
END;

CREATE TRIGGER workspace_plan_sessions_thread_binding_forward_only
BEFORE UPDATE OF source_thread_id
ON workspace_plan_sessions
WHEN NOT (
    OLD.source_thread_id IS NEW.source_thread_id
    OR (
        OLD.status = 'active'
        AND NEW.status = 'active'
        AND OLD.source_thread_id IS NULL
        AND NEW.source_thread_id IS NOT NULL
        AND length(trim(NEW.source_thread_id)) > 0
    )
)
BEGIN
    SELECT RAISE(ABORT, 'workspace plan session thread binding is permanent');
END;

CREATE TRIGGER workspace_plan_sessions_revision_forward_only
BEFORE UPDATE OF latest_revision
ON workspace_plan_sessions
WHEN NOT (
    NEW.latest_revision = OLD.latest_revision
    OR (
        OLD.status = 'active'
        AND NEW.status = 'active'
        AND NEW.latest_revision = OLD.latest_revision + 1
    )
)
BEGIN
    SELECT RAISE(ABORT, 'workspace plan session revision must advance by one');
END;

CREATE TRIGGER workspace_plan_sessions_status_forward_only
BEFORE UPDATE OF status, closed_at_ms
ON workspace_plan_sessions
WHEN NOT (
    (NEW.status = OLD.status AND NEW.closed_at_ms IS OLD.closed_at_ms)
    OR (
        OLD.status = 'active'
        AND NEW.status = 'closed'
        AND OLD.closed_at_ms IS NULL
        AND NEW.closed_at_ms IS NOT NULL
        AND NEW.closed_at_ms >= OLD.created_at_ms
    )
)
BEGIN
    SELECT RAISE(ABORT, 'workspace plan session lifecycle is forward-only');
END;

CREATE TRIGGER workspace_plan_sessions_immutable_delete
BEFORE DELETE ON workspace_plan_sessions
BEGIN
    SELECT RAISE(ABORT, 'workspace plan session cannot be deleted');
END;

CREATE TRIGGER workspace_guide_runs_identity_immutable
BEFORE UPDATE OF
    id, client_id, session_id, source_checkpoint_id, source_checkpoint_revision,
    source_checkpoint_sha256, request_schema_version, request_envelope_json,
    request_envelope_sha256, idempotency_key, trigger, actor, provider, model,
    model_tool_mode, created_at_ms
ON workspace_guide_runs
BEGIN
    SELECT RAISE(ABORT, 'workspace guide run identity is immutable');
END;

CREATE TRIGGER workspace_guide_runs_source_binding_forward_only
BEFORE UPDATE OF source_thread_id, source_turn_id
ON workspace_guide_runs
WHEN NOT (
    (
        NEW.source_thread_id IS OLD.source_thread_id
        AND NEW.source_turn_id IS OLD.source_turn_id
    )
    OR (
        OLD.status = 'running'
        AND OLD.source_thread_id IS NULL
        AND OLD.source_turn_id IS NULL
        AND NEW.source_thread_id IS NOT NULL
        AND NEW.source_turn_id IS NOT NULL
        AND length(trim(NEW.source_thread_id)) > 0
        AND length(trim(NEW.source_turn_id)) > 0
    )
)
BEGIN
    SELECT RAISE(ABORT, 'workspace guide run source binding is permanent');
END;

CREATE TRIGGER workspace_guide_runs_status_forward_only
BEFORE UPDATE OF
    status, terminal_envelope_json, terminal_envelope_sha256, terminal_at_ms
ON workspace_guide_runs
WHEN NOT (
    (
        NEW.status = OLD.status
        AND NEW.terminal_envelope_json IS OLD.terminal_envelope_json
        AND NEW.terminal_envelope_sha256 IS OLD.terminal_envelope_sha256
        AND NEW.terminal_at_ms IS OLD.terminal_at_ms
    )
    OR (
        OLD.status = 'running'
        AND NEW.status IN ('completed', 'failed', 'canceled')
        AND OLD.terminal_envelope_json IS NULL
        AND OLD.terminal_envelope_sha256 IS NULL
        AND OLD.terminal_at_ms IS NULL
        AND NEW.terminal_envelope_json IS NOT NULL
        AND NEW.terminal_envelope_sha256 IS NOT NULL
        AND NEW.terminal_at_ms IS NOT NULL
        AND NEW.terminal_at_ms >= OLD.created_at_ms
    )
)
BEGIN
    SELECT RAISE(ABORT, 'workspace guide run lifecycle is forward-only');
END;

CREATE TRIGGER workspace_guide_runs_immutable_delete
BEFORE DELETE ON workspace_guide_runs
BEGIN
    SELECT RAISE(ABORT, 'workspace guide run cannot be deleted');
END;

CREATE TRIGGER workspace_plan_messages_immutable_update
BEFORE UPDATE ON workspace_plan_messages
BEGIN
    SELECT RAISE(ABORT, 'workspace plan message is immutable');
END;

CREATE TRIGGER workspace_plan_messages_immutable_delete
BEFORE DELETE ON workspace_plan_messages
BEGIN
    SELECT RAISE(ABORT, 'workspace plan message cannot be deleted');
END;

CREATE TRIGGER workspace_planning_turn_claims_immutable_delete
BEFORE DELETE ON workspace_planning_turn_claims
BEGIN
    SELECT RAISE(ABORT, 'workspace planning turn claim cannot be deleted');
END;

CREATE TRIGGER workspace_planning_context_reads_immutable_delete
BEFORE DELETE ON workspace_planning_context_reads
BEGIN
    SELECT RAISE(ABORT, 'workspace planning context read cannot be deleted');
END;

CREATE TRIGGER workspace_plan_revisions_status_forward_only
BEFORE UPDATE OF status, submitted_at_ms
ON workspace_plan_revisions
WHEN NOT (
    (NEW.status = OLD.status AND NEW.submitted_at_ms IS OLD.submitted_at_ms)
    OR (
        OLD.status = 'current'
        AND NEW.status = 'outdated'
        AND OLD.submitted_at_ms IS NULL
        AND NEW.submitted_at_ms IS NULL
    )
    OR (
        OLD.status = 'current'
        AND NEW.status = 'submitted'
        AND OLD.submitted_at_ms IS NULL
        AND NEW.submitted_at_ms IS NOT NULL
        AND NEW.submitted_at_ms >= OLD.created_at_ms
    )
)
BEGIN
    SELECT RAISE(ABORT, 'workspace plan revision lifecycle is forward-only');
END;

CREATE TRIGGER workspace_plan_revisions_immutable_delete
BEFORE DELETE ON workspace_plan_revisions
BEGIN
    SELECT RAISE(ABORT, 'workspace plan revision cannot be deleted');
END;

CREATE TRIGGER workspace_plan_turn_completions_immutable_delete
BEFORE DELETE ON workspace_plan_turn_completions
BEGIN
    SELECT RAISE(ABORT, 'workspace plan turn completion cannot be deleted');
END;

CREATE TRIGGER workspace_plan_turn_evidence_immutable_delete
BEFORE DELETE ON workspace_plan_turn_evidence
BEGIN
    SELECT RAISE(ABORT, 'workspace plan turn evidence cannot be deleted');
END;

CREATE TRIGGER workspace_plan_proposals_status_forward_only
BEFORE UPDATE OF status, resolved_at_ms, resolved_by
ON workspace_plan_proposals
WHEN NOT (
    (
        NEW.status = OLD.status
        AND NEW.resolved_at_ms IS OLD.resolved_at_ms
        AND NEW.resolved_by IS OLD.resolved_by
    )
    OR (
        OLD.status = 'pending'
        AND NEW.status = 'outdated'
        AND OLD.resolved_at_ms IS NULL
        AND OLD.resolved_by IS NULL
        AND NEW.resolved_at_ms IS NULL
        AND NEW.resolved_by IS NULL
    )
    OR (
        OLD.status = 'pending'
        AND NEW.status IN ('accepted', 'declined')
        AND OLD.resolved_at_ms IS NULL
        AND OLD.resolved_by IS NULL
        AND NEW.resolved_at_ms IS NOT NULL
        AND NEW.resolved_at_ms >= OLD.created_at_ms
        AND NEW.resolved_by IS NOT NULL
        AND length(trim(NEW.resolved_by)) > 0
    )
)
BEGIN
    SELECT RAISE(ABORT, 'workspace plan proposal lifecycle is forward-only');
END;

CREATE TRIGGER workspace_plan_proposals_immutable_delete
BEFORE DELETE ON workspace_plan_proposals
BEGIN
    SELECT RAISE(ABORT, 'workspace plan proposal cannot be deleted');
END;

CREATE TRIGGER workspace_agent_turn_completions_immutable_delete
BEFORE DELETE ON workspace_agent_turn_completions
BEGIN
    SELECT RAISE(ABORT, 'workspace agent turn completion cannot be deleted');
END;
