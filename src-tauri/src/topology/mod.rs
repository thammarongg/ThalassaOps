//! Deterministic, read-only service and resource topology projection.
//!
//! The topology module consumes provider-neutral records emitted by the
//! existing Kubernetes, cloud, observability and Operations modules.  It does
//! not perform provider access or expose an IPC command; the application layer
//! owns that boundary in a later sprint task.

mod derive;
pub mod fixtures;
mod ownership;
mod traversal;

pub use fixtures::{
    default_topology_request, fixture_scope, fixture_time, topology_fixture_input, TopologyInput,
};
pub use thalassa_domain::{
    TopologyDirection, TopologyEdge, TopologyEdgeKind, TopologyError, TopologyFilter,
    TopologyMetric, TopologyNode, TopologyNodeKind, TopologyOwnership, TopologyOwnershipRule,
    TopologyOwnershipSelector, TopologyOwnershipSource, TopologyPath, TopologyPathKind,
    TopologyPathTermination, TopologyRequest, TopologySnapshot, TopologySourceKind,
    TopologySummary, TopologyTraversal,
};

use crate::topology::derive::{derive_graph, DerivedGraph};
use crate::topology::traversal::traverse;
use std::collections::{BTreeMap, BTreeSet};
use thalassa_domain::{
    ConsoleHealthState, DrillDownDestination, DrillDownReference, DrillDownTarget, NumberUnit,
    ResourceScope,
};

/// In-memory graph builder backed by provider-neutral source records.
#[derive(Clone, Debug)]
pub struct TopologyBuilder {
    input: TopologyInput,
}

impl TopologyBuilder {
    /// Create a builder from source records.  No provider or network access is
    /// performed by this constructor.
    pub fn from_input(input: TopologyInput) -> Self {
        Self { input }
    }

    /// Build a deterministic topology snapshot for a bounded request.
    pub fn snapshot_at(
        &self,
        request: &TopologyRequest,
    ) -> Result<TopologySnapshot, TopologyError> {
        request.validate()?;
        let mut graph = derive_graph(&self.input);

        let node_ids = graph.nodes.keys().cloned().collect::<BTreeSet<_>>();
        let environment_ids = graph
            .nodes
            .values()
            .filter_map(|node| node.environment_id.clone())
            .collect::<BTreeSet<_>>();
        let mut team_ids = graph
            .nodes
            .values()
            .filter_map(|node| node.ownership.team_id)
            .collect::<BTreeSet<_>>();
        team_ids.extend(self.input.scope.team_id);
        team_ids.extend(
            self.input
                .ownership_rules
                .iter()
                .filter(|rule| rule.validate().is_ok())
                .map(|rule| rule.team_id),
        );
        request.validate_against(&node_ids, &environment_ids, &team_ids, &graph.incident_ids)?;

        let incident_roots = resolve_incident_roots(&mut graph, request)?;
        let traversal_roots = traversal_roots(request, &incident_roots);

        let visible_node_ids = visible_nodes(&graph.nodes, &request.filter);
        let mut nodes = graph
            .nodes
            .into_iter()
            .filter(|(id, _)| visible_node_ids.contains(id))
            .collect::<BTreeMap<_, _>>();

        for root in &incident_roots {
            if let Some(node) = nodes.get_mut(root) {
                node.affected_by_incident = true;
            }
        }

        let edges = graph
            .edges
            .into_iter()
            .filter(|edge| {
                visible_node_ids.contains(&edge.upstream_node_id)
                    && visible_node_ids.contains(&edge.downstream_node_id)
            })
            .collect::<Vec<_>>();

        let paths = if request.traversal.max_depth == 0 || traversal_roots.is_empty() {
            Vec::new()
        } else {
            traverse(&traversal_roots, &nodes, &edges, request.traversal)?
        };

        let summary = topology_summary(&nodes, &edges, &paths, &graph.evidence, &self.input.scope);
        let mut source_status = graph.source_status.into_values().collect::<Vec<_>>();
        source_status.sort_by(|left, right| left.source_key.cmp(&right.source_key));
        let evidence = graph.evidence.into_values().collect::<Vec<_>>();

        let snapshot = TopologySnapshot {
            generated_at: fixture_or_input_timestamp(&self.input),
            scope: self.input.scope.clone(),
            filter: request.filter.clone(),
            focus_node_id: request.focus_node_id.clone(),
            traversal: request.traversal,
            summary,
            nodes: nodes.into_values().collect(),
            edges,
            paths,
            source_status,
            evidence,
        };

        snapshot.validate()?;
        Ok(snapshot)
    }
}

fn fixture_or_input_timestamp(input: &TopologyInput) -> String {
    input
        .generated_at
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn resolve_incident_roots(
    graph: &mut DerivedGraph,
    request: &TopologyRequest,
) -> Result<BTreeSet<String>, TopologyError> {
    let Some(incident_id) = request.filter.incident_id.as_ref() else {
        return Ok(BTreeSet::new());
    };

    if !graph.incident_ids.contains(incident_id) {
        return Err(TopologyError::IncidentNotFound);
    }

    let mut roots = BTreeSet::new();
    if let Some(candidate_roots) = graph.incident_root_nodes.get(incident_id).cloned() {
        for node_id in candidate_roots {
            if graph.nodes.contains_key(&node_id) {
                roots.insert(node_id.clone());
            } else {
                graph.mark_unverified("incidents");
            }
        }
    }
    if let Some(resource_ids) = graph.incident_affected_resources.get(incident_id).cloned() {
        for resource_id in resource_ids {
            if let Some(node_id) = graph.resource_id_nodes.get(&resource_id) {
                roots.insert(node_id.clone());
            } else {
                graph.mark_unverified("incidents");
            }
        }
    }
    Ok(roots)
}

fn traversal_roots(request: &TopologyRequest, incident_roots: &BTreeSet<String>) -> Vec<String> {
    let mut roots = incident_roots.clone();
    if let Some(focus_node_id) = request.focus_node_id.as_ref() {
        roots.insert(focus_node_id.clone());
    }
    roots.into_iter().collect()
}

fn visible_nodes(
    nodes: &BTreeMap<String, TopologyNode>,
    filter: &TopologyFilter,
) -> BTreeSet<String> {
    nodes
        .values()
        .filter(|node| {
            let environment_matches = filter.environment_ids.is_empty()
                || node
                    .environment_id
                    .as_ref()
                    .is_some_and(|environment_id| filter.environment_ids.contains(environment_id));
            let team_matches = filter.team_ids.is_empty()
                || filter
                    .team_ids
                    .iter()
                    .any(|team_id| node.ownership.team_id == Some(*team_id));
            environment_matches && team_matches
        })
        .map(|node| node.id.clone())
        .collect()
}

fn topology_summary(
    nodes: &BTreeMap<String, TopologyNode>,
    edges: &[thalassa_domain::TopologyEdge],
    paths: &[TopologyPath],
    evidence: &BTreeMap<String, thalassa_domain::EvidenceRef>,
    scope: &ResourceScope,
) -> TopologySummary {
    TopologySummary {
        visible_nodes: summary_metric(
            "visible_nodes",
            nodes.len(),
            nodes.values(),
            evidence,
            scope,
        ),
        visible_edges: summary_metric("visible_edges", edges.len(), edges.iter(), evidence, scope),
        affected_nodes: summary_metric(
            "affected_nodes",
            nodes
                .values()
                .filter(|node| node.affected_by_incident)
                .count(),
            nodes.values().filter(|node| node.affected_by_incident),
            evidence,
            scope,
        ),
        probable_paths: summary_metric(
            "probable_paths",
            paths.len(),
            paths.iter(),
            evidence,
            scope,
        ),
    }
}

fn summary_metric<T, I>(
    key: &str,
    count: usize,
    records: I,
    evidence: &BTreeMap<String, thalassa_domain::EvidenceRef>,
    scope: &ResourceScope,
) -> TopologyMetric
where
    I: IntoIterator<Item = T>,
    T: EvidenceIds,
{
    let mut evidence_ids = records
        .into_iter()
        .flat_map(|record| record.evidence_ids().to_vec())
        .filter(|id| evidence.contains_key(id))
        .collect::<BTreeSet<_>>();
    if evidence_ids.is_empty() {
        if let Some(first) = evidence.keys().next() {
            evidence_ids.insert(first.clone());
        }
    }
    let evidence_ids = evidence_ids.into_iter().collect::<Vec<_>>();
    let drill_down = DrillDownTarget {
        destination: DrillDownDestination::Topology,
        evidence_ids: evidence_ids.clone(),
        filter_key: Some(format!("summary:{key}")),
    };
    TopologyMetric {
        key: key.into(),
        value: count as f64,
        unit: NumberUnit::Count,
        evidence_ids: evidence_ids.clone(),
        drill_down,
        drill_down_reference: DrillDownReference {
            source_query: format!("topology:{key}"),
            scope: scope.clone(),
            time_window: None,
            evidence_ids,
        },
    }
}

trait EvidenceIds {
    fn evidence_ids(&self) -> &[String];
}

impl EvidenceIds for &TopologyNode {
    fn evidence_ids(&self) -> &[String] {
        &self.evidence_ids
    }
}

impl EvidenceIds for TopologyNode {
    fn evidence_ids(&self) -> &[String] {
        &self.evidence_ids
    }
}

impl EvidenceIds for &thalassa_domain::TopologyEdge {
    fn evidence_ids(&self) -> &[String] {
        &self.evidence_ids
    }
}

impl EvidenceIds for thalassa_domain::TopologyEdge {
    fn evidence_ids(&self) -> &[String] {
        &self.evidence_ids
    }
}

impl EvidenceIds for &TopologyPath {
    fn evidence_ids(&self) -> &[String] {
        &self.evidence_ids
    }
}

impl EvidenceIds for TopologyPath {
    fn evidence_ids(&self) -> &[String] {
        &self.evidence_ids
    }
}

#[allow(dead_code)]
fn _health_state_rank(state: ConsoleHealthState) -> u8 {
    match state {
        ConsoleHealthState::Healthy => 0,
        ConsoleHealthState::Degraded => 1,
        ConsoleHealthState::Critical => 2,
        ConsoleHealthState::Unknown => 3,
    }
}

#[allow(dead_code)]
fn _default_traversal() -> TopologyTraversal {
    TopologyTraversal {
        direction: TopologyDirection::Both,
        max_depth: 3,
    }
}
