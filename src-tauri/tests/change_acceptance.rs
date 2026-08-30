// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use serde_json::Value;
use tempfile::tempdir;
use thalassa_domain::{
    ChangeRequest, CorrelationQualification, ResourceScope, SourceRecordRef, TimeWindow,
};
use thalassa_ipc::{Capability, CommandEnvelope, CommandName};
use thalassaops::app::{AppState, IpcResult};
use thalassaops::change::{adapters, fixtures as change_fixtures, timeline};
use thalassaops::connectors::InMemoryCredentialStore;
use thalassaops::correlation::SourceRecordStore;
use uuid::Uuid;

fn test_state() -> (tempfile::TempDir, AppState) {
    let directory = tempdir().unwrap();
    let state = AppState::open_with_credential_store(
        directory.path().join("thalassaops.sqlite"),
        Arc::new(InMemoryCredentialStore::default()),
    )
    .unwrap();
    (directory, state)
}

fn request() -> ChangeRequest {
    ChangeRequest {
        window: TimeWindow {
            start: "2026-08-28T08:00:00Z".into(),
            end: "2026-08-28T09:00:00Z".into(),
        },
        evaluated_at: "2026-08-28T09:00:00Z".into(),
        lookback_seconds: 3_600,
        limit: 50,
    }
}

fn snapshot_envelope() -> CommandEnvelope<Value> {
    CommandEnvelope {
        request_id: Uuid::new_v4(),
        command: CommandName::new("change", "snapshot").unwrap(),
        capability: Capability::WorkspaceRead,
        scope: ResourceScope::default(),
        payload: serde_json::to_value(request()).unwrap(),
    }
}

#[test]
fn the_exit_criterion_holds_end_to_end() {
    let (_directory, state) = test_state();
    let IpcResult::Ok {
        value: snapshot, ..
    } = state.change_snapshot(snapshot_envelope())
    else {
        panic!("change.snapshot should succeed")
    };

    assert!(
        !snapshot.associations.is_empty(),
        "a responder must be able to see what changed before a correlated candidate"
    );

    let mut reachable_source_links = 0;
    for association in &snapshot.associations {
        let change = snapshot
            .events
            .iter()
            .find(|event| event.id == association.change_id)
            .expect("every association names a change inside the snapshot");

        // Precedence: the association is a measured interval inside the
        // lookback, never a score.
        assert_eq!(
            association.qualification,
            CorrelationQualification::ProbableStructural
        );
        assert!(association.lead_time_seconds > 0.0);
        assert!(association.lead_time_seconds <= snapshot.lookback_seconds as f64);

        // Structure: an exact shared target or at least one topology path.
        assert!(
            association.target.is_some() || !association.topology_path_ids.is_empty(),
            "temporal proximity alone must not create an association"
        );
        if let Some(target) = &association.target {
            assert!(change.targets.contains(target));
        }

        if let Some(link) = &change.source_link {
            assert!(link.url.starts_with("https://"));
            assert!(!link.url.contains('?'));
            reachable_source_links += 1;
        }
    }

    assert!(
        reachable_source_links > 0,
        "at least one preceding change must be inspectable at its source"
    );
}

#[test]
fn no_snapshot_field_contains_a_credential_email_or_diff_body() {
    let (_directory, state) = test_state();
    let IpcResult::Ok {
        value: snapshot, ..
    } = state.change_snapshot(snapshot_envelope())
    else {
        panic!("change.snapshot should succeed")
    };

    let serialized = serde_json::to_string(&snapshot).unwrap();
    for marker in [
        "Bearer ",
        "private_token",
        "@@ -",
        "\"patch\"",
        "diff --git",
        "authorization",
    ] {
        assert!(
            !serialized.contains(marker),
            "the serialized snapshot must not contain {marker}"
        );
    }
    // An email-shaped actor is rejected at normalization, so no local-part /
    // domain pair may survive anywhere in the serialized snapshot.
    assert!(
        !serialized.chars().any(|character| character == '@'),
        "an email-shaped actor must never reach the snapshot"
    );
}

#[test]
fn shuffled_fixture_order_produces_an_identical_snapshot() {
    let scope = ResourceScope::workspace(
        Uuid::from_u128(31),
        Uuid::from_u128(32),
        Uuid::from_u128(33),
    );
    let window = request().window;

    let ordered = replay_projection(change_fixtures::catalog(), &scope, &window);
    let mut shuffled_catalog = change_fixtures::catalog();
    shuffled_catalog.reverse();
    let shuffled = replay_projection(shuffled_catalog, &scope, &window);

    assert_eq!(ordered, shuffled);
}

fn replay_projection(
    fixtures: Vec<change_fixtures::ChangeFixture>,
    scope: &ResourceScope,
    window: &TimeWindow,
) -> String {
    let mut store = SourceRecordStore::with_scope(scope.clone());
    let output = adapters::replay_from(
        fixtures,
        &mut store,
        scope,
        change_fixtures::fixture_clock(),
    )
    .expect("committed fixtures replay");
    let timeline = timeline::build(&output.events, window, 50).expect("timeline builds");
    let records = output
        .events
        .iter()
        .map(|event| event.source_record.clone())
        .collect::<Vec<SourceRecordRef>>();
    serde_json::to_string(&(output.events, timeline, output.statuses, records))
        .expect("projection serializes")
}
