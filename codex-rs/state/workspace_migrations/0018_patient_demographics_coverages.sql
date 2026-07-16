ALTER TABLE workspace_clients ADD COLUMN legal_first_name TEXT;
ALTER TABLE workspace_clients ADD COLUMN legal_middle_name TEXT;
ALTER TABLE workspace_clients ADD COLUMN legal_last_name TEXT;
ALTER TABLE workspace_clients ADD COLUMN legal_suffix TEXT;
ALTER TABLE workspace_clients ADD COLUMN previous_name TEXT;
ALTER TABLE workspace_clients ADD COLUMN administrative_sex TEXT;
ALTER TABLE workspace_clients ADD COLUMN preferred_language TEXT;
ALTER TABLE workspace_clients ADD COLUMN interpreter_required INTEGER NOT NULL DEFAULT 0
    CHECK (interpreter_required IN (0, 1));

ALTER TABLE workspace_client_contacts ADD COLUMN primary_phone_use TEXT;
ALTER TABLE workspace_client_contacts ADD COLUMN secondary_phone_use TEXT;
ALTER TABLE workspace_client_contacts ADD COLUMN secondary_email TEXT;
ALTER TABLE workspace_client_contacts ADD COLUMN address_line_1 TEXT;
ALTER TABLE workspace_client_contacts ADD COLUMN address_line_2 TEXT;
ALTER TABLE workspace_client_contacts ADD COLUMN city TEXT;
ALTER TABLE workspace_client_contacts ADD COLUMN state_or_province TEXT;
ALTER TABLE workspace_client_contacts ADD COLUMN postal_code TEXT;
ALTER TABLE workspace_client_contacts ADD COLUMN country TEXT;
ALTER TABLE workspace_client_contacts ADD COLUMN address_use TEXT;

CREATE TABLE workspace_coverages (
    id TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    priority INTEGER NOT NULL CHECK (priority BETWEEN 1 AND 3),
    payer_name TEXT,
    plan_name TEXT,
    member_id TEXT,
    group_number TEXT,
    coverage_type TEXT,
    coverage_status TEXT,
    effective_date TEXT,
    termination_date TEXT,
    patient_relationship_to_subscriber TEXT,
    subscriber_first_name TEXT,
    subscriber_middle_name TEXT,
    subscriber_last_name TEXT,
    subscriber_suffix TEXT,
    subscriber_date_of_birth TEXT,
    subscriber_administrative_sex TEXT,
    subscriber_address_same_as_patient INTEGER NOT NULL DEFAULT 1
        CHECK (subscriber_address_same_as_patient IN (0, 1)),
    subscriber_address_line_1 TEXT,
    subscriber_address_line_2 TEXT,
    subscriber_city TEXT,
    subscriber_state_or_province TEXT,
    subscriber_postal_code TEXT,
    subscriber_country TEXT,
    coverage_notes TEXT,
    source_kind TEXT NOT NULL DEFAULT 'structured'
        CHECK (source_kind IN ('legacy_projection', 'structured')),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    UNIQUE(client_id, priority),
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE RESTRICT
);

INSERT INTO workspace_coverages (
    id, client_id, priority, payer_name, plan_name, member_id, group_number,
    coverage_type, coverage_status, coverage_notes, source_kind, created_at_ms,
    updated_at_ms
)
SELECT
    'legacy-primary:' || client_id, client_id, 1, payer_name, plan_name,
    member_id, group_number, coverage_type, coverage_status, coverage_notes,
    'legacy_projection', created_at_ms, updated_at_ms
FROM workspace_client_coverages;

CREATE INDEX workspace_coverages_client_priority_idx
    ON workspace_coverages(client_id, priority);

CREATE TRIGGER workspace_coverages_no_delete
BEFORE DELETE ON workspace_coverages
BEGIN
    SELECT RAISE(ABORT, 'workspace coverage records must not be deleted');
END;

CREATE TRIGGER workspace_client_coverages_insert_projection
AFTER INSERT ON workspace_client_coverages
BEGIN
    INSERT INTO workspace_coverages (
        id, client_id, priority, payer_name, plan_name, member_id, group_number,
        coverage_type, coverage_status, coverage_notes, source_kind,
        created_at_ms, updated_at_ms
    ) VALUES (
        'legacy-primary:' || NEW.client_id, NEW.client_id, 1, NEW.payer_name,
        NEW.plan_name, NEW.member_id, NEW.group_number, NEW.coverage_type,
        NEW.coverage_status, NEW.coverage_notes, 'legacy_projection',
        NEW.created_at_ms, NEW.updated_at_ms
    )
    ON CONFLICT(client_id, priority) DO UPDATE SET
        payer_name = excluded.payer_name,
        plan_name = excluded.plan_name,
        member_id = excluded.member_id,
        group_number = excluded.group_number,
        coverage_type = excluded.coverage_type,
        coverage_status = excluded.coverage_status,
        coverage_notes = excluded.coverage_notes,
        updated_at_ms = excluded.updated_at_ms
    WHERE workspace_coverages.source_kind = 'legacy_projection';
END;

CREATE TRIGGER workspace_client_coverages_update_projection
AFTER UPDATE ON workspace_client_coverages
BEGIN
    UPDATE workspace_coverages
    SET payer_name = NEW.payer_name,
        plan_name = NEW.plan_name,
        member_id = NEW.member_id,
        group_number = NEW.group_number,
        coverage_type = NEW.coverage_type,
        coverage_status = NEW.coverage_status,
        coverage_notes = NEW.coverage_notes,
        updated_at_ms = NEW.updated_at_ms
    WHERE client_id = NEW.client_id
      AND priority = 1
      AND source_kind = 'legacy_projection';
END;

CREATE TRIGGER workspace_client_coverages_delete_projection
AFTER DELETE ON workspace_client_coverages
BEGIN
    UPDATE workspace_coverages
    SET payer_name = NULL,
        plan_name = NULL,
        member_id = NULL,
        group_number = NULL,
        coverage_type = NULL,
        coverage_status = 'inactive',
        coverage_notes = NULL,
        updated_at_ms = OLD.updated_at_ms
    WHERE client_id = OLD.client_id
      AND priority = 1
      AND source_kind = 'legacy_projection';
END;

CREATE TABLE workspace_coverage_card_verifications (
    id TEXT PRIMARY KEY,
    coverage_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
    source_document_id TEXT NOT NULL,
    source_document_version TEXT NOT NULL,
    source_document_content_sha256 TEXT NOT NULL,
    compared_subject TEXT NOT NULL CHECK (compared_subject IN ('beneficiary', 'subscriber')),
    observed_first_name TEXT,
    observed_middle_name TEXT,
    observed_last_name TEXT,
    observed_suffix TEXT,
    observed_member_id TEXT,
    patient_record_version TEXT NOT NULL,
    patient_version TEXT NOT NULL,
    coverage_version TEXT NOT NULL,
    match_result TEXT NOT NULL CHECK (match_result IN ('match', 'mismatch')),
    mismatch_fields_json TEXT NOT NULL,
    actor TEXT NOT NULL,
    content_sha256 TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    FOREIGN KEY(coverage_id) REFERENCES workspace_coverages(id) ON DELETE RESTRICT,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE RESTRICT,
    FOREIGN KEY(source_document_id) REFERENCES workspace_documents(id) ON DELETE RESTRICT,
    CHECK(length(source_document_version) = 64
        AND source_document_version NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(source_document_content_sha256) = 64
        AND source_document_content_sha256 NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(patient_record_version) = 64
        AND patient_record_version NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(patient_version) = 64
        AND patient_version NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(coverage_version) = 64
        AND coverage_version NOT GLOB '*[^0-9a-f]*'),
    CHECK(length(content_sha256) = 64
        AND content_sha256 NOT GLOB '*[^0-9a-f]*')
);

CREATE INDEX workspace_coverage_card_verifications_history_idx
    ON workspace_coverage_card_verifications(coverage_id, created_at_ms DESC, id DESC);

CREATE TRIGGER workspace_coverage_card_verifications_no_update
BEFORE UPDATE ON workspace_coverage_card_verifications
BEGIN
    SELECT RAISE(ABORT, 'workspace coverage verification history is append-only');
END;

CREATE TRIGGER workspace_coverage_card_verifications_no_delete
BEFORE DELETE ON workspace_coverage_card_verifications
BEGIN
    SELECT RAISE(ABORT, 'workspace coverage verification history is append-only');
END;
