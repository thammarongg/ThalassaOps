use std::collections::BTreeSet;

use thalassa_domain::{
    ChangeError, ChangeEvent, CorrelationCandidate, CorrelationQualification, CorrelationRequest,
    CorrelationWindow, DrillDownDestination, DrillDownTarget, EvidenceRef, ResourceScope, Signal,
    SignalTarget, SignalTargetKind, TimeWindow, TopologyDirection, TopologyError, TopologyPath,
    TopologyPathKind, TopologyPathTermination,
};
use thalassaops::change::{adapters, association, fixtures as change_fixtures};
use thalassaops::correlation::adapters::{normalize_operational, normalize_security};
use thalassaops::correlation::{
    correlate_signals, correlation_fixture_catalog, CorrelationInput, SourceRecordStore,
    TopologyCorrelationResolver,
};
use uuid::Uuid;

mod change_support;

use change_support::{fixture_scope, memory_store};

fn target(kind: SignalTargetKind, id: &str) -> SignalTarget {
    SignalTarget {
        kind,
        id: id.into(),
    }
}

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

fn normalized_signals(keys: &[&str]) -> (Vec<Signal>, Vec<EvidenceRef>) {
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

fn candidate_of(
    signals: &[Signal],
    evidence: &[EvidenceRef],
    resolver: &dyn TopologyCorrelationResolver,
) -> CorrelationCandidate {
    let snapshot = correlate_signals(
        CorrelationInput {
            generated_at: "2026-08-28T09:00:00Z".into(),
            scope: signals[0].scope.clone(),
            request: request(),
            signals: signals.to_vec(),
            source_status: Vec::new(),
            evidence: evidence.to_vec(),
            prior_window: None,
            suppression_rules: Vec::new(),
            maintenance_windows: Vec::new(),
            policy_version: 13,
        },
        resolver,
    )
    .expect("candidate snapshot builds");
    snapshot
        .candidates
        .into_iter()
        .next()
        .expect("fixture signals form one candidate")
}

fn replayed_change() -> ChangeEvent {
    let scope = fixture_scope();
    let mut store = memory_store(scope.clone());
    adapters::replay_all(&mut store, &scope, change_fixtures::fixture_clock())
        .expect("change fixtures replay")
        .events
        .into_iter()
        .next()
        .expect("at least one replayed change")
}

fn change_at(occurred_at: &str, target: SignalTarget, scope: &ResourceScope) -> ChangeEvent {
    let mut change = replayed_change();
    change.occurred_at = occurred_at.into();
    change.scope = scope.clone();
    change.targets = vec![target];
    change.drill_down_reference.scope = scope.clone();
    change.validate().expect("test change remains valid");
    change
}

#[derive(Default)]
struct NoTopology;

impl TopologyCorrelationResolver for NoTopology {
    fn relation(
        &self,
        _left: &SignalTarget,
        _right: &SignalTarget,
        _window: &CorrelationWindow,
    ) -> Result<Option<TopologyPath>, TopologyError> {
        Ok(None)
    }
}

struct EchoTopology;

impl TopologyCorrelationResolver for EchoTopology {
    fn relation(
        &self,
        left: &SignalTarget,
        right: &SignalTarget,
        _window: &CorrelationWindow,
    ) -> Result<Option<TopologyPath>, TopologyError> {
        let evidence_ids = vec![
            "evidence-shared-service-alert".into(),
            "evidence-shared-service-anomaly".into(),
        ];
        Ok(Some(TopologyPath {
            id: format!("path:{}:{}", left.id, right.id),
            root_node_id: left.id.clone(),
            terminal_node_id: right.id.clone(),
            node_ids: vec![left.id.clone(), right.id.clone()],
            edge_ids: vec![format!("edge:{}:{}", left.id, right.id)],
            direction: TopologyDirection::Downstream,
            depth: 1,
            confidence: 0.8,
            kind: TopologyPathKind::ProbableStructural,
            termination: TopologyPathTermination::Leaf,
            cycle_edge_id: None,
            evidence_ids: evidence_ids.clone(),
            drill_down: DrillDownTarget {
                destination: DrillDownDestination::Evidence,
                evidence_ids,
                filter_key: None,
            },
        }))
    }
}

fn topology_candidate() -> (CorrelationCandidate, Vec<Signal>, Vec<EvidenceRef>) {
    let (mut signals, evidence) =
        normalized_signals(&["shared-service-alert", "shared-service-anomaly"]);
    signals[0].targets = vec![target(SignalTargetKind::Topology, "node-left")];
    signals[1].targets = vec![target(SignalTargetKind::Topology, "node-right")];
    let candidate = candidate_of(&signals, &evidence, &EchoTopology);
    (candidate, signals, evidence)
}

#[test]
fn temporal_proximity_without_structure_yields_no_association() {
    let scope = thalassaops::correlation::fixture_scope();
    let (signals, evidence) =
        normalized_signals(&["shared-service-alert", "shared-service-anomaly"]);
    let candidate = candidate_of(&signals, &evidence, &NoTopology);
    let change = change_at(
        "2026-08-28T08:55:00Z",
        target(SignalTargetKind::Deployment, "billing-worker"),
        &scope,
    );

    let associations =
        association::associate(&[change], &[candidate], &signals, 3_600.0, &NoTopology)
            .expect("association succeeds");

    assert!(associations.is_empty());
}

#[test]
fn exact_shared_target_within_lookback_associates_as_probable_structural() {
    let scope = thalassaops::correlation::fixture_scope();
    let (signals, evidence) =
        normalized_signals(&["shared-service-alert", "shared-service-anomaly"]);
    let candidate = candidate_of(&signals, &evidence, &NoTopology);
    let change = change_at(
        "2026-08-28T08:55:00Z",
        target(SignalTargetKind::Service, "service/checkout"),
        &scope,
    );

    let associations = association::associate(
        std::slice::from_ref(&change),
        std::slice::from_ref(&candidate),
        &signals,
        3_600.0,
        &NoTopology,
    )
    .expect("association succeeds");

    assert_eq!(associations.len(), 1);
    assert_eq!(associations[0].change_id, change.id);
    assert_eq!(associations[0].candidate_id, candidate.id);
    assert_eq!(
        associations[0].qualification,
        CorrelationQualification::ProbableStructural
    );
    assert_eq!(
        associations[0].target,
        Some(target(SignalTargetKind::Service, "service/checkout"))
    );
    assert!(associations[0].topology_path_ids.is_empty());
    assert_eq!(associations[0].lead_time_seconds, 60.0);
}

#[test]
fn topology_path_qualifies_and_records_path_ids() {
    let (candidate, signals, _evidence) = topology_candidate();
    let scope = signals[0].scope.clone();
    let change = change_at(
        "2026-08-28T08:55:00Z",
        target(SignalTargetKind::Topology, "node-change"),
        &scope,
    );

    let associations =
        association::associate(&[change], &[candidate], &signals, 3_600.0, &EchoTopology)
            .expect("association succeeds");

    assert_eq!(associations.len(), 1);
    assert!(associations[0].target.is_none());
    assert!(!associations[0].topology_path_ids.is_empty());
    assert!(associations[0]
        .topology_path_ids
        .windows(2)
        .all(|window| window[0] <= window[1]));
    assert_eq!(
        associations[0].qualification,
        CorrelationQualification::ProbableStructural
    );
    assert!(associations[0]
        .evidence_ids
        .contains(&"evidence-shared-service-alert".to_owned()));
}

#[test]
fn a_change_after_the_earliest_signal_never_associates() {
    let scope = thalassaops::correlation::fixture_scope();
    let (signals, evidence) =
        normalized_signals(&["shared-service-alert", "shared-service-anomaly"]);
    let candidate = candidate_of(&signals, &evidence, &NoTopology);
    let change = change_at(
        "2026-08-28T08:56:00Z",
        target(SignalTargetKind::Service, "service/checkout"),
        &scope,
    );

    let associations =
        association::associate(&[change], &[candidate], &signals, 3_600.0, &NoTopology)
            .expect("association succeeds");

    assert!(associations.is_empty());
}

#[test]
fn a_change_exactly_at_the_lookback_horizon_associates() {
    let scope = thalassaops::correlation::fixture_scope();
    let (signals, evidence) =
        normalized_signals(&["shared-service-alert", "shared-service-anomaly"]);
    let candidate = candidate_of(&signals, &evidence, &NoTopology);
    let change = change_at(
        "2026-08-28T07:56:00Z",
        target(SignalTargetKind::Service, "service/checkout"),
        &scope,
    );

    let associations =
        association::associate(&[change], &[candidate], &signals, 3_600.0, &NoTopology)
            .expect("association succeeds");

    assert_eq!(associations.len(), 1);
    assert_eq!(associations[0].lead_time_seconds, 3_600.0);
}

#[test]
fn lookback_above_cap_is_rejected() {
    let (signals, evidence) =
        normalized_signals(&["shared-service-alert", "shared-service-anomaly"]);
    let candidate = candidate_of(&signals, &evidence, &NoTopology);

    assert_eq!(
        association::associate(&[], &[candidate], &signals, 86_401.0, &NoTopology),
        Err(ChangeError::InvalidLookback)
    );
}

#[test]
fn all_candidate_signals_without_observed_at_produce_no_association() {
    let scope = thalassaops::correlation::fixture_scope();
    let (signals, evidence) =
        normalized_signals(&["shared-service-alert", "shared-service-anomaly"]);
    let candidate = candidate_of(&signals, &evidence, &NoTopology);
    let mut missing_observed = signals.clone();
    for signal in &mut missing_observed {
        signal.observed_at = None;
    }
    let change = change_at(
        "2026-08-28T08:55:00Z",
        target(SignalTargetKind::Service, "service/checkout"),
        &scope,
    );

    let associations = association::associate(
        &[change],
        &[candidate],
        &missing_observed,
        3_600.0,
        &NoTopology,
    )
    .expect("association succeeds");

    assert!(associations.is_empty());
}

#[test]
fn lead_time_uses_the_earliest_signal_and_is_finite_nonnegative() {
    let scope = thalassaops::correlation::fixture_scope();
    let (signals, evidence) =
        normalized_signals(&["shared-service-alert", "shared-service-anomaly"]);
    let candidate = candidate_of(&signals, &evidence, &NoTopology);
    let change = change_at(
        "2026-08-28T08:55:00Z",
        target(SignalTargetKind::Service, "service/checkout"),
        &scope,
    );

    let associations =
        association::associate(&[change], &[candidate], &signals, 3_600.0, &NoTopology)
            .expect("association succeeds");

    assert_eq!(associations.len(), 1);
    assert!(associations[0].lead_time_seconds.is_finite());
    assert!(associations[0].lead_time_seconds >= 0.0);
    assert_eq!(associations[0].lead_time_seconds, 60.0);
}

#[test]
fn associations_are_sorted_by_candidate_then_change() {
    let scope = thalassaops::correlation::fixture_scope();
    let (signals, evidence) =
        normalized_signals(&["shared-service-alert", "shared-service-anomaly"]);
    let candidate = candidate_of(&signals, &evidence, &NoTopology);
    let earlier = change_at(
        "2026-08-28T08:55:00Z",
        target(SignalTargetKind::Service, "service/checkout"),
        &scope,
    );
    let later = change_at(
        "2026-08-28T08:55:30Z",
        target(SignalTargetKind::Service, "service/checkout"),
        &scope,
    );
    let mut later = later;
    later.id = Uuid::from_u128(2);
    later.validate().expect("test change remains valid");

    let events = vec![later.clone(), earlier.clone()];
    let associations = association::associate(
        &events,
        std::slice::from_ref(&candidate),
        &signals,
        3_600.0,
        &NoTopology,
    )
    .expect("association succeeds");

    let mut expected_change_ids = vec![earlier.id, later.id];
    expected_change_ids.sort();
    assert_eq!(
        associations
            .iter()
            .map(|association| association.change_id)
            .collect::<Vec<_>>(),
        expected_change_ids
    );
    let candidate_ids = associations
        .iter()
        .map(|association| association.candidate_id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(candidate_ids, BTreeSet::from([candidate.id.as_str()]));
}
