CREATE TABLE workspace_data_policy (
    singleton_id INTEGER PRIMARY KEY NOT NULL,
    schema_version INTEGER NOT NULL,
    data_classification TEXT NOT NULL,
    classified_at_ms INTEGER,
    classified_by TEXT,
    CHECK(singleton_id = 1),
    CHECK(schema_version = 1),
    CHECK(data_classification IN ('unclassified', 'synthetic')),
    CHECK(
        (data_classification = 'unclassified' AND classified_at_ms IS NULL
            AND classified_by IS NULL)
        OR (data_classification = 'synthetic' AND classified_at_ms IS NOT NULL
            AND classified_at_ms >= 0 AND classified_by IS NOT NULL
            AND classified_by = trim(classified_by) AND length(classified_by) > 0
            AND length(CAST(classified_by AS BLOB)) <= 256)
    )
);

INSERT INTO workspace_data_policy (
    singleton_id, schema_version, data_classification, classified_at_ms, classified_by
) VALUES (1, 1, 'unclassified', NULL, NULL);

CREATE TRIGGER workspace_data_policy_reject_insert
BEFORE INSERT ON workspace_data_policy
BEGIN
    SELECT RAISE(ABORT, 'workspace data policy singleton cannot be inserted or replaced');
END;

CREATE TRIGGER workspace_data_policy_reject_delete
BEFORE DELETE ON workspace_data_policy
BEGIN
    SELECT RAISE(ABORT, 'workspace data policy singleton cannot be deleted or replaced');
END;

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
    AND NOT EXISTS (SELECT 1 FROM workspace_patient_safety_items)
    AND NOT EXISTS (SELECT 1 FROM workspace_agent_runs)
    AND NOT EXISTS (SELECT 1 FROM workspace_agent_run_sources)
    AND NOT EXISTS (SELECT 1 FROM workspace_note_proposal_decisions)
    AND NOT EXISTS (SELECT 1 FROM workspace_chart_commits)
    AND NOT EXISTS (SELECT 1 FROM workspace_draft_sessions)
    AND NOT EXISTS (SELECT 1 FROM workspace_draft_checkpoints)
    AND NOT EXISTS (SELECT 1 FROM workspace_guide_runs)
)
BEGIN
    SELECT RAISE(ABORT, 'workspace data policy only permits unclassified to synthetic');
END;
