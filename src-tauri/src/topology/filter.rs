//! Scope-safe composition of Environment, Team and Incident selections.

use std::collections::{BTreeMap, BTreeSet};

use super::derive::DerivedGraph;
use thalassa_domain::{
    SourceState, SourceStatus, StatusReason, TopologyError, TopologyFilter, TopologyNode,
    TopologyPath, TopologyRequest,
};

/// Stable reasons that explain why a filter result contains no nodes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EmptyFilterReason {
    IncidentHasNoExactRoot,
    NoMatchingNodes,
}

impl EmptyFilterReason {
    fn detail(self) -> &'static str {
        match self {
            Self::IncidentHasNoExactRoot => "incident_has_no_exact_root",
            Self::NoMatchingNodes => "no_matching_nodes",
        }
    }
}

/// Result of applying all topology filter dimensions to a workspace graph.
#[derive(Clone, Debug)]
pub(crate) struct FilterSelection {
    pub(crate) visible_node_ids: BTreeSet<String>,
    pub(crate) empty_reason: Option<EmptyFilterReason>,
}

/// Resolve IncidentQueueItem roots without accepting IDs outside the graph.
pub(crate) fn resolve_incident_roots(
    graph: &mut DerivedGraph,
    request: &TopologyRequest,
) -> Result<BTreeSet<String>, TopologyError> {
    let Some(incident_id) = request.filter.incident_id.as_ref() else {
        return Ok(BTreeSet::new());
    };

    if !graph.incident_ids.contains(incident_id) {
        return Err(TopologyError::IncidentNotFound);
    }

    let explicit_resource_ids = graph
        .incident_affected_resources
        .get(incident_id)
        .cloned()
        .unwrap_or_default();
    if !explicit_resource_ids.is_empty() {
        let mut roots = BTreeSet::new();
        for resource_id in explicit_resource_ids {
            if let Some(node_id) = graph.resource_id_nodes.get(&resource_id) {
                roots.insert(node_id.clone());
            } else {
                graph.mark_unverified("incidents");
            }
        }
        if roots.is_empty() {
            graph.mark_unverified("incidents");
        }
        return Ok(roots);
    }

    if graph.incident_source_binding_attempts.contains(incident_id) {
        let candidate_roots = graph
            .incident_root_nodes
            .get(incident_id)
            .cloned()
            .unwrap_or_default();
        let mut roots = BTreeSet::new();
        for node_id in candidate_roots {
            if graph.nodes.contains_key(&node_id) {
                roots.insert(node_id);
            } else {
                graph.mark_unverified("incidents");
            }
        }
        if roots.is_empty() {
            graph.mark_unverified("incidents");
        }
        // An adapter source record takes precedence even when it could not
        // resolve an exact node; never replace an ambiguous source identity
        // with a lower-precedence fixture guess.
        return Ok(roots);
    }

    let mut roots = BTreeSet::new();
    if let Some(candidate_roots) = graph.incident_fixture_root_nodes.get(incident_id).cloned() {
        for node_id in candidate_roots {
            if graph.nodes.contains_key(&node_id) {
                roots.insert(node_id);
            } else {
                graph.mark_unverified("incidents");
            }
        }
    }
    if roots.is_empty() {
        // A broad queue scope or missing adapter binding is valid input, but
        // it cannot honestly mark every resource as affected.
        graph.mark_unverified("incidents");
    }
    Ok(roots)
}

/// Add the optional focus node to traversal roots in deterministic order.
pub(crate) fn traversal_roots(
    request: &TopologyRequest,
    incident_roots: &BTreeSet<String>,
) -> Vec<String> {
    if let Some(focus_node_id) = request.focus_node_id.as_ref() {
        return vec![focus_node_id.clone()];
    }
    let roots = incident_roots.clone();
    roots.into_iter().collect()
}

/// Return nodes that are in the Incident selection and satisfy Environment ∩
/// Team. Empty dimensions are intentionally no-ops; values within one
/// dimension are OR'd by the request contract.
pub(crate) fn select_nodes(
    nodes: &BTreeMap<String, TopologyNode>,
    filter: &TopologyFilter,
    incident_roots: &BTreeSet<String>,
    traversal_roots: &[String],
    incident_paths: &[TopologyPath],
) -> FilterSelection {
    let incident_active = filter.incident_id.is_some();
    let mut candidate_node_ids: BTreeSet<String> = if incident_active {
        traversal_roots.iter().cloned().collect()
    } else {
        nodes.keys().cloned().collect()
    };
    if incident_active {
        for path in incident_paths {
            candidate_node_ids.extend(path.node_ids.iter().cloned());
        }
    }

    let visible_node_ids = nodes
        .values()
        .filter(|node| {
            if incident_active && !candidate_node_ids.contains(&node.id) {
                return false;
            }
            environment_matches(node, filter) && team_matches(node, filter)
        })
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();

    let filter_active = !filter.environment_ids.is_empty()
        || !filter.team_ids.is_empty()
        || filter.incident_id.is_some();
    let empty_reason = if filter_active && visible_node_ids.is_empty() {
        if incident_active && incident_roots.is_empty() {
            Some(EmptyFilterReason::IncidentHasNoExactRoot)
        } else {
            Some(EmptyFilterReason::NoMatchingNodes)
        }
    } else {
        None
    };

    FilterSelection {
        visible_node_ids,
        empty_reason,
    }
}

fn environment_matches(node: &TopologyNode, filter: &TopologyFilter) -> bool {
    filter.environment_ids.is_empty()
        || node
            .environment_id
            .as_ref()
            .is_some_and(|environment_id| filter.environment_ids.contains(environment_id))
}

fn team_matches(node: &TopologyNode, filter: &TopologyFilter) -> bool {
    filter.team_ids.is_empty()
        || filter
            .team_ids
            .iter()
            .any(|team_id| node.ownership.team_id == Some(*team_id))
}

/// Encode the empty selection as a typed source-status record. The request's
/// complete `TopologyFilter` remains on `TopologySnapshot`, so consumers can
/// explain both the stable reason and the dimensions that were applied.
pub(crate) fn empty_status(reason: EmptyFilterReason) -> SourceStatus {
    SourceStatus {
        source_key: "topology_filter".into(),
        state: SourceState::Unavailable,
        reason: Some(StatusReason::NoDataInWindow),
        detail: Some(reason.detail().into()),
        observed_at: None,
        evidence_ids: Vec::new(),
    }
}
