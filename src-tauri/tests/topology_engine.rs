use std::collections::BTreeSet;

use thalassa_domain::{
    SourceState, StatusReason, TopologyDirection, TopologyEdgeKind, TopologyError, TopologyFilter,
    TopologyPathTermination, TopologyRequest, TopologyTraversal,
};
use thalassaops::cloud::CloudResource;
use thalassaops::topology::{
    default_topology_request, fixture_scope, topology_fixture_input, TopologyBuilder,
};

fn request_for(
    focus_node_id: Option<String>,
    direction: TopologyDirection,
    max_depth: u16,
) -> TopologyRequest {
    TopologyRequest {
        filter: TopologyFilter {
            environment_ids: Vec::new(),
            team_ids: Vec::new(),
            incident_id: None,
        },
        focus_node_id,
        traversal: TopologyTraversal {
            direction,
            max_depth,
        },
    }
}

fn node_id_by_name(snapshot: &thalassa_domain::TopologySnapshot, name: &str) -> String {
    snapshot
        .nodes
        .iter()
        .find(|node| node.name == name)
        .map(|node| node.id.clone())
        .unwrap_or_else(|| panic!("fixture should contain node {name}"))
}

#[test]
fn healthy_fixture_builds_a_valid_evidence_backed_graph() {
    let snapshot = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&default_topology_request())
        .expect("healthy fixture should build");

    assert!(snapshot.validate().is_ok());
    assert!(snapshot.nodes.iter().any(|node| node.name == "checkout"));
    assert!(snapshot
        .edges
        .iter()
        .any(|edge| edge.kind == TopologyEdgeKind::Owns));
    assert!(snapshot
        .edges
        .iter()
        .all(|edge| !edge.provenance.is_empty() && !edge.evidence_ids.is_empty()));
    assert!(snapshot
        .nodes
        .iter()
        .all(|node| !node.evidence_ids.is_empty()));
}

#[test]
fn zero_summary_counts_do_not_borrow_unrelated_evidence() {
    let snapshot = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&default_topology_request())
        .expect("healthy fixture should build");

    for metric in [
        &snapshot.summary.affected_nodes,
        &snapshot.summary.probable_paths,
    ] {
        assert_eq!(metric.value, 0.0);
        assert!(metric.evidence_ids.is_empty());
        assert!(metric.drill_down.evidence_ids.is_empty());
        assert!(metric.drill_down_reference.evidence_ids.is_empty());
    }
}

#[test]
fn cloud_records_without_matching_evidence_are_not_admitted() {
    let mut input = topology_fixture_input(fixture_scope());
    let mut unattributed: CloudResource = input.cloud_resources[0].clone();
    unattributed.id = "unattributed-resource".into();
    unattributed.name = "unattributed-resource".into();
    input.cloud_resources.push(unattributed);

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("unmatched source records should degrade the source");

    assert!(snapshot
        .nodes
        .iter()
        .all(|node| node.native_id.as_deref() != Some("unattributed-resource")));
    assert!(snapshot.source_status.iter().any(|status| {
        status.source_key == "cloud" && status.state == thalassa_domain::SourceState::Unverified
    }));
}

#[test]
fn evidence_matching_does_not_mix_similar_resource_names() {
    let snapshot = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&default_topology_request())
        .expect("healthy fixture should build");
    let checkout = snapshot
        .nodes
        .iter()
        .find(|node| node.name == "checkout")
        .expect("checkout service should be present");
    let checkout_api = snapshot
        .nodes
        .iter()
        .find(|node| node.name == "checkout-api")
        .expect("checkout workload should be present");

    assert!(checkout
        .evidence_ids
        .contains(&"evidence-topology-k8s-service-checkout".to_string()));
    assert!(
        !checkout
            .evidence_ids
            .contains(&"evidence-topology-k8s-workload-checkout-api".to_string()),
        "checkout evidence: {:?}",
        checkout.evidence_ids
    );
    assert!(checkout_api
        .evidence_ids
        .contains(&"evidence-topology-k8s-workload-checkout-api".to_string()));
    assert!(
        !checkout_api
            .evidence_ids
            .contains(&"evidence-topology-k8s-pod-checkout-api-0".to_string()),
        "checkout api evidence: {:?}",
        checkout_api.evidence_ids
    );
}

#[test]
fn topology_does_not_apply_value_pattern_redaction_to_verified_evidence() {
    let mut input = topology_fixture_input(fixture_scope());
    let evidence = input
        .evidence
        .iter_mut()
        .find(|candidate| candidate.id == "evidence-topology-environment-aws")
        .expect("fixture should contain AWS environment evidence");
    evidence.excerpt = "resource generation 123456".into();

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("verified evidence should remain admitted");
    let admitted = snapshot
        .evidence
        .iter()
        .find(|candidate| candidate.id == "evidence-topology-environment-aws")
        .expect("the changed evidence should remain available");

    assert_eq!(admitted.excerpt, "resource generation 123456");
    assert!(!admitted.redaction.masked);
}

#[test]
fn topology_omits_sensitive_free_text_instead_of_claiming_it_was_masked() {
    let mut input = topology_fixture_input(fixture_scope());
    let evidence = input
        .evidence
        .iter_mut()
        .find(|candidate| candidate.id == "evidence-topology-environment-aws")
        .expect("fixture should contain AWS environment evidence");
    evidence.excerpt = "password=fixture-secret".into();
    let evidence_id = evidence.id.clone();

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("sensitive evidence should degrade the source");

    assert!(snapshot
        .evidence
        .iter()
        .all(|candidate| candidate.id != evidence_id));
    assert!(snapshot.source_status.iter().any(|status| {
        status.source_key == "cloud" && status.state == thalassa_domain::SourceState::Unverified
    }));
    let serialized = serde_json::to_string(&snapshot).expect("snapshot should serialize");
    assert!(!serialized.contains("fixture-secret"));
}

#[test]
fn conflicting_duplicate_evidence_ids_are_rejected_as_ambiguous() {
    let mut input = topology_fixture_input(fixture_scope());
    let mut conflicting = input.evidence[0].clone();
    conflicting.excerpt = "conflicting evidence payload".into();
    input.evidence.push(conflicting);
    let duplicate_id = input.evidence[0].id.clone();

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("ambiguous evidence should degrade its source");

    assert!(snapshot
        .evidence
        .iter()
        .all(|evidence| evidence.id != duplicate_id));
    assert!(snapshot.source_status.iter().any(|status| {
        status.state == thalassa_domain::SourceState::Unverified
            && status.source_key == "observability"
    }));
}

#[test]
fn conflicting_duplicate_incident_ids_are_rejected_as_ambiguous() {
    let mut input = topology_fixture_input(fixture_scope());
    let mut conflicting = input.incident_queue[0].clone();
    conflicting.title = "conflicting incident payload".into();
    input.incident_queue.push(conflicting);

    let mut request = default_topology_request();
    request.filter.incident_id = Some("alert-checkout-s1".into());
    let result = TopologyBuilder::from_input(input).snapshot_at(&request);

    assert_eq!(result.unwrap_err(), TopologyError::IncidentNotFound);
}

#[test]
fn downstream_impact_traverses_structural_paths_from_a_node() {
    let base = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&default_topology_request())
        .expect("healthy fixture should build");
    let checkout_id = node_id_by_name(&base, "checkout");

    let snapshot = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&request_for(
            Some(checkout_id.clone()),
            TopologyDirection::Downstream,
            3,
        ))
        .expect("downstream traversal should build");

    assert!(snapshot.paths.iter().any(|path| {
        path.root_node_id == checkout_id
            && path.direction == TopologyDirection::Downstream
            && path.node_ids.len() > 1
    }));
    assert!(snapshot
        .paths
        .iter()
        .all(|path| path.kind == thalassa_domain::TopologyPathKind::ProbableStructural));
}

#[test]
fn every_path_carries_evidence_for_each_listed_node_and_edge() {
    let base = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&default_topology_request())
        .expect("healthy fixture should build");
    let checkout_id = node_id_by_name(&base, "checkout");
    let snapshot = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&request_for(Some(checkout_id), TopologyDirection::Both, 8))
        .expect("traversal should build");

    for path in &snapshot.paths {
        let mut expected = BTreeSet::new();
        for node_id in &path.node_ids {
            let node = snapshot
                .nodes
                .iter()
                .find(|candidate| candidate.id == *node_id)
                .expect("path node should belong to graph");
            expected.extend(node.evidence_ids.iter().cloned());
        }
        for edge_id in path.edge_ids.iter().chain(path.cycle_edge_id.iter()) {
            let edge = snapshot
                .edges
                .iter()
                .find(|candidate| candidate.id == *edge_id)
                .expect("path edge should belong to graph");
            expected.extend(edge.evidence_ids.iter().cloned());
        }

        assert!(expected
            .iter()
            .all(|evidence_id| path.evidence_ids.contains(evidence_id)));
    }
}

#[test]
fn upstream_impact_reverses_edges_without_changing_their_orientation() {
    let base = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&default_topology_request())
        .expect("healthy fixture should build");
    let database_id = node_id_by_name(&base, "checkout-rds");

    let snapshot = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&request_for(
            Some(database_id.clone()),
            TopologyDirection::Upstream,
            3,
        ))
        .expect("upstream traversal should build");

    assert!(snapshot.paths.iter().any(|path| {
        path.root_node_id == database_id
            && path.direction == TopologyDirection::Upstream
            && path.node_ids.len() > 1
    }));
    for path in &snapshot.paths {
        for edge_id in &path.edge_ids {
            let edge = snapshot
                .edges
                .iter()
                .find(|candidate| candidate.id == *edge_id)
                .expect("path edge should belong to graph");
            assert_ne!(edge.upstream_node_id, edge.downstream_node_id);
        }
    }
}

#[test]
fn cycle_traversal_terminates_and_reports_the_closing_edge() {
    let base = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&default_topology_request())
        .expect("healthy fixture should build");
    let database_id = node_id_by_name(&base, "checkout-rds");

    let snapshot = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&request_for(
            Some(database_id),
            TopologyDirection::Downstream,
            8,
        ))
        .expect("cycle traversal should build");

    let cycle = snapshot
        .paths
        .iter()
        .find(|path| path.termination == TopologyPathTermination::CycleDetected)
        .expect("cycle should be reported explicitly");
    assert!(cycle.cycle_edge_id.is_some());
    let unique_nodes: BTreeSet<_> = cycle.node_ids.iter().collect();
    assert_eq!(unique_nodes.len(), cycle.node_ids.len());
}

#[test]
fn depth_limit_is_reported_when_another_edge_is_eligible() {
    let base = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&default_topology_request())
        .expect("healthy fixture should build");
    let checkout_id = node_id_by_name(&base, "checkout");

    let snapshot = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&request_for(
            Some(checkout_id),
            TopologyDirection::Downstream,
            1,
        ))
        .expect("bounded traversal should build");

    assert!(snapshot
        .paths
        .iter()
        .any(|path| path.termination == TopologyPathTermination::DepthLimit));
}

#[test]
fn unknown_focus_node_is_rejected_with_a_typed_error() {
    let request = request_for(
        Some("node:fixture:missing:service:unknown".into()),
        TopologyDirection::Both,
        3,
    );

    let result =
        TopologyBuilder::from_input(topology_fixture_input(fixture_scope())).snapshot_at(&request);

    assert!(matches!(result, Err(TopologyError::NodeNotFound)));
}

#[test]
fn malformed_fixture_records_are_reported_without_panicking() {
    let mut input = topology_fixture_input(fixture_scope());
    let inventory = input
        .kubernetes
        .get_mut("env-aws-prod")
        .expect("fixture should contain production inventory");
    inventory.topology[0].to_name = "missing-resource".into();

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("malformed source records should degrade the source");

    assert!(snapshot
        .source_status
        .iter()
        .any(|status| status.state == thalassa_domain::SourceState::Unverified));
    assert!(snapshot
        .edges
        .iter()
        .all(|edge| !edge.downstream_node_id.ends_with(":missing-resource")));
}

#[test]
fn repeated_builds_have_identical_serialized_output_and_ordering() {
    let first = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&default_topology_request())
        .expect("first build should succeed");
    let second = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&default_topology_request())
        .expect("second build should succeed");

    assert_eq!(first, second);
    assert_eq!(
        serde_json::to_string(&first).expect("snapshot should serialize"),
        serde_json::to_string(&second).expect("snapshot should serialize")
    );
}

#[test]
fn duplicate_topology_source_statuses_merge_independently_of_input_order() {
    let mut first_input = topology_fixture_input(fixture_scope());
    let mut status_a = first_input
        .source_status
        .iter()
        .find(|status| status.source_key == "cloud")
        .expect("fixture should contain a cloud source status")
        .clone();
    status_a.reason = Some(StatusReason::NotConfigured);
    let mut status_b = status_a.clone();
    status_b.reason = Some(StatusReason::Unreachable);
    first_input
        .source_status
        .retain(|status| status.source_key != "cloud");
    first_input
        .source_status
        .extend([status_a.clone(), status_b.clone()]);

    let mut second_input = first_input.clone();
    second_input
        .source_status
        .retain(|status| status.source_key != "cloud");
    second_input.source_status.extend([status_b, status_a]);

    let first = TopologyBuilder::from_input(first_input)
        .snapshot_at(&default_topology_request())
        .expect("duplicate source statuses should be merged");
    let second = TopologyBuilder::from_input(second_input)
        .snapshot_at(&default_topology_request())
        .expect("duplicate source statuses should be merged");

    assert_eq!(first, second);
}

#[test]
fn malformed_topology_source_statuses_are_unverified_instead_of_fresh() {
    let mut input = topology_fixture_input(fixture_scope());
    input.source_status.push(thalassa_domain::SourceStatus {
        source_key: "account_id".into(),
        state: SourceState::Fresh,
        reason: None,
        detail: None,
        observed_at: None,
        evidence_ids: Vec::new(),
    });

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("malformed source status should not prevent projection");
    let status = snapshot
        .source_status
        .iter()
        .find(|status| status.source_key == "source")
        .expect("malformed source should have an explicit status");
    assert_eq!(status.state, SourceState::Unverified);
    assert_eq!(
        status.detail.as_deref(),
        Some("source record was omitted after validation")
    );
}
