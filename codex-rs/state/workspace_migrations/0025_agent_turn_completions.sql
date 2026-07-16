CREATE TABLE workspace_agent_turn_completions (
    run_id TEXT PRIMARY KEY,
    result_id TEXT NOT NULL UNIQUE,
    packet_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
    source_thread_id TEXT NOT NULL,
    source_turn_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    assistant_message_id TEXT NOT NULL,
    body_sha256 TEXT NOT NULL,
    completion_input_sha256 TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    completed_at_ms INTEGER NOT NULL,
    FOREIGN KEY(run_id) REFERENCES workspace_agent_runs(id) ON DELETE CASCADE,
    FOREIGN KEY(result_id) REFERENCES workspace_agent_results(id) ON DELETE CASCADE,
    FOREIGN KEY(packet_id) REFERENCES workspace_context_packets(id) ON DELETE CASCADE,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE,
    CHECK(length(trim(source_thread_id)) > 0),
    CHECK(length(trim(source_turn_id)) > 0),
    CHECK(length(trim(provider)) > 0),
    CHECK(length(trim(model)) > 0),
    CHECK(length(trim(assistant_message_id)) > 0),
    CHECK(length(body_sha256) = 64 AND body_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(completion_input_sha256) = 64
        AND completion_input_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(trim(idempotency_key)) > 0)
);

CREATE INDEX idx_workspace_agent_turn_completions_packet_completed
ON workspace_agent_turn_completions(packet_id, completed_at_ms DESC);

CREATE TRIGGER workspace_agent_turn_completions_immutable_update
BEFORE UPDATE ON workspace_agent_turn_completions
BEGIN
    SELECT RAISE(ABORT, 'workspace agent turn completion is immutable');
END;
