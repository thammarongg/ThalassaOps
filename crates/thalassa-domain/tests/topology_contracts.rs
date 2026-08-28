// SPDX-License-Identifier: Apache-2.0

use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use thalassa_domain::*;

fn scope() -> ResourceScope {
    ResourceScope::workspace(uuid::Uuid::nil(), uuid::Uuid::nil(), uuid::Uuid::nil())
}

fn assert_round_trip<T>(value: T)
where
    T: DeserializeOwned + PartialEq + Serialize + std::fmt::Debug,
{
    let encoded = serde_json::to_value(&value).expect("contract must serialize");
    let decoded: T = serde_json::from_value(encoded).expect("contract must deserialize");
    assert_eq!(decoded, value);
}

fn drill_down() -> DrillDownTarget {
    DrillDownTarget {
        destination: DrillDownDestination::Topology,
        evidence_ids: vec!["evidence-node".into()],
        filter_key: Some("node:fixture:prod:service:checkout".into()),
    }
}

fn evidence_drill_down() -> DrillDownTarget {
    DrillDownTarget {
        destination: DrillDownDestination::Evidence,
        evidence_ids: vec!["evidence-node".into()],
        filter_key: None,
    }
}

fn drill_down_reference() -> DrillDownReference {
    DrillDownReference {
        source_query: "topology:checkout".into(),
        scope: scope(),
        time_window: None,
        evidence_ids: vec!["evidence-node".into()],
    }
}

fn metric(key: &str, value: f64) -> TopologyMetric {
    TopologyMetric {
        key: key.into(),
        value,
        unit: NumberUnit::Count,
        evidence_ids: vec!["evidence-node".into()],
        drill_down: drill_down(),
        drill_down_reference: drill_down_reference(),
    }
}

fn summary_metric(key: &str, value: f64) -> TopologyMetric {
    TopologyMetric {
        key: key.into(),
        value,
        unit: NumberUnit::Count,
        evidence_ids: vec!["evidence-node".into()],
        drill_down: evidence_drill_down(),
        drill_down_reference: drill_down_reference(),
    }
}

fn ownership() -> TopologyOwnership {
    TopologyOwnership {
        team_id: Some(uuid::Uuid::nil()),
        team_name: Some("Platform".into()),
        source: TopologyOwnershipSource::ExplicitLabel,
        evidence_ids: vec!["evidence-node".into()],
    }
}

fn node(id: &str) -> TopologyNode {
    TopologyNode {
        id: id.into(),
        kind: TopologyNodeKind::Service,
        name: "checkout".into(),
        native_kind: Some("Service".into()),
        native_id: Some("service/checkout".into()),
        environment_id: Some("prod".into()),
        provider: Some("kubernetes".into()),
        scope: scope(),
        status: ConsoleHealthState::Healthy,
        labels: BTreeMap::from([(String::from("app"), String::from("checkout"))]),
        ownership: ownership(),
        metric: Some(metric("request_count", 42.0)),
        affected_by_incident: true,
        evidence_ids: vec!["evidence-node".into()],
        drill_down: drill_down(),
    }
}

fn edge(upstream_node_id: &str, downstream_node_id: &str) -> TopologyEdge {
    TopologyEdge {
        id: "edge-checkout".into(),
        upstream_node_id: upstream_node_id.into(),
        downstream_node_id: downstream_node_id.into(),
        kind: TopologyEdgeKind::RoutesTo,
        provenance: vec![TopologyEdgeProvenance {
            source: TopologySourceKind::Fixture,
            source_key: "fixture-topology".into(),
            observed_at: Some("2026-08-28T09:00:00Z".into()),
        }],
        confidence: 0.9,
        metadata: BTreeMap::from([(String::from("relationship"), String::from("routes_to"))]),
        evidence_ids: vec!["evidence-node".into()],
        drill_down: evidence_drill_down(),
    }
}

fn path() -> TopologyPath {
    TopologyPath {
        id: "path-checkout".into(),
        root_node_id: "node-a".into(),
        terminal_node_id: "node-b".into(),
        node_ids: vec!["node-a".into(), "node-b".into()],
        edge_ids: vec!["edge-checkout".into()],
        direction: TopologyDirection::Downstream,
        depth: 1,
        confidence: 0.9,
        kind: TopologyPathKind::ProbableStructural,
        termination: TopologyPathTermination::Leaf,
        cycle_edge_id: None,
        evidence_ids: vec!["evidence-node".into()],
        drill_down: evidence_drill_down(),
    }
}

fn snapshot() -> TopologySnapshot {
    TopologySnapshot {
        generated_at: "2026-08-28T09:00:00Z".into(),
        scope: scope(),
        filter: TopologyFilter {
            environment_ids: vec!["prod".into()],
            team_ids: vec![uuid::Uuid::nil()],
            incident_id: Some("queue-1".into()),
        },
        focus_node_id: Some("node-a".into()),
        traversal: TopologyTraversal {
            direction: TopologyDirection::Both,
            max_depth: 3,
        },
        summary: TopologySummary {
            visible_nodes: summary_metric("visible_nodes", 2.0),
            visible_edges: summary_metric("visible_edges", 1.0),
            affected_nodes: summary_metric("affected_nodes", 2.0),
            probable_paths: summary_metric("probable_paths", 1.0),
        },
        nodes: vec![node("node-a"), node("node-b")],
        edges: vec![edge("node-a", "node-b")],
        paths: vec![path()],
        source_status: vec![],
        evidence: vec![EvidenceRef {
            id: "evidence-node".into(),
            source_kind: EvidenceSourceKind::Fixture,
            connector_id: None,
            scope: scope(),
            endpoint: "fixture://topology".into(),
            query: Some("topology:checkout".into()),
            observed_at: "2026-08-28T09:00:00Z".into(),
            excerpt: "checkout topology fixture".into(),
            native_url: None,
            redaction: EvidenceRedaction {
                classification_verified: true,
                redaction_verified: true,
                masked: false,
                unparsed: false,
            },
        }],
    }
}

#[test]
fn topology_contracts_round_trip_through_json() {
    assert_round_trip(TopologyNodeKind::Environment);
    assert_round_trip(TopologyOwnershipSource::Fixture);
    assert_round_trip(ownership());
    assert_round_trip(metric("request_count", 42.0));
    assert_round_trip(node("node-a"));
    assert_round_trip(TopologyEdgeKind::DependsOn);
    assert_round_trip(TopologySourceKind::Kubernetes);
    assert_round_trip(TopologyEdgeProvenance {
        source: TopologySourceKind::Cloud,
        source_key: "aws:prod".into(),
        observed_at: None,
    });
    assert_round_trip(edge("node-a", "node-b"));
    assert_round_trip(TopologyDirection::Upstream);
    assert_round_trip(TopologyPathKind::ProbableStructural);
    assert_round_trip(TopologyPathTermination::CycleDetected);
    assert_round_trip(path());
    assert_round_trip(TopologyTraversal {
        direction: TopologyDirection::Both,
        max_depth: 8,
    });
    assert_round_trip(TopologyFilter {
        environment_ids: vec!["prod".into()],
        team_ids: vec![uuid::Uuid::nil()],
        incident_id: None,
    });
    assert_round_trip(TopologyRequest {
        filter: TopologyFilter {
            environment_ids: vec![],
            team_ids: vec![],
            incident_id: None,
        },
        focus_node_id: None,
        traversal: TopologyTraversal {
            direction: TopologyDirection::Downstream,
            max_depth: 0,
        },
    });
    assert_round_trip(TopologySummary {
        visible_nodes: metric("visible_nodes", 2.0),
        visible_edges: metric("visible_edges", 1.0),
        affected_nodes: metric("affected_nodes", 1.0),
        probable_paths: metric("probable_paths", 1.0),
    });
    assert_round_trip(snapshot());
    assert_round_trip(TopologyEvidenceRequest {
        evidence_ids: vec!["evidence-node".into()],
    });
    assert_round_trip(TopologyOwnershipSelector::NodeId {
        node_id: "node-a".into(),
    });
    assert_round_trip(TopologyOwnershipSelector::Label {
        key: "team".into(),
        value: "platform".into(),
    });
    assert_round_trip(TopologyOwnershipSelector::Environment {
        environment_id: "prod".into(),
    });
    assert_round_trip(TopologyOwnershipRule {
        selector: TopologyOwnershipSelector::Environment {
            environment_id: "prod".into(),
        },
        team_id: uuid::Uuid::nil(),
        team_name: "Platform".into(),
        source: TopologyOwnershipSource::EnvironmentDefault,
        evidence_ids: vec!["evidence-node".into()],
    });
    assert_round_trip(DrillDownDestination::Topology);
}

#[test]
fn topology_snapshot_rejects_duplicate_source_status_keys() {
    let mut invalid = snapshot();
    let status = SourceStatus {
        source_key: "fixture".into(),
        state: SourceState::Fresh,
        reason: None,
        detail: None,
        observed_at: None,
        evidence_ids: vec![],
    };
    invalid.source_status = vec![status.clone(), status];

    assert_eq!(invalid.validate(), Err(TopologyError::InvalidRequest));
}

#[test]
fn topology_snapshot_rejects_path_confidence_above_edge_minimum() {
    let mut invalid = snapshot();
    invalid.paths[0].confidence = 0.8;

    assert_eq!(invalid.validate(), Err(TopologyError::InvalidRequest));
}

#[test]
fn topology_node_drill_down_requires_its_backend_issued_node_id() {
    let mut missing_filter_key = node("node-a");
    missing_filter_key.drill_down.filter_key = None;
    assert_eq!(
        missing_filter_key.validate(),
        Err(TopologyError::InvalidRequest)
    );

    let mut wrong_filter_key = node("node-a");
    wrong_filter_key.drill_down.filter_key = Some("node-b".into());
    assert_eq!(
        wrong_filter_key.validate(),
        Err(TopologyError::InvalidRequest)
    );
}

#[test]
fn topology_metrics_reject_negative_counts() {
    let invalid = metric("ready_replicas", -1.0);
    assert_eq!(invalid.validate(), Err(TopologyError::InvalidRequest));
}

#[test]
fn topology_enums_use_explicit_symmetric_wire_values() {
    macro_rules! assert_wire_values {
        ($type:ty, $( $variant:expr => $wire:expr ),+ $(,)?) => {
            $(
                assert_eq!(serde_json::to_value($variant).unwrap(), json!($wire));
                assert_eq!(
                    serde_json::from_value::<$type>(json!($wire)).unwrap(),
                    $variant
                );
            )+
        };
    }

    assert_wire_values!(
        TopologyNodeKind,
        TopologyNodeKind::Environment => "environment",
        TopologyNodeKind::Cluster => "cluster",
        TopologyNodeKind::Namespace => "namespace",
        TopologyNodeKind::Workload => "workload",
        TopologyNodeKind::Service => "service",
        TopologyNodeKind::Pod => "pod",
        TopologyNodeKind::Node => "node",
        TopologyNodeKind::CloudResource => "cloud_resource",
        TopologyNodeKind::ObservabilityTarget => "observability_target",
    );
    assert_wire_values!(
        TopologyOwnershipSource,
        TopologyOwnershipSource::ExplicitLabel => "explicit_label",
        TopologyOwnershipSource::ResourceScope => "resource_scope",
        TopologyOwnershipSource::EnvironmentDefault => "environment_default",
        TopologyOwnershipSource::Fixture => "fixture",
        TopologyOwnershipSource::Unassigned => "unassigned",
    );
    assert_wire_values!(
        TopologyEdgeKind,
        TopologyEdgeKind::Contains => "contains",
        TopologyEdgeKind::Owns => "owns",
        TopologyEdgeKind::Selects => "selects",
        TopologyEdgeKind::RoutesTo => "routes_to",
        TopologyEdgeKind::RunsOn => "runs_on",
        TopologyEdgeKind::DependsOn => "depends_on",
    );
    assert_wire_values!(
        TopologySourceKind,
        TopologySourceKind::Kubernetes => "kubernetes",
        TopologySourceKind::Cloud => "cloud",
        TopologySourceKind::Observability => "observability",
        TopologySourceKind::Fixture => "fixture",
    );
    assert_wire_values!(
        TopologyDirection,
        TopologyDirection::Upstream => "upstream",
        TopologyDirection::Downstream => "downstream",
        TopologyDirection::Both => "both",
    );
    assert_wire_values!(
        TopologyPathKind,
        TopologyPathKind::ProbableStructural => "probable_structural",
    );
    assert_wire_values!(
        TopologyPathTermination,
        TopologyPathTermination::Leaf => "leaf",
        TopologyPathTermination::CycleDetected => "cycle_detected",
        TopologyPathTermination::DepthLimit => "depth_limit",
    );
    assert_eq!(
        serde_json::to_value(DrillDownDestination::Topology).unwrap(),
        json!("topology")
    );
}

#[test]
fn topology_edges_reject_self_loops_and_unknown_node_references() {
    let self_loop = edge("node-a", "node-a");
    assert_eq!(
        self_loop.validate().unwrap_err(),
        TopologyError::InvalidRequest
    );

    let unknown_reference = edge("node-a", "node-missing");
    let known_nodes = BTreeSet::from([String::from("node-a"), String::from("node-b")]);
    assert_eq!(
        unknown_reference
            .validate_against_nodes(&known_nodes)
            .unwrap_err(),
        TopologyError::NodeNotFound
    );
}

#[test]
fn topology_filters_reject_duplicates_and_unknown_references() {
    let duplicate = TopologyFilter {
        environment_ids: vec!["prod".into(), "prod".into()],
        team_ids: vec![],
        incident_id: None,
    };
    assert_eq!(
        duplicate.validate().unwrap_err(),
        TopologyError::InvalidRequest
    );

    let unknown_environment = TopologyFilter {
        environment_ids: vec!["staging".into()],
        team_ids: vec![],
        incident_id: None,
    };
    let known_teams = BTreeSet::from([uuid::Uuid::nil()]);
    let known_environments = BTreeSet::from([String::from("prod")]);
    let known_incidents = BTreeSet::from([String::from("queue-1")]);
    assert_eq!(
        unknown_environment
            .validate_against(&known_environments, &known_teams, &known_incidents)
            .unwrap_err(),
        TopologyError::InvalidRequest
    );

    let unknown_team = TopologyFilter {
        environment_ids: vec![],
        team_ids: vec![uuid::Uuid::from_u128(1)],
        incident_id: None,
    };
    assert_eq!(
        unknown_team
            .validate_against(&known_environments, &known_teams, &known_incidents)
            .unwrap_err(),
        TopologyError::InvalidRequest
    );

    let unknown_incident = TopologyFilter {
        environment_ids: vec![],
        team_ids: vec![],
        incident_id: Some("queue-missing".into()),
    };
    assert_eq!(
        unknown_incident
            .validate_against(&known_environments, &known_teams, &known_incidents)
            .unwrap_err(),
        TopologyError::IncidentNotFound
    );
}

#[test]
fn topology_numbers_reject_non_finite_values_and_out_of_range_confidence() {
    assert_eq!(
        metric("bad", f64::NAN).validate().unwrap_err(),
        TopologyError::NonFiniteNumber(TopologyNumberField::MetricValue)
    );
    assert_eq!(
        metric("bad", f64::INFINITY).validate().unwrap_err(),
        TopologyError::NonFiniteNumber(TopologyNumberField::MetricValue)
    );

    let mut invalid_edge = edge("node-a", "node-b");
    invalid_edge.confidence = 1.1;
    assert_eq!(
        invalid_edge.validate().unwrap_err(),
        TopologyError::ConfidenceOutOfRange
    );

    let mut invalid_path = path();
    invalid_path.confidence = f64::NEG_INFINITY;
    assert_eq!(
        invalid_path.validate().unwrap_err(),
        TopologyError::NonFiniteNumber(TopologyNumberField::PathConfidence)
    );
}

#[test]
fn topology_paths_reject_depth_that_exceeds_the_traversed_nodes() {
    let mut invalid = path();
    invalid.depth = 2;
    assert_eq!(
        invalid.validate().unwrap_err(),
        TopologyError::InvalidRequest
    );
}

#[test]
fn topology_snapshot_rejects_unknown_edge_endpoints() {
    let mut invalid = snapshot();
    invalid.edges[0].downstream_node_id = "node-missing".into();
    assert_eq!(invalid.validate().unwrap_err(), TopologyError::NodeNotFound);
}

#[test]
fn topology_snapshot_rejects_paths_that_do_not_follow_edge_orientation() {
    let mut invalid = snapshot();
    invalid.edges[0].upstream_node_id = "node-b".into();
    invalid.edges[0].downstream_node_id = "node-a".into();
    assert_eq!(
        invalid.validate().unwrap_err(),
        TopologyError::InvalidRequest
    );
}

#[test]
fn topology_snapshot_requires_cycle_edges_to_close_at_the_terminal_node() {
    let mut valid = snapshot();
    let mut cycle_edge = edge("node-b", "node-a");
    cycle_edge.id = "edge-cycle".into();
    valid.edges.push(cycle_edge);
    valid.summary.visible_edges.value = 2.0;
    valid.paths[0].termination = TopologyPathTermination::CycleDetected;
    valid.paths[0].cycle_edge_id = Some("edge-cycle".into());
    assert!(valid.validate().is_ok());

    let mut invalid = valid;
    invalid.edges[1].upstream_node_id = "node-a".into();
    invalid.edges[1].downstream_node_id = "node-b".into();
    assert_eq!(
        invalid.validate().unwrap_err(),
        TopologyError::InvalidRequest
    );

    let mut invalid = snapshot();
    invalid.nodes.push(node("node-c"));
    let mut non_closing_edge = edge("node-b", "node-c");
    non_closing_edge.id = "edge-cycle".into();
    invalid.edges.push(non_closing_edge);
    invalid.summary.visible_nodes.value = 3.0;
    invalid.summary.affected_nodes.value = 3.0;
    invalid.summary.visible_edges.value = 2.0;
    invalid.paths[0].termination = TopologyPathTermination::CycleDetected;
    invalid.paths[0].cycle_edge_id = Some("edge-cycle".into());
    assert_eq!(
        invalid.validate().unwrap_err(),
        TopologyError::InvalidRequest
    );
}

#[test]
fn topology_snapshot_requires_paths_to_include_all_listed_record_evidence() {
    let mut invalid = snapshot();
    invalid.paths[0].evidence_ids.clear();
    invalid.paths[0].drill_down.evidence_ids = vec!["evidence-node".into()];
    assert_eq!(
        invalid.validate().unwrap_err(),
        TopologyError::EvidenceMissing
    );

    let mut invalid = snapshot();
    invalid.paths[0].evidence_ids = vec!["evidence-node".into()];
    invalid.nodes[1].evidence_ids = vec!["evidence-other".into()];
    invalid.nodes[1].drill_down.evidence_ids = vec!["evidence-other".into()];
    invalid.edges[0].evidence_ids = vec!["evidence-other".into()];
    invalid.edges[0].drill_down.evidence_ids = vec!["evidence-other".into()];
    invalid.evidence.push(EvidenceRef {
        id: "evidence-other".into(),
        source_kind: EvidenceSourceKind::Fixture,
        connector_id: None,
        scope: scope(),
        endpoint: "fixture://topology".into(),
        query: Some("topology:other".into()),
        observed_at: "2026-08-28T09:00:00Z".into(),
        excerpt: "other topology fixture".into(),
        native_url: None,
        redaction: EvidenceRedaction {
            classification_verified: true,
            redaction_verified: true,
            masked: false,
            unparsed: false,
        },
    });
    assert_eq!(
        invalid.validate().unwrap_err(),
        TopologyError::InvalidRequest
    );
}

#[test]
fn topology_snapshot_rejects_summary_counts_that_do_not_match_the_graph() {
    let mut invalid = snapshot();
    invalid.summary.visible_nodes.value = 3.0;
    assert_eq!(
        invalid.validate().unwrap_err(),
        TopologyError::InvalidRequest
    );

    let mut invalid = snapshot();
    invalid.summary.visible_edges.value = 2.0;
    assert_eq!(
        invalid.validate().unwrap_err(),
        TopologyError::InvalidRequest
    );

    let mut invalid = snapshot();
    invalid.summary.affected_nodes.value = 1.0;
    assert_eq!(
        invalid.validate().unwrap_err(),
        TopologyError::InvalidRequest
    );

    let mut invalid = snapshot();
    invalid.summary.probable_paths.value = 0.0;
    assert_eq!(
        invalid.validate().unwrap_err(),
        TopologyError::InvalidRequest
    );
}

#[test]
fn topology_snapshot_requires_verified_evidence_for_rendered_records() {
    let valid = snapshot();
    assert!(valid.validate().is_ok());

    let mut unverified = valid;
    unverified.evidence[0].redaction.classification_verified = false;
    assert_eq!(
        unverified.validate().unwrap_err(),
        TopologyError::EvidenceUnverified
    );
}

#[test]
fn topology_edges_and_paths_open_their_evidence_destination() {
    let valid = snapshot();
    assert_eq!(
        valid.edges[0].drill_down.destination,
        DrillDownDestination::Evidence
    );
    assert_eq!(
        valid.paths[0].drill_down.destination,
        DrillDownDestination::Evidence
    );
    assert!(valid.validate().is_ok());
}

#[test]
fn topology_edges_reject_duplicate_provenance_identity() {
    let mut invalid = snapshot();
    let duplicate = invalid.edges[0].provenance[0].clone();
    invalid.edges[0].provenance.push(duplicate);

    assert_eq!(
        invalid.validate().unwrap_err(),
        TopologyError::MalformedSource
    );
}

#[test]
fn topology_evidence_requests_reject_ids_not_emitted_by_the_snapshot() {
    let request = TopologyEvidenceRequest {
        evidence_ids: vec!["evidence-not-emitted".into()],
    };
    let emitted = BTreeSet::from([String::from("evidence-node")]);
    assert_eq!(
        request.validate_against(&emitted).unwrap_err(),
        TopologyError::EvidenceMissing
    );
}

fn typescript_fields(source: &str, type_name: &str) -> BTreeSet<String> {
    let marker = format!("export type {type_name} =");
    let declaration = source
        .split_once(&marker)
        .map(|(_, rest)| rest)
        .unwrap_or_else(|| panic!("missing TypeScript declaration: {type_name}"));
    let body_start = declaration
        .find('{')
        .unwrap_or_else(|| panic!("missing object body: {type_name}"));
    let body = &declaration[body_start + 1..]
        .split_once('}')
        .map(|(body, _)| body)
        .unwrap_or_else(|| panic!("unterminated object body: {type_name}"));
    body.split(';')
        .filter_map(|field| field.trim().split_once(':').map(|(name, _)| name.trim()))
        .map(|name| name.trim_end_matches('?').to_string())
        .filter(|name| !name.is_empty())
        .collect()
}

fn rust_fields(value: Value) -> BTreeSet<String> {
    value
        .as_object()
        .expect("contract sample must serialize as an object")
        .keys()
        .cloned()
        .collect()
}

#[test]
fn topology_rust_and_typescript_object_shapes_are_symmetric_in_both_directions() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/contracts/ipc.ts"),
    )
    .expect("TypeScript IPC contract must be available to the symmetry test");

    let samples = [
        (
            "TopologyOwnership",
            serde_json::to_value(ownership()).unwrap(),
        ),
        (
            "TopologyMetric",
            serde_json::to_value(metric("request_count", 42.0)).unwrap(),
        ),
        (
            "TopologyNode",
            serde_json::to_value(node("node-a")).unwrap(),
        ),
        (
            "TopologyEdgeProvenance",
            serde_json::to_value(TopologyEdgeProvenance {
                source: TopologySourceKind::Fixture,
                source_key: "fixture".into(),
                observed_at: None,
            })
            .unwrap(),
        ),
        (
            "TopologyEdge",
            serde_json::to_value(edge("node-a", "node-b")).unwrap(),
        ),
        ("TopologyPath", serde_json::to_value(path()).unwrap()),
        (
            "TopologyTraversal",
            serde_json::to_value(TopologyTraversal {
                direction: TopologyDirection::Both,
                max_depth: 3,
            })
            .unwrap(),
        ),
        (
            "TopologyFilter",
            serde_json::to_value(TopologyFilter {
                environment_ids: vec![],
                team_ids: vec![],
                incident_id: None,
            })
            .unwrap(),
        ),
        (
            "TopologyRequest",
            serde_json::to_value(TopologyRequest {
                filter: TopologyFilter {
                    environment_ids: vec![],
                    team_ids: vec![],
                    incident_id: None,
                },
                focus_node_id: None,
                traversal: TopologyTraversal {
                    direction: TopologyDirection::Both,
                    max_depth: 3,
                },
            })
            .unwrap(),
        ),
        (
            "TopologySummary",
            serde_json::to_value(TopologySummary {
                visible_nodes: metric("visible_nodes", 2.0),
                visible_edges: metric("visible_edges", 1.0),
                affected_nodes: metric("affected_nodes", 1.0),
                probable_paths: metric("probable_paths", 1.0),
            })
            .unwrap(),
        ),
        (
            "TopologySnapshot",
            serde_json::to_value(snapshot()).unwrap(),
        ),
        (
            "TopologyEvidenceRequest",
            serde_json::to_value(TopologyEvidenceRequest {
                evidence_ids: vec!["evidence-node".into()],
            })
            .unwrap(),
        ),
    ];

    for (type_name, rust_value) in samples {
        let rust = rust_fields(rust_value);
        let typescript = typescript_fields(&source, type_name);
        assert_eq!(rust, typescript, "shape mismatch for {type_name}");
    }
}
