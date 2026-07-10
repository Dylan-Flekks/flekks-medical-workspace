ALTER TABLE workspace_context_packets
ADD COLUMN clinician_actor TEXT NOT NULL DEFAULT 'local human';

ALTER TABLE workspace_context_packets
ADD COLUMN base_note_revision INTEGER;

ALTER TABLE workspace_context_packets
ADD COLUMN authorized_scope_json TEXT NOT NULL DEFAULT '{"version":1,"categories":["packet_snapshot"],"legacy":true}';

ALTER TABLE workspace_context_packets
ADD COLUMN expected_output_kind TEXT NOT NULL DEFAULT 'recommendation';

ALTER TABLE workspace_context_packets
ADD COLUMN submitted_at_ms INTEGER;

ALTER TABLE workspace_context_packets
ADD COLUMN canceled_at_ms INTEGER;

UPDATE workspace_context_packets
SET
    status = 'submitted',
    submitted_at_ms = sent_at_ms
WHERE status IN ('sent', 'result_saved');

UPDATE workspace_context_packets
SET base_note_revision = CAST(json_extract(context_envelope_json, '$.note.revision') AS INTEGER)
WHERE json_valid(context_envelope_json)
  AND json_type(context_envelope_json, '$.note.revision') IN ('integer', 'real');

UPDATE workspace_context_packets
SET authorized_scope_json =
    '{"version":1,"categories":["packet_snapshot"],"legacy":true,' ||
    '"selectedArtifactIds":' || selected_artifact_ids_json || ',' ||
    '"selectedDerivativeIds":' || selected_derivative_ids_json || ',' ||
    '"selectedClipIds":' || selected_clip_ids_json || '}'
WHERE json_valid(selected_artifact_ids_json)
  AND json_valid(selected_derivative_ids_json)
  AND json_valid(selected_clip_ids_json);

CREATE INDEX idx_workspace_context_packets_client_lifecycle
ON workspace_context_packets(client_id, status, created_at_ms DESC);

CREATE TABLE workspace_agent_runs (
    id TEXT PRIMARY KEY,
    packet_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
    note_id TEXT,
    base_note_revision INTEGER,
    context_envelope_sha256 TEXT NOT NULL,
    run_kind TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    provider TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    source_thread_id TEXT,
    source_turn_id TEXT,
    status TEXT NOT NULL,
    error_summary TEXT NOT NULL DEFAULT '',
    started_at_ms INTEGER NOT NULL,
    completed_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    FOREIGN KEY(packet_id) REFERENCES workspace_context_packets(id) ON DELETE CASCADE,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE,
    FOREIGN KEY(note_id) REFERENCES workspace_notes(id) ON DELETE SET NULL,
    UNIQUE(packet_id, idempotency_key)
);

CREATE INDEX idx_workspace_agent_runs_packet_created_at
ON workspace_agent_runs(packet_id, created_at_ms DESC);

CREATE INDEX idx_workspace_agent_runs_client_note_created_at
ON workspace_agent_runs(client_id, note_id, created_at_ms DESC);

CREATE TABLE workspace_agent_run_sources (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    source_entity_type TEXT NOT NULL,
    source_entity_id TEXT NOT NULL,
    source_revision INTEGER,
    display_label TEXT NOT NULL DEFAULT '',
    snapshot_json TEXT NOT NULL,
    content_sha256 TEXT NOT NULL,
    access_purpose TEXT NOT NULL DEFAULT '',
    accessed_at_ms INTEGER NOT NULL,
    FOREIGN KEY(run_id) REFERENCES workspace_agent_runs(id) ON DELETE CASCADE
);

CREATE INDEX idx_workspace_agent_run_sources_run_accessed_at
ON workspace_agent_run_sources(run_id, accessed_at_ms ASC);

ALTER TABLE workspace_agent_results
ADD COLUMN run_id TEXT REFERENCES workspace_agent_runs(id) ON DELETE SET NULL;

ALTER TABLE workspace_agent_results
ADD COLUMN base_note_revision INTEGER;

ALTER TABLE workspace_agent_results
ADD COLUMN packet_context_sha256 TEXT NOT NULL DEFAULT '';

ALTER TABLE workspace_agent_results
ADD COLUMN result_kind TEXT NOT NULL DEFAULT 'recommendation';

ALTER TABLE workspace_agent_results
ADD COLUMN structured_changes_json TEXT NOT NULL DEFAULT '[]';

ALTER TABLE workspace_agent_results
ADD COLUMN rationale_summary TEXT NOT NULL DEFAULT '';

INSERT INTO workspace_agent_runs (
    id,
    packet_id,
    client_id,
    note_id,
    base_note_revision,
    context_envelope_sha256,
    run_kind,
    idempotency_key,
    provider,
    model,
    source_thread_id,
    source_turn_id,
    status,
    error_summary,
    started_at_ms,
    completed_at_ms,
    created_at_ms,
    updated_at_ms
)
SELECT
    'legacy-result:' || result.id,
    result.packet_id,
    result.client_id,
    result.note_id,
    packet.base_note_revision,
    packet.context_envelope_sha256,
    'legacy_import',
    'legacy-result:' || result.id,
    '',
    '',
    NULL,
    result.id,
    'completed',
    '',
    result.created_at_ms,
    result.created_at_ms,
    result.created_at_ms,
    result.updated_at_ms
FROM workspace_agent_results AS result
JOIN workspace_context_packets AS packet ON packet.id = result.packet_id;

UPDATE workspace_agent_results
SET
    run_id = 'legacy-result:' || id,
    base_note_revision = (
        SELECT packet.base_note_revision
        FROM workspace_context_packets AS packet
        WHERE packet.id = workspace_agent_results.packet_id
    ),
    packet_context_sha256 = (
        SELECT packet.context_envelope_sha256
        FROM workspace_context_packets AS packet
        WHERE packet.id = workspace_agent_results.packet_id
    );

CREATE UNIQUE INDEX idx_workspace_agent_results_run
ON workspace_agent_results(run_id)
WHERE run_id IS NOT NULL;

ALTER TABLE workspace_note_proposals
ADD COLUMN agent_result_id TEXT REFERENCES workspace_agent_results(id) ON DELETE SET NULL;

UPDATE workspace_note_proposals
SET agent_result_id = source_turn_id
WHERE source_turn_id IS NOT NULL
  AND EXISTS (
      SELECT 1
      FROM workspace_agent_results AS result
      WHERE result.id = workspace_note_proposals.source_turn_id
        AND result.note_id = workspace_note_proposals.note_id
  );

CREATE UNIQUE INDEX idx_workspace_note_proposals_agent_result
ON workspace_note_proposals(agent_result_id)
WHERE agent_result_id IS NOT NULL;

CREATE TABLE workspace_note_proposal_decisions (
    id TEXT PRIMARY KEY,
    proposal_id TEXT NOT NULL,
    agent_result_id TEXT,
    note_id TEXT NOT NULL,
    base_revision INTEGER NOT NULL,
    decision_kind TEXT NOT NULL,
    change_id TEXT,
    applied_text TEXT,
    applied_text_sha256 TEXT,
    resulting_note_revision INTEGER,
    actor TEXT NOT NULL,
    reason TEXT NOT NULL DEFAULT '',
    created_at_ms INTEGER NOT NULL,
    FOREIGN KEY(proposal_id) REFERENCES workspace_note_proposals(id) ON DELETE CASCADE,
    FOREIGN KEY(agent_result_id) REFERENCES workspace_agent_results(id) ON DELETE SET NULL,
    FOREIGN KEY(note_id) REFERENCES workspace_notes(id) ON DELETE CASCADE
);

CREATE INDEX idx_workspace_note_proposal_decisions_proposal_created_at
ON workspace_note_proposal_decisions(proposal_id, created_at_ms ASC);

CREATE INDEX idx_workspace_note_proposal_decisions_note_created_at
ON workspace_note_proposal_decisions(note_id, created_at_ms DESC);

INSERT INTO workspace_note_proposal_decisions (
    id,
    proposal_id,
    agent_result_id,
    note_id,
    base_revision,
    decision_kind,
    change_id,
    applied_text,
    applied_text_sha256,
    resulting_note_revision,
    actor,
    reason,
    created_at_ms
)
SELECT
    'legacy-decision:' || proposal.id,
    proposal.id,
    proposal.agent_result_id,
    proposal.note_id,
    proposal.base_revision,
    CASE proposal.status
        WHEN 'accepted' THEN 'accepted_all'
        ELSE 'rejected_all'
    END,
    NULL,
    CASE proposal.status
        WHEN 'accepted' THEN proposal.proposed_body
        ELSE NULL
    END,
    NULL,
    CASE proposal.status
        WHEN 'accepted' THEN proposal.base_revision + 1
        ELSE NULL
    END,
    COALESCE(
        (
            SELECT audit.actor
            FROM workspace_audit_events AS audit
            WHERE audit.entity_type = 'note_proposal'
              AND audit.entity_id = proposal.id
              AND audit.action = proposal.status
            ORDER BY audit.created_at_ms DESC
            LIMIT 1
        ),
        'unknown'
    ),
    'backfilled from legacy proposal status',
    COALESCE(proposal.resolved_at_ms, proposal.created_at_ms)
FROM workspace_note_proposals AS proposal
WHERE proposal.status IN ('accepted', 'declined');
