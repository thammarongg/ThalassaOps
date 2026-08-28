//! Deterministic, read-only service and resource topology projection.
//!
//! The topology module consumes provider-neutral records emitted by the
//! existing Kubernetes, cloud, observability and Operations modules.  It does
//! not perform provider access; the application layer owns the IPC boundary
//! and policy checks.

mod derive;
pub(crate) mod evidence;
mod filter;
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
use crate::topology::filter::{
    empty_status, resolve_incident_roots, select_nodes, traversal_roots,
};
use crate::topology::ownership::validate_rules;
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

/// Validate a topology request against the current workspace graph and queue
/// projection without returning a snapshot or performing provider I/O.
pub fn validate_topology_request(
    request: &TopologyRequest,
    input: &TopologyInput,
) -> Result<(), TopologyError> {
    request.validate()?;
    let graph = derive_graph(input);
    validate_request_against_graph(input, &graph, request)
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
        validate_request_against_graph(&self.input, &graph, request)?;

        let incident_roots = resolve_incident_roots(&mut graph, request)?;
        let traversal_roots = traversal_roots(request, &incident_roots);

        // Incident selection is resolved against the complete current graph so
        // that context paths can be identified before Environment/Team
        // intersection removes nodes. The final traversal is rerun below on
        // that reduced graph, ensuring no hidden endpoint can leak through.
        let incident_paths = if request.filter.incident_id.is_some()
            && request.traversal.max_depth > 0
            && !traversal_roots.is_empty()
        {
            traverse(
                &traversal_roots,
                &graph.nodes,
                &graph.edges,
                request.traversal,
            )?
        } else {
            Vec::new()
        };
        let selection = select_nodes(
            &graph.nodes,
            &request.filter,
            &incident_roots,
            &traversal_roots,
            &incident_paths,
        );
        let visible_node_ids = selection.visible_node_ids;
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
        if let Some(reason) = selection.empty_reason {
            source_status.push(empty_status(reason));
        }
        source_status.sort_by(|left, right| left.source_key.cmp(&right.source_key));
        let evidence = graph.evidence.into_values().collect::<Vec<_>>();

        let snapshot = TopologySnapshot {
            generated_at: fixture_or_input_timestamp(&self.input),
            scope: self.input.scope.clone(),
            filter: request.filter.clone(),
            // A focus node removed by an active filter cannot be serialized as
            // part of the filtered graph. Clearing it keeps the result valid
            // and, importantly, never widens the caller's selected scope.
            focus_node_id: request
                .focus_node_id
                .clone()
                .filter(|node_id| nodes.contains_key(node_id)),
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

fn validate_request_against_graph(
    input: &TopologyInput,
    graph: &DerivedGraph,
    request: &TopologyRequest,
) -> Result<(), TopologyError> {
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
    team_ids.extend(input.scope.team_id);
    let known_evidence = graph.evidence.keys().cloned().collect::<BTreeSet<_>>();
    let (admitted_rules, _, _) =
        validate_rules(&input.ownership_rules, &known_evidence, input.scope.team_id);
    team_ids.extend(admitted_rules.iter().map(|rule| rule.team_id));
    request.validate_against(&node_ids, &environment_ids, &team_ids, &graph.incident_ids)
}

fn fixture_or_input_timestamp(input: &TopologyInput) -> String {
    input
        .generated_at
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
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
    let evidence_ids = records
        .into_iter()
        .flat_map(|record| record.evidence_ids().to_vec())
        .filter(|id| evidence.contains_key(id))
        .collect::<BTreeSet<_>>();
    let evidence_ids = evidence_ids.into_iter().collect::<Vec<_>>();
    let drill_down = DrillDownTarget {
        destination: DrillDownDestination::Evidence,
        evidence_ids: evidence_ids.clone(),
        filter_key: None,
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
