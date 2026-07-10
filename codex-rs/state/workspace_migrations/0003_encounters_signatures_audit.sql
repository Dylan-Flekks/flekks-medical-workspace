CREATE TABLE workspace_encounters (
    id TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    status TEXT NOT NULL,
    started_at_ms INTEGER,
    ended_at_ms INTEGER,
    archived_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE
);

CREATE INDEX idx_workspace_encounters_client_updated_at
ON workspace_encounters(client_id, archived_at_ms, updated_at_ms DESC);

ALTER TABLE workspace_notes
ADD COLUMN encounter_id TEXT;

ALTER TABLE workspace_documents
ADD COLUMN encounter_id TEXT;

CREATE TABLE workspace_note_signatures (
    id TEXT PRIMARY KEY,
    note_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    signer TEXT NOT NULL,
    body_sha256 TEXT NOT NULL,
    signed_at_ms INTEGER NOT NULL,
    FOREIGN KEY(note_id) REFERENCES workspace_notes(id) ON DELETE CASCADE,
    UNIQUE(note_id, revision)
);

CREATE INDEX idx_workspace_note_signatures_note_signed_at
ON workspace_note_signatures(note_id, signed_at_ms DESC);

CREATE TABLE workspace_note_addenda (
    id TEXT PRIMARY KEY,
    note_id TEXT NOT NULL,
    base_revision INTEGER NOT NULL,
    body TEXT NOT NULL,
    author TEXT NOT NULL,
    source_thread_id TEXT,
    source_turn_id TEXT,
    created_at_ms INTEGER NOT NULL,
    FOREIGN KEY(note_id) REFERENCES workspace_notes(id) ON DELETE CASCADE
);

CREATE INDEX idx_workspace_note_addenda_note_created_at
ON workspace_note_addenda(note_id, created_at_ms DESC);

ALTER TABLE workspace_audit_events
ADD COLUMN client_id TEXT;

ALTER TABLE workspace_audit_events
ADD COLUMN encounter_id TEXT;

ALTER TABLE workspace_audit_events
ADD COLUMN note_id TEXT;

ALTER TABLE workspace_audit_events
ADD COLUMN document_id TEXT;

ALTER TABLE workspace_audit_events
ADD COLUMN actor_kind TEXT NOT NULL DEFAULT 'human';

ALTER TABLE workspace_audit_events
ADD COLUMN source TEXT NOT NULL DEFAULT 'state';

ALTER TABLE workspace_audit_events
ADD COLUMN success INTEGER NOT NULL DEFAULT 1;

ALTER TABLE workspace_audit_events
ADD COLUMN metadata_json TEXT;

CREATE INDEX idx_workspace_audit_events_client
ON workspace_audit_events(client_id, created_at_ms DESC);

CREATE INDEX idx_workspace_audit_events_note
ON workspace_audit_events(note_id, created_at_ms DESC);
