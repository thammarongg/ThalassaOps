//! Iterative, deterministic upstream/downstream topology traversal.

use std::collections::{BTreeMap, BTreeSet};
use thalassa_domain::{
    DrillDownDestination, DrillDownTarget, TopologyDirection, TopologyEdge, TopologyError,
    TopologyNode, TopologyPath, TopologyPathKind, TopologyPathTermination, TopologyTraversal,
};

#[derive(Clone, Debug)]
struct WalkState {
    root_node_id: String,
    direction: TopologyDirection,
    node_ids: Vec<String>,
    edge_ids: Vec<String>,
    evidence_ids: BTreeSet<String>,
    visited: BTreeSet<String>,
    confidence: f64,
}

#[derive(Clone, Debug)]
struct Neighbor {
    edge_index: usize,
    node_id: String,
}

pub(crate) fn traverse(
    roots: &[String],
    nodes: &BTreeMap<String, TopologyNode>,
    edges: &[TopologyEdge],
    traversal: TopologyTraversal,
) -> Result<Vec<TopologyPath>, TopologyError> {
    traversal.validate()?;
    if traversal.max_depth == 0 {
        return Ok(Vec::new());
    }
    let node_ids = nodes.keys().cloned().collect::<BTreeSet<_>>();
    for edge in edges {
        edge.validate_against_nodes(&node_ids)?;
    }

    let mut paths = Vec::new();
    let mut sorted_roots = roots.to_vec();
    sorted_roots.sort();
    sorted_roots.dedup();

    match traversal.direction {
        TopologyDirection::Upstream | TopologyDirection::Downstream => {
            for root in sorted_roots {
                paths.extend(walk_direction(
                    &root,
                    traversal.direction,
                    traversal.max_depth,
                    nodes,
                    edges,
                ));
            }
        }
        TopologyDirection::Both => {
            for direction in [TopologyDirection::Upstream, TopologyDirection::Downstream] {
                for root in &sorted_roots {
                    paths.extend(walk_direction(
                        root,
                        direction,
                        traversal.max_depth,
                        nodes,
                        edges,
                    ));
                }
            }
        }
    }

    paths.sort_by(|left, right| {
        left.root_node_id
            .cmp(&right.root_node_id)
            .then_with(|| direction_key(left.direction).cmp(direction_key(right.direction)))
            .then_with(|| left.depth.cmp(&right.depth))
            .then_with(|| left.terminal_node_id.cmp(&right.terminal_node_id))
            .then_with(|| left.edge_ids.cmp(&right.edge_ids))
            .then_with(|| left.cycle_edge_id.cmp(&right.cycle_edge_id))
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(paths)
}

fn walk_direction(
    root: &str,
    direction: TopologyDirection,
    max_depth: u16,
    nodes: &BTreeMap<String, TopologyNode>,
    edges: &[TopologyEdge],
) -> Vec<TopologyPath> {
    if !nodes.contains_key(root) {
        return Vec::new();
    }

    let adjacency = build_adjacency(direction, edges, nodes);
    let root_evidence_ids = nodes
        .get(root)
        .map(|node| node.evidence_ids.iter().cloned().collect())
        .unwrap_or_default();
    let mut stack = vec![WalkState {
        root_node_id: root.into(),
        direction,
        node_ids: vec![root.into()],
        edge_ids: Vec::new(),
        evidence_ids: root_evidence_ids,
        visited: BTreeSet::from([root.into()]),
        confidence: 1.0,
    }];
    let mut paths = Vec::new();

    while let Some(state) = stack.pop() {
        let Some(current) = state.node_ids.last().map(String::as_str) else {
            continue;
        };
        let neighbors = match adjacency.get(current) {
            Some(neighbors) => neighbors.clone(),
            None => Vec::new(),
        };
        if neighbors.is_empty() {
            if !state.edge_ids.is_empty() {
                paths.push(path_from_state(
                    state,
                    TopologyPathTermination::Leaf,
                    None,
                    edges,
                ));
            }
            continue;
        }

        let depth = state.edge_ids.len() as u16;
        if depth >= max_depth {
            let mut emitted_cycle = false;
            let mut has_non_cycle_neighbor = false;
            for neighbor in &neighbors {
                if state.visited.contains(&neighbor.node_id) {
                    emitted_cycle = true;
                    let edge = &edges[neighbor.edge_index];
                    paths.push(path_from_state(
                        state.clone(),
                        TopologyPathTermination::CycleDetected,
                        Some(edge.id.clone()),
                        edges,
                    ));
                } else {
                    has_non_cycle_neighbor = true;
                }
            }
            if has_non_cycle_neighbor || !emitted_cycle {
                paths.push(path_from_state(
                    state,
                    TopologyPathTermination::DepthLimit,
                    None,
                    edges,
                ));
            }
            continue;
        }

        let mut pushed = false;
        for neighbor in neighbors.iter().rev() {
            let edge = &edges[neighbor.edge_index];
            if state.visited.contains(&neighbor.node_id) {
                paths.push(path_from_state(
                    state.clone(),
                    TopologyPathTermination::CycleDetected,
                    Some(edge.id.clone()),
                    edges,
                ));
                continue;
            }

            let mut next = state.clone();
            next.node_ids.push(neighbor.node_id.clone());
            next.edge_ids.push(edge.id.clone());
            next.visited.insert(neighbor.node_id.clone());
            next.evidence_ids.extend(edge.evidence_ids.iter().cloned());
            if let Some(node) = nodes.get(&neighbor.node_id) {
                next.evidence_ids.extend(node.evidence_ids.iter().cloned());
            }
            next.confidence = next.confidence.min(edge.confidence);
            stack.push(next);
            pushed = true;
        }

        if !pushed && !state.edge_ids.is_empty() {
            // All outgoing edges closed a cycle, and each closing edge has
            // already emitted its explicit cycle-terminated path.
        }
    }

    paths
}

fn build_adjacency(
    direction: TopologyDirection,
    edges: &[TopologyEdge],
    nodes: &BTreeMap<String, TopologyNode>,
) -> BTreeMap<String, Vec<Neighbor>> {
    let mut adjacency: BTreeMap<String, Vec<Neighbor>> = BTreeMap::new();
    for (edge_index, edge) in edges.iter().enumerate() {
        let (from, to) = match direction {
            TopologyDirection::Downstream => (&edge.upstream_node_id, &edge.downstream_node_id),
            TopologyDirection::Upstream => (&edge.downstream_node_id, &edge.upstream_node_id),
            TopologyDirection::Both => continue,
        };
        if nodes.contains_key(from) && nodes.contains_key(to) {
            adjacency.entry(from.clone()).or_default().push(Neighbor {
                edge_index,
                node_id: to.clone(),
            });
        }
    }
    for neighbors in adjacency.values_mut() {
        neighbors.sort_by(|left, right| {
            edges[left.edge_index]
                .id
                .cmp(&edges[right.edge_index].id)
                .then_with(|| left.node_id.cmp(&right.node_id))
        });
    }
    adjacency
}

fn path_from_state(
    state: WalkState,
    termination: TopologyPathTermination,
    cycle_edge_id: Option<String>,
    edges: &[TopologyEdge],
) -> TopologyPath {
    let mut evidence_ids = state.evidence_ids;
    if let Some(cycle_edge_id) = cycle_edge_id.as_ref() {
        if let Some(edge) = edges.iter().find(|edge| &edge.id == cycle_edge_id) {
            evidence_ids.extend(edge.evidence_ids.iter().cloned());
        }
    }
    let evidence_ids = evidence_ids.into_iter().collect::<Vec<_>>();
    let depth = state.edge_ids.len() as u16;
    let terminal_node_id = match state.node_ids.last() {
        Some(node_id) => node_id.clone(),
        None => String::new(),
    };
    let termination_key = termination_key(termination);
    let cycle_key = cycle_edge_id.as_deref().map_or("none", |edge_id| edge_id);
    let id = format!(
        "path:{}:{}:{}:{}:{}",
        direction_key(state.direction),
        state.root_node_id,
        state.edge_ids.join(","),
        termination_key,
        cycle_key
    );
    TopologyPath {
        id,
        root_node_id: state.root_node_id,
        terminal_node_id,
        node_ids: state.node_ids,
        edge_ids: state.edge_ids,
        direction: state.direction,
        depth,
        confidence: state.confidence,
        kind: TopologyPathKind::ProbableStructural,
        termination,
        cycle_edge_id,
        evidence_ids: evidence_ids.clone(),
        drill_down: DrillDownTarget {
            destination: DrillDownDestination::Evidence,
            evidence_ids,
            filter_key: None,
        },
    }
}

fn direction_key(direction: TopologyDirection) -> &'static str {
    match direction {
        TopologyDirection::Upstream => "upstream",
        TopologyDirection::Downstream => "downstream",
        TopologyDirection::Both => "both",
    }
}

fn termination_key(termination: TopologyPathTermination) -> &'static str {
    match termination {
        TopologyPathTermination::Leaf => "leaf",
        TopologyPathTermination::CycleDetected => "cycle_detected",
        TopologyPathTermination::DepthLimit => "depth_limit",
    }
}
