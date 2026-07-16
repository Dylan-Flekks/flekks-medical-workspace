CREATE TABLE workspace_plan_submission_receipts (
    plan_revision_id TEXT PRIMARY KEY,
    packet_id TEXT NOT NULL UNIQUE,
    agent_run_id TEXT NOT NULL UNIQUE,
    plan_session_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
    plan_content_sha256 TEXT NOT NULL,
    evidence_manifest_sha256 TEXT NOT NULL,
    submitted_by TEXT NOT NULL,
    submitted_at_ms INTEGER NOT NULL,
    FOREIGN KEY(plan_revision_id) REFERENCES workspace_plan_revisions(id) ON DELETE RESTRICT,
    FOREIGN KEY(packet_id) REFERENCES workspace_context_packets(id) ON DELETE RESTRICT,
    FOREIGN KEY(agent_run_id) REFERENCES workspace_agent_runs(id) ON DELETE RESTRICT,
    FOREIGN KEY(plan_session_id) REFERENCES workspace_plan_sessions(id) ON DELETE RESTRICT,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE RESTRICT,
    CHECK(length(plan_content_sha256) = 64
        AND plan_content_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(evidence_manifest_sha256) = 64
        AND evidence_manifest_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(trim(submitted_by)) > 0),
    CHECK(submitted_at_ms > 0)
);

CREATE TRIGGER workspace_plan_submission_receipts_exact_binding_insert
BEFORE INSERT ON workspace_plan_submission_receipts
WHEN NOT EXISTS (
    SELECT 1
    FROM workspace_plan_revisions AS revision
    JOIN workspace_context_packets AS packet ON packet.id = NEW.packet_id
    JOIN workspace_agent_runs AS run ON run.id = NEW.agent_run_id
    WHERE revision.id = NEW.plan_revision_id
      AND revision.status = 'submitted'
      AND revision.submitted_at_ms = NEW.submitted_at_ms
      AND revision.plan_session_id = NEW.plan_session_id
      AND revision.client_id = NEW.client_id
      AND revision.content_sha256 = NEW.plan_content_sha256
      AND revision.evidence_manifest_sha256 = NEW.evidence_manifest_sha256
      AND packet.client_id = revision.client_id
      AND packet.encounter_id IS revision.encounter_id
      AND packet.note_id IS revision.note_id
      AND packet.workspace_plan_revision_id = revision.id
      AND packet.workspace_plan_content_sha256 = revision.content_sha256
      AND packet.workspace_plan_evidence_manifest_sha256 = revision.evidence_manifest_sha256
      AND packet.status IN ('submitted', 'sent', 'result_saved')
      AND run.packet_id = packet.id
      AND run.client_id = revision.client_id
      AND run.note_id IS revision.note_id
      AND run.workspace_plan_revision_id = revision.id
      AND run.workspace_plan_content_sha256 = revision.content_sha256
      AND run.workspace_plan_evidence_manifest_sha256 = revision.evidence_manifest_sha256
      AND run.run_kind = 'agent'
      AND run.status IN ('running', 'completed')
)
BEGIN
    SELECT RAISE(ABORT, 'workspace plan submission receipt must match one submitted revision packet and run');
END;

INSERT INTO workspace_plan_submission_receipts (
    plan_revision_id, packet_id, agent_run_id, plan_session_id, client_id,
    plan_content_sha256, evidence_manifest_sha256, submitted_by, submitted_at_ms
)
SELECT
    revision.id,
    packet.id,
    run.id,
    revision.plan_session_id,
    revision.client_id,
    revision.content_sha256,
    revision.evidence_manifest_sha256,
    audit.actor,
    revision.submitted_at_ms
FROM workspace_plan_revisions AS revision
JOIN workspace_audit_events AS audit
  ON audit.entity_type = 'plan_revision'
 AND audit.entity_id = revision.id
 AND audit.action = 'submitted'
 AND audit.success = 1
JOIN workspace_context_packets AS packet
  ON packet.id = json_extract(
      CASE WHEN json_valid(audit.metadata_json) THEN audit.metadata_json ELSE '{}' END,
      '$.packetId'
  )
JOIN workspace_agent_runs AS run
  ON run.id = json_extract(
      CASE WHEN json_valid(audit.metadata_json) THEN audit.metadata_json ELSE '{}' END,
      '$.agentRunId'
  )
 AND run.packet_id = packet.id
WHERE revision.status = 'submitted'
  AND revision.submitted_at_ms IS NOT NULL
  AND packet.client_id = revision.client_id
  AND packet.workspace_plan_revision_id = revision.id
  AND packet.workspace_plan_content_sha256 = revision.content_sha256
  AND packet.workspace_plan_evidence_manifest_sha256 = revision.evidence_manifest_sha256
  AND run.client_id = revision.client_id
  AND run.workspace_plan_revision_id = revision.id
  AND run.workspace_plan_content_sha256 = revision.content_sha256
  AND run.workspace_plan_evidence_manifest_sha256 = revision.evidence_manifest_sha256;

CREATE INDEX idx_workspace_plan_submission_receipts_packet_run
ON workspace_plan_submission_receipts(packet_id, agent_run_id);

CREATE TRIGGER workspace_plan_submission_receipts_immutable_update
BEFORE UPDATE ON workspace_plan_submission_receipts
BEGIN
    SELECT RAISE(ABORT, 'workspace plan submission receipt is immutable');
END;

CREATE TRIGGER workspace_plan_submission_receipts_immutable_delete
BEFORE DELETE ON workspace_plan_submission_receipts
BEGIN
    SELECT RAISE(ABORT, 'workspace plan submission receipt is immutable');
END;

DROP TRIGGER workspace_data_policy_restrict_update;

CREATE TRIGGER workspace_data_policy_restrict_update
BEFORE UPDATE ON workspace_data_policy
WHEN NOT (
    OLD.singleton_id = 1
    AND OLD.schema_version = 1
    AND OLD.data_classification = 'unclassified'
    AND OLD.classified_at_ms IS NULL
    AND OLD.classified_by IS NULL
    AND NEW.singleton_id = 1
    AND NEW.schema_version = 1
    AND NEW.data_classification = 'synthetic'
    AND NEW.classified_at_ms IS NOT NULL
    AND NEW.classified_at_ms >= 0
    AND NEW.classified_by IS NOT NULL
    AND NEW.classified_by = trim(NEW.classified_by)
    AND length(NEW.classified_by) > 0
    AND length(CAST(NEW.classified_by AS BLOB)) <= 256
    AND NOT EXISTS (SELECT 1 FROM workspace_clients)
    AND NOT EXISTS (SELECT 1 FROM workspace_notes)
    AND NOT EXISTS (SELECT 1 FROM workspace_note_revisions)
    AND NOT EXISTS (SELECT 1 FROM workspace_note_proposals)
    AND NOT EXISTS (SELECT 1 FROM workspace_audit_events)
    AND NOT EXISTS (SELECT 1 FROM workspace_documents)
    AND NOT EXISTS (SELECT 1 FROM workspace_encounters)
    AND NOT EXISTS (SELECT 1 FROM workspace_note_signatures)
    AND NOT EXISTS (SELECT 1 FROM workspace_note_addenda)
    AND NOT EXISTS (SELECT 1 FROM workspace_tasks)
    AND NOT EXISTS (SELECT 1 FROM workspace_context_packets)
    AND NOT EXISTS (SELECT 1 FROM workspace_agent_results)
    AND NOT EXISTS (SELECT 1 FROM workspace_artifact_derivatives)
    AND NOT EXISTS (SELECT 1 FROM workspace_context_clips)
    AND NOT EXISTS (SELECT 1 FROM workspace_client_contacts)
    AND NOT EXISTS (SELECT 1 FROM workspace_client_coverages)
    AND NOT EXISTS (SELECT 1 FROM workspace_coverages)
    AND NOT EXISTS (SELECT 1 FROM workspace_coverage_card_verifications)
    AND NOT EXISTS (SELECT 1 FROM workspace_patient_safety_items)
    AND NOT EXISTS (SELECT 1 FROM workspace_agent_runs)
    AND NOT EXISTS (SELECT 1 FROM workspace_agent_run_sources)
    AND NOT EXISTS (SELECT 1 FROM workspace_agent_turn_completions)
    AND NOT EXISTS (SELECT 1 FROM workspace_note_proposal_decisions)
    AND NOT EXISTS (SELECT 1 FROM workspace_chart_commits)
    AND NOT EXISTS (SELECT 1 FROM workspace_draft_sessions)
    AND NOT EXISTS (SELECT 1 FROM workspace_draft_checkpoints)
    AND NOT EXISTS (SELECT 1 FROM workspace_guide_runs)
    AND NOT EXISTS (SELECT 1 FROM workspace_plan_sessions)
    AND NOT EXISTS (SELECT 1 FROM workspace_plan_messages)
    AND NOT EXISTS (SELECT 1 FROM workspace_planning_turn_claims)
    AND NOT EXISTS (SELECT 1 FROM workspace_planning_context_reads)
    AND NOT EXISTS (SELECT 1 FROM workspace_plan_revisions)
    AND NOT EXISTS (SELECT 1 FROM workspace_plan_proposals)
    AND NOT EXISTS (SELECT 1 FROM workspace_plan_turn_completions)
    AND NOT EXISTS (SELECT 1 FROM workspace_plan_turn_evidence)
    AND NOT EXISTS (SELECT 1 FROM workspace_plan_submission_receipts)
)
BEGIN
    SELECT RAISE(ABORT, 'workspace data policy only permits unclassified to synthetic');
END;
