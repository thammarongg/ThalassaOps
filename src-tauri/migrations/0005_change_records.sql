CREATE TABLE IF NOT EXISTS change_source_record (
    content_digest TEXT PRIMARY KEY,
    source_kind    TEXT NOT NULL,
    native_id      TEXT,
    revision       TEXT,
    occurred_at    TEXT NOT NULL,
    admitted_at    TEXT NOT NULL,
    body           TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS change_source_record_occurred_at
    ON change_source_record (occurred_at);
