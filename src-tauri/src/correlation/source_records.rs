//! Immutable, workspace-local retention for source records admitted by a
//! signal adapter.
//!
//! The adapter indexes a source record; it does not replace the record with a
//! normalized message.  This module owns the small append-only ledger used by
//! the in-memory replay path and the corresponding SQLite representation.

use chrono::DateTime;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thalassa_domain::{
    CorrelationError, EvidenceRef, EvidenceSourceKind, ResourceScope, SourceRecordRef,
};
use thalassa_policy::{DataClass, EgressDestination, EgressRequest, PolicyRuntime};
use thiserror::Error;

use super::ReplayableSignalFixture;

/// SQL used by the local migration and by a standalone source-record store.
///
/// The expression indexes treat an absent revision as one stable empty value,
/// which gives the optional wire field the same uniqueness semantics as the
/// source-record identity used by the adapter.
pub const SOURCE_RECORDS_TABLE_SQL: &str = r#"
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

CREATE TABLE IF NOT EXISTS source_record_evidence (
    evidence_id TEXT PRIMARY KEY,
    evidence_json TEXT NOT NULL
);
"#;

const DEFAULT_RETAINED_AT: &str = "1970-01-01T00:00:00Z";

/// One complete post-policy source record retained for correlation.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SourceRecord {
    pub source_kind: EvidenceSourceKind,
    pub native_id: Option<String>,
    pub revision: Option<String>,
    pub content_digest: String,
    pub scope: ResourceScope,
    pub observed_at: Option<String>,
    pub ingested_at: Option<String>,
    pub redacted_payload: Value,
    pub evidence_ids: Vec<String>,
    pub retained_at: String,
}

impl SourceRecord {
    /// Return the complete structurally faithful post-policy JSON value.
    pub fn payload(&self) -> &Value {
        &self.redacted_payload
    }

    /// Serialize the retained value for a local storage/evidence boundary.
    pub fn redacted_payload_json(&self) -> String {
        serde_json::to_string(&self.redacted_payload)
            .expect("a serde_json::Value always serializes")
    }
}

/// Descriptive alias for callers that want to distinguish a retained row from
/// the `SourceRecordRef` carried by a Signal.
pub type RetainedSourceRecord = SourceRecord;

/// Local input accepted by [`SourceRecordStore::retain`].
#[derive(Clone, Debug, PartialEq)]
pub struct SourceRecordInput {
    pub source_kind: EvidenceSourceKind,
    pub native_id: Option<String>,
    pub revision: Option<String>,
    pub scope: ResourceScope,
    pub recorded_json: Value,
    pub observed_at: Option<String>,
    pub ingested_at: Option<String>,
    pub evidence: Vec<EvidenceRef>,
}

impl SourceRecordInput {
    pub fn new(
        source_kind: EvidenceSourceKind,
        native_id: Option<String>,
        revision: Option<String>,
        scope: ResourceScope,
        recorded_json: Value,
        evidence: Vec<EvidenceRef>,
    ) -> Self {
        Self {
            source_kind,
            native_id,
            revision,
            scope,
            recorded_json,
            observed_at: None,
            ingested_at: None,
            evidence,
        }
    }

    /// Build an input from an already admitted replay fixture.
    pub fn from_fixture(
        fixture: &ReplayableSignalFixture,
        native_id: Option<String>,
        revision: Option<String>,
    ) -> Self {
        Self {
            source_kind: fixture.source_kind,
            native_id,
            revision,
            scope: fixture.scope.clone(),
            recorded_json: fixture.recorded_json.clone(),
            observed_at: fixture.observed_at.clone(),
            ingested_at: fixture.ingested_at.clone(),
            evidence: fixture.evidence.clone(),
        }
    }

    pub fn with_times(mut self, observed_at: Option<String>, ingested_at: Option<String>) -> Self {
        self.observed_at = observed_at;
        self.ingested_at = ingested_at;
        self
    }

    pub fn with_evidence(mut self, evidence: Vec<EvidenceRef>) -> Self {
        self.evidence = evidence;
        self
    }
}

impl From<&ReplayableSignalFixture> for SourceRecordInput {
    fn from(fixture: &ReplayableSignalFixture) -> Self {
        Self::from_fixture(fixture, None, None)
    }
}

impl From<ReplayableSignalFixture> for SourceRecordInput {
    fn from(fixture: ReplayableSignalFixture) -> Self {
        Self::from(&fixture)
    }
}

/// Typed failures for source admission.  Error details intentionally never
/// include provider payloads, credentials or raw provider error strings.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum SourceRecordError {
    #[error("source record scope is invalid")]
    InvalidScope,
    #[error("source record is outside the current scope")]
    ScopeMismatch,
    #[error("source record evidence is missing")]
    EvidenceMissing,
    #[error("source record evidence is invalid")]
    InvalidEvidence,
    #[error("source record evidence source does not match the record")]
    SourceMismatch,
    #[error("source record contains a duplicate evidence ID")]
    DuplicateEvidence,
    #[error("source record identity is empty or unsafe")]
    UnsafeIdentity,
    #[error("source record timestamp is invalid")]
    InvalidTimestamp,
    #[error("source record payload must be an object or array")]
    InvalidPayload,
    #[error("source record identity conflicts with a retained revision")]
    AmbiguousSourceIdentity,
    #[error("local source-record policy denied retention")]
    PolicyDenied,
    #[error("correlation contract rejected the source record")]
    Contract(#[source] CorrelationError),
    #[error("local source-record database operation failed")]
    Database(String),
}

type RecordKey = (EvidenceSourceKind, String, Option<String>);
type NativeKey = (EvidenceSourceKind, String, Option<String>);

/// Append-only source-record ledger.
///
/// The default constructor is deterministic and in-memory, which is useful
/// for replay fixtures and tests.  `with_connection` additionally mirrors new
/// rows into the local SQLite table; no network or provider operation is ever
/// performed by this type.
pub struct SourceRecordStore {
    records: BTreeMap<RecordKey, SourceRecord>,
    native_index: BTreeMap<NativeKey, String>,
    evidence: BTreeMap<String, EvidenceRef>,
    scope: Option<ResourceScope>,
    policy: PolicyRuntime,
    connection: Option<Connection>,
}

impl Default for SourceRecordStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceRecordStore {
    pub fn new() -> Self {
        Self {
            records: BTreeMap::new(),
            native_index: BTreeMap::new(),
            evidence: BTreeMap::new(),
            scope: None,
            policy: PolicyRuntime::baseline(),
            connection: None,
        }
    }

    /// Construct a store that admits only records contained by `scope`.
    pub fn with_scope(scope: ResourceScope) -> Self {
        let mut store = Self::new();
        store.scope = Some(scope);
        store
    }

    pub fn with_policy(policy: PolicyRuntime) -> Self {
        let mut store = Self::new();
        store.policy = policy;
        store
    }

    pub fn scoped(scope: ResourceScope, policy: PolicyRuntime) -> Self {
        let mut store = Self::with_scope(scope);
        store.policy = policy;
        store
    }

    /// Open a store over an existing local SQLite connection.
    ///
    /// The constructor loads retained rows into the same append-only index so
    /// duplicate replay and native-identity conflict checks also work after a
    /// process restart.  Callers normally run the app migration first; the
    /// idempotent table DDL here keeps the standalone helper safe for tests.
    pub fn with_connection(connection: Connection) -> Result<Self, SourceRecordError> {
        Self::with_connection_and_policy(connection, PolicyRuntime::baseline())
    }

    pub fn with_connection_and_policy(
        connection: Connection,
        policy: PolicyRuntime,
    ) -> Result<Self, SourceRecordError> {
        connection
            .execute_batch(SOURCE_RECORDS_TABLE_SQL)
            .map_err(database_error)?;
        let mut store = Self::with_policy(policy);
        store.load_connection(&connection)?;
        store.connection = Some(connection);
        Ok(store)
    }

    /// Open a store over a SQLite connection while restricting loaded records
    /// and evidence to one workspace scope.
    pub fn with_connection_and_scope_and_policy(
        connection: Connection,
        scope: ResourceScope,
        policy: PolicyRuntime,
    ) -> Result<Self, SourceRecordError> {
        connection
            .execute_batch(SOURCE_RECORDS_TABLE_SQL)
            .map_err(database_error)?;
        let mut store = Self::scoped(scope, policy);
        store.load_connection(&connection)?;
        store.connection = Some(connection);
        Ok(store)
    }

    pub fn with_connection_and_scope(
        connection: Connection,
        scope: ResourceScope,
    ) -> Result<Self, SourceRecordError> {
        Self::with_connection_and_scope_and_policy(connection, scope, PolicyRuntime::baseline())
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn records(&self) -> impl Iterator<Item = &SourceRecord> {
        self.records.values()
    }

    pub fn all(&self) -> Vec<SourceRecord> {
        self.records.values().cloned().collect()
    }

    pub fn get(&self, reference: &SourceRecordRef) -> Option<&SourceRecord> {
        self.records
            .get(&(
                reference.source_kind,
                reference.content_digest.clone(),
                reference.revision.clone(),
            ))
            .filter(|record| record.native_id == reference.native_id)
            .filter(|record| {
                self.scope
                    .as_ref()
                    .map(|scope| scope.contains(&record.scope))
                    .unwrap_or(true)
            })
    }

    pub fn lookup(&self, reference: &SourceRecordRef) -> Option<&SourceRecord> {
        self.get(reference)
    }

    pub fn payload_for(&self, reference: &SourceRecordRef) -> Option<&Value> {
        self.get(reference).map(SourceRecord::payload)
    }

    pub fn evidence(&self, id: &str) -> Option<&EvidenceRef> {
        self.evidence.get(id).filter(|evidence| {
            self.scope
                .as_ref()
                .map(|scope| scope.contains(&evidence.scope))
                .unwrap_or(true)
        })
    }

    pub fn evidence_refs(&self) -> impl Iterator<Item = &EvidenceRef> {
        self.evidence.values()
    }

    pub fn get_evidence(&self, id: &str) -> Option<&EvidenceRef> {
        self.evidence(id)
    }

    pub fn evidence_for_record(
        &self,
        reference: &SourceRecordRef,
    ) -> Result<Vec<EvidenceRef>, SourceRecordError> {
        let record = self
            .get(reference)
            .ok_or(SourceRecordError::EvidenceMissing)?;
        self.evidence_for(&record.evidence_ids)
    }

    pub fn evidence_for(&self, ids: &[String]) -> Result<Vec<EvidenceRef>, SourceRecordError> {
        if ids.is_empty() {
            return Err(SourceRecordError::EvidenceMissing);
        }
        let mut result = Vec::with_capacity(ids.len());
        let mut seen = BTreeSet::new();
        for id in ids {
            if !seen.insert(id) {
                return Err(SourceRecordError::DuplicateEvidence);
            }
            let evidence = self
                .evidence
                .get(id)
                .ok_or(SourceRecordError::EvidenceMissing)?;
            if self
                .scope
                .as_ref()
                .is_some_and(|scope| !scope.contains(&evidence.scope))
            {
                return Err(SourceRecordError::ScopeMismatch);
            }
            result.push(evidence.clone());
        }
        Ok(result)
    }

    /// Retain a complete post-policy source record and return its source-only
    /// bridge for a normalized Signal.
    pub fn retain<I>(&mut self, input: I) -> Result<SourceRecordRef, SourceRecordError>
    where
        I: Into<SourceRecordInput>,
    {
        let input = input.into();
        let (prepared, prepared_evidence) = self.prepare(input)?;
        if prepared_evidence.iter().any(|evidence| {
            self.evidence
                .get(&evidence.id)
                .is_some_and(|existing| existing != evidence)
        }) {
            return Err(SourceRecordError::DuplicateEvidence);
        }
        let key = (
            prepared.source_kind,
            prepared.content_digest.clone(),
            prepared.revision.clone(),
        );

        if let Some(existing) = self.records.get(&key) {
            if existing.native_id != prepared.native_id {
                return Err(SourceRecordError::AmbiguousSourceIdentity);
            }
            if existing.scope != prepared.scope {
                return Err(SourceRecordError::ScopeMismatch);
            }
        }

        if let Some(native_id) = prepared.native_id.as_deref() {
            let native_key = (
                prepared.source_kind,
                native_id.to_owned(),
                prepared.revision.clone(),
            );
            if let Some(existing_digest) = self.native_index.get(&native_key) {
                if existing_digest != &prepared.content_digest {
                    return Err(SourceRecordError::AmbiguousSourceIdentity);
                }
            }
        }

        let merged = self.records.get(&key).map(|existing| {
            let mut merged = existing.clone();
            let mut evidence_ids = merged.evidence_ids.iter().cloned().collect::<BTreeSet<_>>();
            evidence_ids.extend(prepared.evidence_ids.iter().cloned());
            merged.evidence_ids = evidence_ids.into_iter().collect();
            merged
        });
        let persisted = merged.as_ref().unwrap_or(&prepared);
        self.persist(persisted, &prepared_evidence)?;
        if let Some(existing) = self.records.get_mut(&key) {
            // A replay never replaces the retained JSON.  Its evidence IDs
            // are unioned so every repeated source reference remains usable.
            existing.evidence_ids = persisted.evidence_ids.clone();
            for evidence in prepared_evidence {
                self.evidence.entry(evidence.id.clone()).or_insert(evidence);
            }
            return Ok(source_record_ref(existing));
        }

        if let Some(native_id) = prepared.native_id.as_deref() {
            self.native_index.insert(
                (
                    prepared.source_kind,
                    native_id.to_owned(),
                    prepared.revision.clone(),
                ),
                prepared.content_digest.clone(),
            );
        }
        for evidence in &prepared_evidence {
            self.evidence.insert(evidence.id.clone(), evidence.clone());
        }
        let reference = source_record_ref(&prepared);
        self.records.insert(key, prepared);
        Ok(reference)
    }

    pub fn retain_fixture(
        &mut self,
        fixture: &ReplayableSignalFixture,
        native_id: Option<String>,
        revision: Option<String>,
    ) -> Result<SourceRecordRef, SourceRecordError> {
        fixture
            .validate_for_replay()
            .map_err(SourceRecordError::Contract)?;
        self.retain(SourceRecordInput::from_fixture(
            fixture, native_id, revision,
        ))
    }

    /// Compute the same stable digest used for a retained source payload.
    ///
    /// Change admission uses this before retaining the row so the evidence ID
    /// can be minted deterministically and then checked by the existing
    /// source-record evidence store.  The helper applies the established
    /// masking semantics without mutating the caller's value.
    pub(crate) fn content_digest_for(value: &Value) -> String {
        let mut redacted = value.clone();
        mask_source_value(&mut redacted);
        content_digest(&redacted)
    }

    /// Persist one change payload in Sprint 14's append-only table.
    ///
    /// Evidence remains owned by [`SourceRecordStore::retain`].  This method
    /// only writes the change payload row and deliberately uses INSERT OR
    /// IGNORE: a replay can never update or delete a previously retained row.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn persist_change_record(
        &mut self,
        content_digest: &str,
        source_kind: EvidenceSourceKind,
        native_id: Option<&str>,
        revision: Option<&str>,
        occurred_at: &str,
        admitted_at: &str,
        body: &Value,
    ) -> Result<(), SourceRecordError> {
        let Some(connection) = self.connection.as_mut() else {
            return Ok(());
        };
        let body_json = serde_json::to_string(body).map_err(database_serde_error)?;
        let transaction = connection.transaction().map_err(database_error)?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO change_source_record (content_digest, source_kind, native_id, revision, occurred_at, admitted_at, body) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    content_digest,
                    source_kind_wire(source_kind),
                    native_id,
                    revision,
                    occurred_at,
                    admitted_at,
                    body_json,
                ],
            )
            .map_err(database_error)?;
        transaction.commit().map_err(database_error)
    }

    fn prepare(
        &self,
        input: SourceRecordInput,
    ) -> Result<(SourceRecord, Vec<EvidenceRef>), SourceRecordError> {
        if !input.scope.is_bounded() {
            return Err(SourceRecordError::InvalidScope);
        }
        if let Some(scope) = &self.scope {
            if !scope.contains(&input.scope) {
                return Err(SourceRecordError::ScopeMismatch);
            }
        }
        if !self
            .policy
            .evaluate_egress(EgressRequest::verified(
                DataClass::Internal,
                EgressDestination::LocalStorage,
            ))
            .is_allowed()
        {
            return Err(SourceRecordError::PolicyDenied);
        }
        validate_optional_timestamp(input.observed_at.as_deref())?;
        validate_optional_timestamp(input.ingested_at.as_deref())?;
        validate_optional_identity(input.native_id.as_deref())?;
        validate_optional_identity(input.revision.as_deref())?;
        if !input.recorded_json.is_object() && !input.recorded_json.is_array() {
            return Err(SourceRecordError::InvalidPayload);
        }
        let evidence_ids = validate_evidence(&input.evidence, &input.scope, input.source_kind)?;

        let mut redacted_payload = input.recorded_json;
        mask_source_value(&mut redacted_payload);
        if contains_forbidden_payload_data(&redacted_payload) {
            return Err(SourceRecordError::InvalidPayload);
        }
        let content_digest = content_digest(&redacted_payload);
        let retained_at = input
            .ingested_at
            .clone()
            .or_else(|| input.observed_at.clone())
            .unwrap_or_else(|| DEFAULT_RETAINED_AT.to_owned());
        Ok((
            SourceRecord {
                source_kind: input.source_kind,
                native_id: input.native_id,
                revision: input.revision,
                content_digest,
                scope: input.scope,
                observed_at: input.observed_at,
                ingested_at: input.ingested_at,
                redacted_payload,
                evidence_ids,
                retained_at,
            },
            input.evidence,
        ))
    }

    fn persist(
        &mut self,
        record: &SourceRecord,
        evidence: &[EvidenceRef],
    ) -> Result<(), SourceRecordError> {
        let Some(connection) = self.connection.as_mut() else {
            return Ok(());
        };
        let source_kind = source_kind_wire(record.source_kind);
        let scope_json = serde_json::to_string(&record.scope).map_err(database_serde_error)?;
        let evidence_ids_json =
            serde_json::to_string(&record.evidence_ids).map_err(database_serde_error)?;
        let payload_json =
            serde_json::to_string(&record.redacted_payload).map_err(database_serde_error)?;
        let existing_native: Option<String> = if let Some(native_id) = &record.native_id {
            connection
                .query_row(
                    "SELECT content_digest FROM source_records WHERE source_kind = ?1 AND native_id = ?2 AND COALESCE(revision, '') = COALESCE(?3, '')",
                    params![source_kind, native_id, record.revision],
                    |row| row.get(0),
                )
                .optional()
                .map_err(database_error)?
        } else {
            None
        };
        if existing_native.is_some_and(|digest| digest != record.content_digest) {
            return Err(SourceRecordError::AmbiguousSourceIdentity);
        }
        let existing_scope_json: Option<String> = connection
            .query_row(
                "SELECT scope FROM source_records WHERE source_kind = ?1 AND content_digest = ?2 AND COALESCE(revision, '') = COALESCE(?3, '')",
                params![source_kind, record.content_digest, record.revision],
                |row| row.get(0),
            )
            .optional()
            .map_err(database_error)?;
        if let Some(existing_scope_json) = existing_scope_json {
            let existing_scope = serde_json::from_str::<ResourceScope>(&existing_scope_json)
                .map_err(database_serde_error)?;
            if existing_scope != record.scope {
                // The identity index intentionally excludes scope, so an
                // already-retained row must never be updated from another
                // workspace or scope when this scoped store did not load it.
                return Err(SourceRecordError::ScopeMismatch);
            }
        }
        let transaction = connection.transaction().map_err(database_error)?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO source_records (source_kind, native_id, revision, content_digest, scope, observed_at, ingested_at, redacted_payload_json, evidence_ids, retained_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    source_kind,
                    record.native_id,
                    record.revision,
                    record.content_digest,
                    scope_json,
                    record.observed_at,
                    record.ingested_at,
                    payload_json,
                    evidence_ids_json,
                    record.retained_at,
                ],
            )
            .map_err(database_error)?;
        for item in evidence {
            let evidence_json = serde_json::to_string(item).map_err(database_serde_error)?;
            let existing_json: Option<String> = transaction
                .query_row(
                    "SELECT evidence_json FROM source_record_evidence WHERE evidence_id = ?1",
                    [item.id.as_str()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(database_error)?;
            if let Some(existing_json) = existing_json {
                let existing = serde_json::from_str::<EvidenceRef>(&existing_json)
                    .map_err(database_serde_error)?;
                if existing.id != item.id || existing != *item {
                    return Err(SourceRecordError::DuplicateEvidence);
                }
            }
            transaction
                .execute(
                    "INSERT OR IGNORE INTO source_record_evidence (evidence_id, evidence_json) VALUES (?1, ?2)",
                    params![item.id, evidence_json],
                )
                .map_err(database_error)?;
        }
        transaction
            .execute(
                "UPDATE source_records SET evidence_ids = ?1 WHERE source_kind = ?2 AND content_digest = ?3 AND COALESCE(revision, '') = COALESCE(?4, '')",
                params![evidence_ids_json, source_kind, record.content_digest, record.revision],
            )
            .map_err(database_error)?;
        transaction.commit().map_err(database_error)?;
        Ok(())
    }

    fn load_connection(&mut self, connection: &Connection) -> Result<(), SourceRecordError> {
        let mut evidence_statement = connection
            .prepare(
                "SELECT evidence_id, evidence_json FROM source_record_evidence ORDER BY evidence_id",
            )
            .map_err(database_error)?;
        let evidence_rows = evidence_statement
            .query_map([], |row| {
                let id = row.get::<_, String>(0)?;
                let evidence = parse_json::<EvidenceRef>(&row.get::<_, String>(1)?)?;
                Ok((id, evidence))
            })
            .map_err(database_error)?;
        for row in evidence_rows {
            let (id, evidence) = row.map_err(database_error)?;
            if id != evidence.id {
                return Err(SourceRecordError::Database(
                    "local source-record evidence identity mismatch".into(),
                ));
            }
            if self
                .scope
                .as_ref()
                .is_some_and(|scope| !scope.contains(&evidence.scope))
            {
                continue;
            }
            if !evidence.scope.is_bounded() {
                return Err(SourceRecordError::InvalidScope);
            }
            validate_evidence(
                std::slice::from_ref(&evidence),
                &evidence.scope,
                evidence.source_kind,
            )?;
            self.evidence.insert(id, evidence);
        }
        let mut statement = connection
            .prepare("SELECT source_kind, native_id, revision, content_digest, scope, observed_at, ingested_at, redacted_payload_json, evidence_ids, retained_at FROM source_records ORDER BY source_kind, content_digest, COALESCE(revision, '')")
            .map_err(database_error)?;
        let rows = statement
            .query_map([], |row| {
                let source_kind = parse_source_kind(&row.get::<_, String>(0)?)?;
                let native_id = row.get(1)?;
                let revision = row.get(2)?;
                let content_digest = row.get(3)?;
                let scope = parse_json::<ResourceScope>(&row.get::<_, String>(4)?)?;
                let observed_at = row.get(5)?;
                let ingested_at = row.get(6)?;
                let redacted_payload = parse_json::<Value>(&row.get::<_, String>(7)?)?;
                let evidence_ids = parse_json::<Vec<String>>(&row.get::<_, String>(8)?)?;
                let retained_at = row.get(9)?;
                Ok(SourceRecord {
                    source_kind,
                    native_id,
                    revision,
                    content_digest,
                    scope,
                    observed_at,
                    ingested_at,
                    redacted_payload,
                    evidence_ids,
                    retained_at,
                })
            })
            .map_err(database_error)?;
        for row in rows {
            let record = row.map_err(database_error)?;
            if self
                .scope
                .as_ref()
                .is_some_and(|scope| !scope.contains(&record.scope))
            {
                continue;
            }
            validate_loaded_record(&record, &self.evidence)?;
            // Migration 0003 stored only evidence IDs.  Keep such a source
            // row for identity/conflict checks, but leave its evidence
            // unresolved until the source is replayed after migration 0004.
            // Callers still receive `EvidenceMissing` rather than fabricated
            // evidence when they try to resolve the old reference.
            let key = (
                record.source_kind,
                record.content_digest.clone(),
                record.revision.clone(),
            );
            if let Some(native_id) = &record.native_id {
                self.native_index.insert(
                    (
                        record.source_kind,
                        native_id.clone(),
                        record.revision.clone(),
                    ),
                    record.content_digest.clone(),
                );
            }
            self.records.insert(key, record);
        }
        Ok(())
    }
}

/// Validate rows read from SQLite before they can participate in a new
/// snapshot.  The local database is durable input, not a reason to bypass the
/// same redaction, identity and evidence checks used for fresh replay data.
/// Legacy rows from migration 0003 may refer to evidence that is no longer in
/// the companion table; those IDs remain unresolved and surface as
/// `EvidenceMissing` when a caller asks to resolve them.
fn validate_loaded_record(
    record: &SourceRecord,
    evidence: &BTreeMap<String, EvidenceRef>,
) -> Result<(), SourceRecordError> {
    if !record.scope.is_bounded() {
        return Err(SourceRecordError::InvalidScope);
    }
    validate_optional_timestamp(record.observed_at.as_deref())?;
    validate_optional_timestamp(record.ingested_at.as_deref())?;
    validate_optional_timestamp(Some(record.retained_at.as_str()))?;
    validate_optional_identity(record.native_id.as_deref())?;
    validate_optional_identity(record.revision.as_deref())?;
    validate_identity(&record.content_digest)?;
    if !record.redacted_payload.is_object() && !record.redacted_payload.is_array() {
        return Err(SourceRecordError::InvalidPayload);
    }
    if contains_forbidden_payload_data(&record.redacted_payload) {
        return Err(SourceRecordError::InvalidPayload);
    }
    if content_digest(&record.redacted_payload) != record.content_digest {
        return Err(SourceRecordError::Database(
            "local source-record content digest mismatch".into(),
        ));
    }
    let mut evidence_ids = BTreeSet::new();
    for evidence_id in &record.evidence_ids {
        validate_identity(evidence_id)?;
        if !evidence_ids.insert(evidence_id) {
            return Err(SourceRecordError::DuplicateEvidence);
        }
        let Some(item) = evidence.get(evidence_id) else {
            continue;
        };
        if item.source_kind != record.source_kind {
            return Err(SourceRecordError::SourceMismatch);
        }
        if !record.scope.contains(&item.scope) {
            return Err(SourceRecordError::ScopeMismatch);
        }
    }
    Ok(())
}

fn source_record_ref(record: &SourceRecord) -> SourceRecordRef {
    SourceRecordRef {
        source_kind: record.source_kind,
        native_id: record.native_id.clone(),
        revision: record.revision.clone(),
        content_digest: record.content_digest.clone(),
        evidence_ids: record.evidence_ids.clone(),
    }
}

fn validate_evidence(
    evidence: &[EvidenceRef],
    scope: &ResourceScope,
    source_kind: EvidenceSourceKind,
) -> Result<Vec<String>, SourceRecordError> {
    if evidence.is_empty() {
        return Err(SourceRecordError::EvidenceMissing);
    }
    let mut ids = BTreeSet::new();
    for item in evidence {
        if item.source_kind != source_kind {
            return Err(SourceRecordError::SourceMismatch);
        }
        if !scope.contains(&item.scope) {
            return Err(SourceRecordError::ScopeMismatch);
        }
        if !item.redaction.classification_verified
            || !item.redaction.redaction_verified
            || (item.redaction.unparsed && item.redaction.masked)
        {
            return Err(SourceRecordError::InvalidEvidence);
        }
        validate_identity(&item.id)?;
        validate_evidence_text(&item.connector_id)?;
        if item.endpoint.trim().is_empty() {
            return Err(SourceRecordError::InvalidEvidence);
        }
        validate_evidence_text(&Some(item.endpoint.clone()))?;
        validate_evidence_text(&item.query)?;
        DateTime::parse_from_rfc3339(&item.observed_at)
            .map_err(|_| SourceRecordError::InvalidEvidence)?;
        validate_evidence_text(&Some(item.observed_at.clone()))?;
        validate_evidence_text(&Some(item.excerpt.clone()))?;
        validate_evidence_text(&item.native_url)?;
        if !ids.insert(item.id.clone()) {
            return Err(SourceRecordError::DuplicateEvidence);
        }
    }
    Ok(ids.into_iter().collect())
}

fn validate_evidence_text(value: &Option<String>) -> Result<(), SourceRecordError> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.chars().any(char::is_control) || contains_forbidden_text(value) {
        return Err(SourceRecordError::InvalidEvidence);
    }
    Ok(())
}

fn validate_optional_timestamp(value: Option<&str>) -> Result<(), SourceRecordError> {
    if let Some(value) = value {
        DateTime::parse_from_rfc3339(value).map_err(|_| SourceRecordError::InvalidTimestamp)?;
    }
    Ok(())
}

fn validate_optional_identity(value: Option<&str>) -> Result<(), SourceRecordError> {
    if let Some(value) = value {
        validate_identity(value)?;
    }
    Ok(())
}

fn validate_identity(value: &str) -> Result<(), SourceRecordError> {
    if value.trim().is_empty()
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        || contains_forbidden_text(value)
    {
        return Err(SourceRecordError::UnsafeIdentity);
    }
    Ok(())
}

fn contains_forbidden_text(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "password",
        "passwd",
        "secret",
        "token",
        "credential",
        "authorization",
        "cookie",
        "bearer",
        "api_key",
        "access_key",
        "private_key",
        "arn:",
        "/subscriptions/",
        "subscription_id",
        "account_id",
        "pagination",
        "cursor",
        "next_link",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
        || contains_sensitive_account_id(&lower)
}

fn contains_sensitive_account_id(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if lower.contains("sha256:") || lower.contains("dedup:v1:") || lower.contains("candidate:v1:") {
        return false;
    }
    if looks_like_uuid(value) {
        return false;
    }
    let mut run_length = 0usize;
    for character in value.chars() {
        if character.is_ascii_digit() {
            run_length = run_length.saturating_add(1);
        } else {
            if run_length >= 12 {
                return true;
            }
            run_length = 0;
        }
    }
    run_length >= 12
}

fn looks_like_uuid(value: &str) -> bool {
    let parts = value.split('-').collect::<Vec<_>>();
    parts.len() == 5
        && [8, 4, 4, 4, 12]
            .iter()
            .zip(parts.iter())
            .all(|(length, part)| {
                part.len() == *length && part.chars().all(|c| c.is_ascii_hexdigit())
            })
}

fn mask_source_value(value: &mut Value) {
    match value {
        Value::Object(object) => {
            // Reuse the established recursive masking semantics first.
            crate::observability::masking::mask_json_object(object);
            for (key, value) in object.iter_mut() {
                let lower = key.to_ascii_lowercase();
                if ["authorization", "cookie", "private_key"]
                    .iter()
                    .any(|marker| lower.contains(marker))
                {
                    *value = Value::String(crate::observability::masking::REDACTED.into());
                } else {
                    mask_source_value(value);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                mask_source_value(value);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn contains_forbidden_payload_data(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            let lower = key.to_ascii_lowercase();
            let key_is_forbidden = [
                "account",
                "subscription",
                "pagination",
                "cursor",
                "next_link",
                "raw_error",
                "provider_error",
                "error",
            ]
            .iter()
            .any(|marker| lower.contains(marker));
            (key_is_forbidden
                && !matches!(value, Value::String(masked) if masked == crate::observability::masking::REDACTED))
                || contains_forbidden_payload_data(value)
        }),
        Value::Array(values) => values.iter().any(contains_forbidden_payload_data),
        Value::String(value) => contains_forbidden_text(value),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut entries = object.iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(right.0));
            let mut canonical = Map::new();
            for (key, value) in entries {
                canonical.insert(key.clone(), canonical_json(value));
            }
            Value::Object(canonical)
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        value => value.clone(),
    }
}

fn content_digest(value: &Value) -> String {
    let canonical = canonical_json(value);
    let encoded = serde_json::to_vec(&canonical).expect("JSON values are serializable");
    let digest = Sha256::digest(encoded);
    format!("sha256:{digest:x}")
}

fn source_kind_wire(source_kind: EvidenceSourceKind) -> &'static str {
    match source_kind {
        EvidenceSourceKind::Alertmanager => "alertmanager",
        EvidenceSourceKind::Prometheus => "prometheus",
        EvidenceSourceKind::Kubernetes => "kubernetes",
        EvidenceSourceKind::Cloud => "cloud",
        EvidenceSourceKind::HealthCheck => "health_check",
        EvidenceSourceKind::Fixture => "fixture",
        EvidenceSourceKind::Trivy => "trivy",
        EvidenceSourceKind::Falco => "falco",
        EvidenceSourceKind::Kyverno => "kyverno",
        EvidenceSourceKind::OpaGatekeeper => "opa_gatekeeper",
        EvidenceSourceKind::GitHub => "github",
        EvidenceSourceKind::GitLab => "gitlab",
        EvidenceSourceKind::ArgoCd => "argo_cd",
    }
}

fn parse_source_kind(value: &str) -> rusqlite::Result<EvidenceSourceKind> {
    serde_json::from_str(&format!("\"{value}\"")).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn parse_json<T: serde::de::DeserializeOwned>(value: &str) -> rusqlite::Result<T> {
    serde_json::from_str(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn database_error(error: rusqlite::Error) -> SourceRecordError {
    SourceRecordError::Database(error.to_string())
}

fn database_serde_error(error: serde_json::Error) -> SourceRecordError {
    SourceRecordError::Database(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use thalassa_domain::{EvidenceRedaction, EvidenceRef};
    use uuid::Uuid;

    fn scope() -> ResourceScope {
        ResourceScope::workspace(Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3))
    }

    fn evidence(id: &str) -> EvidenceRef {
        EvidenceRef {
            id: id.into(),
            source_kind: EvidenceSourceKind::Alertmanager,
            connector_id: None,
            scope: scope(),
            endpoint: "fixture://test".into(),
            query: None,
            observed_at: "2026-08-28T09:00:00Z".into(),
            excerpt: "synthetic evidence".into(),
            native_url: None,
            redaction: EvidenceRedaction {
                classification_verified: true,
                redaction_verified: true,
                masked: false,
                unparsed: false,
            },
        }
    }

    #[test]
    fn canonical_digest_ignores_object_insertion_order() {
        let first = SourceRecordInput::new(
            EvidenceSourceKind::Alertmanager,
            Some("alert".into()),
            None,
            scope(),
            json!({"b": 2, "a": 1}),
            vec![evidence("evidence-a")],
        );
        let second = SourceRecordInput::new(
            EvidenceSourceKind::Alertmanager,
            Some("alert".into()),
            None,
            scope(),
            json!({"a": 1, "b": 2}),
            vec![evidence("evidence-b")],
        );
        let mut store = SourceRecordStore::new();
        let first_ref = store.retain(first).unwrap();
        let second_ref = store.retain(second).unwrap();
        assert_eq!(first_ref.content_digest, second_ref.content_digest);
        assert_eq!(store.len(), 1);
    }
}
