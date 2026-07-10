CREATE TABLE workspace_patient_safety_items (
    id TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    category TEXT NOT NULL CHECK(category IN ('allergy', 'medication', 'condition', 'precaution')),
    name TEXT NOT NULL,
    reaction TEXT,
    severity TEXT,
    dose TEXT,
    route TEXT,
    frequency TEXT,
    status TEXT,
    recorded_date TEXT,
    notes TEXT NOT NULL DEFAULT '',
    archived_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    FOREIGN KEY(client_id) REFERENCES workspace_clients(id) ON DELETE CASCADE
);

CREATE INDEX idx_workspace_patient_safety_items_client_category
ON workspace_patient_safety_items(client_id, category, archived_at_ms, updated_at_ms DESC);
