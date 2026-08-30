use thalassa_domain::{
    ChangeActorKind, ChangeKind, ChangeOutcome, EvidenceSourceKind, SignalTarget, SignalTargetKind,
    SourceState,
};
use thalassaops::change::{adapters, fixtures};
mod change_support;

use change_support::{fixture_scope, memory_store};

#[test]
fn every_fixture_normalizes_to_exactly_one_event() {
    let scope = fixture_scope();
    let mut store = memory_store(scope.clone());
    let output = adapters::replay_all(&mut store, &scope, fixtures::fixture_clock())
        .expect("replay succeeds");
    assert_eq!(output.events.len(), 9);
}

#[test]
fn merged_pull_request_maps_to_code_merge_with_rejected_email_actor() {
    let scope = fixture_scope();
    let mut store = memory_store(scope.clone());
    let output = adapters::replay_all(&mut store, &scope, fixtures::fixture_clock()).unwrap();
    let event = output
        .events
        .iter()
        .find(|e| e.source == EvidenceSourceKind::GitHub && e.kind == ChangeKind::CodeMerge)
        .expect("merged pull request present");

    assert_eq!(event.actor.kind, ChangeActorKind::Unknown);
    assert!(event.actor.handle.is_none());
    assert!(output
        .statuses
        .iter()
        .any(|s| s.state != SourceState::Fresh));
}

#[test]
fn credentialed_link_is_dropped_not_emitted() {
    let scope = fixture_scope();
    let mut store = memory_store(scope.clone());
    let output = adapters::replay_all(&mut store, &scope, fixtures::fixture_clock()).unwrap();
    let event = output
        .events
        .iter()
        .find(|e| e.source == EvidenceSourceKind::GitLab && e.kind == ChangeKind::Deployment)
        .expect("gitlab deployment present");

    assert!(event.source_link.is_none());
}

#[test]
fn failed_argo_sync_maps_to_failed_outcome_and_rollback_to_rollback_kind() {
    let scope = fixture_scope();
    let mut store = memory_store(scope.clone());
    let output = adapters::replay_all(&mut store, &scope, fixtures::fixture_clock()).unwrap();
    assert!(output
        .events
        .iter()
        .any(|e| e.source == EvidenceSourceKind::ArgoCd
            && e.kind == ChangeKind::Sync
            && e.outcome == ChangeOutcome::Failed));
    assert!(output.events.iter().any(|e| e.kind == ChangeKind::Rollback));
}

#[test]
fn replay_is_order_independent() {
    let scope = fixture_scope();
    let mut first_store = memory_store(scope.clone());
    let first = adapters::replay_all(&mut first_store, &scope, fixtures::fixture_clock()).unwrap();

    let mut shuffled = fixtures::catalog();
    shuffled.reverse();
    let mut second_store = memory_store(scope.clone());
    let second = adapters::replay_from(
        shuffled,
        &mut second_store,
        &scope,
        fixtures::fixture_clock(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_string(&first.events).unwrap(),
        serde_json::to_string(&second.events).unwrap()
    );
}

#[test]
fn change_targets_use_the_correlation_deployment_naming_convention() {
    let scope = fixture_scope();
    let mut store = memory_store(scope.clone());
    let output = adapters::replay_all(&mut store, &scope, fixtures::fixture_clock()).unwrap();

    let targets: Vec<&SignalTarget> = output
        .events
        .iter()
        .flat_map(|event| event.targets.iter())
        .collect();
    assert!(!targets.is_empty());
    for target in targets {
        // Sprint 13 signals name a deployment `deployment/<name>`; a change
        // that names the same deployment must produce the identical value or
        // association degrades into a string heuristic.
        assert_eq!(target.kind, SignalTargetKind::Deployment);
        assert!(
            target.id.starts_with("deployment/"),
            "unexpected change target id {}",
            target.id
        );
    }
}
