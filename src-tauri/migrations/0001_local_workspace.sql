CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS principals (
    id TEXT PRIMARY KEY,
    document_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS organizations (
    id TEXT PRIMARY KEY,
    document_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS teams (
    id TEXT PRIMARY KEY,
    document_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,
    document_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS memberships (
    id TEXT PRIMARY KEY,
    document_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS policy_store (
    id TEXT PRIMARY KEY,
    version INTEGER NOT NULL,
    document_json TEXT NOT NULL,
    migrated_at TEXT NOT NULL
);
