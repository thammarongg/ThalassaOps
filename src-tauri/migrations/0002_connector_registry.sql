CREATE TABLE IF NOT EXISTS connector_instances (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    display_name TEXT NOT NULL,
    enabled INTEGER NOT NULL,
    config_metadata_json TEXT NOT NULL,
    credential_reference TEXT,
    health_state TEXT NOT NULL,
    last_checked_at TEXT,
    last_successful_sync_at TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS connector_test_logs (
    id TEXT PRIMARY KEY,
    connector_id TEXT NOT NULL REFERENCES connector_instances(id) ON DELETE CASCADE,
    checked_at TEXT NOT NULL,
    outcome TEXT NOT NULL,
    message TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS connector_test_logs_connector_checked_idx
    ON connector_test_logs (connector_id, checked_at DESC);
