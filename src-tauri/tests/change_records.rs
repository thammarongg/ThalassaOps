use rusqlite::Connection;
use tempfile::tempdir;
use thalassa_domain::{ChangeActorKind, ResourceScope, SourceState};
use thalassaops::app::AppState;
use thalassaops::change::{fixtures, normalize, records};
use thalassaops::correlation::SourceRecordStore;
use uuid::Uuid;

fn fixture_scope() -> ResourceScope {
    ResourceScope::workspace(Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3))
}

fn memory_store(scope: ResourceScope) -> SourceRecordStore {
    let connection = Connection::open_in_memory().expect("in-memory database");
    connection
        .execute_batch(include_str!("../migrations/0005_change_records.sql"))
        .expect("change-record migration applies");
    SourceRecordStore::with_connection_and_scope(connection, scope)
        .expect("source-record store opens")
}

fn admit_fixture(store: &mut SourceRecordStore, path_suffix: &str) -> records::AdmittedRecord {
    let fixture = fixtures::catalog()
        .into_iter()
        .find(|fixture| fixture.path.ends_with(path_suffix))
        .expect("fixture present");
    records::admit(
        store,
        fixture.payload,
        fixture.source,
        &fixture_scope(),
        fixtures::fixture_clock(),
    )
    .expect("record admitted")
}

#[test]
fn admitted_record_preserves_unknown_fields_and_drops_diff_bodies() {
    let mut store = memory_store(fixture_scope());
    let admitted = admit_fixture(&mut store, "argocd/sync-failed.json");

    assert!(admitted.body.get("unknownOperatorField").is_some());
    assert!(admitted.record_ref.content_digest.len() >= 32);
}

#[test]
fn diff_bodies_never_enter_the_retained_record() {
    let mut store = memory_store(fixture_scope());
    let admitted = admit_fixture(&mut store, "github/push.json");

    let serialized = serde_json::to_string(&admitted.body).unwrap();
    assert!(!serialized.contains("\"patch\""));
    assert!(!serialized.contains("@@ -"));
}

#[test]
fn retained_records_survive_a_reopen_of_the_database() {
    let directory = tempdir().unwrap();
    let database_path = directory.path().join("thalassaops.sqlite");
    let state = AppState::open(&database_path).expect("app database opens");
    let scope = ResourceScope::workspace(
        state.bootstrap.workspace.id,
        state.bootstrap.team.id,
        state.bootstrap.organization.id,
    );
    let digests = {
        let connection = Connection::open(&database_path).unwrap();
        let mut store = SourceRecordStore::with_connection_and_scope_and_policy(
            connection,
            scope.clone(),
            state.policy.clone(),
        )
        .unwrap();
        fixtures::catalog()
            .into_iter()
            .map(|fixture| {
                records::admit(
                    &mut store,
                    fixture.payload,
                    fixture.source,
                    &scope,
                    fixtures::fixture_clock(),
                )
                .expect("record admitted")
                .record_ref
                .content_digest
            })
            .collect::<Vec<_>>()
    };

    drop(state);
    let reopened = AppState::open(&database_path).expect("app database reopens");
    let connection = Connection::open(&database_path).unwrap();
    let migration: i64 = connection
        .query_row(
            "SELECT version FROM schema_migrations WHERE version = 5",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(migration, 5);
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM change_source_record", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, digests.len() as i64);
    for digest in digests {
        let stored: String = connection
            .query_row(
                "SELECT content_digest FROM change_source_record WHERE content_digest = ?1",
                [&digest],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored, digest);
    }
    assert_eq!(reopened.bootstrap.workspace.id, scope.workspace_id.unwrap());
}

#[test]
fn admitted_evidence_resolves_through_the_sprint_13_evidence_store() {
    let scope = fixture_scope();
    let mut store = memory_store(scope);
    let admitted = admit_fixture(&mut store, "github/push.json");

    assert!(!admitted.evidence.is_empty());
    for evidence in &admitted.evidence {
        assert!(
            records::resolve_evidence(&store, &evidence.id).is_some(),
            "evidence must resolve through the existing source_record_evidence store"
        );
    }
}

#[test]
fn committed_fixture_identities_pass_the_safe_identity_control() {
    let scope = fixture_scope();
    let mut store = memory_store(scope.clone());
    for fixture in fixtures::catalog() {
        let admitted = records::admit(
            &mut store,
            fixture.payload,
            fixture.source,
            &scope,
            fixtures::fixture_clock(),
        )
        .expect("record admitted");
        admitted
            .record_ref
            .validate()
            .expect("fixture source identity is safe");
    }
}

#[test]
fn normalization_records_typed_downgrades_for_unsafe_actor_and_link() {
    let scope = fixture_scope();
    let mut store = memory_store(scope);
    let admitted = admit_fixture(&mut store, "github/pull-request-merged.json");
    let normalized = normalize::to_change_event(&admitted).expect("record normalizes");

    assert_eq!(normalized.event.actor.kind, ChangeActorKind::Unknown);
    assert!(normalized.event.actor.handle.is_none());
    assert!(normalized.event.source_link.is_some());
    assert!(normalized
        .statuses
        .iter()
        .any(|status| status.state != SourceState::Fresh));
}

#[test]
fn normalization_rejects_a_missing_occurred_at_without_using_ingestion_time() {
    let mut store = memory_store(fixture_scope());
    let mut admitted = admit_fixture(&mut store, "github/push.json");
    admitted.body["head_commit"]
        .as_object_mut()
        .unwrap()
        .remove("timestamp");

    assert_eq!(
        normalize::to_change_event(&admitted),
        Err(thalassa_domain::ChangeError::MissingTimestamp)
    );
}
