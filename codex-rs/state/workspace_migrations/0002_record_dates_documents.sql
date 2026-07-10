ALTER TABLE workspace_clients
ADD COLUMN record_start_date TEXT;

ALTER TABLE workspace_clients
ADD COLUMN record_end_date TEXT;

CREATE TABLE workspace_documents (
    id TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    title TEXT NOT NULL,
    kind TEXT NOT NULL,
    local_path TEXT NOT NULL,
    notes TEXT NOT NULL DEFAULT '',
    archived_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE
);

CREATE INDEX idx_workspace_documents_client_updated_at
ON workspace_documents(client_id, archived_at_ms, updated_at_ms DESC);
