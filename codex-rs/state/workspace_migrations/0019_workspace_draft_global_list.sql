-- Migration 0018 is reserved by the checkpoint-bound context packet change.
CREATE INDEX idx_workspace_draft_sessions_status_updated
ON workspace_draft_sessions(status, updated_at_ms DESC, id DESC);
