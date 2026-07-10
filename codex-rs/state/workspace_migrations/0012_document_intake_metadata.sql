ALTER TABLE workspace_documents
ADD COLUMN original_path TEXT NOT NULL DEFAULT '';

ALTER TABLE workspace_documents
ADD COLUMN reference_kind TEXT NOT NULL DEFAULT 'local_reference';

ALTER TABLE workspace_documents
ADD COLUMN vault_path TEXT NOT NULL DEFAULT '';

ALTER TABLE workspace_documents
ADD COLUMN content_sha256 TEXT;

ALTER TABLE workspace_documents
ADD COLUMN thumbnail_path TEXT NOT NULL DEFAULT '';

ALTER TABLE workspace_documents
ADD COLUMN thumbnail_status TEXT NOT NULL DEFAULT 'none';

ALTER TABLE workspace_documents
ADD COLUMN thumbnail_mime_type TEXT;

ALTER TABLE workspace_documents
ADD COLUMN intake_source TEXT NOT NULL DEFAULT '';

ALTER TABLE workspace_documents
ADD COLUMN imported_at_ms INTEGER;

UPDATE workspace_documents
SET original_path = local_path
WHERE original_path = '';
