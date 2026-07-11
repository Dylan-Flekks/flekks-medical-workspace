CREATE TABLE workspace_guide_runs (
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
    CHECK(model_tool_mode = 'disabled'),
    CHECK(status IN ('running', 'completed', 'failed', 'canceled')),
    CHECK(
        (source_thread_id IS NULL AND source_turn_id IS NULL)
        OR (source_thread_id IS NOT NULL AND source_turn_id IS NOT NULL)
    ),
    CHECK(
        (status = 'running' AND source_thread_id IS NULL AND source_turn_id IS NULL
            AND terminal_envelope_json IS NULL AND terminal_envelope_sha256 IS NULL
            AND terminal_at_ms IS NULL)
        OR (status != 'running' AND terminal_envelope_json IS NOT NULL
            AND terminal_envelope_sha256 IS NOT NULL AND terminal_at_ms IS NOT NULL)
    ),
    CHECK(status != 'completed' OR source_thread_id IS NOT NULL)
);

CREATE UNIQUE INDEX idx_workspace_guide_runs_one_active_session
ON workspace_guide_runs(session_id)
WHERE status = 'running';

CREATE INDEX idx_workspace_guide_runs_client_session_created
ON workspace_guide_runs(client_id, session_id, created_at_ms DESC, id DESC);

CREATE INDEX idx_workspace_guide_runs_checkpoint_created
ON workspace_guide_runs(source_checkpoint_id, created_at_ms DESC);
