CREATE TABLE workspace_clients (
    id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    preferred_name TEXT,
    date_of_birth TEXT,
    sex_or_gender TEXT,
    external_id TEXT,
    summary TEXT NOT NULL DEFAULT '',
    archived_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_workspace_clients_updated_at
ON workspace_clients(archived_at_ms, updated_at_ms DESC);

CREATE TABLE workspace_notes (
    id TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    title TEXT NOT NULL,
    kind TEXT NOT NULL,
    body TEXT NOT NULL,
    status TEXT NOT NULL,
    current_revision INTEGER NOT NULL,
    archived_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE
);

CREATE INDEX idx_workspace_notes_client_updated_at
ON workspace_notes(client_id, archived_at_ms, updated_at_ms DESC);

CREATE TABLE workspace_note_revisions (
    note_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    body TEXT NOT NULL,
    actor TEXT NOT NULL,
    source_thread_id TEXT,
    source_turn_id TEXT,
    summary TEXT,
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY(note_id, revision),
    FOREIGN KEY(note_id) REFERENCES workspace_notes(id) ON DELETE CASCADE
);

CREATE TABLE workspace_note_proposals (
    id TEXT PRIMARY KEY,
    note_id TEXT NOT NULL,
    base_revision INTEGER NOT NULL,
    proposed_body TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL,
    source_thread_id TEXT,
    source_turn_id TEXT,
    created_at_ms INTEGER NOT NULL,
    resolved_at_ms INTEGER,
    FOREIGN KEY(note_id) REFERENCES workspace_notes(id) ON DELETE CASCADE
);

CREATE INDEX idx_workspace_note_proposals_note_status
ON workspace_note_proposals(note_id, status, created_at_ms DESC);

CREATE TABLE workspace_audit_events (
    id TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    action TEXT NOT NULL,
    actor TEXT NOT NULL,
    source_thread_id TEXT,
    source_turn_id TEXT,
    summary TEXT NOT NULL DEFAULT '',
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_workspace_audit_events_entity
ON workspace_audit_events(entity_type, entity_id, created_at_ms DESC);
