use std::collections::BTreeSet;

use thalassa_domain::{
    SourceState, StatusReason, TopologyDirection, TopologyEdgeKind, TopologyError, TopologyFilter,
    TopologyPathTermination, TopologyRequest, TopologyTraversal,
};
use thalassaops::cloud::CloudResource;
use thalassaops::observability::alertmanager::ResourceReference;
use thalassaops::topology::{
    default_topology_request, fixture_scope, topology_fixture_input, TopologyBuilder,
};
use uuid::Uuid;

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
fn partially_missing_environment_evidence_omits_the_environment_node() {
    let mut input = topology_fixture_input(fixture_scope());
    input.environments[0]
        .evidence_ids
        .push("evidence-topology-missing".into());

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("missing evidence should degrade the source");

    assert!(snapshot
        .nodes
        .iter()
        .all(|node| node.name != "AWS production"));
    assert!(snapshot
        .source_status
        .iter()
        .any(|status| status.source_key == "cloud" && status.state == SourceState::Unverified));
}

#[test]
fn partially_missing_environment_metric_evidence_omits_the_metric() {
    let mut input = topology_fixture_input(fixture_scope());
    input.environments[0]
        .resource_count
        .evidence_ids
        .push("evidence-topology-missing".into());

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("missing metric evidence should degrade the source");
    let environment = snapshot
        .nodes
        .iter()
        .find(|node| node.name == "AWS production")
        .expect("the environment record should remain visible");

    assert!(environment.metric.is_none());
    assert!(snapshot
        .source_status
        .iter()
        .any(|status| status.source_key == "cloud" && status.state == SourceState::Unverified));
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
fn evidence_matching_prefers_typed_resource_identity() {
    let mut input = topology_fixture_input(fixture_scope());
    let production = input
        .kubernetes
        .get_mut("env-aws-prod")
        .expect("fixture should contain the production inventory");
    let service = production
        .resources
        .iter()
        .find(|item| item.resource.kind == "Service")
        .expect("fixture should contain a service")
        .clone();
    let mut same_name_service = service;
    same_name_service.resource.id = Uuid::from_u128(0x00000000000000000000000000000998);
    same_name_service.resource.native_id = Some("uid-service-checkout-api".into());
    same_name_service.resource.name = "prod/checkout-api".into();
    production.resources.push(same_name_service);

    let mut service_evidence = input
        .evidence
        .iter()
        .find(|evidence| evidence.id == "evidence-topology-k8s-service-checkout")
        .expect("fixture should contain service evidence")
        .clone();
    service_evidence.id = "evidence-topology-k8s-service-checkout-api".into();
    service_evidence.query = Some("service-checkout-api".into());
    service_evidence.excerpt = "checkout API service".into();
    input.evidence.push(service_evidence);

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("same-name resources should remain valid");
    let service = snapshot
        .nodes
        .iter()
        .find(|node| {
            node.kind == thalassa_domain::TopologyNodeKind::Service && node.name == "checkout-api"
        })
        .expect("the same-name service should be present");
    let workload = snapshot
        .nodes
        .iter()
        .find(|node| {
            node.kind == thalassa_domain::TopologyNodeKind::Workload && node.name == "checkout-api"
        })
        .expect("the workload should be present");

    assert!(service
        .evidence_ids
        .contains(&"evidence-topology-k8s-service-checkout-api".to_string()));
    assert!(!service
        .evidence_ids
        .contains(&"evidence-topology-k8s-workload-checkout-api".to_string()));
    assert!(workload
        .evidence_ids
        .contains(&"evidence-topology-k8s-workload-checkout-api".to_string()));
    assert!(!workload
        .evidence_ids
        .contains(&"evidence-topology-k8s-service-checkout-api".to_string()));
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
fn topology_omits_pagination_cursor_evidence() {
    let mut input = topology_fixture_input(fixture_scope());
    let evidence_id = input.evidence[0].id.clone();
    input.evidence[0].query = Some("nextLink=opaque-page-value".into());

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("cursor-bearing evidence should degrade the source");
    let serialized = serde_json::to_string(&snapshot).expect("snapshot should serialize");
    assert!(!serialized.contains("opaque-page-value"));
    assert!(!snapshot
        .evidence
        .iter()
        .any(|evidence| evidence.id == evidence_id));
}

#[test]
fn topology_omits_credential_marker_evidence() {
    let mut input = topology_fixture_input(fixture_scope());
    let evidence_id = input.evidence[0].id.clone();
    input.evidence[0].query = Some("api_key=opaque-value".into());

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("credential-bearing evidence should degrade the source");
    let serialized = serde_json::to_string(&snapshot).expect("snapshot should serialize");
    assert!(!serialized.contains("opaque-value"));
    assert!(!snapshot
        .evidence
        .iter()
        .any(|evidence| evidence.id == evidence_id));
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
fn cycle_closing_edge_contributes_to_path_confidence() {
    let mut input = topology_fixture_input(fixture_scope());
    let closing_edge = input
        .fixture_edges
        .iter_mut()
        .find(|edge| edge.id == "edge:fixture:replica-depends-on-rds")
        .expect("fixture should contain the cycle closing edge");
    closing_edge.confidence = 0.2;

    let base = TopologyBuilder::from_input(input.clone())
        .snapshot_at(&default_topology_request())
        .expect("healthy fixture should build");
    let database_id = node_id_by_name(&base, "checkout-rds");
    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&request_for(
            Some(database_id),
            TopologyDirection::Downstream,
            8,
        ))
        .expect("cycle traversal should build");

    let cycle = snapshot
        .paths
        .iter()
        .find(|path| {
            path.termination == TopologyPathTermination::CycleDetected
                && path.cycle_edge_id.as_deref() == Some("edge:fixture:replica-depends-on-rds")
        })
        .expect("the modified cycle should be reported explicitly");
    assert_eq!(cycle.confidence, 0.2);
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

#[test]
fn sensitive_topology_source_status_fields_are_unverified_instead_of_fresh() {
    let mut input = topology_fixture_input(fixture_scope());
    input.source_status[0].observed_at = Some("token=opaque-fixture-value".into());

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("unsafe source status fields should not prevent projection");
    let status = snapshot
        .source_status
        .iter()
        .find(|status| status.source_key == "cloud")
        .expect("fixture should contain a cloud source status");
    assert_eq!(status.state, SourceState::Unverified);
    assert_eq!(status.observed_at, None);
    assert_eq!(
        status.detail.as_deref(),
        Some("source record was omitted after validation")
    );
}

#[test]
fn unsafe_topology_metric_keys_are_omitted_and_mark_source_unverified() {
    let mut input = topology_fixture_input(fixture_scope());
    input.environments[0].resource_count.key = "account_id".into();

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("unsafe metric identity should not prevent projection");
    let environment = snapshot
        .nodes
        .iter()
        .find(|node| node.name == "AWS production")
        .expect("fixture should contain the AWS environment node");
    assert!(environment.metric.is_none());
    assert!(snapshot
        .source_status
        .iter()
        .any(|status| status.source_key == "cloud" && status.state == SourceState::Unverified));
}

#[test]
fn conflicting_topology_edge_identities_are_rejected_as_ambiguous() {
    let mut input = topology_fixture_input(fixture_scope());
    let mut conflicting_edge = input.fixture_edges[0].clone();
    conflicting_edge.downstream_node_id = input.fixture_edges[1].downstream_node_id.clone();
    input.fixture_edges.push(conflicting_edge);

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("conflicting edge identity should not prevent projection");
    assert!(!snapshot
        .edges
        .iter()
        .any(|edge| edge.id == "edge:fixture:checkout-depends-on-api"));
    assert!(snapshot
        .source_status
        .iter()
        .any(|status| status.source_key == "fixtures" && status.state == SourceState::Unverified));
}

#[test]
fn exact_topology_edge_duplicates_are_collapsed() {
    let mut input = topology_fixture_input(fixture_scope());
    input.fixture_edges.push(input.fixture_edges[0].clone());

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("exact duplicate edge should be collapsed");
    assert_eq!(
        snapshot
            .edges
            .iter()
            .filter(|edge| edge.id == "edge:fixture:checkout-depends-on-api")
            .count(),
        1
    );
    assert!(snapshot
        .source_status
        .iter()
        .find(|status| status.source_key == "fixtures")
        .is_some_and(|status| status.state == SourceState::Fresh));
}

#[test]
fn namespace_is_part_of_kubernetes_identity_without_a_native_id() {
    let mut input = topology_fixture_input(fixture_scope());
    let production = input
        .kubernetes
        .get_mut("env-aws-prod")
        .expect("fixture should contain the production inventory");
    let service_index = production
        .resources
        .iter()
        .position(|item| item.resource.kind == "Service")
        .expect("fixture should contain a service");
    production.resources[service_index].resource.native_id = None;
    let mut staging_service = production.resources[service_index].clone();
    staging_service.resource.id = Uuid::from_u128(0x00000000000000000000000000000999);
    staging_service.resource.name = "staging/checkout".into();
    staging_service
        .resource
        .labels
        .insert("namespace".into(), "staging".into());
    staging_service.resource.native_id = None;
    production.resources.push(staging_service);

    let mut staging_evidence = input
        .evidence
        .iter()
        .find(|evidence| evidence.id == "evidence-topology-k8s-service-checkout")
        .expect("fixture should contain service evidence")
        .clone();
    staging_evidence.id = "evidence-topology-k8s-service-checkout-staging".into();
    staging_evidence.query = Some("staging/checkout".into());
    staging_evidence.excerpt = "staging checkout service".into();
    input.evidence.push(staging_evidence);

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("namespace-qualified resources should remain valid");
    assert_eq!(
        snapshot
            .nodes
            .iter()
            .filter(|node| {
                node.kind == thalassa_domain::TopologyNodeKind::Service
                    && node.environment_id.as_deref() == Some("env-aws-prod")
            })
            .count(),
        2
    );
}

#[test]
fn conflicting_embedded_and_explicit_namespaces_do_not_bind_observability() {
    let mut input = topology_fixture_input(fixture_scope());
    let production = input
        .kubernetes
        .get_mut("env-aws-prod")
        .expect("fixture should contain the production inventory");
    let service = production
        .resources
        .iter()
        .find(|item| item.resource.kind == "Service")
        .expect("fixture should contain a service")
        .clone();
    let mut conflicting_service = service;
    conflicting_service.resource.id = Uuid::from_u128(0x00000000000000000000000000000998);
    conflicting_service.resource.name = "staging/checkout".into();
    conflicting_service.resource.native_id = Some("uid-service-checkout-staging".into());
    production.resources.push(conflicting_service);

    input.alerts[0].resource_reference = ResourceReference::Resolved {
        namespace: "staging".into(),
        kind: "Service".into(),
        name: "prod/checkout".into(),
    };
    input
        .evidence
        .iter_mut()
        .find(|evidence| evidence.id == "evidence-topology-alert-checkout")
        .expect("fixture should contain alert evidence")
        .query = Some("alert-checkout-s1".into());

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("conflicting resource references should degrade observability");
    let staging = snapshot
        .nodes
        .iter()
        .find(|node| node.native_id.as_deref() == Some("uid-service-checkout-staging"))
        .expect("the conflicting service should remain in the graph");
    assert!(!staging
        .evidence_ids
        .contains(&"evidence-topology-alert-checkout".to_string()));
    assert!(snapshot.source_status.iter().any(|status| {
        status.source_key == "observability" && status.state == SourceState::Unverified
    }));
}

#[test]
fn duplicate_kubernetes_namespaces_do_not_select_an_arbitrary_containment_parent() {
    let mut input = topology_fixture_input(fixture_scope());
    let production = input
        .kubernetes
        .get_mut("env-aws-prod")
        .expect("fixture should contain the production inventory");
    let namespace = production
        .resources
        .iter()
        .find(|item| item.resource.kind == "Namespace" && item.resource.name == "prod")
        .expect("fixture should contain the production namespace")
        .clone();
    let mut duplicate = namespace;
    duplicate.resource.id = Uuid::from_u128(0x00000000000000000000000000000998);
    duplicate.resource.native_id = Some("uid-namespace-prod-duplicate".into());
    production.resources.push(duplicate);

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("ambiguous namespace identity should not prevent projection");
    let namespace_ids: BTreeSet<_> = snapshot
        .nodes
        .iter()
        .filter(|node| {
            node.kind == thalassa_domain::TopologyNodeKind::Namespace && node.name == "prod"
        })
        .map(|node| node.id.clone())
        .collect();
    let service_id = node_id_by_name(&snapshot, "checkout");
    assert!(namespace_ids.len() > 1);
    assert!(!snapshot.edges.iter().any(|edge| {
        edge.kind == TopologyEdgeKind::Contains
            && namespace_ids.contains(&edge.upstream_node_id)
            && edge.downstream_node_id == service_id
    }));
    assert!(snapshot.source_status.iter().any(|status| {
        status.source_key == "kubernetes:env-aws-prod" && status.state == SourceState::Unverified
    }));
}

#[test]
fn unsafe_fixture_edge_ids_are_not_emitted() {
    let mut input = topology_fixture_input(fixture_scope());
    input.fixture_edges[0].id = "arn:aws:iam::123456789012:role/topology".into();

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("unsafe fixture edge identity should be omitted");
    assert!(snapshot
        .edges
        .iter()
        .all(|edge| !edge.id.contains("arn:aws:iam::123456789012")));
    assert!(snapshot
        .source_status
        .iter()
        .any(|status| status.source_key == "fixtures" && status.state == SourceState::Unverified));
}

#[test]
fn duplicate_resource_ids_do_not_select_an_arbitrary_incident_root() {
    let mut input = topology_fixture_input(fixture_scope());
    let production = input
        .kubernetes
        .get_mut("env-aws-prod")
        .expect("fixture should contain the production inventory");
    let service = production
        .resources
        .iter()
        .find(|item| item.resource.kind == "Service")
        .expect("fixture should contain a service")
        .clone();
    let mut duplicate = service.clone();
    duplicate.resource.name = "prod/checkout-other".into();
    duplicate.resource.native_id = Some("uid-service-checkout-other".into());
    production.resources.push(duplicate);

    let mut duplicate_evidence = input
        .evidence
        .iter()
        .find(|evidence| evidence.id == "evidence-topology-k8s-service-checkout")
        .expect("fixture should contain service evidence")
        .clone();
    duplicate_evidence.id = "evidence-topology-k8s-service-checkout-other".into();
    duplicate_evidence.query = Some("checkout-other".into());
    duplicate_evidence.excerpt = "checkout-other service".into();
    input.evidence.push(duplicate_evidence);

    let mut request = default_topology_request();
    request.filter.incident_id = Some("alert-checkout-s1".into());
    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&request)
        .expect("ambiguous resource identity should not prevent projection");
    assert_eq!(snapshot.summary.affected_nodes.value, 0.0);
    assert!(snapshot.nodes.iter().all(|node| !node.affected_by_incident));
    assert!(snapshot.source_status.iter().any(|status| {
        status.source_key == "incidents" && status.state == SourceState::Unverified
    }));
}

#[test]
fn conflicting_cloud_node_identities_are_omitted() {
    let mut input = topology_fixture_input(fixture_scope());
    let mut conflicting = input.cloud_resources[0].clone();
    conflicting.name = "checkout-rds-alias".into();
    input.cloud_resources.push(conflicting);

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("conflicting cloud identity should not prevent projection");
    assert!(!snapshot
        .nodes
        .iter()
        .any(|node| { node.id == "node:cloud:env-aws-prod:cloud_resource:checkout-rds" }));
    assert!(snapshot
        .source_status
        .iter()
        .any(|status| status.source_key == "cloud" && status.state == SourceState::Unverified));
}

#[test]
fn exact_cloud_node_duplicates_are_collapsed() {
    let mut input = topology_fixture_input(fixture_scope());
    input.cloud_resources.push(input.cloud_resources[0].clone());

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("exact cloud duplicate should be collapsed");
    assert_eq!(
        snapshot
            .nodes
            .iter()
            .filter(|node| { node.id == "node:cloud:env-aws-prod:cloud_resource:checkout-rds" })
            .count(),
        1
    );
    assert!(snapshot
        .source_status
        .iter()
        .find(|status| status.source_key == "cloud")
        .is_some_and(|status| status.state == SourceState::Fresh));
}

#[test]
fn conflicting_observability_alerts_are_not_selected_by_input_order() {
    let mut first_input = topology_fixture_input(fixture_scope());
    let mut conflicting_alert = first_input.alerts[0].clone();
    conflicting_alert
        .labels
        .insert("environment".into(), "env-gcp-staging".into());
    conflicting_alert.resource_reference = ResourceReference::Resolved {
        namespace: "staging".into(),
        kind: "Service".into(),
        name: "catalog".into(),
    };
    let mut catalog_evidence = first_input
        .evidence
        .iter()
        .find(|evidence| evidence.id == "evidence-topology-alert-checkout")
        .expect("fixture should contain alert evidence")
        .clone();
    catalog_evidence.id = "evidence-topology-alert-catalog".into();
    catalog_evidence.query = Some("catalog".into());
    catalog_evidence.excerpt = "catalog alert is firing".into();
    first_input.evidence.push(catalog_evidence);
    first_input.alerts.push(conflicting_alert.clone());

    let mut second_input = first_input.clone();
    second_input.alerts.reverse();

    let first = TopologyBuilder::from_input(first_input)
        .snapshot_at(&default_topology_request())
        .expect("conflicting alerts should not prevent projection");
    let second = TopologyBuilder::from_input(second_input)
        .snapshot_at(&default_topology_request())
        .expect("conflicting alerts should not prevent projection");

    assert_eq!(first, second);
    assert!(first.nodes.iter().all(|node| {
        !node.evidence_ids.iter().any(|id| {
            id == "evidence-topology-alert-checkout" || id == "evidence-topology-alert-catalog"
        })
    }));
    assert!(first.source_status.iter().any(|status| {
        status.source_key == "observability" && status.state == SourceState::Unverified
    }));
}

#[test]
fn conflicting_observability_metrics_are_not_selected_by_input_order() {
    let mut input = topology_fixture_input(fixture_scope());
    let mut conflicting_metric = input.metrics[0].clone();
    conflicting_metric
        .labels
        .insert("environment".into(), "env-gcp-staging".into());
    conflicting_metric
        .labels
        .insert("namespace".into(), "staging".into());
    conflicting_metric
        .labels
        .insert("service".into(), "catalog".into());
    input.metrics.push(conflicting_metric);

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("conflicting metrics should not prevent projection");
    assert!(snapshot.nodes.iter().all(|node| {
        !node
            .evidence_ids
            .iter()
            .any(|id| id == "evidence-topology-metric-checkout")
    }));
    assert!(snapshot.source_status.iter().any(|status| {
        status.source_key == "observability" && status.state == SourceState::Unverified
    }));
}
