ALTER TABLE workspace_context_packets
ADD COLUMN workspace_plan_revision_id TEXT
    REFERENCES workspace_plan_revisions(id) ON DELETE RESTRICT
    CHECK(workspace_plan_revision_id IS NULL OR length(trim(workspace_plan_revision_id)) > 0);

ALTER TABLE workspace_context_packets
ADD COLUMN workspace_plan_content_sha256 TEXT
    CHECK(workspace_plan_content_sha256 IS NULL OR (
        length(workspace_plan_content_sha256) = 64
        AND workspace_plan_content_sha256 NOT GLOB '*[^0-9a-f]*'
    ));

ALTER TABLE workspace_context_packets
ADD COLUMN workspace_plan_evidence_manifest_sha256 TEXT
    CHECK(workspace_plan_evidence_manifest_sha256 IS NULL OR (
        length(workspace_plan_evidence_manifest_sha256) = 64
        AND workspace_plan_evidence_manifest_sha256 NOT GLOB '*[^0-9a-f]*'
    ));

CREATE INDEX idx_workspace_context_packets_plan_revision
ON workspace_context_packets(workspace_plan_revision_id, created_at_ms DESC);

CREATE TRIGGER workspace_context_packets_plan_binding_complete_insert
BEFORE INSERT ON workspace_context_packets
WHEN NOT (
    (NEW.workspace_plan_revision_id IS NULL
        AND NEW.workspace_plan_content_sha256 IS NULL
        AND NEW.workspace_plan_evidence_manifest_sha256 IS NULL)
    OR
    (NEW.workspace_plan_revision_id IS NOT NULL
        AND NEW.workspace_plan_content_sha256 IS NOT NULL
        AND NEW.workspace_plan_evidence_manifest_sha256 IS NOT NULL)
)
BEGIN
    SELECT RAISE(ABORT, 'workspace context packet plan binding must be complete');
END;

CREATE TRIGGER workspace_context_packets_plan_binding_immutable
BEFORE UPDATE OF
    workspace_plan_revision_id,
    workspace_plan_content_sha256,
    workspace_plan_evidence_manifest_sha256
ON workspace_context_packets
BEGIN
    SELECT RAISE(ABORT, 'workspace context packet plan binding is immutable');
END;

ALTER TABLE workspace_agent_runs
ADD COLUMN workspace_plan_revision_id TEXT
    REFERENCES workspace_plan_revisions(id) ON DELETE RESTRICT
    CHECK(workspace_plan_revision_id IS NULL OR length(trim(workspace_plan_revision_id)) > 0);

ALTER TABLE workspace_agent_runs
ADD COLUMN workspace_plan_content_sha256 TEXT
    CHECK(workspace_plan_content_sha256 IS NULL OR (
        length(workspace_plan_content_sha256) = 64
        AND workspace_plan_content_sha256 NOT GLOB '*[^0-9a-f]*'
    ));

ALTER TABLE workspace_agent_runs
ADD COLUMN workspace_plan_evidence_manifest_sha256 TEXT
    CHECK(workspace_plan_evidence_manifest_sha256 IS NULL OR (
        length(workspace_plan_evidence_manifest_sha256) = 64
        AND workspace_plan_evidence_manifest_sha256 NOT GLOB '*[^0-9a-f]*'
    ));

CREATE INDEX idx_workspace_agent_runs_plan_revision
ON workspace_agent_runs(workspace_plan_revision_id, created_at_ms DESC);

CREATE TRIGGER workspace_agent_runs_plan_binding_complete_insert
BEFORE INSERT ON workspace_agent_runs
WHEN NOT (
    (NEW.workspace_plan_revision_id IS NULL
        AND NEW.workspace_plan_content_sha256 IS NULL
        AND NEW.workspace_plan_evidence_manifest_sha256 IS NULL)
    OR
    (NEW.workspace_plan_revision_id IS NOT NULL
        AND NEW.workspace_plan_content_sha256 IS NOT NULL
        AND NEW.workspace_plan_evidence_manifest_sha256 IS NOT NULL)
)
BEGIN
    SELECT RAISE(ABORT, 'workspace agent run plan binding must be complete');
END;

CREATE TRIGGER workspace_agent_runs_plan_binding_matches_packet_insert
BEFORE INSERT ON workspace_agent_runs
WHEN NOT EXISTS (
    SELECT 1
    FROM workspace_context_packets AS packet
    WHERE packet.id = NEW.packet_id
      AND packet.workspace_plan_revision_id IS NEW.workspace_plan_revision_id
      AND packet.workspace_plan_content_sha256 IS NEW.workspace_plan_content_sha256
      AND packet.workspace_plan_evidence_manifest_sha256
          IS NEW.workspace_plan_evidence_manifest_sha256
)
BEGIN
    SELECT RAISE(ABORT, 'workspace agent run plan binding must match its packet');
END;

CREATE TRIGGER workspace_agent_runs_plan_binding_immutable
BEFORE UPDATE OF
    workspace_plan_revision_id,
    workspace_plan_content_sha256,
    workspace_plan_evidence_manifest_sha256
ON workspace_agent_runs
BEGIN
    SELECT RAISE(ABORT, 'workspace agent run plan binding is immutable');
END;
