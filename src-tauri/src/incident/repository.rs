//! Transactional SQLite persistence for the Incident write model.
//!
//! Current state and the ordered timeline events of one accepted mutation are
//! written in a single immediate transaction: a failure leaves neither partial
//! current state nor orphan audit rows.  Creation and later writes are
//! idempotent on their originating request IDs; a new later write is also
//! optimistic on `expected_version`.  The repository never calls a wall clock —
//! every stored timestamp comes from the aggregate mutation it is given.

use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior};
use thalassa_domain::{
    BusinessImpact, ConsoleEvidenceId, Incident, IncidentDisposition, IncidentEventKind,
    IncidentId, IncidentMutation, IncidentPage, IncidentRole, IncidentRoleAssignment,
    IncidentSeverity, IncidentSeverityOverride, IncidentSourceKind, IncidentStatus,
    IncidentTimelineEvent, IncidentTimelinePage, IncidentTimelinePayload, IncidentTrigger,
    IncidentTriggerId, Membership, PrincipalId, ResourceScope, SignalId, INCIDENT_CURSOR_MAXIMUM,
};
use uuid::Uuid;

const INCIDENT_MIGRATION: &str = include_str!("../../migrations/0006_incidents.sql");

/// Maximum characters accepted for a stored creation fingerprint.
const FINGERPRINT_MAXIMUM: usize = 200;

/// Typed persistence failures.  Messages never carry provider payloads,
/// credentials or report text.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum IncidentStoreError {
    #[error("local incident database operation failed")]
    Database(String),
    #[error("incident serialization failed")]
    Serialization(String),
    #[error("stored incident data is not a valid {0}")]
    Corruption(String),
    #[error("incident was not found in this workspace")]
    NotFound,
    #[error("incident version {expected} is stale; stored version is {actual}")]
    VersionConflict { expected: u64, actual: u64 },
    #[error("the request identifier was reused with different content")]
    IdempotencyConflict,
    #[error("incident event sequence {actual} does not continue the timeline at {expected}")]
    InvalidEventSequence { expected: u64, actual: u64 },
    #[error("incident mutation is not internally consistent: {0}")]
    InvalidMutation(String),
    #[error("incident pagination arguments are invalid")]
    InvalidPagination,
}

fn database_error(error: rusqlite::Error) -> IncidentStoreError {
    IncidentStoreError::Database(error.to_string())
}

fn serialization_error(error: serde_json::Error) -> IncidentStoreError {
    IncidentStoreError::Serialization(error.to_string())
}

fn corruption(kind: &str) -> IncidentStoreError {
    IncidentStoreError::Corruption(kind.into())
}

fn invalid(reason: &str) -> IncidentStoreError {
    IncidentStoreError::InvalidMutation(reason.into())
}

/// One internal creation record.  The repository never receives raw IPC input:
/// the application layer resolves triggers and validates the command first.
///
/// `request_fingerprint` is the lowercase digest of the canonical serialized
/// creation command.  It carries no source payload and no report text; it
/// exists only to tell an identical replay from a reused request identifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncidentCreationRecord {
    pub mutation: IncidentMutation,
    pub triggers: Vec<IncidentTrigger>,
    pub request_fingerprint: String,
}

/// Local incident store.  One instance owns one SQLite connection.
pub struct SqliteIncidentRepository {
    connection: Connection,
}

impl SqliteIncidentRepository {
    /// Opens (creating when absent) the local incident store at `path`.
    ///
    /// The incident schema is applied idempotently so the store is usable
    /// before, during or after the application's own migration pass.
    pub fn open(path: &Path) -> Result<Self, IncidentStoreError> {
        let connection = Connection::open(path).map_err(database_error)?;
        Self::from_connection(connection)
    }

    /// Builds a store over an existing connection, enabling the foreign-key
    /// enforcement the incident schema relies on.
    pub fn from_connection(connection: Connection) -> Result<Self, IncidentStoreError> {
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(database_error)?;
        connection
            .execute_batch(INCIDENT_MIGRATION)
            .map_err(database_error)?;
        Ok(Self { connection })
    }

    /// Persists one explicit creation in a single transaction.
    ///
    /// Replaying the same request identifier with the same fingerprint returns
    /// the stored incident unchanged; reusing it with different content is an
    /// [`IncidentStoreError::IdempotencyConflict`].
    pub fn create(
        &mut self,
        record: IncidentCreationRecord,
    ) -> Result<IncidentMutation, IncidentStoreError> {
        let incident = &record.mutation.incident;
        if incident.version != 1 {
            return Err(invalid("a created incident starts at version 1"));
        }
        validate_events(&record.mutation, 0)?;
        if record.request_fingerprint.trim().is_empty()
            || record.request_fingerprint.chars().count() > FINGERPRINT_MAXIMUM
            || record
                .request_fingerprint
                .chars()
                .any(|character| character.is_control())
        {
            return Err(invalid("creation fingerprint is empty or unbounded"));
        }
        let create_request_id = record
            .mutation
            .events
            .first()
            .map(|event| event.request_id)
            .ok_or_else(|| invalid("creation appends at least one event"))?;
        if record
            .mutation
            .events
            .iter()
            .any(|event| event.request_id != create_request_id)
        {
            return Err(invalid("creation events share one request identifier"));
        }
        let mut stored_trigger_ids: Vec<IncidentTriggerId> =
            record.triggers.iter().map(|trigger| trigger.id).collect();
        stored_trigger_ids.sort();
        stored_trigger_ids.dedup();
        if stored_trigger_ids != incident.trigger_ids {
            return Err(invalid("stored triggers do not match the aggregate"));
        }
        let workspace_id = workspace_of(incident)?;

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;

        let existing: Option<(String, String)> = transaction
            .query_row(
                "SELECT id, create_request_fingerprint FROM incident WHERE create_request_id = ?1",
                [create_request_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(database_error)?;
        if let Some((stored_id, stored_fingerprint)) = existing {
            if stored_fingerprint != record.request_fingerprint {
                return Err(IncidentStoreError::IdempotencyConflict);
            }
            let stored_id = parse_uuid(&stored_id, "incident id")?;
            let incident = load_incident(&transaction, workspace_id, stored_id)?
                .ok_or(IncidentStoreError::NotFound)?;
            let creation_event_limit = i64::try_from(record.mutation.events.len())
                .map_err(|_| invalid("creation event count exceeds the stored integer range"))?;
            let events = load_events_for_request(
                &transaction,
                stored_id,
                create_request_id,
                creation_event_limit,
            )?;
            transaction.commit().map_err(database_error)?;
            return Ok(IncidentMutation { incident, events });
        }

        insert_incident(
            &transaction,
            incident,
            create_request_id,
            &record.request_fingerprint,
        )?;
        for trigger in &record.triggers {
            insert_trigger(&transaction, incident.id, trigger)?;
        }
        for assignment in &incident.roles {
            insert_role(&transaction, incident.id, assignment)?;
        }
        for event in &record.mutation.events {
            insert_event(&transaction, event)?;
        }
        transaction.commit().map_err(database_error)?;
        Ok(record.mutation)
    }

    /// Applies one accepted post-creation mutation under optimistic
    /// concurrency: a matching request replay returns the stored result;
    /// otherwise current state, role reconciliation and appended events all
    /// commit together or not at all.
    pub fn apply_mutation(
        &mut self,
        mutation: IncidentMutation,
    ) -> Result<IncidentMutation, IncidentStoreError> {
        let incident = &mutation.incident;
        let expected_version = incident
            .version
            .checked_sub(1)
            .filter(|version| *version > 0)
            .ok_or_else(|| invalid("a mutation advances an existing version"))?;
        let workspace_id = workspace_of(incident)?;
        let first_event = mutation
            .events
            .first()
            .ok_or_else(|| invalid("a mutation appends at least one event"))?;
        let actor_id = first_event.actor_id;
        let occurred_at = first_event.occurred_at;

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;

        let stored_incident = load_incident(&transaction, workspace_id, incident.id)?
            .ok_or(IncidentStoreError::NotFound)?;
        let replay_limit =
            i64::try_from(
                mutation.events.len().checked_add(1).ok_or_else(|| {
                    invalid("mutation event count exceeds the stored integer range")
                })?,
            )
            .map_err(|_| invalid("mutation event count exceeds the stored integer range"))?;
        let stored_events = load_events_for_request(
            &transaction,
            incident.id,
            first_event.request_id,
            replay_limit,
        )?;
        if !stored_events.is_empty() {
            if stored_events.len() == mutation.events.len()
                && stored_events
                    .iter()
                    .zip(&mutation.events)
                    .all(|(stored, submitted)| event_content_matches(stored, submitted))
            {
                transaction.commit().map_err(database_error)?;
                return Ok(IncidentMutation {
                    incident: stored_incident,
                    events: stored_events,
                });
            }
            return Err(IncidentStoreError::IdempotencyConflict);
        }

        let stored_version = stored_incident.version;
        if stored_version != expected_version {
            return Err(IncidentStoreError::VersionConflict {
                expected: expected_version,
                actual: stored_version,
            });
        }

        let highest: i64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(sequence), 0) FROM incident_timeline_event WHERE incident_id = ?1",
                [incident.id.to_string()],
                |row| row.get(0),
            )
            .map_err(database_error)?;
        let highest = u64::try_from(highest).map_err(|_| corruption("event sequence"))?;
        validate_events(&mutation, highest)?;
        // The service preflights this lookup for a stable workspace-scoped
        // error. Recheck after BEGIN IMMEDIATE: SQLite serializes competing
        // writes, so a concurrent delete cannot invalidate the reference
        // between validation and the current-state update.
        validate_duplicate_reference(&transaction, workspace_id, incident)?;
        validate_role_principals(&transaction, workspace_id, &mutation)?;

        let updated = transaction
            .execute(
                "UPDATE incident SET
                     scope_json = ?1,
                     summary = ?2,
                     business_impact_json = ?3,
                     severity = ?4,
                     derived_severity = ?5,
                     severity_override_json = ?6,
                     status = ?7,
                     disposition = ?8,
                     duplicate_of_incident_id = ?9,
                     signal_ids_json = ?10,
                     evidence_ids_json = ?11,
                     hypothesis_ids_json = ?12,
                     action_ids_json = ?13,
                     version = ?14,
                     updated_at = ?15
                 WHERE id = ?16 AND workspace_id = ?17 AND version = ?18",
                rusqlite::params![
                    to_json(&incident.scope)?,
                    incident.summary,
                    to_json(&incident.business_impact)?,
                    severity_wire(&incident.current_severity()),
                    severity_wire(&incident.derived_severity),
                    optional_json(incident.severity_override.as_ref())?,
                    status_wire(incident.status),
                    incident.disposition.as_ref().map(disposition_wire),
                    incident.duplicate_of_incident_id.map(|id| id.to_string()),
                    to_json(&incident.signal_ids)?,
                    to_json(&incident.evidence_ids)?,
                    to_json(&incident.hypothesis_ids)?,
                    to_json(&incident.action_ids)?,
                    to_i64(incident.version)?,
                    incident.updated_at.to_rfc3339(),
                    incident.id.to_string(),
                    workspace_id.to_string(),
                    to_i64(expected_version)?,
                ],
            )
            .map_err(database_error)?;
        if updated != 1 {
            return Err(IncidentStoreError::VersionConflict {
                expected: expected_version,
                actual: stored_version,
            });
        }

        reconcile_roles(&transaction, incident, actor_id, occurred_at)?;
        for event in &mutation.events {
            insert_event(&transaction, event)?;
        }
        transaction.commit().map_err(database_error)?;
        Ok(mutation)
    }

    /// Loads the current incident and a bounded set of events for a mutation
    /// request.  The service uses this before checking `expected_version`, so
    /// a retry can replay after the first write has advanced the version.
    pub(crate) fn replay_mutation(
        &mut self,
        workspace_id: Uuid,
        incident_id: IncidentId,
        request_id: Uuid,
        event_limit: i64,
    ) -> Result<Option<IncidentMutation>, IncidentStoreError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(database_error)?;
        let incident = load_incident(&transaction, workspace_id, incident_id)?
            .ok_or(IncidentStoreError::NotFound)?;
        let events = load_events_for_request(&transaction, incident_id, request_id, event_limit)?;
        transaction.commit().map_err(database_error)?;
        if events.is_empty() {
            Ok(None)
        } else {
            Ok(Some(IncidentMutation { incident, events }))
        }
    }

    /// Highest stored event sequence for one incident, scoped to the caller's
    /// workspace.  The next accepted mutation numbers its events from here.
    pub fn highest_event_sequence(
        &self,
        workspace_id: Uuid,
        incident_id: IncidentId,
    ) -> Result<u64, IncidentStoreError> {
        let exists: Option<i64> = self
            .connection
            .query_row(
                "SELECT 1 FROM incident WHERE id = ?1 AND workspace_id = ?2",
                [incident_id.to_string(), workspace_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(database_error)?;
        if exists.is_none() {
            return Err(IncidentStoreError::NotFound);
        }
        let highest: i64 = self
            .connection
            .query_row(
                "SELECT COALESCE(MAX(sequence), 0) FROM incident_timeline_event WHERE incident_id = ?1",
                [incident_id.to_string()],
                |row| row.get(0),
            )
            .map_err(database_error)?;
        u64::try_from(highest).map_err(|_| corruption("event sequence"))
    }

    /// Total stored incidents.  Read-only proof that a rejected command, a
    /// replay or a projection wrote nothing.
    pub fn incident_count(&self) -> Result<u64, IncidentStoreError> {
        let count: i64 = self
            .connection
            .query_row("SELECT COUNT(*) FROM incident", [], |row| row.get(0))
            .map_err(database_error)?;
        u64::try_from(count).map_err(|_| corruption("incident count"))
    }

    /// Reads one incident, scoped to the caller's workspace.
    pub fn get(
        &self,
        workspace_id: Uuid,
        incident_id: IncidentId,
    ) -> Result<Incident, IncidentStoreError> {
        load_incident(&self.connection, workspace_id, incident_id)?
            .ok_or(IncidentStoreError::NotFound)
    }

    /// Resolves a principal against the caller's workspace membership.  A
    /// principal in another workspace is intentionally indistinguishable from
    /// an unknown principal to callers.
    pub(crate) fn ensure_principal_in_workspace(
        &self,
        workspace_id: Uuid,
        principal_id: PrincipalId,
    ) -> Result<(), IncidentStoreError> {
        ensure_principal_in_workspace(&self.connection, workspace_id, principal_id)
    }

    /// Reads one bounded page of workspace incidents, newest update first.
    pub fn list(
        &self,
        workspace_id: Uuid,
        cursor: Option<&str>,
        limit: u16,
    ) -> Result<IncidentPage, IncidentStoreError> {
        if limit == 0 || limit > 100 {
            return Err(IncidentStoreError::InvalidPagination);
        }
        let window = i64::from(limit) + 1;
        let mut rows = match cursor {
            None => {
                let mut statement = self
                    .connection
                    .prepare(
                        "SELECT * FROM incident WHERE workspace_id = ?1
                         ORDER BY updated_at DESC, id ASC LIMIT ?2",
                    )
                    .map_err(database_error)?;
                let mapped = statement
                    .query_map(
                        rusqlite::params![workspace_id.to_string(), window],
                        read_incident,
                    )
                    .map_err(database_error)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(database_error)?;
                mapped
            }
            Some(cursor) => {
                let (updated_at, id) = parse_cursor(cursor)?;
                let mut statement = self
                    .connection
                    .prepare(
                        "SELECT * FROM incident WHERE workspace_id = ?1
                           AND (updated_at < ?2 OR (updated_at = ?2 AND id > ?3))
                         ORDER BY updated_at DESC, id ASC LIMIT ?4",
                    )
                    .map_err(database_error)?;
                let mapped = statement
                    .query_map(
                        rusqlite::params![workspace_id.to_string(), updated_at, id, window],
                        read_incident,
                    )
                    .map_err(database_error)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(database_error)?;
                mapped
            }
        };

        let has_more = rows.len() > usize::from(limit);
        if has_more {
            rows.truncate(usize::from(limit));
        }
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            items.push(hydrate_incident(&self.connection, row?)?);
        }
        let next_cursor = if has_more {
            items
                .last()
                .map(|incident| format_cursor(incident.updated_at, incident.id))
        } else {
            None
        };
        Ok(IncidentPage { items, next_cursor })
    }

    /// Reads one bounded, ordered page of an incident's immutable timeline.
    pub fn timeline(
        &self,
        workspace_id: Uuid,
        incident_id: IncidentId,
        after_sequence: Option<u64>,
        limit: u16,
    ) -> Result<IncidentTimelinePage, IncidentStoreError> {
        if limit == 0 || limit > 100 {
            return Err(IncidentStoreError::InvalidPagination);
        }
        if after_sequence == Some(0) {
            return Err(IncidentStoreError::InvalidPagination);
        }
        let exists: Option<i64> = self
            .connection
            .query_row(
                "SELECT 1 FROM incident WHERE id = ?1 AND workspace_id = ?2",
                [incident_id.to_string(), workspace_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(database_error)?;
        if exists.is_none() {
            return Err(IncidentStoreError::NotFound);
        }

        let after = to_i64(after_sequence.unwrap_or(0))?;
        let mut events = load_events(
            &self.connection,
            incident_id,
            after,
            i64::from(limit).saturating_add(1),
        )?;
        let has_more = events.len() > usize::from(limit);
        if has_more {
            events.truncate(usize::from(limit));
        }
        let next_sequence = if has_more {
            events.last().map(|event| event.sequence)
        } else {
            None
        };
        Ok(IncidentTimelinePage {
            incident_id,
            events,
            next_sequence,
        })
    }
}

fn workspace_of(incident: &Incident) -> Result<Uuid, IncidentStoreError> {
    incident
        .scope
        .workspace_id
        .filter(|id| !id.is_nil())
        .ok_or_else(|| invalid("incident scope carries a workspace"))
}

fn validate_duplicate_reference(
    connection: &Connection,
    workspace_id: Uuid,
    incident: &Incident,
) -> Result<(), IncidentStoreError> {
    if incident.disposition != Some(IncidentDisposition::Duplicate) {
        return Ok(());
    }
    let duplicate_of = incident
        .duplicate_of_incident_id
        .ok_or_else(|| invalid("duplicate disposition carries a target incident"))?;
    let exists: Option<i64> = connection
        .query_row(
            "SELECT 1 FROM incident WHERE id = ?1 AND workspace_id = ?2",
            [duplicate_of.to_string(), workspace_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(database_error)?;
    if exists.is_none() {
        return Err(IncidentStoreError::NotFound);
    }
    Ok(())
}

fn ensure_principal_in_workspace(
    connection: &Connection,
    workspace_id: Uuid,
    principal_id: PrincipalId,
) -> Result<(), IncidentStoreError> {
    let principal_exists: Option<i64> = connection
        .query_row(
            "SELECT 1 FROM principals WHERE id = ?1",
            [principal_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(database_error)?;
    if principal_exists.is_none() {
        return Err(IncidentStoreError::NotFound);
    }

    let membership_document: Option<String> = connection
        .query_row(
            "SELECT document_json FROM memberships WHERE id = ?1",
            [principal_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(database_error)?;
    let Some(membership_document) = membership_document else {
        return Err(IncidentStoreError::NotFound);
    };
    let membership: Membership = from_json(&membership_document, "membership")?;
    let workspace_scope = ResourceScope {
        workspace_id: Some(workspace_id),
        ..Default::default()
    };
    if membership.principal_id != principal_id || !membership.grants(&workspace_scope) {
        return Err(IncidentStoreError::NotFound);
    }
    Ok(())
}

fn validate_role_principals(
    connection: &Connection,
    workspace_id: Uuid,
    mutation: &IncidentMutation,
) -> Result<(), IncidentStoreError> {
    for event in &mutation.events {
        if event.kind != IncidentEventKind::RoleChanged {
            continue;
        }
        let IncidentTimelinePayload::RoleChanged(payload) = &event.payload else {
            continue;
        };
        if let Some(principal_id) = payload.current_principal_id {
            ensure_principal_in_workspace(connection, workspace_id, principal_id)?;
        }
    }
    Ok(())
}

/// Every appended event must continue the stored timeline contiguously, belong
/// to this incident and appear exactly once per kind for one request.
fn validate_events(
    mutation: &IncidentMutation,
    highest_stored_sequence: u64,
) -> Result<(), IncidentStoreError> {
    if mutation.events.is_empty() {
        return Err(invalid("a write appends at least one event"));
    }
    let mut expected = highest_stored_sequence
        .checked_add(1)
        .ok_or_else(|| invalid("event sequence overflow"))?;
    let mut seen: Vec<(Uuid, IncidentEventKind)> = Vec::new();
    for event in &mutation.events {
        if event.incident_id != mutation.incident.id {
            return Err(invalid("event belongs to another incident"));
        }
        if event.sequence != expected {
            return Err(IncidentStoreError::InvalidEventSequence {
                expected,
                actual: event.sequence,
            });
        }
        let key = (event.request_id, event.kind);
        if seen.contains(&key) {
            return Err(invalid("one request appends each event kind at most once"));
        }
        seen.push(key);
        expected = expected
            .checked_add(1)
            .ok_or_else(|| invalid("event sequence overflow"))?;
    }
    Ok(())
}

/// Compares the immutable request content of two events while ignoring values
/// regenerated for a retry (the event row ID, sequence and timestamp).
fn event_content_matches(
    stored: &IncidentTimelineEvent,
    submitted: &IncidentTimelineEvent,
) -> bool {
    stored.incident_id == submitted.incident_id
        && stored.kind == submitted.kind
        && stored.actor_id == submitted.actor_id
        && stored.reason == submitted.reason
        && stored.request_id == submitted.request_id
        && stored.policy_version == submitted.policy_version
        && stored.payload == submitted.payload
}

fn insert_incident(
    connection: &Connection,
    incident: &Incident,
    create_request_id: Uuid,
    fingerprint: &str,
) -> Result<(), IncidentStoreError> {
    let organization_id = incident
        .scope
        .organization_id
        .ok_or_else(|| invalid("incident scope carries an organization"))?;
    let team_id = incident
        .scope
        .team_id
        .ok_or_else(|| invalid("incident scope carries a team"))?;
    let workspace_id = workspace_of(incident)?;
    connection
        .execute(
            "INSERT INTO incident (
                 id, organization_id, team_id, workspace_id, scope_json, summary,
                 business_impact_json, severity, derived_severity, severity_override_json,
                 status, disposition, duplicate_of_incident_id, signal_ids_json,
                 evidence_ids_json, hypothesis_ids_json, action_ids_json, version,
                 create_request_id, create_request_fingerprint, created_at, updated_at
             ) VALUES (
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                 ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
             )",
            rusqlite::params![
                incident.id.to_string(),
                organization_id.to_string(),
                team_id.to_string(),
                workspace_id.to_string(),
                to_json(&incident.scope)?,
                incident.summary,
                to_json(&incident.business_impact)?,
                severity_wire(&incident.current_severity()),
                severity_wire(&incident.derived_severity),
                optional_json(incident.severity_override.as_ref())?,
                status_wire(incident.status),
                incident.disposition.as_ref().map(disposition_wire),
                incident.duplicate_of_incident_id.map(|id| id.to_string()),
                to_json(&incident.signal_ids)?,
                to_json(&incident.evidence_ids)?,
                to_json(&incident.hypothesis_ids)?,
                to_json(&incident.action_ids)?,
                to_i64(incident.version)?,
                create_request_id.to_string(),
                fingerprint,
                incident.created_at.to_rfc3339(),
                incident.updated_at.to_rfc3339(),
            ],
        )
        .map_err(database_error)?;
    Ok(())
}

fn insert_trigger(
    connection: &Connection,
    incident_id: IncidentId,
    trigger: &IncidentTrigger,
) -> Result<(), IncidentStoreError> {
    connection
        .execute(
            "INSERT INTO incident_trigger (
                 id, incident_id, source_kind, source_id, source_record_digest,
                 scope_json, observed_at, signal_id, evidence_ids_json, report_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                trigger.id.to_string(),
                incident_id.to_string(),
                source_kind_wire(trigger.source_kind),
                trigger.source_id,
                trigger.source_record_digest,
                to_json(&trigger.scope)?,
                trigger.observed_at.to_rfc3339(),
                trigger.signal_id.map(|id| id.to_string()),
                to_json(&trigger.evidence_ids)?,
                optional_json(trigger.report.as_ref())?,
            ],
        )
        .map_err(database_error)?;
    Ok(())
}

fn insert_role(
    connection: &Connection,
    incident_id: IncidentId,
    assignment: &IncidentRoleAssignment,
) -> Result<(), IncidentStoreError> {
    connection
        .execute(
            "INSERT INTO incident_role_assignment (
                 id, incident_id, role, principal_id, assigned_by, assigned_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                Uuid::new_v4().to_string(),
                incident_id.to_string(),
                role_wire(assignment.role),
                assignment.principal_id.to_string(),
                assignment.assigned_by.to_string(),
                assignment.assigned_at.to_rfc3339(),
            ],
        )
        .map_err(database_error)?;
    Ok(())
}

fn insert_event(
    connection: &Connection,
    event: &IncidentTimelineEvent,
) -> Result<(), IncidentStoreError> {
    connection
        .execute(
            "INSERT INTO incident_timeline_event (
                 id, incident_id, sequence, event_kind, actor_id, reason,
                 occurred_at, request_id, policy_version, payload_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                event.id.to_string(),
                event.incident_id.to_string(),
                to_i64(event.sequence)?,
                event_kind_wire(event.kind),
                event.actor_id.to_string(),
                event.reason,
                event.occurred_at.to_rfc3339(),
                event.request_id.to_string(),
                to_i64(event.policy_version)?,
                to_json(&event.payload)?,
            ],
        )
        .map_err(database_error)?;
    Ok(())
}

/// Releases assignments the aggregate no longer holds and inserts the new
/// ones, in that order: an exclusive role's replacement would otherwise
/// collide with the partial unique index that guards it.
fn reconcile_roles(
    connection: &Connection,
    incident: &Incident,
    actor_id: PrincipalId,
    occurred_at: DateTime<Utc>,
) -> Result<(), IncidentStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT id, role, principal_id FROM incident_role_assignment
             WHERE incident_id = ?1 AND released_at IS NULL ORDER BY rowid ASC",
        )
        .map_err(database_error)?;
    let active = statement
        .query_map([incident.id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(database_error)?;

    let mut retained: Vec<(IncidentRole, PrincipalId)> = Vec::new();
    for (id, role, principal_id) in &active {
        let role = parse_role(role)?;
        let principal_id = parse_uuid(principal_id, "principal id")?;
        if incident
            .roles
            .iter()
            .any(|assignment| assignment.role == role && assignment.principal_id == principal_id)
        {
            retained.push((role, principal_id));
            continue;
        }
        connection
            .execute(
                "UPDATE incident_role_assignment SET released_by = ?1, released_at = ?2
                 WHERE id = ?3",
                rusqlite::params![actor_id.to_string(), occurred_at.to_rfc3339(), id],
            )
            .map_err(database_error)?;
    }

    for assignment in &incident.roles {
        if retained.iter().any(|(role, principal_id)| {
            *role == assignment.role && *principal_id == assignment.principal_id
        }) {
            continue;
        }
        insert_role(connection, incident.id, assignment)?;
    }
    Ok(())
}

/// The raw incident row, before its triggers and roles are joined in.
struct IncidentRow {
    id: Uuid,
    scope: ResourceScope,
    summary: String,
    business_impact: BusinessImpact,
    derived_severity: IncidentSeverity,
    severity_override: Option<IncidentSeverityOverride>,
    status: IncidentStatus,
    disposition: Option<IncidentDisposition>,
    duplicate_of_incident_id: Option<Uuid>,
    signal_ids: Vec<SignalId>,
    evidence_ids: Vec<ConsoleEvidenceId>,
    hypothesis_ids: Vec<Uuid>,
    action_ids: Vec<Uuid>,
    version: u64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

fn read_incident(row: &Row<'_>) -> rusqlite::Result<Result<IncidentRow, IncidentStoreError>> {
    let id: String = row.get("id")?;
    let scope: String = row.get("scope_json")?;
    let summary: String = row.get("summary")?;
    let business_impact: String = row.get("business_impact_json")?;
    let derived_severity: String = row.get("derived_severity")?;
    let severity_override: Option<String> = row.get("severity_override_json")?;
    let status: String = row.get("status")?;
    let disposition: Option<String> = row.get("disposition")?;
    let duplicate: Option<String> = row.get("duplicate_of_incident_id")?;
    let signal_ids: String = row.get("signal_ids_json")?;
    let evidence_ids: String = row.get("evidence_ids_json")?;
    let hypothesis_ids: String = row.get("hypothesis_ids_json")?;
    let action_ids: String = row.get("action_ids_json")?;
    let version: i64 = row.get("version")?;
    let created_at: String = row.get("created_at")?;
    let updated_at: String = row.get("updated_at")?;

    Ok((|| {
        Ok(IncidentRow {
            id: parse_uuid(&id, "incident id")?,
            scope: from_json(&scope, "resource scope")?,
            summary,
            business_impact: from_json(&business_impact, "business impact")?,
            derived_severity: parse_severity(&derived_severity)?,
            severity_override: match severity_override {
                Some(value) => Some(from_json(&value, "severity override")?),
                None => None,
            },
            status: parse_status(&status)?,
            disposition: match disposition {
                Some(value) => Some(parse_disposition(&value)?),
                None => None,
            },
            duplicate_of_incident_id: match duplicate {
                Some(value) => Some(parse_uuid(&value, "incident id")?),
                None => None,
            },
            signal_ids: from_json(&signal_ids, "signal identifiers")?,
            evidence_ids: from_json(&evidence_ids, "evidence identifiers")?,
            hypothesis_ids: from_json(&hypothesis_ids, "hypothesis identifiers")?,
            action_ids: from_json(&action_ids, "action identifiers")?,
            version: u64::try_from(version).map_err(|_| corruption("incident version"))?,
            created_at: parse_timestamp(&created_at)?,
            updated_at: parse_timestamp(&updated_at)?,
        })
    })())
}

fn load_incident(
    connection: &Connection,
    workspace_id: Uuid,
    incident_id: IncidentId,
) -> Result<Option<Incident>, IncidentStoreError> {
    let row = connection
        .query_row(
            "SELECT * FROM incident WHERE id = ?1 AND workspace_id = ?2",
            [incident_id.to_string(), workspace_id.to_string()],
            read_incident,
        )
        .optional()
        .map_err(database_error)?;
    match row {
        Some(row) => Ok(Some(hydrate_incident(connection, row?)?)),
        None => Ok(None),
    }
}

/// Joins stored provenance and active roles onto a raw row.  Trigger IDs are
/// read back in identifier order, which is the order the aggregate holds them.
fn hydrate_incident(
    connection: &Connection,
    row: IncidentRow,
) -> Result<Incident, IncidentStoreError> {
    let mut trigger_statement = connection
        .prepare("SELECT id FROM incident_trigger WHERE incident_id = ?1 ORDER BY id ASC")
        .map_err(database_error)?;
    let trigger_ids = trigger_statement
        .query_map([row.id.to_string()], |trigger| trigger.get::<_, String>(0))
        .map_err(database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(database_error)?
        .iter()
        .map(|id| parse_uuid(id, "trigger id"))
        .collect::<Result<Vec<_>, _>>()?;

    let mut role_statement = connection
        .prepare(
            "SELECT role, principal_id, assigned_by, assigned_at FROM incident_role_assignment
             WHERE incident_id = ?1 AND released_at IS NULL ORDER BY rowid ASC",
        )
        .map_err(database_error)?;
    let role_rows = role_statement
        .query_map([row.id.to_string()], |role| {
            Ok((
                role.get::<_, String>(0)?,
                role.get::<_, String>(1)?,
                role.get::<_, String>(2)?,
                role.get::<_, String>(3)?,
            ))
        })
        .map_err(database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(database_error)?;
    let mut roles = Vec::with_capacity(role_rows.len());
    for (role, principal_id, assigned_by, assigned_at) in &role_rows {
        roles.push(IncidentRoleAssignment {
            role: parse_role(role)?,
            principal_id: parse_uuid(principal_id, "principal id")?,
            assigned_by: parse_uuid(assigned_by, "principal id")?,
            assigned_at: parse_timestamp(assigned_at)?,
        });
    }

    let owning_team_id = row
        .scope
        .team_id
        .ok_or_else(|| corruption("incident scope"))?;
    Ok(Incident {
        id: row.id,
        summary: row.summary,
        scope: row.scope,
        owning_team_id,
        business_impact: row.business_impact,
        derived_severity: row.derived_severity,
        severity_override: row.severity_override,
        status: row.status,
        disposition: row.disposition,
        duplicate_of_incident_id: row.duplicate_of_incident_id,
        trigger_ids,
        signal_ids: row.signal_ids,
        evidence_ids: row.evidence_ids,
        hypothesis_ids: row.hypothesis_ids,
        action_ids: row.action_ids,
        roles,
        version: row.version,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

type RawEventRow = (
    String,
    String,
    i64,
    String,
    String,
    Option<String>,
    String,
    String,
    i64,
    String,
);

fn load_events(
    connection: &Connection,
    incident_id: IncidentId,
    after_sequence: i64,
    limit: i64,
) -> Result<Vec<IncidentTimelineEvent>, IncidentStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT id, incident_id, sequence, event_kind, actor_id, reason,
                    occurred_at, request_id, policy_version, payload_json
             FROM incident_timeline_event
             WHERE incident_id = ?1 AND sequence > ?2
             ORDER BY sequence ASC LIMIT ?3",
        )
        .map_err(database_error)?;
    let rows = statement
        .query_map(
            rusqlite::params![incident_id.to_string(), after_sequence, limit],
            read_event_row,
        )
        .map_err(database_error)?;
    decode_event_rows(rows)
}

fn load_events_for_request(
    connection: &Connection,
    incident_id: IncidentId,
    request_id: Uuid,
    limit: i64,
) -> Result<Vec<IncidentTimelineEvent>, IncidentStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT id, incident_id, sequence, event_kind, actor_id, reason,
                    occurred_at, request_id, policy_version, payload_json
             FROM incident_timeline_event
             WHERE incident_id = ?1 AND request_id = ?2
             ORDER BY sequence ASC LIMIT ?3",
        )
        .map_err(database_error)?;
    let rows = statement
        .query_map(
            rusqlite::params![incident_id.to_string(), request_id.to_string(), limit],
            read_event_row,
        )
        .map_err(database_error)?;
    decode_event_rows(rows)
}

fn read_event_row(row: &Row<'_>) -> rusqlite::Result<RawEventRow> {
    Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, i64>(2)?,
        row.get::<_, String>(3)?,
        row.get::<_, String>(4)?,
        row.get::<_, Option<String>>(5)?,
        row.get::<_, String>(6)?,
        row.get::<_, String>(7)?,
        row.get::<_, i64>(8)?,
        row.get::<_, String>(9)?,
    ))
}

fn decode_event_rows(
    rows: impl Iterator<Item = rusqlite::Result<RawEventRow>>,
) -> Result<Vec<IncidentTimelineEvent>, IncidentStoreError> {
    let rows = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(database_error)?;
    let mut events = Vec::with_capacity(rows.len());
    for row in &rows {
        let payload: IncidentTimelinePayload = from_json(&row.9, "timeline payload")?;
        events.push(IncidentTimelineEvent {
            id: parse_uuid(&row.0, "event id")?,
            incident_id: parse_uuid(&row.1, "incident id")?,
            sequence: u64::try_from(row.2).map_err(|_| corruption("event sequence"))?,
            kind: parse_event_kind(&row.3)?,
            actor_id: parse_uuid(&row.4, "principal id")?,
            reason: row.5.clone(),
            occurred_at: parse_timestamp(&row.6)?,
            request_id: parse_uuid(&row.7, "request id")?,
            policy_version: u64::try_from(row.8).map_err(|_| corruption("policy version"))?,
            payload,
        });
    }
    Ok(events)
}

fn format_cursor(updated_at: DateTime<Utc>, id: Uuid) -> String {
    format!("{}|{}", updated_at.to_rfc3339(), id)
}

fn parse_cursor(cursor: &str) -> Result<(String, String), IncidentStoreError> {
    if cursor.trim().is_empty()
        || cursor.chars().count() > INCIDENT_CURSOR_MAXIMUM
        || cursor.chars().any(|character| character.is_control())
    {
        return Err(IncidentStoreError::InvalidPagination);
    }
    let (timestamp, id) = cursor
        .split_once('|')
        .ok_or(IncidentStoreError::InvalidPagination)?;
    let timestamp = DateTime::parse_from_rfc3339(timestamp)
        .map_err(|_| IncidentStoreError::InvalidPagination)?
        .with_timezone(&Utc);
    let id = Uuid::parse_str(id).map_err(|_| IncidentStoreError::InvalidPagination)?;
    Ok((timestamp.to_rfc3339(), id.to_string()))
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<String, IncidentStoreError> {
    serde_json::to_string(value).map_err(serialization_error)
}

fn optional_json<T: serde::Serialize>(
    value: Option<&T>,
) -> Result<Option<String>, IncidentStoreError> {
    match value {
        Some(value) => Ok(Some(to_json(value)?)),
        None => Ok(None),
    }
}

fn from_json<T: serde::de::DeserializeOwned>(
    value: &str,
    kind: &str,
) -> Result<T, IncidentStoreError> {
    serde_json::from_str(value).map_err(|_| corruption(kind))
}

fn to_i64(value: u64) -> Result<i64, IncidentStoreError> {
    i64::try_from(value).map_err(|_| invalid("value exceeds the stored integer range"))
}

fn parse_uuid(value: &str, kind: &str) -> Result<Uuid, IncidentStoreError> {
    Uuid::parse_str(value).map_err(|_| corruption(kind))
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, IncidentStoreError> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| corruption("timestamp"))
}

fn severity_wire(severity: &IncidentSeverity) -> &'static str {
    match severity {
        IncidentSeverity::S1 => "S1",
        IncidentSeverity::S2 => "S2",
        IncidentSeverity::S3 => "S3",
        IncidentSeverity::S4 => "S4",
        IncidentSeverity::S5 => "S5",
    }
}

fn parse_severity(value: &str) -> Result<IncidentSeverity, IncidentStoreError> {
    match value {
        "S1" => Ok(IncidentSeverity::S1),
        "S2" => Ok(IncidentSeverity::S2),
        "S3" => Ok(IncidentSeverity::S3),
        "S4" => Ok(IncidentSeverity::S4),
        "S5" => Ok(IncidentSeverity::S5),
        _ => Err(corruption("incident severity")),
    }
}

fn status_wire(status: IncidentStatus) -> &'static str {
    match status {
        IncidentStatus::Detected => "detected",
        IncidentStatus::Triage => "triage",
        IncidentStatus::Investigating => "investigating",
        IncidentStatus::Mitigating => "mitigating",
        IncidentStatus::Monitoring => "monitoring",
        IncidentStatus::Resolved => "resolved",
        IncidentStatus::Closed => "closed",
        IncidentStatus::Reopened => "reopened",
    }
}

fn parse_status(value: &str) -> Result<IncidentStatus, IncidentStoreError> {
    match value {
        "detected" => Ok(IncidentStatus::Detected),
        "triage" => Ok(IncidentStatus::Triage),
        "investigating" => Ok(IncidentStatus::Investigating),
        "mitigating" => Ok(IncidentStatus::Mitigating),
        "monitoring" => Ok(IncidentStatus::Monitoring),
        "resolved" => Ok(IncidentStatus::Resolved),
        "closed" => Ok(IncidentStatus::Closed),
        "reopened" => Ok(IncidentStatus::Reopened),
        _ => Err(corruption("incident status")),
    }
}

fn disposition_wire(disposition: &IncidentDisposition) -> &'static str {
    match disposition {
        IncidentDisposition::Duplicate => "duplicate",
        IncidentDisposition::FalsePositive => "false_positive",
        IncidentDisposition::Suppressed => "suppressed",
        IncidentDisposition::Cancelled => "cancelled",
        IncidentDisposition::Informational => "informational",
    }
}

fn parse_disposition(value: &str) -> Result<IncidentDisposition, IncidentStoreError> {
    match value {
        "duplicate" => Ok(IncidentDisposition::Duplicate),
        "false_positive" => Ok(IncidentDisposition::FalsePositive),
        "suppressed" => Ok(IncidentDisposition::Suppressed),
        "cancelled" => Ok(IncidentDisposition::Cancelled),
        "informational" => Ok(IncidentDisposition::Informational),
        _ => Err(corruption("incident disposition")),
    }
}

fn source_kind_wire(kind: IncidentSourceKind) -> &'static str {
    match kind {
        IncidentSourceKind::Alert => "alert",
        IncidentSourceKind::Anomaly => "anomaly",
        IncidentSourceKind::UserReport => "user_report",
        IncidentSourceKind::ScheduledHealthCheck => "scheduled_health_check",
        IncidentSourceKind::VulnerabilityFinding => "vulnerability_finding",
        IncidentSourceKind::ManualReport => "manual_report",
    }
}

fn role_wire(role: IncidentRole) -> &'static str {
    match role {
        IncidentRole::Owner => "owner",
        IncidentRole::IncidentCommander => "incident_commander",
        IncidentRole::TechnicalLead => "technical_lead",
        IncidentRole::CommunicationsLead => "communications_lead",
        IncidentRole::Approver => "approver",
        IncidentRole::ChangeOwner => "change_owner",
        IncidentRole::Stakeholder => "stakeholder",
    }
}

fn parse_role(value: &str) -> Result<IncidentRole, IncidentStoreError> {
    match value {
        "owner" => Ok(IncidentRole::Owner),
        "incident_commander" => Ok(IncidentRole::IncidentCommander),
        "technical_lead" => Ok(IncidentRole::TechnicalLead),
        "communications_lead" => Ok(IncidentRole::CommunicationsLead),
        "approver" => Ok(IncidentRole::Approver),
        "change_owner" => Ok(IncidentRole::ChangeOwner),
        "stakeholder" => Ok(IncidentRole::Stakeholder),
        _ => Err(corruption("incident role")),
    }
}

fn event_kind_wire(kind: IncidentEventKind) -> &'static str {
    match kind {
        IncidentEventKind::IncidentCreated => "incident_created",
        IncidentEventKind::TriggersAttached => "triggers_attached",
        IncidentEventKind::StatusTransitioned => "status_transitioned",
        IncidentEventKind::SeverityChanged => "severity_changed",
        IncidentEventKind::DispositionChanged => "disposition_changed",
        IncidentEventKind::RoleChanged => "role_changed",
    }
}

fn parse_event_kind(value: &str) -> Result<IncidentEventKind, IncidentStoreError> {
    match value {
        "incident_created" => Ok(IncidentEventKind::IncidentCreated),
        "triggers_attached" => Ok(IncidentEventKind::TriggersAttached),
        "status_transitioned" => Ok(IncidentEventKind::StatusTransitioned),
        "severity_changed" => Ok(IncidentEventKind::SeverityChanged),
        "disposition_changed" => Ok(IncidentEventKind::DispositionChanged),
        "role_changed" => Ok(IncidentEventKind::RoleChanged),
        _ => Err(corruption("incident event kind")),
    }
}
