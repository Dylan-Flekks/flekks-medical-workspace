CREATE TABLE workspace_context_packets (
    id TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    encounter_id TEXT,
    note_id TEXT,
    human_request TEXT NOT NULL,
    selected_artifact_ids_json TEXT NOT NULL DEFAULT '[]',
    artifact_summary TEXT NOT NULL DEFAULT '',
    chart_context_summary TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    sent_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE,
    FOREIGN KEY(encounter_id) REFERENCES workspace_encounters(id) ON DELETE SET NULL,
    FOREIGN KEY(note_id) REFERENCES workspace_notes(id) ON DELETE SET NULL
);

CREATE INDEX idx_workspace_context_packets_client_sent_at
ON workspace_context_packets(client_id, sent_at_ms DESC);

CREATE INDEX idx_workspace_context_packets_note_sent_at
ON workspace_context_packets(note_id, sent_at_ms DESC);

CREATE TABLE workspace_agent_results (
    id TEXT PRIMARY KEY,
    packet_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
    note_id TEXT,
    body TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    FOREIGN KEY(packet_id) REFERENCES workspace_context_packets(id) ON DELETE CASCADE,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE,
    FOREIGN KEY(note_id) REFERENCES workspace_notes(id) ON DELETE SET NULL
);

CREATE INDEX idx_workspace_agent_results_packet_created_at
ON workspace_agent_results(packet_id, created_at_ms DESC);

CREATE INDEX idx_workspace_agent_results_client_created_at
ON workspace_agent_results(client_id, created_at_ms DESC);
