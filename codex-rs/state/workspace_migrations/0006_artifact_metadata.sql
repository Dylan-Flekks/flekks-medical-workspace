ALTER TABLE workspace_documents
ADD COLUMN scope TEXT NOT NULL DEFAULT 'patient';

ALTER TABLE workspace_documents
ADD COLUMN detected_kind TEXT NOT NULL DEFAULT '';

ALTER TABLE workspace_documents
ADD COLUMN mime_type TEXT;

ALTER TABLE workspace_documents
ADD COLUMN file_size_bytes INTEGER;

ALTER TABLE workspace_documents
ADD COLUMN modified_at_ms INTEGER;

ALTER TABLE workspace_documents
ADD COLUMN sha256 TEXT;

ALTER TABLE workspace_documents
ADD COLUMN tags TEXT NOT NULL DEFAULT '';

ALTER TABLE workspace_documents
ADD COLUMN source_label TEXT NOT NULL DEFAULT '';

ALTER TABLE workspace_documents
ADD COLUMN existence_status TEXT NOT NULL DEFAULT 'unknown';

ALTER TABLE workspace_documents
ADD COLUMN metadata_json TEXT NOT NULL DEFAULT '{}';
