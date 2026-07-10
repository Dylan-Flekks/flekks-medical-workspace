CREATE TABLE workspace_context_clips (
    id TEXT PRIMARY KEY,
    derivative_id TEXT NOT NULL,
    document_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
    encounter_id TEXT,
    note_id TEXT,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    review_status TEXT NOT NULL,
    source_method TEXT NOT NULL,
    page_range TEXT NOT NULL DEFAULT '',
    timestamp_range TEXT NOT NULL DEFAULT '',
    line_range TEXT NOT NULL DEFAULT '',
    segment_label TEXT NOT NULL DEFAULT '',
    tags TEXT NOT NULL DEFAULT '',
    metadata_json TEXT NOT NULL DEFAULT '{}',
    archived_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    FOREIGN KEY(derivative_id) REFERENCES workspace_artifact_derivatives(id) ON DELETE CASCADE,
    FOREIGN KEY(document_id) REFERENCES workspace_documents(id) ON DELETE CASCADE,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE,
    FOREIGN KEY(encounter_id) REFERENCES workspace_encounters(id) ON DELETE SET NULL,
    FOREIGN KEY(note_id) REFERENCES workspace_notes(id) ON DELETE SET NULL
);

CREATE INDEX idx_workspace_context_clips_client_updated_at
ON workspace_context_clips(client_id, updated_at_ms DESC);

CREATE INDEX idx_workspace_context_clips_derivative_updated_at
ON workspace_context_clips(derivative_id, updated_at_ms DESC);

ALTER TABLE workspace_context_packets
ADD COLUMN selected_clip_ids_json TEXT NOT NULL DEFAULT '[]';

ALTER TABLE workspace_context_packets
ADD COLUMN clip_summary TEXT NOT NULL DEFAULT '';
