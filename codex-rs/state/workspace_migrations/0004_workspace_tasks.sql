CREATE TABLE workspace_tasks (
    id TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    encounter_id TEXT,
    note_id TEXT,
    document_id TEXT,
    title TEXT NOT NULL,
    details TEXT NOT NULL DEFAULT '',
    kind TEXT NOT NULL,
    status TEXT NOT NULL,
    priority TEXT NOT NULL,
    due_date TEXT,
    assigned_to TEXT,
    completed_at_ms INTEGER,
    archived_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE,
    FOREIGN KEY(encounter_id) REFERENCES workspace_encounters(id) ON DELETE SET NULL,
    FOREIGN KEY(note_id) REFERENCES workspace_notes(id) ON DELETE SET NULL,
    FOREIGN KEY(document_id) REFERENCES workspace_documents(id) ON DELETE SET NULL
);

CREATE INDEX idx_workspace_tasks_client_status_updated_at
ON workspace_tasks(client_id, archived_at_ms, status, updated_at_ms DESC);

CREATE INDEX idx_workspace_tasks_note
ON workspace_tasks(note_id, archived_at_ms, updated_at_ms DESC);

CREATE INDEX idx_workspace_tasks_document
ON workspace_tasks(document_id, archived_at_ms, updated_at_ms DESC);
