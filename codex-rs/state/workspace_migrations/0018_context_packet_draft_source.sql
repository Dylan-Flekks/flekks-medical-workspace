ALTER TABLE workspace_context_packets ADD COLUMN source_draft_session_id TEXT;
ALTER TABLE workspace_context_packets ADD COLUMN source_draft_checkpoint_id TEXT;
ALTER TABLE workspace_context_packets ADD COLUMN source_draft_checkpoint_revision INTEGER;
ALTER TABLE workspace_context_packets ADD COLUMN source_draft_checkpoint_sha256 TEXT;

CREATE UNIQUE INDEX idx_workspace_context_packets_source_draft_checkpoint
ON workspace_context_packets(source_draft_checkpoint_id)
WHERE source_draft_checkpoint_id IS NOT NULL;

CREATE TRIGGER workspace_context_packets_validate_draft_source_insert
BEFORE INSERT ON workspace_context_packets
WHEN
    (NEW.source_draft_session_id IS NULL) != (NEW.source_draft_checkpoint_id IS NULL)
    OR (NEW.source_draft_session_id IS NULL) != (NEW.source_draft_checkpoint_revision IS NULL)
    OR (NEW.source_draft_session_id IS NULL) != (NEW.source_draft_checkpoint_sha256 IS NULL)
    OR (
        NEW.source_draft_session_id IS NOT NULL
        AND (
            length(trim(NEW.source_draft_session_id)) = 0
            OR length(trim(NEW.source_draft_checkpoint_id)) = 0
            OR NEW.source_draft_checkpoint_revision < 1
            OR length(NEW.source_draft_checkpoint_sha256) != 64
            OR NEW.source_draft_checkpoint_sha256 GLOB '*[^0-9a-f]*'
        )
    )
BEGIN
    SELECT RAISE(ABORT, 'invalid context packet draft source');
END;

CREATE TRIGGER workspace_context_packets_draft_source_is_immutable
BEFORE UPDATE ON workspace_context_packets
WHEN
    OLD.source_draft_session_id IS NOT NEW.source_draft_session_id
    OR OLD.source_draft_checkpoint_id IS NOT NEW.source_draft_checkpoint_id
    OR OLD.source_draft_checkpoint_revision IS NOT NEW.source_draft_checkpoint_revision
    OR OLD.source_draft_checkpoint_sha256 IS NOT NEW.source_draft_checkpoint_sha256
BEGIN
    SELECT RAISE(ABORT, 'context packet draft source is immutable');
END;
