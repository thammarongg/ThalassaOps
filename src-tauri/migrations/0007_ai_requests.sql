-- Sprint 17 AI gateway audit records: request metadata and one row per attempt.
--
-- Deliberate deviation from the design's conceptual schema: the request's
-- budget and typed enum values are stored as JSON so this table can round-trip
-- the frozen domain contract without adding provider-specific columns. No
-- prompt, instruction or completion content is stored in either table.

CREATE TABLE IF NOT EXISTS ai_requests (
    id TEXT PRIMARY KEY,
    principal_id TEXT NOT NULL,
    data_class TEXT NOT NULL,
    declaration TEXT NOT NULL,
    budget_json TEXT NOT NULL,
    finish_reason TEXT,
    policy_version INTEGER NOT NULL CHECK (policy_version > 0),
    started_at TEXT NOT NULL,
    finished_at TEXT,
    error_reason TEXT
);
CREATE INDEX IF NOT EXISTS ai_requests_principal_started_idx
    ON ai_requests (principal_id, started_at DESC, id DESC);

CREATE TABLE IF NOT EXISTS ai_request_attempts (
    request_id TEXT NOT NULL REFERENCES ai_requests(id),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    provider_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    outcome_json TEXT NOT NULL,
    input_tokens INTEGER NOT NULL CHECK (input_tokens >= 0),
    output_tokens INTEGER NOT NULL CHECK (output_tokens >= 0),
    cost_micros INTEGER CHECK (cost_micros IS NULL OR cost_micros >= 0),
    PRIMARY KEY (request_id, ordinal)
);
