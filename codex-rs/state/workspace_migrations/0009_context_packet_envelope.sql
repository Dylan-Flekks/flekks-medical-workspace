ALTER TABLE workspace_context_packets
ADD COLUMN context_envelope_json TEXT NOT NULL DEFAULT '{}';
