use std::collections::BTreeSet;

use thalassa_domain::{
    ResourceScope, SourceState, SourceStatus, StatusReason, TopologyDirection, TopologyFilter,
    TopologyRequest, TopologyTraversal,
};
use thalassaops::topology::{
    default_topology_request, fixture_scope, topology_fixture_input, TopologyBuilder,
};

fn request(filter: TopologyFilter, max_depth: u16) -> TopologyRequest {
    TopologyRequest {
        filter,
        focus_node_id: None,
        traversal: TopologyTraversal {
            direction: TopologyDirection::Both,
            max_depth,
        },
    }
}

fn team_id() -> uuid::Uuid {
    fixture_scope()
        .team_id
        .expect("fixture has a canonical team")
}

#[test]
fn environment_filter_selects_only_the_requested_environment() {
    let filter = TopologyFilter {
        environment_ids: vec!["env-gcp-staging".into()],
        team_ids: vec![],
        incident_id: None,
    };
    let snapshot = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&request(filter.clone(), 0))
        .expect("environment filter should build");

    assert!(!snapshot.nodes.is_empty());
    assert!(snapshot
        .nodes
        .iter()
        .all(|node| node.environment_id.as_deref() == Some("env-gcp-staging")));
    assert_eq!(snapshot.filter, filter);
}

#[test]
fn team_filter_uses_team_ids_and_excludes_unassigned_nodes() {
    let filter = TopologyFilter {
        environment_ids: vec![],
        team_ids: vec![team_id()],
        incident_id: None,
    };
    let snapshot = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&request(filter.clone(), 0))
        .expect("team filter should build");

    assert!(!snapshot.nodes.is_empty());
    assert!(snapshot
        .nodes
        .iter()
        .all(|node| node.ownership.team_id == Some(team_id())));
    assert!(!snapshot
        .nodes
        .iter()
        .any(|node| node.name == "unassigned-worker"));
    assert_eq!(snapshot.filter, filter);
}

#[test]
fn incident_filter_selects_affected_roots_from_the_sprint_eleven_queue() {
    let filter = TopologyFilter {
        environment_ids: vec![],
        team_ids: vec![],
        incident_id: Some("alert-checkout-s1".into()),
    };
    let snapshot = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&request(filter.clone(), 0))
        .expect("incident filter should build");

    assert_eq!(snapshot.nodes.len(), 1);
    assert_eq!(snapshot.nodes[0].name, "checkout");
    assert!(snapshot.nodes[0].affected_by_incident);
    assert_eq!(snapshot.filter, filter);
}

#[test]
fn incident_root_resolution_prefers_explicit_resource_ids() {
    let mut input = topology_fixture_input(fixture_scope());
    input.incident_root_nodes.insert(
        "alert-checkout-s1".into(),
        vec!["node:kubernetes:env-aws-prod:workload:uid-workload-unassigned-worker".into()],
    );
    let filter = TopologyFilter {
        environment_ids: vec![],
        team_ids: vec![],
        incident_id: Some("alert-checkout-s1".into()),
    };

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&request(filter, 0))
        .expect("incident filter should build");

    assert_eq!(snapshot.nodes.len(), 1);
    assert_eq!(snapshot.nodes[0].name, "checkout");
}

#[test]
fn incident_root_resolution_prefers_exact_source_binding_over_fixture_binding() {
    let mut input = topology_fixture_input(fixture_scope());
    input
        .incident_queue
        .first_mut()
        .expect("fixture incident")
        .affected_scope
        .resource_ids
        .clear();
    input.incident_root_nodes.insert(
        "alert-checkout-s1".into(),
        vec!["node:kubernetes:env-aws-prod:workload:uid-workload-unassigned-worker".into()],
    );
    let filter = TopologyFilter {
        environment_ids: vec![],
        team_ids: vec![],
        incident_id: Some("alert-checkout-s1".into()),
    };

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&request(filter, 0))
        .expect("incident filter should build");

    assert_eq!(snapshot.nodes.len(), 1);
    assert_eq!(snapshot.nodes[0].name, "checkout");
}

#[test]
fn invalid_source_binding_does_not_fall_back_to_fixture_binding() {
    let mut input = topology_fixture_input(fixture_scope());
    input
        .incident_queue
        .first_mut()
        .expect("fixture incident")
        .affected_scope
        .resource_ids
        .clear();
    input.alerts[0].resource_reference =
        thalassaops::observability::alertmanager::ResourceReference::Resolved {
            namespace: "prod".into(),
            kind: "Service".into(),
            name: "missing".into(),
        };
    let filter = TopologyFilter {
        environment_ids: vec![],
        team_ids: vec![],
        incident_id: Some("alert-checkout-s1".into()),
    };

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&request(filter, 0))
        .expect("invalid incident roots should degrade the projection");

    assert!(snapshot.nodes.is_empty());
    assert_eq!(snapshot.summary.affected_nodes.value, 0.0);
    assert!(snapshot.source_status.iter().any(|status| {
        status.source_key == "incidents" && status.state == SourceState::Unverified
    }));
}

#[test]
fn environment_team_and_incident_filters_intersect_without_widening() {
    let filter = TopologyFilter {
        environment_ids: vec!["env-aws-prod".into()],
        team_ids: vec![team_id()],
        incident_id: Some("alert-checkout-s1".into()),
    };
    let snapshot = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&request(filter.clone(), 3))
        .expect("composed filters should build");

    assert!(!snapshot.nodes.is_empty());
    assert!(snapshot.nodes.iter().all(|node| {
        node.environment_id.as_deref() == Some("env-aws-prod")
            && node.ownership.team_id == Some(team_id())
    }));
    assert!(snapshot.nodes.iter().any(|node| node.affected_by_incident));
    assert_eq!(snapshot.filter, filter);
}

#[test]
fn empty_filter_result_reports_reason_and_applied_filter() {
    let mut input = topology_fixture_input(fixture_scope());
    input.alerts.clear();
    input.metrics.clear();
    input
        .incident_queue
        .first_mut()
        .expect("fixture incident")
        .affected_scope
        .resource_ids
        .clear();
    let unassigned_id = "node:kubernetes:env-aws-prod:workload:uid-workload-unassigned-worker";
    input
        .incident_root_nodes
        .insert("alert-checkout-s1".into(), vec![unassigned_id.into()]);
    let filter = TopologyFilter {
        environment_ids: vec!["env-aws-prod".into()],
        team_ids: vec![team_id()],
        incident_id: Some("alert-checkout-s1".into()),
    };
    let request = TopologyRequest {
        filter: filter.clone(),
        focus_node_id: None,
        traversal: TopologyTraversal {
            direction: TopologyDirection::Downstream,
            max_depth: 3,
        },
    };
    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&request)
        .expect("an empty intersection is a valid result");

    assert!(snapshot.nodes.is_empty());
    assert_eq!(snapshot.filter, filter);
    let empty_status = snapshot
        .source_status
        .iter()
        .find(|status| status.source_key == "topology_filter")
        .expect("empty result should carry a filter status");
    assert_eq!(empty_status.state, SourceState::Unavailable);
    assert_eq!(empty_status.reason, Some(StatusReason::NoDataInWindow));
    assert_eq!(empty_status.detail.as_deref(), Some("no_matching_nodes"));
}

#[test]
fn empty_filter_status_does_not_duplicate_an_input_status_key() {
    let mut input = topology_fixture_input(fixture_scope());
    input.source_status.push(SourceStatus {
        source_key: "topology_filter".into(),
        state: SourceState::Fresh,
        reason: None,
        detail: None,
        observed_at: None,
        evidence_ids: Vec::new(),
    });
    let filter = TopologyFilter {
        environment_ids: vec!["env-gcp-staging".into()],
        team_ids: vec![],
        incident_id: Some("alert-checkout-s1".into()),
    };
    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&request(filter, 0))
        .expect("an empty filtered graph should build");

    let filter_statuses = snapshot
        .source_status
        .iter()
        .filter(|status| status.source_key == "topology_filter")
        .collect::<Vec<_>>();
    assert_eq!(filter_statuses.len(), 1);
    assert_eq!(filter_statuses[0].state, SourceState::Unavailable);
    assert_eq!(
        filter_statuses[0].reason,
        Some(StatusReason::NoDataInWindow)
    );
}

#[test]
fn scope_foreign_node_never_surfaces_through_any_filter() {
    let mut input = topology_fixture_input(fixture_scope());
    let foreign_scope = ResourceScope::workspace(
        uuid::Uuid::from_u128(0x000000000000000000000000000000f1),
        uuid::Uuid::from_u128(0x000000000000000000000000000000f2),
        uuid::Uuid::from_u128(0x000000000000000000000000000000f3),
    );
    let inventory = input
        .kubernetes
        .get_mut("env-aws-prod")
        .expect("fixture production inventory");
    let mut foreign = inventory
        .resources
        .iter()
        .find(|resource| resource.resource.name == "prod/unassigned-worker")
        .expect("fixture unassigned resource")
        .clone();
    foreign.resource.id = uuid::Uuid::from_u128(0x000000000000000000000000000000f4);
    foreign.resource.name = "prod/foreign-worker".into();
    foreign.resource.native_id = Some("uid-foreign-worker".into());
    foreign.resource.scope = foreign_scope;
    inventory.resources.push(foreign);
    let mut foreign_cloud = input
        .cloud_resources
        .first()
        .expect("fixture cloud resource")
        .clone();
    foreign_cloud.environment_id = "env-foreign".into();
    foreign_cloud.id = "foreign-cloud-resource".into();
    foreign_cloud.name = "foreign-cloud-resource".into();
    input.cloud_resources.push(foreign_cloud);

    let requests = [
        default_topology_request(),
        request(
            TopologyFilter {
                environment_ids: vec!["env-aws-prod".into()],
                team_ids: vec![],
                incident_id: None,
            },
            0,
        ),
        request(
            TopologyFilter {
                environment_ids: vec![],
                team_ids: vec![team_id()],
                incident_id: None,
            },
            0,
        ),
        request(
            TopologyFilter {
                environment_ids: vec![],
                team_ids: vec![],
                incident_id: Some("alert-checkout-s1".into()),
            },
            3,
        ),
    ];
    for request in requests {
        let snapshot = TopologyBuilder::from_input(input.clone())
            .snapshot_at(&request)
            .expect("foreign source records should be omitted");
        assert!(!snapshot
            .nodes
            .iter()
            .any(|node| node.name == "foreign-worker"));
    }
}

#[test]
fn filter_results_have_stable_node_sets() {
    let filter = TopologyFilter {
        environment_ids: vec!["env-aws-prod".into(), "env-gcp-staging".into()],
        team_ids: vec![team_id()],
        incident_id: None,
    };
    let first = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&request(filter.clone(), 0))
        .expect("first filtered snapshot");
    let second = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&request(filter, 0))
        .expect("second filtered snapshot");
    let first_ids: BTreeSet<_> = first.nodes.iter().map(|node| node.id.clone()).collect();
    let second_ids: BTreeSet<_> = second.nodes.iter().map(|node| node.id.clone()).collect();
    assert_eq!(first_ids, second_ids);
}
