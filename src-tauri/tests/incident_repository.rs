// SPDX-License-Identifier: Apache-2.0

//! Task 4 persistence proofs: migration shape, atomic creation, idempotency,
//! optimistic concurrency, workspace isolation and timeline immutability.

use chrono::{DateTime, TimeZone, Utc};
use tempfile::TempDir;
use thalassa_domain::{
    BusinessImpact, EnterpriseIdentity, ImpactDimensions, ImpactLevel, ImpactTrajectory, Incident,
    IncidentCreateCommand, IncidentEventKind, IncidentMutation, IncidentReport, IncidentRole,
    IncidentRoleAssignment, IncidentSourceKind, IncidentStatus, IncidentTransition,
    IncidentTrigger, Membership, Principal, PrincipalId, PrincipalKind, ResourceScope,
    TriageContext,
};
use thalassaops::incident::{IncidentCreationRecord, IncidentStoreError, SqliteIncidentRepository};
use uuid::Uuid;

const ACTOR: PrincipalId = Uuid::from_u128(0xa0);
const REPLACEMENT: PrincipalId = Uuid::from_u128(0xa9);
const CREATE_REQUEST: Uuid = Uuid::from_u128(0xb0);
const SECOND_REQUEST: Uuid = Uuid::from_u128(0xb1);
const THIRD_REQUEST: Uuid = Uuid::from_u128(0xb2);
const TEAM: Uuid = Uuid::from_u128(0xc0);
const WORKSPACE: Uuid = Uuid::from_u128(0xd0);
const OTHER_WORKSPACE: Uuid = Uuid::from_u128(0xd1);
const ORGANIZATION: Uuid = Uuid::from_u128(0xe0);
const POLICY_VERSION: u64 = 7;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 28, 9, 0, 0).unwrap()
}

fn later() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 28, 10, 0, 0).unwrap()
}

fn scope_for(workspace_id: Uuid) -> ResourceScope {
    ResourceScope::workspace(workspace_id, TEAM, ORGANIZATION)
}

fn business_impact() -> BusinessImpact {
    BusinessImpact {
        level: ImpactLevel::High,
        summary: "Checkout unavailable for customers".into(),
        customer_scope: "production customers".into(),
        service_criticality: "tier-0".into(),
        trajectory: ImpactTrajectory::Stable,
        dimensions: ImpactDimensions {
            availability: ImpactLevel::High,
            customer_reach: ImpactLevel::Medium,
            business_criticality: ImpactLevel::Medium,
            data_integrity: ImpactLevel::None,
            security_privacy: ImpactLevel::None,
            financial_contractual: ImpactLevel::Low,
            trajectory: ImpactTrajectory::Stable,
            production: true,
        },
        evidence_ids: vec!["evidence-checkout".into()],
    }
}

fn manual_trigger(workspace_id: Uuid, nonce: u128) -> IncidentTrigger {
    IncidentTrigger {
        id: Uuid::from_u128(0x1100 + nonce),
        source_kind: IncidentSourceKind::ManualReport,
        source_id: format!("manual-report-{nonce}"),
        source_record_digest: None,
        scope: scope_for(workspace_id),
        observed_at: now(),
        signal_id: None,
        evidence_ids: vec!["evidence-manual-report".into()],
        report: Some(IncidentReport {
            reporter_id: Some(ACTOR),
            summary: "Checkout is returning errors".into(),
        }),
    }
}

fn alert_trigger(workspace_id: Uuid, nonce: u128) -> IncidentTrigger {
    IncidentTrigger {
        id: Uuid::from_u128(0x1200 + nonce),
        source_kind: IncidentSourceKind::Alert,
        source_id: format!("alert-checkout-{nonce}"),
        source_record_digest: Some("sha256:abcdef0123456789".into()),
        scope: scope_for(workspace_id),
        observed_at: now(),
        signal_id: Some(Uuid::from_u128(0x516)),
        evidence_ids: vec!["evidence-alert-checkout".into()],
        report: None,
    }
}

fn create_command(workspace_id: Uuid, nonce: u128) -> IncidentCreateCommand {
    IncidentCreateCommand {
        summary: "Checkout errors reported by operator".into(),
        scope: scope_for(workspace_id),
        owning_team_id: TEAM,
        triggers: vec![
            manual_trigger(workspace_id, nonce),
            alert_trigger(workspace_id, nonce),
        ],
        business_impact: business_impact(),
        initial_roles: vec![IncidentRoleAssignment {
            role: IncidentRole::Owner,
            principal_id: ACTOR,
            assigned_by: ACTOR,
            assigned_at: now(),
        }],
    }
}

fn creation_record(workspace_id: Uuid, request_id: Uuid, nonce: u128) -> IncidentCreationRecord {
    let command = create_command(workspace_id, nonce);
    let triggers = command.triggers.clone();
    let mutation = Incident::create(command, ACTOR, request_id, POLICY_VERSION, now())
        .expect("creation command is valid");
    IncidentCreationRecord {
        mutation,
        triggers,
        request_fingerprint: format!("sha256:{:064x}", request_id.as_u128()),
    }
}

struct Fixture {
    _directory: TempDir,
    database_path: std::path::PathBuf,
    repository: SqliteIncidentRepository,
}

fn seed_principals(database_path: &std::path::Path, workspace_id: Uuid, ids: &[PrincipalId]) {
    let connection = rusqlite::Connection::open(database_path).expect("the database opens");
    connection
        .execute_batch(include_str!("../migrations/0001_local_workspace.sql"))
        .expect("the identity schema applies");
    for principal_id in ids {
        let principal = Principal {
            id: *principal_id,
            kind: PrincipalKind::Local,
            display_name: format!("Principal {principal_id}"),
            identity: EnterpriseIdentity {
                subject: principal_id.to_string(),
                ..Default::default()
            },
            created_at: now(),
        };
        let membership = Membership::workspace_owner(*principal_id, workspace_id);
        connection
            .execute(
                "INSERT INTO principals (id, document_json) VALUES (?1, ?2)",
                rusqlite::params![
                    principal_id.to_string(),
                    serde_json::to_string(&principal).expect("principal serializes")
                ],
            )
            .expect("principal inserts");
        connection
            .execute(
                "INSERT INTO memberships (id, document_json) VALUES (?1, ?2)",
                rusqlite::params![
                    principal_id.to_string(),
                    serde_json::to_string(&membership).expect("membership serializes")
                ],
            )
            .expect("membership inserts");
    }
}

fn fixture() -> Fixture {
    let directory = TempDir::new().expect("temporary directory");
    let database_path = directory.path().join("incidents.sqlite3");
    let repository = SqliteIncidentRepository::open(&database_path).expect("repository opens");
    seed_principals(&database_path, WORKSPACE, &[ACTOR, REPLACEMENT]);
    Fixture {
        _directory: directory,
        database_path,
        repository,
    }
}

fn triage_mutation(incident: &Incident, request_id: Uuid, sequence: u64) -> IncidentMutation {
    incident
        .transition(
            incident.version,
            sequence,
            IncidentTransition::Triage(TriageContext {
                business_impact: business_impact(),
                owner: ACTOR,
                duplicate_checked: true,
            }),
            ACTOR,
            request_id,
            POLICY_VERSION,
            later(),
        )
        .expect("triage transition is valid")
}

#[test]
fn migration_creates_every_incident_table_and_immutability_trigger() {
    let fixture = fixture();
    let connection = rusqlite::Connection::open(&fixture.database_path).expect("database opens");

    for table in [
        "incident",
        "incident_trigger",
        "incident_role_assignment",
        "incident_timeline_event",
    ] {
        let found: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .expect("schema query succeeds");
        assert_eq!(found, 1, "{table} should exist after migration");
    }

    for trigger in ["incident_timeline_no_update", "incident_timeline_no_delete"] {
        let found: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name = ?1",
                [trigger],
                |row| row.get(0),
            )
            .expect("schema query succeeds");
        assert_eq!(found, 1, "{trigger} should exist after migration");
    }

    // Opening the same database twice must not fail or duplicate schema.
    SqliteIncidentRepository::open(&fixture.database_path).expect("repository reopens");
}

#[test]
fn create_is_atomic_idempotent_and_timeline_is_immutable() {
    let mut fixture = fixture();
    let record = creation_record(WORKSPACE, CREATE_REQUEST, 1);
    let first = fixture
        .repository
        .create(record.clone())
        .expect("first creation succeeds");
    let repeated = fixture
        .repository
        .create(record)
        .expect("replayed creation succeeds");
    assert_eq!(first, repeated);

    let stored = fixture
        .repository
        .get(WORKSPACE, first.incident.id)
        .expect("incident is readable");
    assert_eq!(stored, first.incident);

    let timeline = fixture
        .repository
        .timeline(WORKSPACE, first.incident.id, None, 100)
        .expect("timeline is readable");
    assert_eq!(timeline.events.len(), 2);
    assert_eq!(timeline.events, first.events);
    assert_eq!(timeline.next_sequence, None);

    let connection = rusqlite::Connection::open(&fixture.database_path).expect("database opens");
    assert!(connection
        .execute("DELETE FROM incident_timeline_event", [])
        .is_err());
    assert!(connection
        .execute("UPDATE incident_timeline_event SET actor_id = 'x'", [])
        .is_err());
}

#[test]
fn idempotent_creation_replay_returns_only_creation_events() {
    let mut fixture = fixture();
    let record = creation_record(WORKSPACE, CREATE_REQUEST, 1);
    let first = fixture
        .repository
        .create(record.clone())
        .expect("first creation succeeds");
    fixture
        .repository
        .apply_mutation(triage_mutation(&first.incident, SECOND_REQUEST, 3))
        .expect("later mutation succeeds");

    let replayed = fixture
        .repository
        .create(record)
        .expect("replayed creation succeeds");

    assert_eq!(replayed.events, first.events);
}

#[test]
fn applying_a_mutation_again_replays_the_original_result_without_writing() {
    let mut fixture = fixture();
    let created = fixture
        .repository
        .create(creation_record(WORKSPACE, CREATE_REQUEST, 1))
        .expect("creation succeeds");
    let applied = fixture
        .repository
        .apply_mutation(triage_mutation(&created.incident, SECOND_REQUEST, 3))
        .expect("first mutation succeeds");
    let before_retry = fixture
        .repository
        .timeline(WORKSPACE, created.incident.id, None, 100)
        .expect("timeline is readable");

    let replayed = fixture
        .repository
        .apply_mutation(applied.clone())
        .expect("the mutation retry is replayed");

    assert_eq!(replayed, applied);
    assert_eq!(
        fixture
            .repository
            .timeline(WORKSPACE, created.incident.id, None, 100)
            .expect("timeline is readable"),
        before_retry
    );
}

#[test]
fn applying_a_mutation_request_id_with_different_content_is_rejected() {
    let mut fixture = fixture();
    let created = fixture
        .repository
        .create(creation_record(WORKSPACE, CREATE_REQUEST, 1))
        .expect("creation succeeds");
    let applied = fixture
        .repository
        .apply_mutation(triage_mutation(&created.incident, SECOND_REQUEST, 3))
        .expect("first mutation succeeds");
    let mut divergent = applied.clone();
    divergent.events[0].actor_id = Uuid::from_u128(0xa9);

    let error = fixture
        .repository
        .apply_mutation(divergent)
        .expect_err("a diverging mutation retry is rejected");

    assert!(matches!(error, IncidentStoreError::IdempotencyConflict));
}

#[test]
fn replaying_a_request_id_with_a_different_fingerprint_is_rejected() {
    let mut fixture = fixture();
    let record = creation_record(WORKSPACE, CREATE_REQUEST, 1);
    fixture
        .repository
        .create(record.clone())
        .expect("first creation succeeds");

    let mut divergent = record;
    divergent.request_fingerprint = "sha256:0000000000000000".into();
    let error = fixture
        .repository
        .create(divergent)
        .expect_err("a diverging replay is rejected");
    assert!(matches!(error, IncidentStoreError::IdempotencyConflict));
}

#[test]
fn a_preflight_creation_failure_leaves_no_state_and_no_orphan_events() {
    let mut fixture = fixture();
    let mut record = creation_record(WORKSPACE, CREATE_REQUEST, 1);
    // Two triggers claiming one source identity violate the stored uniqueness
    // constraint; the whole write must roll back.
    record.triggers[1] = IncidentTrigger {
        id: Uuid::from_u128(0x112),
        ..manual_trigger(WORKSPACE, 1)
    };
    let incident_id = record.mutation.incident.id;

    assert!(fixture.repository.create(record).is_err());

    assert!(matches!(
        fixture.repository.get(WORKSPACE, incident_id),
        Err(IncidentStoreError::NotFound)
    ));
    let connection = rusqlite::Connection::open(&fixture.database_path).expect("database opens");
    for table in [
        "incident",
        "incident_trigger",
        "incident_role_assignment",
        "incident_timeline_event",
    ] {
        let count: i64 = connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count succeeds");
        assert_eq!(count, 0, "{table} should be empty after a failed creation");
    }
}

#[test]
fn a_post_insert_creation_failure_rolls_back_every_incident_row() {
    let mut fixture = fixture();
    let mut record = creation_record(WORKSPACE, CREATE_REQUEST, 1);
    // The first role insert succeeds, then the second role violates the
    // active-exclusive-role index. This failure is deliberately after the
    // incident, triggers and first role have been inserted in the transaction.
    let duplicate_role = record.mutation.incident.roles[0].clone();
    record.mutation.incident.roles.push(duplicate_role);
    let incident_id = record.mutation.incident.id;

    let error = fixture
        .repository
        .create(record)
        .expect_err("a duplicate active role fails after partial inserts");
    assert!(matches!(error, IncidentStoreError::Database(_)));

    assert!(matches!(
        fixture.repository.get(WORKSPACE, incident_id),
        Err(IncidentStoreError::NotFound)
    ));
    let connection = rusqlite::Connection::open(&fixture.database_path).expect("database opens");
    for table in [
        "incident",
        "incident_trigger",
        "incident_role_assignment",
        "incident_timeline_event",
    ] {
        let count: i64 = connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count succeeds");
        assert_eq!(
            count, 0,
            "{table} should be unchanged after a post-insert failure"
        );
    }
}

#[test]
fn stale_version_does_not_append_or_overwrite() {
    let mut fixture = fixture();
    let created = fixture
        .repository
        .create(creation_record(WORKSPACE, CREATE_REQUEST, 1))
        .expect("creation succeeds");

    let accepted = fixture
        .repository
        .apply_mutation(triage_mutation(&created.incident, SECOND_REQUEST, 3))
        .expect("first triage is accepted");
    assert_eq!(accepted.incident.status, IncidentStatus::Triage);
    assert_eq!(accepted.incident.version, 2);

    let before = fixture
        .repository
        .timeline(WORKSPACE, accepted.incident.id, None, 100)
        .expect("timeline is readable");
    let stored_before = fixture
        .repository
        .get(WORKSPACE, accepted.incident.id)
        .expect("incident is readable");

    let stale = triage_mutation(&created.incident, THIRD_REQUEST, 3);
    let error = fixture
        .repository
        .apply_mutation(stale)
        .expect_err("a stale write is rejected");
    assert!(matches!(
        error,
        IncidentStoreError::VersionConflict {
            expected: 1,
            actual: 2
        }
    ));

    assert_eq!(
        fixture
            .repository
            .timeline(WORKSPACE, accepted.incident.id, None, 100)
            .expect("timeline is readable"),
        before
    );
    assert_eq!(
        fixture
            .repository
            .get(WORKSPACE, accepted.incident.id)
            .expect("incident is readable"),
        stored_before
    );
}

#[test]
fn a_stale_event_sequence_is_reallocated_rather_than_rejected() {
    let mut fixture = fixture();
    let created = fixture
        .repository
        .create(creation_record(WORKSPACE, CREATE_REQUEST, 1))
        .expect("creation succeeds");
    let highest_before = fixture
        .repository
        .highest_event_sequence(WORKSPACE, created.incident.id)
        .expect("highest event sequence is readable");

    let mut mutation = triage_mutation(&created.incident, SECOND_REQUEST, highest_before + 1);
    // Simulate a stale read: the caller believes sequence 1 is still free.
    mutation.events[0].sequence = 1;

    fixture
        .repository
        .apply_mutation(mutation)
        .expect("a stale sequence is reallocated, not rejected");

    let sequences: Vec<u64> = fixture
        .repository
        .timeline(WORKSPACE, created.incident.id, None, 100)
        .expect("timeline is readable")
        .events
        .iter()
        .map(|event| event.sequence)
        .collect();
    let mut unique = sequences.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(sequences.len(), unique.len(), "sequences must stay unique");
    assert_eq!(
        *sequences.iter().max().expect("at least one event"),
        highest_before + 1,
        "the appended event takes the next free sequence"
    );
}

#[test]
fn event_sequences_are_reallocated_to_continue_the_stored_timeline() {
    let mut fixture = fixture();
    let created = fixture
        .repository
        .create(creation_record(WORKSPACE, CREATE_REQUEST, 1))
        .expect("creation succeeds");

    let gapped = triage_mutation(&created.incident, SECOND_REQUEST, 9);
    let applied = fixture
        .repository
        .apply_mutation(gapped)
        .expect("a caller-provided sequence is advisory");
    assert_eq!(applied.events[0].sequence, 3);

    let events = fixture
        .repository
        .timeline(WORKSPACE, created.incident.id, None, 100)
        .expect("timeline is readable")
        .events;
    let sequences: Vec<u64> = events.iter().map(|event| event.sequence).collect();
    assert_eq!(sequences, vec![1, 2, 3]);
}

#[test]
fn multi_event_rebase_preserves_mutation_event_order() {
    let mut fixture = fixture();
    let created = fixture
        .repository
        .create(creation_record(WORKSPACE, CREATE_REQUEST, 1))
        .expect("creation succeeds");
    let highest_before = fixture
        .repository
        .highest_event_sequence(WORKSPACE, created.incident.id)
        .expect("highest event sequence is readable");

    let mut changed_impact = business_impact();
    changed_impact.summary = "Checkout unavailable for every customer".into();
    let mutation = created
        .incident
        .transition(
            created.incident.version,
            9,
            IncidentTransition::Triage(TriageContext {
                business_impact: changed_impact,
                owner: REPLACEMENT,
                duplicate_checked: true,
            }),
            ACTOR,
            SECOND_REQUEST,
            POLICY_VERSION,
            later(),
        )
        .expect("triage transition emits three events");

    let expected_kinds = vec![
        IncidentEventKind::StatusTransitioned,
        IncidentEventKind::SeverityChanged,
        IncidentEventKind::RoleChanged,
    ];
    assert_eq!(
        mutation
            .events
            .iter()
            .map(|event| event.kind)
            .collect::<Vec<_>>(),
        expected_kinds
    );

    fixture
        .repository
        .apply_mutation(mutation)
        .expect("a gapped base sequence is rebased");

    let stored_events = fixture
        .repository
        .timeline(WORKSPACE, created.incident.id, None, 100)
        .expect("timeline is readable")
        .events;
    let appended_events: Vec<_> = stored_events
        .iter()
        .filter(|event| event.sequence > highest_before)
        .collect();
    assert_eq!(
        appended_events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        vec![highest_before + 1, highest_before + 2, highest_before + 3]
    );
    assert_eq!(
        appended_events
            .iter()
            .map(|event| event.kind)
            .collect::<Vec<_>>(),
        expected_kinds
    );
}

#[test]
fn reads_are_workspace_isolated() {
    let mut fixture = fixture();
    let mine = fixture
        .repository
        .create(creation_record(WORKSPACE, CREATE_REQUEST, 1))
        .expect("creation succeeds");
    let theirs = fixture
        .repository
        .create(creation_record(OTHER_WORKSPACE, SECOND_REQUEST, 2))
        .expect("creation succeeds");

    assert!(matches!(
        fixture.repository.get(OTHER_WORKSPACE, mine.incident.id),
        Err(IncidentStoreError::NotFound)
    ));
    assert!(matches!(
        fixture
            .repository
            .timeline(OTHER_WORKSPACE, mine.incident.id, None, 100),
        Err(IncidentStoreError::NotFound)
    ));

    let page = fixture
        .repository
        .list(WORKSPACE, None, 100)
        .expect("list succeeds");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, mine.incident.id);
    assert_eq!(page.next_cursor, None);

    let other_page = fixture
        .repository
        .list(OTHER_WORKSPACE, None, 100)
        .expect("list succeeds");
    assert_eq!(other_page.items.len(), 1);
    assert_eq!(other_page.items[0].id, theirs.incident.id);
}

#[test]
fn list_and_timeline_pages_are_stable_and_bounded() {
    let mut fixture = fixture();
    let first = fixture
        .repository
        .create(creation_record(WORKSPACE, CREATE_REQUEST, 1))
        .expect("creation succeeds");
    // A second incident in the same workspace, advanced so its update time is
    // strictly newer than the first.
    let second = fixture
        .repository
        .create(creation_record(WORKSPACE, SECOND_REQUEST, 2))
        .expect("creation succeeds");
    let advanced = fixture
        .repository
        .apply_mutation(triage_mutation(&second.incident, THIRD_REQUEST, 3))
        .expect("triage is accepted");

    let page = fixture
        .repository
        .list(WORKSPACE, None, 1)
        .expect("first page succeeds");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, advanced.incident.id);
    let cursor = page.next_cursor.expect("a continuation cursor is returned");

    let next = fixture
        .repository
        .list(WORKSPACE, Some(&cursor), 1)
        .expect("second page succeeds");
    assert_eq!(next.items.len(), 1);
    assert_eq!(next.items[0].id, first.incident.id);
    assert_eq!(next.next_cursor, None);

    let head = fixture
        .repository
        .timeline(WORKSPACE, advanced.incident.id, None, 2)
        .expect("timeline head succeeds");
    assert_eq!(head.events.len(), 2);
    assert_eq!(head.next_sequence, Some(2));
    let tail = fixture
        .repository
        .timeline(WORKSPACE, advanced.incident.id, Some(2), 100)
        .expect("timeline tail succeeds");
    assert_eq!(tail.events, advanced.events);
    assert_eq!(tail.next_sequence, None);
}

#[test]
fn pagination_arguments_are_validated_without_clamping() {
    let mut fixture = fixture();
    let created = fixture
        .repository
        .create(creation_record(WORKSPACE, CREATE_REQUEST, 1))
        .expect("creation succeeds");

    for limit in [0, 101] {
        assert!(matches!(
            fixture.repository.list(WORKSPACE, None, limit),
            Err(IncidentStoreError::InvalidPagination)
        ));
        assert!(matches!(
            fixture
                .repository
                .timeline(WORKSPACE, created.incident.id, None, limit),
            Err(IncidentStoreError::InvalidPagination)
        ));
    }
    assert!(matches!(
        fixture.repository.list(WORKSPACE, Some("not-a-cursor"), 10),
        Err(IncidentStoreError::InvalidPagination)
    ));
    assert!(matches!(
        fixture
            .repository
            .timeline(WORKSPACE, created.incident.id, Some(0), 10),
        Err(IncidentStoreError::InvalidPagination)
    ));
}

#[test]
fn role_history_is_retained_while_current_state_holds_active_roles() {
    let mut fixture = fixture();
    let created = fixture
        .repository
        .create(creation_record(WORKSPACE, CREATE_REQUEST, 1))
        .expect("creation succeeds");

    let replacement = created
        .incident
        .assign_role(
            created.incident.version,
            3,
            thalassa_domain::IncidentRoleCommand::Replace {
                role: IncidentRole::Owner,
                principal_id: REPLACEMENT,
            },
            ACTOR,
            SECOND_REQUEST,
            POLICY_VERSION,
            later(),
        )
        .expect("role replacement is valid");
    let applied = fixture
        .repository
        .apply_mutation(replacement)
        .expect("role replacement is accepted");

    let stored = fixture
        .repository
        .get(WORKSPACE, applied.incident.id)
        .expect("incident is readable");
    assert_eq!(stored, applied.incident);
    assert_eq!(stored.roles.len(), 1);
    assert_eq!(stored.roles[0].principal_id, Uuid::from_u128(0xa9));

    let connection = rusqlite::Connection::open(&fixture.database_path).expect("database opens");
    let released: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM incident_role_assignment WHERE released_at IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .expect("count succeeds");
    assert_eq!(released, 1, "the replaced owner keeps an audit row");
}

#[test]
fn role_mutation_rejects_an_unknown_principal_inside_the_write_transaction() {
    let mut fixture = fixture();
    let created = fixture
        .repository
        .create(creation_record(WORKSPACE, CREATE_REQUEST, 1))
        .expect("creation succeeds");
    let mutation = created
        .incident
        .assign_role(
            created.incident.version,
            3,
            thalassa_domain::IncidentRoleCommand::Assign {
                role: IncidentRole::IncidentCommander,
                principal_id: Uuid::from_u128(0xbeef),
            },
            ACTOR,
            SECOND_REQUEST,
            POLICY_VERSION,
            later(),
        )
        .expect("role assignment is valid at the domain boundary");

    let connection = rusqlite::Connection::open(&fixture.database_path).expect("database opens");
    let before_roles: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM incident_role_assignment WHERE incident_id = ?1",
            [created.incident.id.to_string()],
            |row| row.get(0),
        )
        .expect("role count succeeds");
    let before_events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM incident_timeline_event WHERE incident_id = ?1",
            [created.incident.id.to_string()],
            |row| row.get(0),
        )
        .expect("timeline count succeeds");

    let error = fixture
        .repository
        .apply_mutation(mutation)
        .expect_err("unknown principals are rejected by the repository transaction");
    assert!(matches!(error, IncidentStoreError::NotFound));
    assert_eq!(
        fixture
            .repository
            .get(WORKSPACE, created.incident.id)
            .expect("incident is readable"),
        created.incident
    );

    let after_roles: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM incident_role_assignment WHERE incident_id = ?1",
            [created.incident.id.to_string()],
            |row| row.get(0),
        )
        .expect("role count succeeds");
    let after_events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM incident_timeline_event WHERE incident_id = ?1",
            [created.incident.id.to_string()],
            |row| row.get(0),
        )
        .expect("timeline count succeeds");
    assert_eq!(after_roles, before_roles);
    assert_eq!(after_events, before_events);
}

#[test]
fn a_comment_appends_after_a_racing_transition_without_writing_the_version() {
    let mut fixture = fixture();
    let created = fixture
        .repository
        .create(creation_record(WORKSPACE, CREATE_REQUEST, 1))
        .expect("creation succeeds");
    let incident = created.incident.clone();

    // The commenter builds its mutation from the aggregate it observed.  This
    // is the two-writer race Task 1 could not stage: a comment carries no
    // version predicate, so it is the first write that can genuinely contend.
    let comment = incident
        .add_comment(
            1,
            "checked the dashboards",
            ACTOR,
            SECOND_REQUEST,
            POLICY_VERSION,
            later(),
        )
        .expect("the comment is valid");

    // Another writer transitions first, so the stored row is a version ahead
    // and the timeline is two events taller than the commenter believes.
    let applied = fixture
        .repository
        .apply_mutation(triage_mutation(&incident, THIRD_REQUEST, 3))
        .expect("the transition lands first");

    let stored = fixture
        .repository
        .append_comment(comment)
        .expect("a comment behind a concurrent transition still appends");

    assert_eq!(stored.events.len(), 1);
    assert_eq!(stored.events[0].kind, IncidentEventKind::Commented);
    assert!(
        stored.events[0].sequence
            > applied
                .events
                .last()
                .expect("the transition appended events")
                .sequence,
        "the comment is numbered after the transition it lost to"
    );
    assert_eq!(
        stored.incident.version, applied.incident.version,
        "the caller is handed the stored version, not its stale copy"
    );

    let reloaded = fixture
        .repository
        .get(WORKSPACE, incident.id)
        .expect("incident is readable");
    assert_eq!(
        reloaded.version, applied.incident.version,
        "a comment writes no version"
    );
    assert_eq!(
        reloaded.status, applied.incident.status,
        "a comment overwrites no incident state"
    );
    assert_eq!(
        reloaded.updated_at,
        later(),
        "a comment touches only updated_at"
    );

    let timeline = fixture
        .repository
        .timeline(WORKSPACE, incident.id, None, 100)
        .expect("timeline is readable");
    let sequences: Vec<u64> = timeline.events.iter().map(|event| event.sequence).collect();
    let mut unique = sequences.clone();
    unique.dedup();
    assert_eq!(sequences, unique, "timeline sequences stay unique");
    assert_eq!(
        timeline.events.len(),
        created.events.len() + applied.events.len() + 1
    );
}

#[test]
fn replaying_a_comment_request_id_returns_the_stored_comment() {
    let mut fixture = fixture();
    let created = fixture
        .repository
        .create(creation_record(WORKSPACE, CREATE_REQUEST, 1))
        .expect("creation succeeds");
    let comment = created
        .incident
        .add_comment(
            1,
            "checked the dashboards",
            ACTOR,
            SECOND_REQUEST,
            POLICY_VERSION,
            later(),
        )
        .expect("the comment is valid");

    let first = fixture
        .repository
        .append_comment(comment.clone())
        .expect("the comment appends");
    let replay = fixture
        .repository
        .append_comment(comment)
        .expect("the retry replays rather than appending twice");

    assert_eq!(first.events, replay.events);
    assert_eq!(
        fixture
            .repository
            .timeline(WORKSPACE, created.incident.id, None, 100)
            .expect("timeline is readable")
            .events
            .len(),
        created.events.len() + 1,
        "a replayed comment appends nothing"
    );
}

#[test]
fn a_comment_on_an_incident_from_another_workspace_is_not_found() {
    let mut fixture = fixture();
    let created = fixture
        .repository
        .create(creation_record(OTHER_WORKSPACE, CREATE_REQUEST, 1))
        .expect("creation succeeds");
    let mut foreign = created.incident.clone();
    foreign.scope = scope_for(WORKSPACE);

    let comment = foreign
        .add_comment(
            1,
            "should not land",
            ACTOR,
            SECOND_REQUEST,
            POLICY_VERSION,
            later(),
        )
        .expect("the comment is valid");

    assert!(matches!(
        fixture.repository.append_comment(comment),
        Err(IncidentStoreError::NotFound)
    ));
    assert_eq!(
        fixture
            .repository
            .timeline(OTHER_WORKSPACE, created.incident.id, None, 100)
            .expect("timeline is readable")
            .events
            .len(),
        created.events.len(),
        "the foreign timeline is untouched"
    );
}

#[test]
fn two_comments_allocated_from_the_same_observation_do_not_collide() {
    let mut fixture = fixture();
    let created = fixture
        .repository
        .create(creation_record(WORKSPACE, CREATE_REQUEST, 1))
        .expect("creation succeeds");
    let incident = created.incident.clone();

    // Both writers read the same timeline height and both ask for the sequence
    // after it.  Neither carries a version predicate, so neither can be
    // rejected: only the in-transaction reallocation stands between them and a
    // `UNIQUE (incident_id, sequence)` violation.
    let first = incident
        .add_comment(1, "first", ACTOR, SECOND_REQUEST, POLICY_VERSION, later())
        .expect("the first comment is valid");
    let second = incident
        .add_comment(1, "second", ACTOR, THIRD_REQUEST, POLICY_VERSION, later())
        .expect("the second comment is valid");

    let first = fixture
        .repository
        .append_comment(first)
        .expect("the first comment appends");
    let second = fixture
        .repository
        .append_comment(second)
        .expect("the second comment appends rather than colliding");

    let highest_at_creation = created
        .events
        .last()
        .expect("creation appended events")
        .sequence;
    assert_eq!(first.events[0].sequence, highest_at_creation + 1);
    assert_eq!(second.events[0].sequence, highest_at_creation + 2);

    let timeline = fixture
        .repository
        .timeline(WORKSPACE, incident.id, None, 100)
        .expect("timeline is readable");
    assert_eq!(timeline.events.len(), created.events.len() + 2);
    let sequences: Vec<u64> = timeline.events.iter().map(|event| event.sequence).collect();
    let mut unique = sequences.clone();
    unique.dedup();
    assert_eq!(sequences, unique, "timeline sequences stay unique");
}

#[test]
fn the_version_free_path_refuses_an_event_that_carries_state() {
    let mut fixture = fixture();
    let created = fixture
        .repository
        .create(creation_record(WORKSPACE, CREATE_REQUEST, 1))
        .expect("creation succeeds");

    // A transition event through the comment path would be recorded on the
    // timeline while none of its aggregate state was ever applied.
    let transition = triage_mutation(&created.incident, SECOND_REQUEST, 3);
    assert!(matches!(
        fixture.repository.append_comment(transition),
        Err(IncidentStoreError::InvalidMutation(_))
    ));

    let mut with_reason = created
        .incident
        .add_comment(
            1,
            "carries a reason",
            ACTOR,
            THIRD_REQUEST,
            POLICY_VERSION,
            later(),
        )
        .expect("the comment is valid");
    with_reason.events[0].reason = Some("not a comment field".into());
    assert!(matches!(
        fixture.repository.append_comment(with_reason),
        Err(IncidentStoreError::InvalidMutation(_))
    ));

    assert_eq!(
        fixture
            .repository
            .timeline(WORKSPACE, created.incident.id, None, 100)
            .expect("timeline is readable")
            .events
            .len(),
        created.events.len(),
        "a rejected append writes nothing"
    );
}

#[test]
fn a_late_comment_never_drags_updated_at_backwards() {
    let mut fixture = fixture();
    let created = fixture
        .repository
        .create(creation_record(WORKSPACE, CREATE_REQUEST, 1))
        .expect("creation succeeds");
    let incident = created.incident.clone();

    // The comment is built at `now()`; the transition that beats it to the
    // store is stamped `later()`.  `updated_at` orders the incident queue, so
    // letting the comment win would reshuffle it.
    let comment = incident
        .add_comment(
            1,
            "built earlier",
            ACTOR,
            SECOND_REQUEST,
            POLICY_VERSION,
            now(),
        )
        .expect("the comment is valid");
    fixture
        .repository
        .apply_mutation(triage_mutation(&incident, THIRD_REQUEST, 3))
        .expect("the transition lands first");

    let stored = fixture
        .repository
        .append_comment(comment)
        .expect("the comment still appends");

    assert_eq!(stored.incident.updated_at, later());
    assert_eq!(
        fixture
            .repository
            .get(WORKSPACE, incident.id)
            .expect("incident is readable")
            .updated_at,
        later(),
        "the stored timestamp does not move backwards"
    );
}
