use thalassa_domain::{
    SourceState, TopologyFilter, TopologyOwnershipRule, TopologyOwnershipSelector,
    TopologyOwnershipSource, TopologyRequest, TopologyTraversal,
};
use thalassaops::topology::{
    default_topology_request, fixture_scope, topology_fixture_input, TopologyBuilder,
};

fn node_by_name<'a>(
    snapshot: &'a thalassa_domain::TopologySnapshot,
    name: &str,
) -> &'a thalassa_domain::TopologyNode {
    snapshot
        .nodes
        .iter()
        .find(|node| node.name == name)
        .unwrap_or_else(|| panic!("fixture should contain node {name}"))
}

#[test]
fn ownership_resolves_single_owner_with_documented_precedence() {
    let snapshot = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&default_topology_request())
        .expect("fixture ownership should resolve");

    let checkout = node_by_name(&snapshot, "checkout");
    assert_eq!(
        checkout.ownership.source,
        TopologyOwnershipSource::ExplicitLabel
    );
    assert_eq!(checkout.ownership.team_name.as_deref(), Some("Platform"));
    assert_eq!(
        checkout.ownership.evidence_ids,
        vec!["evidence-topology-ownership-platform"]
    );

    let pod = node_by_name(&snapshot, "checkout-api-0");
    assert_eq!(pod.ownership.source, TopologyOwnershipSource::ResourceScope);
    assert_eq!(pod.ownership.team_name.as_deref(), Some("Platform"));

    let staging = node_by_name(&snapshot, "catalog-api");
    assert_eq!(
        staging.ownership.source,
        TopologyOwnershipSource::ResourceScope
    );
    assert_eq!(staging.ownership.team_name.as_deref(), Some("Platform"));
}

#[test]
fn exact_node_mapping_wins_over_label_mapping_deterministically() {
    let mut input = topology_fixture_input(fixture_scope());
    let checkout_id = "node:kubernetes:env-aws-prod:service:uid-service-checkout";
    input.ownership_rules.push(TopologyOwnershipRule {
        selector: TopologyOwnershipSelector::NodeId {
            node_id: checkout_id.into(),
        },
        team_id: fixture_scope().team_id.expect("fixture team"),
        team_name: "Checkout".into(),
        source: TopologyOwnershipSource::Fixture,
        evidence_ids: vec!["evidence-topology-ownership-environment".into()],
    });

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("node mapping should resolve");
    let checkout = node_by_name(&snapshot, "checkout");
    assert_eq!(checkout.ownership.source, TopologyOwnershipSource::Fixture);
    assert_eq!(checkout.ownership.team_name.as_deref(), Some("Checkout"));
}

#[test]
fn conflicting_equal_specificity_ownership_is_reported_and_stays_unassigned() {
    let mut input = topology_fixture_input(fixture_scope());
    input.ownership_rules.push(TopologyOwnershipRule {
        selector: TopologyOwnershipSelector::Label {
            key: "team".into(),
            value: "platform".into(),
        },
        team_id: fixture_scope().team_id.expect("fixture team"),
        team_name: "Payments".into(),
        source: TopologyOwnershipSource::ExplicitLabel,
        evidence_ids: vec!["evidence-topology-ownership-environment".into()],
    });

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("ambiguous ownership should degrade the ownership source");
    let checkout = node_by_name(&snapshot, "checkout");
    assert_eq!(
        checkout.ownership.source,
        TopologyOwnershipSource::Unassigned
    );
    assert_eq!(checkout.ownership.team_id, None);
    assert_eq!(checkout.ownership.team_name, None);
    assert!(snapshot.source_status.iter().any(|status| {
        status.source_key == "ownership"
            && status.state == SourceState::Unverified
            && status.detail.as_deref() == Some("ambiguous_ownership")
    }));
}

#[test]
fn unowned_node_remains_explicitly_unassigned_without_owner_evidence() {
    let snapshot = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&default_topology_request())
        .expect("fixture ownership should resolve");
    let worker = node_by_name(&snapshot, "unassigned-worker");
    assert_eq!(worker.ownership.source, TopologyOwnershipSource::Unassigned);
    assert_eq!(worker.ownership.team_id, None);
    assert_eq!(worker.ownership.team_name, None);
    assert!(worker.ownership.evidence_ids.is_empty());
    assert!(!worker.evidence_ids.is_empty());
}

#[test]
fn malformed_or_duplicate_rules_are_not_selected_by_input_order() {
    let mut input = topology_fixture_input(fixture_scope());
    let checkout_id = "node:kubernetes:env-aws-prod:service:uid-service-checkout";
    let duplicate = TopologyOwnershipRule {
        selector: TopologyOwnershipSelector::NodeId {
            node_id: checkout_id.into(),
        },
        team_id: fixture_scope().team_id.expect("fixture team"),
        team_name: "Checkout".into(),
        source: TopologyOwnershipSource::Fixture,
        evidence_ids: vec!["evidence-topology-ownership-environment".into()],
    };
    input.ownership_rules.push(duplicate.clone());
    input.ownership_rules.push(duplicate);

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("duplicate rules should degrade the ownership source");
    let checkout = node_by_name(&snapshot, "checkout");
    assert_eq!(
        checkout.ownership.source,
        TopologyOwnershipSource::Unassigned
    );
    assert!(snapshot.source_status.iter().any(|status| {
        status.source_key == "ownership" && status.state == SourceState::Unverified
    }));
}

#[test]
fn ownership_rule_for_a_team_outside_the_workspace_is_not_emitted_or_filterable() {
    let mut input = topology_fixture_input(fixture_scope());
    input.ownership_rules[0].team_id = uuid::Uuid::from_u128(0x00000000000000000000000000000099);
    let snapshot = TopologyBuilder::from_input(input.clone())
        .snapshot_at(&default_topology_request())
        .expect("out-of-scope ownership rules should be omitted");
    let checkout = node_by_name(&snapshot, "checkout");
    assert_ne!(
        checkout.ownership.team_id,
        Some(uuid::Uuid::from_u128(0x00000000000000000000000000000099))
    );
    assert!(snapshot.source_status.iter().any(|status| {
        status.source_key == "ownership" && status.state == SourceState::Unverified
    }));

    let foreign_team_filter = TopologyRequest {
        filter: TopologyFilter {
            environment_ids: vec![],
            team_ids: vec![uuid::Uuid::from_u128(0x00000000000000000000000000000099)],
            incident_id: None,
        },
        focus_node_id: None,
        traversal: TopologyTraversal {
            direction: thalassa_domain::TopologyDirection::Both,
            max_depth: 0,
        },
    };
    assert!(matches!(
        TopologyBuilder::from_input(input).snapshot_at(&foreign_team_filter),
        Err(thalassa_domain::TopologyError::InvalidRequest)
    ));
}

#[test]
fn ownership_tests_use_the_public_snapshot_seam() {
    let request = TopologyRequest {
        filter: TopologyFilter {
            environment_ids: vec![],
            team_ids: vec![],
            incident_id: None,
        },
        focus_node_id: None,
        traversal: TopologyTraversal {
            direction: thalassa_domain::TopologyDirection::Both,
            max_depth: 0,
        },
    };
    let snapshot = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&request)
        .expect("ownership is resolved before traversal");
    assert!(snapshot.nodes.iter().all(|node| {
        node.ownership.team_id.is_some()
            || node.ownership.source == TopologyOwnershipSource::Unassigned
    }));
}
