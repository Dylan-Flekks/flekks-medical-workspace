CREATE TABLE workspace_client_contacts (
    client_id TEXT PRIMARY KEY,
    primary_phone TEXT,
    secondary_phone TEXT,
    email TEXT,
    preferred_contact_method TEXT,
    emergency_contact_name TEXT,
    emergency_contact_relationship TEXT,
    emergency_contact_phone TEXT,
    emergency_contact_email TEXT,
    contact_notes TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE
);

CREATE TABLE workspace_client_coverages (
    client_id TEXT PRIMARY KEY,
    payer_name TEXT,
    plan_name TEXT,
    member_id TEXT,
    group_number TEXT,
    coverage_type TEXT,
    coverage_status TEXT,
    coverage_notes TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE
);
