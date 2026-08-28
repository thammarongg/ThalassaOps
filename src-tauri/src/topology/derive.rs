//! Source adapters and graph construction for the topology projection.

use super::fixtures::TopologyInput;
use super::ownership::{resolve_ownership, validate_rules};
use crate::cloud::{CloudHealthState, CloudResource, CloudResourceType};
use crate::kubernetes::{KubernetesHealth, KubernetesResource};
use crate::observability::alertmanager::{NormalizedAlert, ResourceReference};
use crate::observability::masking::{mask_json_object, sensitive_key, REDACTED};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use thalassa_domain::{
    ConsoleHealthState, CriticalNumber, DrillDownDestination, DrillDownReference, DrillDownTarget,
    EvidenceRef, EvidenceSourceKind, MetricFixture, NumberUnit, Resource, ResourceId,
    ResourceScope, SourceState, SourceStatus, TopologyEdge, TopologyEdgeKind,
    TopologyEdgeProvenance, TopologyError, TopologyMetric, TopologyNode, TopologyNodeKind,
    TopologyOwnership, TopologyOwnershipSource, TopologySourceKind,
};

/// Intermediate graph used by the orchestration and traversal layers.
#[derive(Clone, Debug)]
pub(crate) struct DerivedGraph {
    pub(crate) nodes: BTreeMap<String, TopologyNode>,
    pub(crate) edges: Vec<TopologyEdge>,
    pub(crate) source_status: BTreeMap<String, SourceStatus>,
    pub(crate) evidence: BTreeMap<String, EvidenceRef>,
    pub(crate) incident_ids: BTreeSet<String>,
    pub(crate) incident_root_nodes: BTreeMap<String, Vec<String>>,
    pub(crate) incident_affected_resources: BTreeMap<String, Vec<ResourceId>>,
    incident_source_ids: BTreeMap<String, String>,
    pub(crate) resource_id_nodes: BTreeMap<ResourceId, String>,
    node_lookup: BTreeMap<(String, String, String), Vec<String>>,
    k8s_resources: BTreeMap<String, KubernetesResource>,
}

impl DerivedGraph {
    pub(crate) fn mark_unverified(&mut self, source_key: &str) {
        let source_key = safe_source_key(source_key);
        let entry = self
            .source_status
            .entry(source_key.clone())
            .or_insert_with(|| SourceStatus {
                source_key: safe_source_key(&source_key),
                state: SourceState::Unverified,
                reason: None,
                detail: Some("source record was omitted after validation".into()),
                observed_at: None,
                evidence_ids: Vec::new(),
            });
        entry.state = SourceState::Unverified;
        entry.reason = None;
        entry.detail = Some("source record was omitted after validation".into());
        entry
            .evidence_ids
            .retain(|evidence_id| self.evidence.contains_key(evidence_id));
    }

    fn source_evidence_ids(
        &self,
        source_kind: EvidenceSourceKind,
        hints: &[&str],
        fallback_source_key: Option<&str>,
    ) -> Vec<String> {
        let normalized_hints = hints
            .iter()
            .filter_map(|hint| normalized_hint(hint))
            .collect::<Vec<_>>();
        let mut matched = self
            .evidence
            .values()
            .filter(|evidence| evidence.source_kind == source_kind)
            .filter(|evidence| {
                !normalized_hints.is_empty()
                    && normalized_hints.iter().any(|hint| {
                        evidence.id.to_ascii_lowercase().contains(hint)
                            || evidence.excerpt.to_ascii_lowercase().contains(hint)
                            || evidence
                                .query
                                .as_deref()
                                .is_some_and(|query| query.to_ascii_lowercase().contains(hint))
                    })
            })
            .map(|evidence| evidence.id.clone())
            .collect::<BTreeSet<_>>();
        if matched.is_empty() {
            if let Some(source_key) = fallback_source_key {
                if let Some(status) = self.source_status.get(source_key) {
                    matched.extend(
                        status
                            .evidence_ids
                            .iter()
                            .filter(|evidence_id| self.evidence.contains_key(*evidence_id))
                            .cloned(),
                    );
                }
            }
        }
        if matched.is_empty() {
            matched.extend(
                self.evidence
                    .values()
                    .filter(|evidence| evidence.source_kind == source_kind)
                    .map(|evidence| evidence.id.clone()),
            );
        }
        matched.into_iter().collect()
    }

    fn add_node(&mut self, node: TopologyNode) {
        let node_id = node.id.clone();
        if let Some(existing) = self.nodes.get_mut(&node_id) {
            existing.evidence_ids.extend(node.evidence_ids);
            existing.evidence_ids.sort();
            existing.evidence_ids.dedup();
            for (key, value) in node.labels {
                existing.labels.entry(key).or_insert(value);
            }
            if existing.metric.is_none() {
                existing.metric = node.metric;
            }
            if health_rank(node.status) > health_rank(existing.status) {
                existing.status = node.status;
            }
            existing.drill_down.evidence_ids = existing.evidence_ids.clone();
        } else {
            self.nodes.insert(node_id, node);
        }
    }

    fn add_lookup(&mut self, environment_id: &str, kind: TopologyNodeKind, name: &str, id: &str) {
        let key = (
            environment_id.into(),
            topology_kind_name(kind).into(),
            name.into(),
        );
        let values = self.node_lookup.entry(key).or_default();
        if !values.iter().any(|candidate| candidate == id) {
            values.push(id.into());
            values.sort();
        }
    }

    fn add_edge(&mut self, edge: TopologyEdge) {
        if let Some(existing) = self
            .edges
            .iter_mut()
            .find(|candidate| candidate.id == edge.id)
        {
            existing.confidence = existing.confidence.min(edge.confidence);
            existing.evidence_ids.extend(edge.evidence_ids);
            existing.evidence_ids.sort();
            existing.evidence_ids.dedup();
            existing.provenance.extend(edge.provenance);
            existing.provenance.sort_by(provenance_order);
            existing.provenance.dedup_by(|left, right| left == right);
            for (key, value) in edge.metadata {
                existing.metadata.entry(key).or_insert(value);
            }
            existing.drill_down.evidence_ids = existing.evidence_ids.clone();
        } else {
            self.edges.push(edge);
            self.edges.sort_by(|left, right| left.id.cmp(&right.id));
        }
    }
}

pub(crate) fn derive_graph(input: &TopologyInput) -> DerivedGraph {
    let mut graph = DerivedGraph {
        nodes: BTreeMap::new(),
        edges: Vec::new(),
        source_status: BTreeMap::new(),
        evidence: BTreeMap::new(),
        incident_ids: BTreeSet::new(),
        incident_root_nodes: BTreeMap::new(),
        incident_affected_resources: BTreeMap::new(),
        incident_source_ids: BTreeMap::new(),
        resource_id_nodes: BTreeMap::new(),
        node_lookup: BTreeMap::new(),
        k8s_resources: BTreeMap::new(),
    };

    admit_evidence(input, &mut graph);
    load_source_status(input, &mut graph);
    load_incident_queue(input, &mut graph);
    derive_environments(input, &mut graph);
    derive_kubernetes(input, &mut graph);
    derive_cloud(input, &mut graph);
    derive_fixture_edges(input, &mut graph);
    derive_observability(input, &mut graph);
    resolve_node_ownership(input, &mut graph);

    graph.edges.sort_by(|left, right| left.id.cmp(&right.id));
    graph
}

fn load_incident_queue(input: &TopologyInput, graph: &mut DerivedGraph) {
    let mut queue = input.incident_queue.clone();
    queue.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| left.source_id.cmp(&right.source_id))
    });
    for item in queue {
        let valid = safe_identifier(&item.id)
            && input.scope.contains(&item.scope)
            && input.scope.contains(&item.affected_scope)
            && !item.evidence_ids.is_empty()
            && item
                .evidence_ids
                .iter()
                .all(|evidence_id| graph.evidence.contains_key(evidence_id));
        if !valid || graph.incident_ids.contains(&item.id) {
            graph.mark_unverified("incidents");
            continue;
        }
        graph.incident_ids.insert(item.id.clone());
        graph
            .incident_affected_resources
            .insert(item.id.clone(), item.affected_scope.resource_ids.clone());
        graph
            .incident_source_ids
            .insert(item.id.clone(), item.source_id.clone());
        graph.incident_root_nodes.insert(
            item.id.clone(),
            input
                .incident_root_nodes
                .get(&item.id)
                .cloned()
                .unwrap_or_default(),
        );
    }
}

fn admit_evidence(input: &TopologyInput, graph: &mut DerivedGraph) {
    let mut source_evidence = input.evidence.clone();
    source_evidence.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| left.endpoint.cmp(&right.endpoint))
            .then_with(|| left.excerpt.cmp(&right.excerpt))
    });
    for evidence in source_evidence {
        let source_key = evidence_source_status_key(evidence.source_kind);
        if evidence.id.trim().is_empty()
            || !safe_identifier(&evidence.id)
            || !evidence.redaction.classification_verified
            || !evidence.redaction.redaction_verified
            || !input.scope.contains(&evidence.scope)
        {
            graph.mark_unverified(source_key);
            continue;
        }
        let Some(sanitized) = sanitize_evidence(evidence) else {
            graph.mark_unverified(source_key);
            continue;
        };
        graph
            .evidence
            .entry(sanitized.id.clone())
            .or_insert(sanitized);
    }
}

fn load_source_status(input: &TopologyInput, graph: &mut DerivedGraph) {
    let mut statuses = input.source_status.clone();
    statuses.sort_by(|left, right| left.source_key.cmp(&right.source_key));
    for mut status in statuses {
        let source_key = safe_source_key(&status.source_key);
        if source_key != status.source_key {
            status.source_key = source_key.clone();
        }
        status.detail = status.detail.as_deref().map(sanitize_text);
        status.observed_at = sanitize_optional_text(status.observed_at.as_deref());
        let original_evidence_count = status.evidence_ids.len();
        status.evidence_ids.retain(|evidence_id| {
            graph.evidence.contains_key(evidence_id) && safe_identifier(evidence_id)
        });
        if status.evidence_ids.len() != original_evidence_count {
            status.state = SourceState::Unverified;
            status.reason = None;
            status.detail = Some("source record was omitted after validation".into());
        }
        status.evidence_ids.sort();
        status.evidence_ids.dedup();
        merge_source_status(graph, status);
    }
}

fn merge_source_status(graph: &mut DerivedGraph, status: SourceStatus) {
    let key = status.source_key.clone();
    if let Some(existing) = graph.source_status.get_mut(&key) {
        let should_replace = source_state_rank(status.state) > source_state_rank(existing.state)
            || (source_state_rank(status.state) == source_state_rank(existing.state)
                && status_preference(&status) < status_preference(existing));
        if should_replace {
            existing.state = status.state;
            existing.reason = status.reason;
            existing.detail = status.detail;
            existing.observed_at = status.observed_at;
        }
        existing.evidence_ids.extend(status.evidence_ids);
        existing.evidence_ids.sort();
        existing.evidence_ids.dedup();
    } else {
        graph.source_status.insert(key, status);
    }
}

fn status_preference(status: &SourceStatus) -> (String, String, Vec<String>) {
    let detail = match status.detail.as_ref() {
        Some(detail) => detail.clone(),
        None => String::new(),
    };
    let observed_at = match status.observed_at.as_ref() {
        Some(observed_at) => observed_at.clone(),
        None => String::new(),
    };
    (detail, observed_at, status.evidence_ids.clone())
}

fn derive_environments(input: &TopologyInput, graph: &mut DerivedGraph) {
    let mut environments = input.environments.clone();
    environments.sort_by(|left, right| left.environment_id.cmp(&right.environment_id));
    for environment in environments {
        if !safe_identifier(&environment.environment_id) || environment.name.trim().is_empty() {
            graph.mark_unverified("cloud");
            continue;
        }
        let mut evidence_ids = admitted_ids(&environment.evidence_ids, graph);
        if evidence_ids.is_empty() {
            evidence_ids = graph.source_evidence_ids(
                EvidenceSourceKind::Cloud,
                &[&environment.environment_id, &environment.name],
                Some("cloud"),
            );
        }
        if evidence_ids.is_empty() {
            graph.mark_unverified("cloud");
            continue;
        }
        let metric = topology_metric_from_critical_number(
            &environment.resource_count,
            &evidence_ids,
            input.scope.clone(),
            format!("environment:{}", environment.environment_id),
            graph,
            "cloud",
        );
        let node_id = format!(
            "node:fixture:{}:environment:{}",
            environment.environment_id, environment.environment_id
        );
        let node = TopologyNode {
            id: node_id.clone(),
            kind: TopologyNodeKind::Environment,
            name: sanitize_text(&environment.name),
            native_kind: Some("EnvironmentStatus".into()),
            native_id: Some(environment.environment_id.clone()),
            environment_id: Some(environment.environment_id.clone()),
            provider: environment.provider.as_deref().map(sanitize_text),
            scope: input.scope.clone(),
            status: environment.health,
            labels: BTreeMap::new(),
            ownership: unassigned_ownership(),
            metric,
            affected_by_incident: false,
            evidence_ids: evidence_ids.clone(),
            drill_down: topology_drill_down(evidence_ids, Some(&node_id)),
        };
        graph.add_node(node);
        graph.add_lookup(
            &environment.environment_id,
            TopologyNodeKind::Environment,
            &environment.environment_id,
            &node_id,
        );
    }
}

fn derive_kubernetes(input: &TopologyInput, graph: &mut DerivedGraph) {
    for (environment_id, inventory) in &input.kubernetes {
        if !safe_identifier(environment_id) {
            graph.mark_unverified("kubernetes");
            continue;
        }
        if !has_environment_node(graph, environment_id) {
            // An inventory key is not itself a scope grant. Require the
            // environment projection admitted for this workspace before
            // exposing any resources under that key.
            graph.mark_unverified(&format!("kubernetes:{environment_id}"));
            continue;
        }
        let environment_node_id = ensure_environment_node(input, graph, environment_id);
        let mut resources = inventory.resources.clone();
        resources.sort_by(|left, right| resource_order(&left.resource, &right.resource));
        for resource in resources {
            derive_kubernetes_resource(input, graph, environment_id, resource);
        }
        derive_kubernetes_containment(input, graph, environment_id, &environment_node_id);
        derive_kubernetes_edges(input, graph, environment_id, inventory);
    }
}

fn ensure_environment_node(
    input: &TopologyInput,
    graph: &mut DerivedGraph,
    environment_id: &str,
) -> String {
    if let Some(node_id) = graph
        .nodes
        .values()
        .find(|node| {
            node.kind == TopologyNodeKind::Environment
                && node.environment_id.as_deref() == Some(environment_id)
        })
        .map(|node| node.id.clone())
    {
        return node_id;
    }
    let evidence_ids = graph.source_evidence_ids(
        EvidenceSourceKind::Kubernetes,
        &[environment_id],
        Some(&format!("kubernetes:{environment_id}")),
    );
    let node_id = format!("node:kubernetes:{environment_id}:environment:{environment_id}");
    if evidence_ids.is_empty() {
        graph.mark_unverified(&format!("kubernetes:{environment_id}"));
        return node_id;
    }
    graph.add_node(TopologyNode {
        id: node_id.clone(),
        kind: TopologyNodeKind::Environment,
        name: sanitize_text(environment_id),
        native_kind: Some("KubernetesEnvironment".into()),
        native_id: Some(environment_id.into()),
        environment_id: Some(environment_id.into()),
        provider: Some("kubernetes".into()),
        scope: input.scope.clone(),
        status: ConsoleHealthState::Unknown,
        labels: BTreeMap::new(),
        ownership: unassigned_ownership(),
        metric: None,
        affected_by_incident: false,
        evidence_ids: evidence_ids.clone(),
        drill_down: topology_drill_down(evidence_ids, Some(&node_id)),
    });
    graph.add_lookup(
        environment_id,
        TopologyNodeKind::Environment,
        environment_id,
        &node_id,
    );
    node_id
}

fn has_environment_node(graph: &DerivedGraph, environment_id: &str) -> bool {
    graph.nodes.values().any(|node| {
        node.kind == TopologyNodeKind::Environment
            && node.environment_id.as_deref() == Some(environment_id)
    })
}

fn derive_kubernetes_resource(
    input: &TopologyInput,
    graph: &mut DerivedGraph,
    environment_id: &str,
    item: KubernetesResource,
) {
    let Some(kind) = map_kubernetes_kind(&item.resource.kind) else {
        graph.mark_unverified(&format!("kubernetes:{environment_id}"));
        return;
    };
    if !input.scope.contains(&item.resource.scope)
        || item.resource.name.trim().is_empty()
        || item
            .resource
            .native_id
            .as_deref()
            .is_some_and(|native_id| !safe_identifier(native_id))
    {
        graph.mark_unverified(&format!("kubernetes:{environment_id}"));
        return;
    }
    let (namespace, canonical_name) = canonical_resource_name(&item.resource.name);
    if !safe_identifier(canonical_name) || namespace.is_some_and(|value| !safe_identifier(value)) {
        graph.mark_unverified(&format!("kubernetes:{environment_id}"));
        return;
    }
    let identity = match item
        .resource
        .native_id
        .as_deref()
        .filter(|native_id| safe_identifier(native_id))
    {
        Some(native_id) => native_id,
        None => canonical_name,
    };
    let node_id = format!(
        "node:kubernetes:{environment_id}:{}:{identity}",
        topology_kind_name(kind)
    );
    let mut evidence_ids = graph.source_evidence_ids(
        EvidenceSourceKind::Kubernetes,
        &[canonical_name, &item.resource.name],
        Some(&format!("kubernetes:{environment_id}")),
    );
    if evidence_ids.is_empty() {
        graph.mark_unverified(&format!("kubernetes:{environment_id}"));
        return;
    }
    evidence_ids.sort();
    let metric = item.replicas.as_ref().and_then(|replicas| {
        let value = f64::from(replicas.ready);
        if !value.is_finite() {
            graph.mark_unverified(&format!("kubernetes:{environment_id}"));
            None
        } else {
            Some(topology_metric(
                format!("ready_replicas:{canonical_name}"),
                value,
                NumberUnit::Count,
                evidence_ids.clone(),
                input.scope.clone(),
                format!("kubernetes:{environment_id}:{canonical_name}"),
            ))
        }
    });
    let labels = sanitize_labels(&item.resource.labels);
    let status = map_kubernetes_health(item.health.clone());
    let node = TopologyNode {
        id: node_id.clone(),
        kind,
        name: canonical_name.into(),
        native_kind: Some(item.resource.kind.clone()),
        native_id: item.resource.native_id.clone(),
        environment_id: Some(environment_id.into()),
        provider: item.resource.provider.as_deref().map(sanitize_text),
        scope: item.resource.scope.clone(),
        status,
        labels,
        ownership: unassigned_ownership(),
        metric,
        affected_by_incident: false,
        evidence_ids: evidence_ids.clone(),
        drill_down: topology_drill_down(evidence_ids, Some(&node_id)),
    };
    graph.add_node(node);
    graph.add_lookup(
        environment_id,
        kind,
        &resource_lookup_name(namespace, canonical_name),
        &node_id,
    );
    graph.add_lookup(environment_id, kind, canonical_name, &node_id);
    graph
        .resource_id_nodes
        .insert(item.resource.id, node_id.clone());
    graph.k8s_resources.insert(node_id, item);
}

fn derive_kubernetes_containment(
    _input: &TopologyInput,
    graph: &mut DerivedGraph,
    environment_id: &str,
    environment_node_id: &str,
) {
    let Some(environment_node) = graph.nodes.get(environment_node_id).cloned() else {
        return;
    };
    let namespace_ids = graph
        .nodes
        .values()
        .filter(|node| {
            node.kind == TopologyNodeKind::Namespace
                && node.environment_id.as_deref() == Some(environment_id)
        })
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    for namespace_id in namespace_ids {
        let Some(namespace_node) = graph.nodes.get(&namespace_id).cloned() else {
            continue;
        };
        let evidence_ids =
            union_evidence(&environment_node.evidence_ids, &namespace_node.evidence_ids);
        let edge = make_edge(
            environment_node_id,
            &namespace_id,
            TopologyEdgeKind::Contains,
            TopologySourceKind::Kubernetes,
            &format!("kubernetes:{environment_id}"),
            source_observed_at(graph, &format!("kubernetes:{environment_id}")),
            1.0,
            BTreeMap::from([("relationship".into(), "contains".into())]),
            evidence_ids,
        );
        if let Some(edge) = edge {
            graph.add_edge(edge);
        }
    }

    let namespace_nodes = graph
        .nodes
        .values()
        .filter(|node| {
            node.kind == TopologyNodeKind::Namespace
                && node.environment_id.as_deref() == Some(environment_id)
        })
        .cloned()
        .collect::<Vec<_>>();
    let resource_nodes = graph
        .nodes
        .values()
        .filter(|node| {
            node.environment_id.as_deref() == Some(environment_id)
                && node.kind != TopologyNodeKind::Namespace
                && node.kind != TopologyNodeKind::Environment
                && node.provider.as_deref() == Some("kubernetes")
        })
        .cloned()
        .collect::<Vec<_>>();
    for node in resource_nodes {
        let Some(resource) = graph.k8s_resources.get(&node.id) else {
            continue;
        };
        let (namespace, _) = canonical_resource_name(&resource.resource.name);
        let Some(namespace) = namespace else {
            continue;
        };
        let Some(namespace_node) = namespace_nodes
            .iter()
            .find(|candidate| candidate.name == namespace)
        else {
            continue;
        };
        let evidence_ids = union_evidence(&namespace_node.evidence_ids, &node.evidence_ids);
        let Some(edge) = make_edge(
            &namespace_node.id,
            &node.id,
            TopologyEdgeKind::Contains,
            TopologySourceKind::Kubernetes,
            &format!("kubernetes:{environment_id}"),
            source_observed_at(graph, &format!("kubernetes:{environment_id}")),
            1.0,
            BTreeMap::from([("relationship".into(), "contains".into())]),
            evidence_ids,
        ) else {
            continue;
        };
        graph.add_edge(edge);
    }
}

fn derive_kubernetes_edges(
    _input: &TopologyInput,
    graph: &mut DerivedGraph,
    environment_id: &str,
    inventory: &crate::kubernetes::KubernetesInventory,
) {
    let mut source_edges = inventory.topology.clone();
    source_edges.sort_by(|left, right| {
        left.from_kind
            .cmp(&right.from_kind)
            .then_with(|| left.from_name.cmp(&right.from_name))
            .then_with(|| left.to_kind.cmp(&right.to_kind))
            .then_with(|| left.to_name.cmp(&right.to_name))
            .then_with(|| left.relationship.cmp(&right.relationship))
    });
    for source_edge in source_edges {
        let Some(kind) = map_relationship(&source_edge.relationship) else {
            graph.mark_unverified(&format!("kubernetes:{environment_id}"));
            continue;
        };
        let Some(upstream_node_id) = resolve_kubernetes_endpoint(
            graph,
            environment_id,
            &source_edge.from_kind,
            &source_edge.from_name,
        ) else {
            graph.mark_unverified(&format!("kubernetes:{environment_id}"));
            continue;
        };
        let Some(downstream_node_id) = resolve_kubernetes_endpoint(
            graph,
            environment_id,
            &source_edge.to_kind,
            &source_edge.to_name,
        ) else {
            graph.mark_unverified(&format!("kubernetes:{environment_id}"));
            continue;
        };
        let Some(upstream_node) = graph.nodes.get(&upstream_node_id) else {
            graph.mark_unverified(&format!("kubernetes:{environment_id}"));
            continue;
        };
        let Some(downstream_node) = graph.nodes.get(&downstream_node_id) else {
            graph.mark_unverified(&format!("kubernetes:{environment_id}"));
            continue;
        };
        let evidence_ids =
            union_evidence(&upstream_node.evidence_ids, &downstream_node.evidence_ids);
        let confidence = if kind == TopologyEdgeKind::Owns
            && owner_uid_matches(graph, &upstream_node_id, &downstream_node_id)
        {
            1.0
        } else {
            0.9
        };
        let Some(edge) = make_edge(
            &upstream_node_id,
            &downstream_node_id,
            kind,
            TopologySourceKind::Kubernetes,
            &format!("kubernetes:{environment_id}"),
            source_observed_at(graph, &format!("kubernetes:{environment_id}")),
            confidence,
            BTreeMap::from([("relationship".into(), source_edge.relationship)]),
            evidence_ids,
        ) else {
            graph.mark_unverified(&format!("kubernetes:{environment_id}"));
            continue;
        };
        graph.add_edge(edge);
    }
}

fn owner_uid_matches(
    graph: &DerivedGraph,
    upstream_node_id: &str,
    downstream_node_id: &str,
) -> bool {
    let Some(upstream) = graph.k8s_resources.get(upstream_node_id) else {
        return false;
    };
    let Some(downstream) = graph.k8s_resources.get(downstream_node_id) else {
        return false;
    };
    let Some(owner) = downstream.owner.as_ref() else {
        return false;
    };
    owner.uid.as_deref() == upstream.resource.native_id.as_deref()
}

fn resolve_kubernetes_endpoint(
    graph: &DerivedGraph,
    environment_id: &str,
    kind: &str,
    name: &str,
) -> Option<String> {
    let kind = map_kubernetes_kind(kind)?;
    let (_, canonical_name) = canonical_resource_name(name);
    let mut candidates = match graph.node_lookup.get(&(
        environment_id.into(),
        topology_kind_name(kind).into(),
        name.into(),
    )) {
        Some(values) => values.clone(),
        None => Vec::new(),
    };
    if candidates.is_empty() {
        candidates = match graph.node_lookup.get(&(
            environment_id.into(),
            topology_kind_name(kind).into(),
            canonical_name.into(),
        )) {
            Some(values) => values.clone(),
            None => Vec::new(),
        };
    }
    candidates.sort();
    candidates.dedup();
    if candidates.len() == 1 {
        candidates.into_iter().next()
    } else {
        None
    }
}

fn derive_cloud(input: &TopologyInput, graph: &mut DerivedGraph) {
    let mut resources = input.cloud_resources.clone();
    resources.sort_by(|left, right| {
        left.environment_id
            .cmp(&right.environment_id)
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| left.name.cmp(&right.name))
    });
    for resource in resources {
        derive_cloud_resource(input, graph, resource);
    }
}

fn derive_cloud_resource(input: &TopologyInput, graph: &mut DerivedGraph, resource: CloudResource) {
    if !safe_identifier(&resource.environment_id)
        || resource.id.trim().is_empty()
        || !safe_identifier(&resource.id)
        || resource.name.trim().is_empty()
        || !safe_identifier(&resource.name)
    {
        graph.mark_unverified("cloud");
        return;
    }
    if !has_environment_node(graph, &resource.environment_id) {
        // CloudResource has no independent ResourceScope field. Its
        // environment projection is therefore the scope anchor; an unknown
        // environment must never become an orphan node that a filter can
        // surface.
        graph.mark_unverified("cloud");
        return;
    }
    let kind = match resource.resource_type {
        CloudResourceType::KubernetesCluster => TopologyNodeKind::Cluster,
        CloudResourceType::ComputeInstance => TopologyNodeKind::CloudResource,
    };
    let mut evidence_ids = graph.source_evidence_ids(
        EvidenceSourceKind::Cloud,
        &[&resource.id, &resource.name, &resource.environment_id],
        Some("cloud"),
    );
    if evidence_ids.is_empty() {
        graph.mark_unverified("cloud");
        return;
    }
    evidence_ids.sort();
    let node_id = format!(
        "node:cloud:{}:{}:{}",
        resource.environment_id,
        topology_kind_name(kind),
        resource.id
    );
    let mut labels = BTreeMap::new();
    labels.insert("location".into(), sanitize_text(&resource.location));
    labels.insert(
        "resource_type".into(),
        cloud_resource_type_name(resource.resource_type).into(),
    );
    labels.insert("status".into(), cloud_health_name(resource.health).into());
    let node = TopologyNode {
        id: node_id.clone(),
        kind,
        name: sanitize_text(&resource.name),
        native_kind: Some(cloud_resource_type_name(resource.resource_type).into()),
        native_id: Some(resource.id.clone()),
        environment_id: Some(resource.environment_id.clone()),
        provider: Some(cloud_provider_name(resource.provider).into()),
        scope: input.scope.clone(),
        status: map_cloud_health(resource.health),
        labels,
        ownership: unassigned_ownership(),
        metric: None,
        affected_by_incident: false,
        evidence_ids: evidence_ids.clone(),
        drill_down: topology_drill_down(evidence_ids, Some(&node_id)),
    };
    graph.add_node(node);
    graph.add_lookup(&resource.environment_id, kind, &resource.name, &node_id);
    if let Some(environment_node) = graph.nodes.values().find(|node| {
        node.kind == TopologyNodeKind::Environment
            && node.environment_id.as_deref() == Some(resource.environment_id.as_str())
    }) {
        let evidence_ids = union_evidence(
            &environment_node.evidence_ids,
            &evidence_ids_for_node(graph, &node_id),
        );
        if let Some(edge) = make_edge(
            &environment_node.id,
            &node_id,
            TopologyEdgeKind::Contains,
            TopologySourceKind::Cloud,
            "cloud",
            source_observed_at(graph, "cloud"),
            1.0,
            BTreeMap::from([("relationship".into(), "contains".into())]),
            evidence_ids,
        ) {
            graph.add_edge(edge);
        }
    } else {
        graph.mark_unverified("cloud");
    }
}

fn derive_fixture_edges(input: &TopologyInput, graph: &mut DerivedGraph) {
    let mut fixture_edges = input.fixture_edges.clone();
    fixture_edges.sort_by(|left, right| left.id.cmp(&right.id));
    for edge in fixture_edges {
        if edge.validate().is_err()
            || !graph.nodes.contains_key(&edge.upstream_node_id)
            || !graph.nodes.contains_key(&edge.downstream_node_id)
            || edge
                .provenance
                .iter()
                .any(|provenance| !safe_identifier(&provenance.source_key))
            || edge
                .evidence_ids
                .iter()
                .any(|evidence_id| !graph.evidence.contains_key(evidence_id))
        {
            graph.mark_unverified("fixtures");
            continue;
        }
        let mut edge = edge;
        edge.metadata = sanitize_labels(&edge.metadata);
        for provenance in &mut edge.provenance {
            provenance.source_key = sanitize_text(&provenance.source_key);
            provenance.observed_at = provenance.observed_at.as_deref().map(sanitize_text);
        }
        edge.provenance.sort_by(provenance_order);
        edge.provenance.dedup_by(|left, right| left == right);
        edge.evidence_ids.sort();
        edge.evidence_ids.dedup();
        edge.drill_down = topology_drill_down(edge.evidence_ids.clone(), Some(&edge.id));
        if edge.drill_down.evidence_ids.is_empty() {
            graph.mark_unverified("fixtures");
            continue;
        }
        graph.add_edge(edge);
    }
}

fn derive_observability(input: &TopologyInput, graph: &mut DerivedGraph) {
    let mut alerts = input.alerts.clone();
    alerts.sort_by(|left, right| left.fingerprint.cmp(&right.fingerprint));
    for alert in alerts {
        attach_alert(graph, &alert);
    }
    let mut metrics = input.metrics.clone();
    metrics.sort_by(|left, right| left.key.cmp(&right.key));
    for metric in metrics {
        if !input.scope.contains(&metric.scope) {
            graph.mark_unverified("observability");
            continue;
        }
        attach_metric(graph, &metric);
    }
}

fn attach_alert(graph: &mut DerivedGraph, alert: &NormalizedAlert) {
    let (namespace, kind, name) = match &alert.resource_reference {
        ResourceReference::Resolved {
            namespace,
            kind,
            name,
        } => (namespace.as_str(), kind.as_str(), name.as_str()),
        ResourceReference::Unresolved { .. } => {
            graph.mark_unverified("observability");
            return;
        }
    };
    let environment_hint = alert.labels.get("environment").map(String::as_str);
    let candidates = find_observability_candidates(graph, environment_hint, namespace, kind, name);
    if candidates.len() != 1 {
        graph.mark_unverified("observability");
        return;
    }
    let evidence_ids = graph.source_evidence_ids(
        EvidenceSourceKind::Alertmanager,
        &[&alert.fingerprint, name],
        Some("observability"),
    );
    if evidence_ids.is_empty() {
        graph.mark_unverified("observability");
        return;
    }
    let node_id = candidates[0].clone();
    attach_evidence_to_node(graph, &node_id, evidence_ids);
    bind_incident_source(graph, &alert.fingerprint, &node_id);
}

fn attach_metric(graph: &mut DerivedGraph, metric: &MetricFixture) {
    let environment_hint = metric.labels.get("environment").map(String::as_str);
    let target = metric_target(&metric.labels);
    let Some((kind, name)) = target else {
        graph.mark_unverified("observability");
        return;
    };
    let namespace = match metric.labels.get("namespace") {
        Some(namespace) => namespace.as_str(),
        None => "",
    };
    let candidates = find_observability_candidates(graph, environment_hint, namespace, kind, name);
    if candidates.len() != 1 {
        graph.mark_unverified("observability");
        return;
    }
    let evidence_ids = graph.source_evidence_ids(
        EvidenceSourceKind::Prometheus,
        &[&metric.key, &metric.source.query, name],
        Some("observability"),
    );
    if evidence_ids.is_empty() {
        graph.mark_unverified("observability");
        return;
    }
    let node_id = candidates[0].clone();
    attach_evidence_to_node(graph, &node_id, evidence_ids);
    bind_incident_source(graph, &metric.key, &node_id);
}

fn bind_incident_source(graph: &mut DerivedGraph, source_id: &str, node_id: &str) {
    let incident_ids = graph
        .incident_source_ids
        .iter()
        .filter(|(_, candidate_source_id)| candidate_source_id.as_str() == source_id)
        .map(|(incident_id, _)| incident_id.clone())
        .collect::<Vec<_>>();
    for incident_id in incident_ids {
        let roots = graph.incident_root_nodes.entry(incident_id).or_default();
        if !roots.iter().any(|candidate| candidate == node_id) {
            roots.push(node_id.into());
            roots.sort();
        }
    }
}

fn metric_target(labels: &BTreeMap<String, String>) -> Option<(&str, &str)> {
    let targets = [
        ("service", "Service"),
        ("deployment", "Deployment"),
        ("statefulset", "StatefulSet"),
        ("daemonset", "DaemonSet"),
        ("workload", "Deployment"),
        ("pod", "Pod"),
    ];
    let matches = targets
        .iter()
        .filter_map(|(label, kind)| labels.get(*label).map(|name| (*kind, name.as_str())))
        .collect::<Vec<_>>();
    if matches.len() == 1 {
        matches.into_iter().next()
    } else if matches.is_empty() {
        labels.get("app").map(|name| ("Service", name.as_str()))
    } else {
        None
    }
}

fn find_observability_candidates(
    graph: &DerivedGraph,
    environment_hint: Option<&str>,
    namespace: &str,
    kind: &str,
    name: &str,
) -> Vec<String> {
    let Some(kind) = map_kubernetes_kind(kind) else {
        return Vec::new();
    };
    let (_, canonical_name) = canonical_resource_name(name);
    let mut candidates = Vec::new();
    for ((environment_id, candidate_kind, candidate_name), node_ids) in &graph.node_lookup {
        if candidate_kind != topology_kind_name(kind) {
            continue;
        }
        if candidate_name != canonical_name && candidate_name != name {
            continue;
        }
        let namespace_matches = if namespace.trim().is_empty() {
            true
        } else {
            let namespaced_name = format!("{namespace}/{canonical_name}");
            candidate_name == &namespaced_name
                || (candidate_name == canonical_name
                    && node_ids.iter().any(|node_id| {
                        graph
                            .k8s_resources
                            .get(node_id)
                            .is_some_and(|resource| !resource.resource.name.contains('/'))
                    }))
        };
        if !namespace_matches {
            continue;
        }
        if let Some(environment_hint) = environment_hint {
            let environment_node = graph.nodes.values().find(|node| {
                node.kind == TopologyNodeKind::Environment
                    && node.environment_id.as_deref() == Some(environment_id.as_str())
            });
            let hint = environment_hint.to_ascii_lowercase();
            let matches_environment = environment_id.eq_ignore_ascii_case(environment_hint)
                || environment_node.is_some_and(|node| {
                    node.name.to_ascii_lowercase().contains(&hint)
                        || node
                            .environment_id
                            .as_deref()
                            .is_some_and(|value| value.eq_ignore_ascii_case(environment_hint))
                });
            if matches_environment {
                candidates.extend(node_ids.iter().cloned());
            }
        } else {
            candidates.extend(node_ids.iter().cloned());
        }
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

fn attach_evidence_to_node(graph: &mut DerivedGraph, node_id: &str, evidence_ids: Vec<String>) {
    let Some(node) = graph.nodes.get_mut(node_id) else {
        graph.mark_unverified("observability");
        return;
    };
    node.evidence_ids.extend(evidence_ids);
    node.evidence_ids.sort();
    node.evidence_ids.dedup();
    node.drill_down.evidence_ids = node.evidence_ids.clone();
}

fn resolve_node_ownership(input: &TopologyInput, graph: &mut DerivedGraph) {
    let known_evidence = graph.evidence.keys().cloned().collect::<BTreeSet<_>>();
    let (ownership_rules, rejected_selectors, invalid_rules) =
        validate_rules(&input.ownership_rules, &known_evidence, input.scope.team_id);
    if invalid_rules {
        graph.mark_unverified("ownership");
    }
    let mut node_ids = graph.nodes.keys().cloned().collect::<Vec<_>>();
    node_ids.sort();
    for node_id in node_ids {
        let Some(node) = graph.nodes.get(&node_id).cloned() else {
            continue;
        };
        let ownership = match resolve_ownership(&node, &ownership_rules, &rejected_selectors) {
            Ok(ownership) => ownership,
            Err(TopologyError::EvidenceMissing) => {
                mark_ownership_issue(graph, "ownership_evidence_missing");
                unassigned_ownership()
            }
            Err(TopologyError::MalformedSource) => {
                mark_ownership_issue(graph, "ambiguous_ownership");
                unassigned_ownership()
            }
            Err(_) => unassigned_ownership(),
        };
        if let Some(node) = graph.nodes.get_mut(&node_id) {
            node.ownership = ownership;
        }
    }
}

fn mark_ownership_issue(graph: &mut DerivedGraph, detail: &str) {
    graph.mark_unverified("ownership");
    if let Some(status) = graph.source_status.get_mut("ownership") {
        status.detail = Some(detail.into());
    }
}

#[allow(clippy::too_many_arguments)]
fn make_edge(
    upstream_node_id: &str,
    downstream_node_id: &str,
    kind: TopologyEdgeKind,
    source: TopologySourceKind,
    source_key: &str,
    observed_at: Option<String>,
    confidence: f64,
    metadata: BTreeMap<String, String>,
    evidence_ids: Vec<String>,
) -> Option<TopologyEdge> {
    if upstream_node_id == downstream_node_id
        || !safe_identifier(upstream_node_id)
        || !safe_identifier(downstream_node_id)
        || !safe_identifier(source_key)
        || !confidence.is_finite()
        || !(0.0..=1.0).contains(&confidence)
        || evidence_ids.is_empty()
    {
        return None;
    }
    let kind_key = topology_edge_kind_name(kind);
    let id = format!("edge:{kind_key}:{upstream_node_id}:{downstream_node_id}:{source_key}");
    let mut evidence_ids = evidence_ids;
    evidence_ids.sort();
    evidence_ids.dedup();
    let provenance = vec![TopologyEdgeProvenance {
        source,
        source_key: source_key.into(),
        observed_at: observed_at.filter(|value| !value.trim().is_empty()),
    }];
    Some(TopologyEdge {
        id: id.clone(),
        upstream_node_id: upstream_node_id.into(),
        downstream_node_id: downstream_node_id.into(),
        kind,
        provenance,
        confidence,
        metadata: sanitize_labels(&metadata),
        evidence_ids: evidence_ids.clone(),
        drill_down: topology_drill_down(evidence_ids, Some(&id)),
    })
}

fn topology_metric_from_critical_number(
    number: &CriticalNumber,
    fallback_evidence_ids: &[String],
    scope: ResourceScope,
    source_query: String,
    graph: &mut DerivedGraph,
    source_key: &str,
) -> Option<TopologyMetric> {
    let value = match number.value.trim().parse::<f64>() {
        Ok(value) if value.is_finite() => value,
        _ => {
            graph.mark_unverified(source_key);
            return None;
        }
    };
    let evidence_ids = admitted_ids(&number.evidence_ids, graph);
    let evidence_ids = if evidence_ids.is_empty() {
        fallback_evidence_ids.to_vec()
    } else {
        evidence_ids
    };
    if evidence_ids.is_empty() {
        graph.mark_unverified(source_key);
        None
    } else {
        Some(topology_metric(
            number.key.clone(),
            value,
            number.unit,
            evidence_ids,
            scope,
            source_query,
        ))
    }
}

fn topology_metric(
    key: String,
    value: f64,
    unit: NumberUnit,
    evidence_ids: Vec<String>,
    scope: ResourceScope,
    source_query: String,
) -> TopologyMetric {
    let mut evidence_ids = evidence_ids;
    evidence_ids.sort();
    evidence_ids.dedup();
    TopologyMetric {
        key,
        value,
        unit,
        evidence_ids: evidence_ids.clone(),
        drill_down: topology_drill_down(evidence_ids.clone(), None),
        drill_down_reference: DrillDownReference {
            source_query,
            scope,
            time_window: None,
            evidence_ids,
        },
    }
}

fn topology_drill_down(evidence_ids: Vec<String>, filter_key: Option<&str>) -> DrillDownTarget {
    DrillDownTarget {
        destination: DrillDownDestination::Topology,
        evidence_ids,
        filter_key: filter_key.map(sanitize_text),
    }
}

fn unassigned_ownership() -> TopologyOwnership {
    TopologyOwnership {
        team_id: None,
        team_name: None,
        source: TopologyOwnershipSource::Unassigned,
        evidence_ids: Vec::new(),
    }
}

fn evidence_ids_for_node(graph: &DerivedGraph, node_id: &str) -> Vec<String> {
    match graph.nodes.get(node_id) {
        Some(node) => node.evidence_ids.clone(),
        None => Vec::new(),
    }
}

fn admitted_ids(ids: &[String], graph: &DerivedGraph) -> Vec<String> {
    let mut result = ids
        .iter()
        .filter(|id| graph.evidence.contains_key(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    result.retain(|id| safe_identifier(id));
    result.into_iter().collect()
}

fn union_evidence(left: &[String], right: &[String]) -> Vec<String> {
    left.iter()
        .chain(right.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn source_observed_at(graph: &DerivedGraph, source_key: &str) -> Option<String> {
    graph
        .source_status
        .get(source_key)
        .and_then(|status| status.observed_at.clone())
}

fn sanitize_evidence(mut evidence: EvidenceRef) -> Option<EvidenceRef> {
    let mut changed = false;
    let (endpoint, endpoint_changed) = scrub_text(&evidence.endpoint);
    evidence.endpoint = endpoint;
    changed |= endpoint_changed;
    if let Some(query) = evidence.query.as_deref() {
        let (value, value_changed) = scrub_text(query);
        evidence.query = Some(value);
        changed |= value_changed;
    }
    let (excerpt, excerpt_changed) = scrub_text(&evidence.excerpt);
    evidence.excerpt = excerpt;
    changed |= excerpt_changed;
    if let Some(native_url) = evidence.native_url.as_deref() {
        let (value, value_changed) = scrub_text(native_url);
        evidence.native_url = Some(value);
        changed |= value_changed;
    }
    if let Some(connector_id) = evidence.connector_id.as_deref() {
        let (value, value_changed) = scrub_text(connector_id);
        evidence.connector_id = Some(value);
        changed |= value_changed;
    }
    evidence.observed_at = sanitize_text(&evidence.observed_at);
    if changed {
        if evidence.redaction.unparsed {
            return None;
        }
        evidence.redaction.masked = true;
    }
    Some(evidence)
}

fn sanitize_labels(labels: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut object = Map::new();
    for (key, value) in labels {
        object.insert(key.clone(), Value::String(value.clone()));
    }
    let _masked = mask_json_object(&mut object);
    object
        .into_iter()
        .filter(|(key, _)| {
            !sensitive_key(key) && !contains_sensitive_marker(&key.to_ascii_lowercase())
        })
        .filter_map(|(key, value)| {
            let text = value.as_str()?;
            Some((key, sanitize_text(text)))
        })
        .collect()
}

fn sanitize_optional_text(value: Option<&str>) -> Option<String> {
    value.map(sanitize_text)
}

fn sanitize_text(value: &str) -> String {
    scrub_text(value).0
}

fn scrub_text(value: &str) -> (String, bool) {
    let lower = value.to_ascii_lowercase();
    if lower.contains("raw provider error")
        || lower.contains("authorization")
        || lower.contains("credential_reference")
    {
        return (REDACTED.into(), true);
    }
    let mut changed = false;
    let sanitized = value
        .split_whitespace()
        .map(|token| {
            let token_lower = token.to_ascii_lowercase();
            if contains_sensitive_marker(&token_lower) || contains_sensitive_numeric_run(token) {
                changed = true;
                REDACTED
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    (sanitized, changed)
}

fn contains_sensitive_marker(value: &str) -> bool {
    [
        "password",
        "passwd",
        "secret",
        "token",
        "credential",
        "authorization",
        "bearer",
        "account",
        "account_id",
        "account-id",
        "project",
        "project_id",
        "project-id",
        "subscription",
        "subscription_id",
        "subscription-id",
        "cursor",
        "arn:",
        "/subscriptions/",
        "projects/",
        "pagination_cursor",
        "sk-live-",
    ]
    .iter()
    .any(|marker| value.contains(marker))
}

fn contains_sensitive_numeric_run(value: &str) -> bool {
    let mut run_length = 0usize;
    for character in value.chars() {
        if character.is_ascii_digit() {
            run_length = run_length.saturating_add(1);
        } else {
            if (6..=12).contains(&run_length) {
                return true;
            }
            run_length = 0;
        }
    }
    (6..=12).contains(&run_length)
}

fn safe_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && !value.chars().any(char::is_control)
        && !contains_sensitive_marker(&value.to_ascii_lowercase())
}

fn normalized_hint(value: &str) -> Option<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn safe_source_key(value: &str) -> String {
    if safe_identifier(value) {
        value.into()
    } else {
        "source".into()
    }
}

fn evidence_source_status_key(source_kind: EvidenceSourceKind) -> &'static str {
    match source_kind {
        EvidenceSourceKind::Kubernetes => "kubernetes",
        EvidenceSourceKind::Cloud => "cloud",
        EvidenceSourceKind::Alertmanager | EvidenceSourceKind::Prometheus => "observability",
        EvidenceSourceKind::HealthCheck | EvidenceSourceKind::Fixture => "fixtures",
    }
}

fn canonical_resource_name(name: &str) -> (Option<&str>, &str) {
    match name.split_once('/') {
        Some((namespace, resource_name)) if !namespace.is_empty() && !resource_name.is_empty() => {
            (Some(namespace), resource_name)
        }
        _ => (None, name),
    }
}

fn resource_lookup_name(namespace: Option<&str>, canonical_name: &str) -> String {
    match namespace {
        Some(namespace) => format!("{namespace}/{canonical_name}"),
        None => canonical_name.into(),
    }
}

fn resource_order(left: &Resource, right: &Resource) -> std::cmp::Ordering {
    left.kind
        .cmp(&right.kind)
        .then_with(|| left.name.cmp(&right.name))
        .then_with(|| left.native_id.cmp(&right.native_id))
        .then_with(|| left.id.cmp(&right.id))
}

fn map_kubernetes_kind(kind: &str) -> Option<TopologyNodeKind> {
    match kind {
        "Pod" => Some(TopologyNodeKind::Pod),
        "Service" => Some(TopologyNodeKind::Service),
        "Node" => Some(TopologyNodeKind::Node),
        "Namespace" => Some(TopologyNodeKind::Namespace),
        "Deployment" | "StatefulSet" | "DaemonSet" => Some(TopologyNodeKind::Workload),
        _ => None,
    }
}

fn map_relationship(relationship: &str) -> Option<TopologyEdgeKind> {
    match relationship {
        "owns" => Some(TopologyEdgeKind::Owns),
        "selects" => Some(TopologyEdgeKind::Selects),
        _ => None,
    }
}

fn map_kubernetes_health(health: KubernetesHealth) -> ConsoleHealthState {
    match health {
        KubernetesHealth::Healthy => ConsoleHealthState::Healthy,
        KubernetesHealth::Unknown => ConsoleHealthState::Unknown,
        KubernetesHealth::Degraded
        | KubernetesHealth::CrashLoopBackOff
        | KubernetesHealth::OomKilled
        | KubernetesHealth::Pending => ConsoleHealthState::Degraded,
    }
}

fn map_cloud_health(health: CloudHealthState) -> ConsoleHealthState {
    match health {
        CloudHealthState::Healthy => ConsoleHealthState::Healthy,
        CloudHealthState::Degraded => ConsoleHealthState::Degraded,
        CloudHealthState::Unavailable => ConsoleHealthState::Critical,
        CloudHealthState::Unknown => ConsoleHealthState::Unknown,
    }
}

fn cloud_provider_name(provider: crate::cloud::CloudProvider) -> &'static str {
    match provider {
        crate::cloud::CloudProvider::Aws => "aws",
        crate::cloud::CloudProvider::Azure => "azure",
        crate::cloud::CloudProvider::Gcp => "gcp",
    }
}

fn cloud_resource_type_name(resource_type: CloudResourceType) -> &'static str {
    match resource_type {
        CloudResourceType::KubernetesCluster => "kubernetes_cluster",
        CloudResourceType::ComputeInstance => "compute_instance",
    }
}

fn cloud_health_name(health: CloudHealthState) -> &'static str {
    match health {
        CloudHealthState::Healthy => "healthy",
        CloudHealthState::Degraded => "degraded",
        CloudHealthState::Unavailable => "unavailable",
        CloudHealthState::Unknown => "unknown",
    }
}

fn topology_kind_name(kind: TopologyNodeKind) -> &'static str {
    match kind {
        TopologyNodeKind::Environment => "environment",
        TopologyNodeKind::Cluster => "cluster",
        TopologyNodeKind::Namespace => "namespace",
        TopologyNodeKind::Workload => "workload",
        TopologyNodeKind::Service => "service",
        TopologyNodeKind::Pod => "pod",
        TopologyNodeKind::Node => "node",
        TopologyNodeKind::CloudResource => "cloud_resource",
        TopologyNodeKind::ObservabilityTarget => "observability_target",
    }
}

fn topology_edge_kind_name(kind: TopologyEdgeKind) -> &'static str {
    match kind {
        TopologyEdgeKind::Contains => "contains",
        TopologyEdgeKind::Owns => "owns",
        TopologyEdgeKind::Selects => "selects",
        TopologyEdgeKind::RoutesTo => "routes_to",
        TopologyEdgeKind::RunsOn => "runs_on",
        TopologyEdgeKind::DependsOn => "depends_on",
    }
}

fn provenance_order(
    left: &TopologyEdgeProvenance,
    right: &TopologyEdgeProvenance,
) -> std::cmp::Ordering {
    topology_source_kind_name(left.source)
        .cmp(topology_source_kind_name(right.source))
        .then_with(|| left.source_key.cmp(&right.source_key))
        .then_with(|| left.observed_at.cmp(&right.observed_at))
}

fn topology_source_kind_name(kind: TopologySourceKind) -> &'static str {
    match kind {
        TopologySourceKind::Kubernetes => "kubernetes",
        TopologySourceKind::Cloud => "cloud",
        TopologySourceKind::Observability => "observability",
        TopologySourceKind::Fixture => "fixture",
    }
}

fn source_state_rank(state: SourceState) -> u8 {
    match state {
        SourceState::Fresh => 0,
        SourceState::Stale => 1,
        SourceState::Unavailable => 2,
        SourceState::Unverified => 3,
    }
}

fn health_rank(state: ConsoleHealthState) -> u8 {
    match state {
        ConsoleHealthState::Healthy => 0,
        ConsoleHealthState::Degraded => 1,
        ConsoleHealthState::Critical => 2,
        ConsoleHealthState::Unknown => 3,
    }
}
