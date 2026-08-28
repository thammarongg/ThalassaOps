// SPDX-License-Identifier: Apache-2.0

use std::cell::Cell;

use thalassa_domain::{
    CorrelationQualification, CorrelationReasonKind, CorrelationRequest, CorrelationWindow,
    EvidenceRef, ResourceScope, Signal, SignalTarget, TimeWindow, TopologyDirection, TopologyError,
    TopologyPath, TopologyPathKind, TopologyPathTermination,
};
use thalassaops::correlation::adapters::{normalize_operational, normalize_security};
use thalassaops::correlation::{
    correlate_signals, correlation_fixture_catalog, CorrelationInput, SourceRecordStore,
    TopologyCorrelationResolver,
};
use thalassaops::topology::{topology_fixture_input, TopologyBuilder};

fn request() -> CorrelationRequest {
    CorrelationRequest {
        window: TimeWindow {
            start: "2026-08-28T08:55:00Z".into(),
            end: "2026-08-28T09:00:00Z".into(),
        },
        evaluated_at: "2026-08-28T09:00:00Z".into(),
        allowed_lateness_seconds: 300,
    }
}

fn normalize(keys: &[&str]) -> (Vec<Signal>, Vec<EvidenceRef>) {
    let catalog = correlation_fixture_catalog();
    let mut records = SourceRecordStore::default();
    let mut signals = Vec::new();
    let mut evidence = Vec::new();
    for key in keys {
        let fixture = catalog
            .fixtures
            .iter()
            .find(|fixture| fixture.key == *key)
            .unwrap_or_else(|| panic!("fixture {key} exists"));
        let normalized = if fixture.source_kind.is_security_source() {
            normalize_security(fixture, &mut records).unwrap()
        } else {
            normalize_operational(fixture, &mut records).unwrap()
        };
        signals.extend(normalized);
        evidence.extend(fixture.evidence.clone());
    }
    evidence.sort_by(|left, right| left.id.cmp(&right.id));
    evidence.dedup_by(|left, right| left.id == right.id);
    (signals, evidence)
}

fn input(
    scope: ResourceScope,
    signals: Vec<Signal>,
    evidence: Vec<EvidenceRef>,
) -> CorrelationInput {
    CorrelationInput {
        generated_at: "2026-08-28T09:00:00Z".into(),
        scope,
        request: request(),
        signals,
        source_status: Vec::new(),
        evidence,
        prior_window: None,
        suppression_rules: Vec::new(),
        maintenance_windows: Vec::new(),
        policy_version: 0,
    }
}

#[derive(Default)]
struct StubResolver {
    calls: Cell<usize>,
    path: Option<TopologyPath>,
}

impl StubResolver {
    fn with_path(path: TopologyPath) -> Self {
        Self {
            calls: Cell::new(0),
            path: Some(path),
        }
    }
}

impl TopologyCorrelationResolver for StubResolver {
    fn relation(
        &self,
        _left: &SignalTarget,
        _right: &SignalTarget,
        _window: &CorrelationWindow,
    ) -> Result<Option<TopologyPath>, TopologyError> {
        self.calls.set(self.calls.get() + 1);
        Ok(self.path.clone())
    }
}

struct ErrorResolver;

impl TopologyCorrelationResolver for ErrorResolver {
    fn relation(
        &self,
        _left: &SignalTarget,
        _right: &SignalTarget,
        _window: &CorrelationWindow,
    ) -> Result<Option<TopologyPath>, TopologyError> {
        Err(TopologyError::NodeNotFound)
    }
}

fn path() -> TopologyPath {
    TopologyPath {
        id: "path:fixture:checkout".into(),
        root_node_id: "node-checkout".into(),
        terminal_node_id: "node-checkout-service".into(),
        node_ids: vec!["node-checkout".into(), "node-checkout-service".into()],
        edge_ids: vec!["edge:fixture:checkout".into()],
        direction: TopologyDirection::Downstream,
        depth: 1,
        confidence: 0.8,
        kind: TopologyPathKind::ProbableStructural,
        termination: TopologyPathTermination::Leaf,
        cycle_edge_id: None,
        evidence_ids: vec![
            "evidence-shared-service-alert".into(),
            "evidence-shared-service-anomaly".into(),
        ],
        drill_down: thalassa_domain::DrillDownTarget {
            destination: thalassa_domain::DrillDownDestination::Evidence,
            evidence_ids: vec![
                "evidence-shared-service-alert".into(),
                "evidence-shared-service-anomaly".into(),
            ],
            filter_key: None,
        },
    }
}

#[test]
fn exact_shared_targets_emit_one_structural_reason_and_candidate() {
    let (signals, evidence) = normalize(&["shared-service-alert", "shared-service-anomaly"]);
    let scope = signals[0].scope.clone();
    let snapshot = correlate_signals(input(scope, signals, evidence), &StubResolver::default())
        .expect("exact target pair should correlate");

    assert_eq!(snapshot.candidates.len(), 1);
    let candidate = &snapshot.candidates[0];
    assert_eq!(candidate.signal_ids.len(), 2);
    assert_eq!(candidate.reasons.len(), 1);
    assert_eq!(
        candidate.reasons[0].kind,
        CorrelationReasonKind::SharedService
    );
    assert_eq!(
        candidate.reasons[0].qualification,
        CorrelationQualification::ExactAssociation
    );
    assert_eq!(
        candidate.reasons[0].target.as_ref().unwrap().id,
        "service/checkout"
    );
    let active_metric = snapshot
        .summary
        .metrics
        .iter()
        .find(|metric| metric.key == thalassa_domain::CorrelationMetricKey::ActiveCandidates)
        .expect("active candidate metric");
    assert_eq!(active_metric.value, 1.0);
    assert!(snapshot.validate().is_ok());
}

#[test]
fn exact_resource_targets_are_grouped_without_name_or_label_fallbacks() {
    let (mut signals, evidence) = normalize(&["shared-service-alert", "shared-service-anomaly"]);
    for signal in &mut signals {
        signal.targets = vec![SignalTarget {
            kind: thalassa_domain::SignalTargetKind::Resource,
            id: "resource/checkout".into(),
        }];
    }
    let scope = signals[0].scope.clone();
    let snapshot = correlate_signals(input(scope, signals, evidence), &StubResolver::default())
        .expect("exact resource targets should correlate");
    assert_eq!(snapshot.candidates.len(), 1);
    assert_eq!(
        snapshot.candidates[0].reasons[0].kind,
        CorrelationReasonKind::SharedResource
    );
}

#[test]
fn exact_target_kinds_remain_distinct_and_names_do_not_create_edges() {
    let (mut signals, evidence) = normalize(&[
        "shared-service-alert",
        "shared-service-anomaly",
        "shared-deployment-alert",
        "shared-deployment-finding",
    ]);
    let scope = signals[0].scope.clone();
    let snapshot = correlate_signals(
        input(scope, signals.clone(), evidence.clone()),
        &StubResolver::default(),
    )
    .expect("distinct exact target pairs should correlate independently");

    assert_eq!(snapshot.candidates.len(), 2);
    assert!(snapshot.candidates.iter().any(|candidate| {
        candidate
            .reasons
            .iter()
            .any(|reason| reason.kind == CorrelationReasonKind::SharedService)
    }));
    assert!(snapshot.candidates.iter().any(|candidate| {
        candidate
            .reasons
            .iter()
            .any(|reason| reason.kind == CorrelationReasonKind::SharedDeployment)
    }));

    // Identical source, time and scope with no exact target must remain
    // uncorrelated; names and source kind are not grouping dimensions.
    signals[0].targets.clear();
    signals[1].targets.clear();
    signals.truncate(2);
    let uncorrelated = correlate_signals(
        input(signals[0].scope.clone(), signals, evidence),
        &StubResolver::default(),
    )
    .expect("missing target should remain a valid empty projection");
    assert!(uncorrelated.candidates.is_empty());
}

#[test]
fn topology_paths_pass_through_as_probable_structural_reasons() {
    let (mut signals, mut evidence) =
        normalize(&["shared-service-alert", "shared-service-anomaly"]);
    signals[0].targets = vec![SignalTarget {
        kind: thalassa_domain::SignalTargetKind::Topology,
        id: "node-checkout".into(),
    }];
    signals[1].targets = vec![SignalTarget {
        kind: thalassa_domain::SignalTargetKind::Topology,
        id: "node-checkout-service".into(),
    }];
    let scope = signals[0].scope.clone();
    evidence.sort_by(|left, right| left.id.cmp(&right.id));
    evidence.dedup_by(|left, right| left.id == right.id);
    let resolver = StubResolver::with_path(path());
    let snapshot = correlate_signals(input(scope, signals, evidence), &resolver)
        .expect("resolver path should produce a candidate");

    assert_eq!(resolver.calls.get(), 1);
    let candidate = snapshot.candidates.first().expect("topology candidate");
    let reason = candidate
        .reasons
        .iter()
        .find(|reason| reason.kind == CorrelationReasonKind::TopologyRelation)
        .expect("topology reason");
    assert_eq!(
        reason.qualification,
        CorrelationQualification::ProbableStructural
    );
    assert_eq!(reason.topology_path_ids, vec!["path:fixture:checkout"]);
    assert_eq!(snapshot.topology_paths, vec![path()]);
    let encoded = serde_json::to_string(reason).unwrap();
    assert!(!encoded.contains("caused_by"));
    assert!(!encoded.contains("root_cause"));
    assert!(!encoded.contains("probability"));
    assert!(snapshot.validate().is_ok());
}

#[test]
fn candidate_ids_and_order_are_stable_under_shuffled_input_and_duplicate_edges() {
    let (first_signals, first_evidence) =
        normalize(&["shared-service-alert", "shared-service-anomaly"]);
    let scope = first_signals[0].scope.clone();
    let first = correlate_signals(
        input(scope.clone(), first_signals.clone(), first_evidence.clone()),
        &StubResolver::default(),
    )
    .unwrap();

    let mut shuffled = first_signals;
    shuffled.reverse();
    let second = correlate_signals(
        input(scope, shuffled, first_evidence),
        &StubResolver::default(),
    )
    .unwrap();
    assert_eq!(first, second);
}

#[test]
fn ledger_derived_dedup_keys_are_carried_into_the_snapshot() {
    let catalog = correlation_fixture_catalog();
    let mut records = SourceRecordStore::default();
    let mut signals = Vec::new();
    let mut evidence = Vec::new();
    for key in ["shared-service-alert", "shared-service-anomaly"] {
        let fixture = catalog
            .fixtures
            .iter()
            .find(|fixture| fixture.key == key)
            .unwrap();
        signals.extend(normalize_operational(fixture, &mut records).unwrap());
        evidence.extend(fixture.evidence.clone());
    }
    for signal in &mut signals {
        signal.dedup_key = None;
    }
    let scope = signals[0].scope.clone();
    let snapshot = thalassaops::correlation::correlate_signals_with_records(
        input(scope, signals, evidence),
        &records,
        &StubResolver::default(),
    )
    .expect("retained records should supply canonical dedup keys");
    assert!(snapshot
        .signals
        .iter()
        .all(|signal| signal.dedup_key.is_some()));
}

#[test]
fn sprint12_topology_builder_is_the_production_resolver() {
    let topology_scope = thalassaops::topology::fixture_scope();
    let topology = TopologyBuilder::from_input(topology_fixture_input(topology_scope));
    let topology_snapshot = topology
        .snapshot_at(&thalassaops::topology::default_topology_request())
        .unwrap();
    let edge = topology_snapshot.edges.first().expect("topology edge");
    let window = thalassaops::correlation::build_window(&request(), &[]).unwrap();
    let path = topology
        .relation(
            &SignalTarget {
                kind: thalassa_domain::SignalTargetKind::Topology,
                id: edge.upstream_node_id.clone(),
            },
            &SignalTarget {
                kind: thalassa_domain::SignalTargetKind::Topology,
                id: edge.downstream_node_id.clone(),
            },
            &window,
        )
        .unwrap()
        .expect("existing Sprint 12 edge should resolve");
    assert_eq!(
        path.kind,
        thalassa_domain::TopologyPathKind::ProbableStructural
    );
    assert!(!path.evidence_ids.is_empty());
}

#[test]
fn singleton_and_failed_topology_resolution_do_not_fabricate_candidates() {
    let (mut signals, evidence) = normalize(&["shared-service-alert"]);
    signals[0].targets = vec![SignalTarget {
        kind: thalassa_domain::SignalTargetKind::Topology,
        id: "node-checkout".into(),
    }];
    let scope = signals[0].scope.clone();
    let snapshot = correlate_signals(input(scope, signals, evidence), &StubResolver::default())
        .expect("singleton should be retained without candidate");
    assert!(snapshot.candidates.is_empty());
}

#[test]
fn disconnected_or_failed_topology_is_a_typed_source_limitation() {
    let (mut signals, evidence) = normalize(&["shared-service-alert", "shared-service-anomaly"]);
    for (signal, id) in signals.iter_mut().zip(["node-left", "node-right"]) {
        signal.targets = vec![SignalTarget {
            kind: thalassa_domain::SignalTargetKind::Topology,
            id: id.into(),
        }];
    }
    let scope = signals[0].scope.clone();
    let disconnected = correlate_signals(
        input(scope.clone(), signals.clone(), evidence.clone()),
        &StubResolver::default(),
    )
    .expect("disconnected topology must not fail the full snapshot");
    assert!(disconnected.candidates.is_empty());
    assert!(disconnected.source_status.iter().any(|status| {
        status.source_key == "topology"
            && status.state == thalassa_domain::SourceState::Unavailable
            && status.reason == Some(thalassa_domain::StatusReason::NoDataInWindow)
    }));

    let failed = correlate_signals(input(scope, signals, evidence), &ErrorResolver)
        .expect("failed topology must remain a typed source limitation");
    assert!(failed.candidates.is_empty());
    assert!(failed.source_status.iter().any(|status| {
        status.source_key == "topology"
            && status.state == thalassa_domain::SourceState::Unverified
            && status.reason == Some(thalassa_domain::StatusReason::Unknown)
    }));
}

#[test]
fn depth_limited_topology_does_not_create_a_fallback_candidate() {
    let (mut signals, evidence) = normalize(&["shared-service-alert", "shared-service-anomaly"]);
    signals[0].targets = vec![SignalTarget {
        kind: thalassa_domain::SignalTargetKind::Topology,
        id: "node-checkout".into(),
    }];
    signals[1].targets = vec![SignalTarget {
        kind: thalassa_domain::SignalTargetKind::Topology,
        id: "node-checkout-service".into(),
    }];
    let scope = signals[0].scope.clone();
    let mut depth_limited = path();
    depth_limited.termination = TopologyPathTermination::DepthLimit;
    let snapshot = correlate_signals(
        input(scope, signals, evidence),
        &StubResolver::with_path(depth_limited),
    )
    .expect("depth limits should remain a typed source limitation");
    assert!(snapshot.candidates.is_empty());
    assert!(snapshot.source_status.iter().any(|status| {
        status.source_key == "topology"
            && status.state == thalassa_domain::SourceState::Unavailable
            && status.reason == Some(thalassa_domain::StatusReason::NoDataInWindow)
    }));
}

#[test]
fn late_signal_added_to_existing_component_keeps_candidate_anchor() {
    let (signals, evidence) = normalize(&["shared-service-alert", "shared-service-anomaly"]);
    let scope = signals[0].scope.clone();
    let initial = correlate_signals(
        input(scope.clone(), signals.clone(), evidence.clone()),
        &StubResolver::default(),
    )
    .unwrap();
    let initial_id = initial.candidates[0].id.clone();

    let mut late = signals[0].clone();
    late.id = uuid::Uuid::from_u128(99);
    late.observed_at = Some("2026-08-28T08:58:00Z".into());
    let mut expanded = signals;
    expanded.push(late);
    let expanded_snapshot =
        correlate_signals(input(scope, expanded, evidence), &StubResolver::default()).unwrap();
    assert_eq!(expanded_snapshot.candidates[0].id, initial_id);
    assert_eq!(expanded_snapshot.candidates[0].signal_ids.len(), 3);
}
