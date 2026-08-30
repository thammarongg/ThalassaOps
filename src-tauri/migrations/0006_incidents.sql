-- Sprint 15 incident write model: current state, provenance, responder roles
-- and an append-only audit timeline.
--
-- Deviation from the plan's schema block: `create_request_fingerprint` is
-- stored on `incident`.  Idempotent creation must tell an identical replay
-- (return the stored incident) from a reused request ID carrying different
-- command content (reject).  The fingerprint is a digest of the canonical
-- creation command and cannot be recomputed from stored state, so it is
-- persisted beside the request ID it belongs to.

CREATE TABLE IF NOT EXISTS incident (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    team_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    scope_json TEXT NOT NULL,
    summary TEXT NOT NULL,
    business_impact_json TEXT NOT NULL,
    severity TEXT NOT NULL,
    derived_severity TEXT NOT NULL,
    severity_override_json TEXT,
    status TEXT NOT NULL,
    disposition TEXT,
    duplicate_of_incident_id TEXT,
    signal_ids_json TEXT NOT NULL,
    evidence_ids_json TEXT NOT NULL,
    hypothesis_ids_json TEXT NOT NULL,
    action_ids_json TEXT NOT NULL,
    version INTEGER NOT NULL CHECK (version > 0),
    create_request_id TEXT NOT NULL UNIQUE,
    create_request_fingerprint TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS incident_workspace_updated_idx
    ON incident (workspace_id, updated_at DESC, id);

CREATE TABLE IF NOT EXISTS incident_trigger (
    id TEXT PRIMARY KEY,
    incident_id TEXT NOT NULL REFERENCES incident(id),
    source_kind TEXT NOT NULL,
    source_id TEXT NOT NULL,
    source_record_digest TEXT,
    scope_json TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    signal_id TEXT,
    evidence_ids_json TEXT NOT NULL,
    report_json TEXT,
    UNIQUE (incident_id, source_kind, source_id)
);

CREATE TABLE IF NOT EXISTS incident_role_assignment (
    id TEXT PRIMARY KEY,
    incident_id TEXT NOT NULL REFERENCES incident(id),
    role TEXT NOT NULL,
    principal_id TEXT NOT NULL,
    assigned_by TEXT NOT NULL,
    assigned_at TEXT NOT NULL,
    released_by TEXT,
    released_at TEXT
);
CREATE INDEX IF NOT EXISTS incident_active_role_idx
    ON incident_role_assignment (incident_id, role, released_at);
CREATE UNIQUE INDEX IF NOT EXISTS incident_one_active_exclusive_role
    ON incident_role_assignment (incident_id, role)
    WHERE released_at IS NULL AND role <> 'stakeholder';
CREATE UNIQUE INDEX IF NOT EXISTS incident_one_active_stakeholder
    ON incident_role_assignment (incident_id, role, principal_id)
    WHERE released_at IS NULL AND role = 'stakeholder';

CREATE TABLE IF NOT EXISTS incident_timeline_event (
    id TEXT PRIMARY KEY,
    incident_id TEXT NOT NULL REFERENCES incident(id),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    event_kind TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    reason TEXT,
    occurred_at TEXT NOT NULL,
    request_id TEXT NOT NULL,
    policy_version INTEGER NOT NULL,
    payload_json TEXT NOT NULL,
    UNIQUE (incident_id, sequence),
    UNIQUE (incident_id, request_id, event_kind)
);
CREATE TRIGGER IF NOT EXISTS incident_timeline_no_update
BEFORE UPDATE ON incident_timeline_event BEGIN SELECT RAISE(ABORT, 'incident timeline is append-only'); END;
CREATE TRIGGER IF NOT EXISTS incident_timeline_no_delete
BEFORE DELETE ON incident_timeline_event BEGIN SELECT RAISE(ABORT, 'incident timeline is append-only'); END;
