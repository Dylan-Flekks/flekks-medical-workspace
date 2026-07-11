-- Migrations 0018 and 0019 are supplied by the packet checkpoint and global draft list changes.
ALTER TABLE workspace_draft_sessions ADD COLUMN session_creation_key TEXT;

CREATE UNIQUE INDEX idx_workspace_draft_sessions_client_creation_key
ON workspace_draft_sessions(client_id, session_creation_key)
WHERE session_creation_key IS NOT NULL;
