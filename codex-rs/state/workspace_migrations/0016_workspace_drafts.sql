CREATE TABLE workspace_draft_sessions (
    id TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    current_revision INTEGER NOT NULL DEFAULT 0,
    created_by TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    closed_at_ms INTEGER,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE
);

CREATE INDEX idx_workspace_draft_sessions_client_status_updated
ON workspace_draft_sessions(client_id, status, updated_at_ms DESC);

CREATE TABLE workspace_draft_checkpoints (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
    encounter_id TEXT,
    note_id TEXT,
    base_note_revision INTEGER,
    schema_version INTEGER NOT NULL,
    revision INTEGER NOT NULL,
    draft_json TEXT NOT NULL,
    content_sha256 TEXT NOT NULL,
    trigger TEXT NOT NULL,
    actor TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    FOREIGN KEY(session_id) REFERENCES workspace_draft_sessions(id) ON DELETE CASCADE,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE,
    UNIQUE(session_id, revision),
    UNIQUE(session_id, content_sha256)
);

CREATE INDEX idx_workspace_draft_checkpoints_client_created
ON workspace_draft_checkpoints(client_id, created_at_ms DESC);
