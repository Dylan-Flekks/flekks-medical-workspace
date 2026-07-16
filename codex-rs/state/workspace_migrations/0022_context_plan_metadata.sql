ALTER TABLE workspace_context_packets
ADD COLUMN workspace_profile TEXT NOT NULL DEFAULT 'medical';

ALTER TABLE workspace_context_packets
ADD COLUMN plan_schema_version INTEGER NOT NULL DEFAULT 1;

ALTER TABLE workspace_context_packets
ADD COLUMN source_checkpoint_id TEXT;

ALTER TABLE workspace_context_packets
ADD COLUMN source_checkpoint_sha256 TEXT;

ALTER TABLE workspace_context_packets
ADD COLUMN readiness_json TEXT NOT NULL DEFAULT '{"version":1,"warnings":[],"acknowledgements":[],"legacy":true}';

CREATE INDEX idx_workspace_context_packets_source_checkpoint
ON workspace_context_packets(source_checkpoint_id);
