CREATE TABLE workspace_chart_commits (
    id TEXT PRIMARY KEY,
    idempotency_key TEXT NOT NULL UNIQUE,
    schema_version INTEGER NOT NULL,
    request_sha256 TEXT NOT NULL,
    request_json TEXT NOT NULL,
    client_id TEXT NOT NULL,
    actor TEXT NOT NULL,
    reason TEXT NOT NULL DEFAULT '',
    changed_entity_kinds_json TEXT NOT NULL,
    result_json TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE
);

CREATE INDEX idx_workspace_chart_commits_client_created_at
ON workspace_chart_commits(client_id, created_at_ms DESC);
