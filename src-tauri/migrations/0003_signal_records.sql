CREATE TABLE IF NOT EXISTS source_records (
    source_kind TEXT NOT NULL,
    native_id TEXT,
    revision TEXT,
    content_digest TEXT NOT NULL,
    scope TEXT NOT NULL,
    observed_at TEXT,
    ingested_at TEXT,
    redacted_payload_json TEXT NOT NULL,
    evidence_ids TEXT NOT NULL,
    retained_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS source_records_identity_idx
    ON source_records (source_kind, content_digest, COALESCE(revision, ''));

CREATE UNIQUE INDEX IF NOT EXISTS source_records_native_identity_idx
    ON source_records (source_kind, native_id, COALESCE(revision, ''))
    WHERE native_id IS NOT NULL;
