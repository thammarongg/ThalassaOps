import type {
  ConsoleEvidenceId,
  ConsoleHealthState,
  DrillDownReference,
  DrillDownTarget,
  EvidenceRef,
  IncidentQueueItem,
  ResourceScope,
  TopologyDirection,
  TopologyEdge,
  TopologyEdgeKind,
  TopologyMetric,
  TopologyNode,
  TopologyNodeKind,
  TopologyOwnership,
  TopologyOwnershipSource,
  TopologyPath,
  TopologyPathTermination,
  TopologySnapshot,
  TopologySourceKind
} from "../../contracts/ipc";

/**
 * Deterministic Sprint 12 topology fixtures.
 *
 * These values mirror the Rust fixture catalog shape
 * (docs/design/sprint-12-resource-topology.md): a two-environment graph with
 * an AWS production chain (checkout service → checkout-api workload →
 * checkout-rds cloud resource), a two-edge fixture cycle, ownership variants
 * and a Sprint 11 queue item bound to the checkout service. Every node, edge,
 * ownership mapping and metric carries verified fixture evidence.
 */

const scope: ResourceScope = { resource_ids: [] };
const generatedAt = "2026-08-28T09:00:00Z";

const teamPlatform = "11111111-1111-4111-8111-111111111111";
const teamData = "22222222-2222-4222-8222-222222222222";
const teamPayments = "33333333-3333-4333-8333-333333333333";
const teamStaging = "44444444-4444-4444-8444-444444444444";

const checkoutNodeId = "node:kubernetes:env-aws-prod:service:checkout";
const checkoutApiNodeId = "node:kubernetes:env-aws-prod:workload:checkout-api";
const checkoutPodNodeId = "node:kubernetes:env-aws-prod:pod:checkout-api-0";
const workerNodeId = "node:kubernetes:env-aws-prod:pod:unassigned-worker";
const rdsNodeId = "node:cloud:env-aws-prod:cloud_resource:checkout-rds";
const topicNodeId = "node:cloud:env-aws-prod:cloud_resource:orders-topic";
const paymentsNodeId = "node:kubernetes:env-aws-prod:service:payments-svc";
const awsEnvironmentNodeId = "node:cloud:env-aws-prod:environment:env-aws-prod";
const gcpEnvironmentNodeId = "node:cloud:env-gcp-staging:environment:env-gcp-staging";
const stagingOrdersNodeId = "node:kubernetes:env-gcp-staging:service:staging-orders";
const stagingApiNodeId = "node:kubernetes:env-gcp-staging:workload:staging-orders-api";

const evidenceFor = (id: ConsoleEvidenceId, excerpt: string): EvidenceRef => ({
  id,
  source_kind: "fixture",
  connector_id: null,
  scope,
  endpoint: "fixture://topology",
  query: "topology:snapshot",
  observed_at: generatedAt,
  excerpt,
  native_url: null,
  redaction: {
    classification_verified: true,
    redaction_verified: true,
    masked: false,
    unparsed: false
  }
});

const topologyDrillDown = (
  filterKey: string,
  evidenceIds: ConsoleEvidenceId[]
): DrillDownTarget => ({
  destination: "topology",
  evidence_ids: evidenceIds,
  filter_key: filterKey
});

const evidenceDrillDown = (evidenceIds: ConsoleEvidenceId[]): DrillDownTarget => ({
  destination: "evidence",
  evidence_ids: evidenceIds,
  filter_key: null
});

const reference = (evidenceIds: ConsoleEvidenceId[]): DrillDownReference => ({
  source_query: "topology:snapshot",
  scope,
  time_window: null,
  evidence_ids: evidenceIds
});

type NodeSpec = {
  id: string;
  kind: TopologyNodeKind;
  name: string;
  nativeKind?: string;
  nativeId?: string;
  environmentId?: string;
  provider?: string;
  status?: ConsoleHealthState;
  labels?: Record<string, string>;
  team?: { id: string; name: string };
  ownershipSource: TopologyOwnershipSource;
  scopeTeamId?: string;
  metric?: { key: string; value: number };
};

const nodeEvidenceId = (spec: NodeSpec) => `evidence-topology-${spec.kind}-${spec.name}`;

const buildNode = (spec: NodeSpec): TopologyNode => {
  const evidenceIds = [nodeEvidenceId(spec)];
  const ownership: TopologyOwnership = spec.team
    ? {
        team_id: spec.team.id,
        team_name: spec.team.name,
        source: spec.ownershipSource,
        evidence_ids: evidenceIds
      }
    : { team_id: null, team_name: null, source: "unassigned", evidence_ids: [] };
  const metricEvidenceId = spec.metric
    ? `evidence-topology-metric-${spec.metric.key}-${spec.name}`
    : null;
  return {
    id: spec.id,
    kind: spec.kind,
    name: spec.name,
    native_kind: spec.nativeKind ?? null,
    native_id: spec.nativeId ?? null,
    environment_id: spec.environmentId ?? null,
    provider: spec.provider ?? null,
    scope: spec.scopeTeamId ? { team_id: spec.scopeTeamId, resource_ids: [] } : scope,
    status: spec.status ?? "healthy",
    labels: spec.labels ?? {},
    ownership,
    metric:
      spec.metric && metricEvidenceId
        ? {
            key: spec.metric.key,
            value: spec.metric.value,
            unit: "count",
            evidence_ids: [metricEvidenceId],
            drill_down: topologyDrillDown(spec.id, [metricEvidenceId]),
            drill_down_reference: reference([metricEvidenceId])
          }
        : null,
    affected_by_incident: false,
    evidence_ids: evidenceIds,
    drill_down: topologyDrillDown(spec.id, evidenceIds)
  };
};

const baseNodeSpecs = (): NodeSpec[] => [
  {
    id: awsEnvironmentNodeId,
    kind: "environment",
    name: "AWS production",
    nativeId: "env-aws-prod",
    environmentId: "env-aws-prod",
    provider: "aws",
    team: { id: teamPlatform, name: "Platform" },
    ownershipSource: "environment_default"
  },
  {
    id: gcpEnvironmentNodeId,
    kind: "environment",
    name: "GCP staging",
    nativeId: "env-gcp-staging",
    environmentId: "env-gcp-staging",
    provider: "gcp",
    team: { id: teamStaging, name: "Staging" },
    ownershipSource: "environment_default"
  },
  {
    id: checkoutNodeId,
    kind: "service",
    name: "checkout",
    nativeKind: "Service",
    nativeId: "checkout",
    environmentId: "env-aws-prod",
    status: "degraded",
    labels: { "app.kubernetes.io/name": "checkout", team: "platform" },
    team: { id: teamPlatform, name: "Platform" },
    ownershipSource: "explicit_label",
    metric: { key: "request_count", value: 1250 }
  },
  {
    id: checkoutApiNodeId,
    kind: "workload",
    name: "checkout-api",
    nativeKind: "Deployment",
    nativeId: "checkout-api",
    environmentId: "env-aws-prod",
    team: { id: teamPlatform, name: "Platform" },
    ownershipSource: "environment_default"
  },
  {
    id: checkoutPodNodeId,
    kind: "pod",
    name: "checkout-api-0",
    nativeKind: "Pod",
    nativeId: "checkout-api-0",
    environmentId: "env-aws-prod",
    team: { id: teamPlatform, name: "Platform" },
    ownershipSource: "resource_scope",
    scopeTeamId: teamPlatform,
    metric: { key: "restart_count", value: 3 }
  },
  {
    id: workerNodeId,
    kind: "pod",
    name: "unassigned-worker",
    nativeKind: "Pod",
    nativeId: "unassigned-worker",
    environmentId: "env-aws-prod",
    ownershipSource: "unassigned"
  },
  {
    id: rdsNodeId,
    kind: "cloud_resource",
    name: "checkout-rds",
    nativeKind: "aws_db_instance",
    nativeId: "checkout-rds",
    environmentId: "env-aws-prod",
    provider: "aws",
    labels: { team: "data" },
    team: { id: teamData, name: "Data" },
    ownershipSource: "explicit_label"
  },
  {
    id: topicNodeId,
    kind: "cloud_resource",
    name: "orders-topic",
    nativeKind: "aws_sns_topic",
    nativeId: "orders-topic",
    environmentId: "env-aws-prod",
    provider: "aws",
    team: { id: teamPayments, name: "Payments" },
    ownershipSource: "fixture"
  },
  {
    id: paymentsNodeId,
    kind: "service",
    name: "payments-svc",
    nativeKind: "Service",
    nativeId: "payments-svc",
    environmentId: "env-aws-prod",
    team: { id: teamPayments, name: "Payments" },
    ownershipSource: "fixture"
  },
  {
    id: stagingOrdersNodeId,
    kind: "service",
    name: "staging-orders",
    nativeKind: "Service",
    nativeId: "staging-orders",
    environmentId: "env-gcp-staging",
    team: { id: teamStaging, name: "Staging" },
    ownershipSource: "environment_default"
  },
  {
    id: stagingApiNodeId,
    kind: "workload",
    name: "staging-orders-api",
    nativeKind: "Deployment",
    nativeId: "staging-orders-api",
    environmentId: "env-gcp-staging",
    team: { id: teamStaging, name: "Staging" },
    ownershipSource: "environment_default"
  }
];

const k8sSourceKey = "kubernetes:env-aws-prod";
const cloudSourceKey = "cloud:env-aws-prod";
const gcpSourceKey = "kubernetes:env-gcp-staging";
const fixtureSourceKey = "fixture:dependencies";

type EdgeSpec = {
  upstream: string;
  downstream: string;
  kind: TopologyEdgeKind;
  source: TopologySourceKind;
  sourceKey: string;
  confidence: number;
};

const edgeId = (spec: EdgeSpec) =>
  `edge:${spec.source}:${spec.kind}:${spec.upstream}-${spec.downstream}`;

const buildEdge = (spec: EdgeSpec): TopologyEdge => {
  const evidenceIds = [`evidence-topology-edge-${spec.kind}-${spec.upstream}-${spec.downstream}`];
  return {
    id: edgeId(spec),
    upstream_node_id: spec.upstream,
    downstream_node_id: spec.downstream,
    kind: spec.kind,
    provenance: [{ source: spec.source, source_key: spec.sourceKey, observed_at: generatedAt }],
    confidence: spec.confidence,
    metadata: {},
    evidence_ids: evidenceIds,
    drill_down: evidenceDrillDown(evidenceIds)
  };
};

const edgeEnvContainsCheckout: EdgeSpec = {
  upstream: awsEnvironmentNodeId,
  downstream: checkoutNodeId,
  kind: "contains",
  source: "kubernetes",
  sourceKey: k8sSourceKey,
  confidence: 1
};
const edgeEnvContainsCheckoutApi: EdgeSpec = {
  upstream: awsEnvironmentNodeId,
  downstream: checkoutApiNodeId,
  kind: "contains",
  source: "kubernetes",
  sourceKey: k8sSourceKey,
  confidence: 1
};
const edgeEnvContainsCheckoutPod: EdgeSpec = {
  upstream: awsEnvironmentNodeId,
  downstream: checkoutPodNodeId,
  kind: "contains",
  source: "kubernetes",
  sourceKey: k8sSourceKey,
  confidence: 1
};
const edgeEnvContainsWorker: EdgeSpec = {
  upstream: awsEnvironmentNodeId,
  downstream: workerNodeId,
  kind: "contains",
  source: "kubernetes",
  sourceKey: k8sSourceKey,
  confidence: 1
};
const edgeEnvContainsPayments: EdgeSpec = {
  upstream: awsEnvironmentNodeId,
  downstream: paymentsNodeId,
  kind: "contains",
  source: "kubernetes",
  sourceKey: k8sSourceKey,
  confidence: 1
};
const edgeEnvContainsRds: EdgeSpec = {
  upstream: awsEnvironmentNodeId,
  downstream: rdsNodeId,
  kind: "contains",
  source: "cloud",
  sourceKey: cloudSourceKey,
  confidence: 1
};
const edgeEnvContainsTopic: EdgeSpec = {
  upstream: awsEnvironmentNodeId,
  downstream: topicNodeId,
  kind: "contains",
  source: "cloud",
  sourceKey: cloudSourceKey,
  confidence: 1
};
const edgeWorkloadOwnsPod: EdgeSpec = {
  upstream: checkoutApiNodeId,
  downstream: checkoutPodNodeId,
  kind: "owns",
  source: "kubernetes",
  sourceKey: k8sSourceKey,
  confidence: 1
};
const edgeServiceSelectsPod: EdgeSpec = {
  upstream: checkoutNodeId,
  downstream: checkoutPodNodeId,
  kind: "selects",
  source: "kubernetes",
  sourceKey: k8sSourceKey,
  confidence: 0.9
};
const edgeCheckoutDependsOnApi: EdgeSpec = {
  upstream: checkoutNodeId,
  downstream: checkoutApiNodeId,
  kind: "depends_on",
  source: "fixture",
  sourceKey: fixtureSourceKey,
  confidence: 0.8
};
const edgeApiDependsOnRds: EdgeSpec = {
  upstream: checkoutApiNodeId,
  downstream: rdsNodeId,
  kind: "depends_on",
  source: "fixture",
  sourceKey: fixtureSourceKey,
  confidence: 0.7
};
const edgeRdsDependsOnTopic: EdgeSpec = {
  upstream: rdsNodeId,
  downstream: topicNodeId,
  kind: "depends_on",
  source: "fixture",
  sourceKey: fixtureSourceKey,
  confidence: 0.5
};
const edgeTopicDependsOnPayments: EdgeSpec = {
  upstream: topicNodeId,
  downstream: paymentsNodeId,
  kind: "depends_on",
  source: "fixture",
  sourceKey: fixtureSourceKey,
  confidence: 0.5
};
const edgeApiDependsOnWorker: EdgeSpec = {
  upstream: checkoutApiNodeId,
  downstream: workerNodeId,
  kind: "depends_on",
  source: "fixture",
  sourceKey: fixtureSourceKey,
  confidence: 0.6
};
const edgeWorkerDependsOnApi: EdgeSpec = {
  upstream: workerNodeId,
  downstream: checkoutApiNodeId,
  kind: "depends_on",
  source: "fixture",
  sourceKey: fixtureSourceKey,
  confidence: 0.6
};
const edgeGcpContainsOrders: EdgeSpec = {
  upstream: gcpEnvironmentNodeId,
  downstream: stagingOrdersNodeId,
  kind: "contains",
  source: "kubernetes",
  sourceKey: gcpSourceKey,
  confidence: 1
};
const edgeGcpContainsApi: EdgeSpec = {
  upstream: gcpEnvironmentNodeId,
  downstream: stagingApiNodeId,
  kind: "contains",
  source: "kubernetes",
  sourceKey: gcpSourceKey,
  confidence: 1
};
const edgeStagingOrdersDependsOnApi: EdgeSpec = {
  upstream: stagingOrdersNodeId,
  downstream: stagingApiNodeId,
  kind: "depends_on",
  source: "fixture",
  sourceKey: fixtureSourceKey,
  confidence: 0.8
};

const baseEdgeSpecs = (): EdgeSpec[] => [
  edgeEnvContainsCheckout,
  edgeEnvContainsCheckoutApi,
  edgeEnvContainsCheckoutPod,
  edgeEnvContainsWorker,
  edgeEnvContainsPayments,
  edgeEnvContainsRds,
  edgeEnvContainsTopic,
  edgeWorkloadOwnsPod,
  edgeServiceSelectsPod,
  edgeCheckoutDependsOnApi,
  edgeApiDependsOnRds,
  edgeRdsDependsOnTopic,
  edgeTopicDependsOnPayments,
  edgeApiDependsOnWorker,
  edgeWorkerDependsOnApi,
  edgeGcpContainsOrders,
  edgeGcpContainsApi,
  edgeStagingOrdersDependsOnApi
];

type PathSpec = {
  id: string;
  rootId: string;
  terminalId: string;
  nodeIds: string[];
  edgeIds: string[];
  direction: TopologyDirection;
  depth: number;
  confidence: number;
  termination: TopologyPathTermination;
  cycleEdgeId?: string;
};

const buildPath = (spec: PathSpec, nodes: TopologyNode[], edges: TopologyEdge[]): TopologyPath => {
  const nodesById = new Map(nodes.map((node) => [node.id, node]));
  const edgesById = new Map(edges.map((edge) => [edge.id, edge]));
  const evidenceIds = [
    ...new Set([
      ...spec.nodeIds.flatMap((id) => nodesById.get(id)?.evidence_ids ?? []),
      ...spec.edgeIds.flatMap((id) => edgesById.get(id)?.evidence_ids ?? []),
      ...(spec.cycleEdgeId ? (edgesById.get(spec.cycleEdgeId)?.evidence_ids ?? []) : [])
    ])
  ].sort();
  return {
    id: `path:fixture:${spec.direction}:${spec.id}`,
    root_node_id: spec.rootId,
    terminal_node_id: spec.terminalId,
    node_ids: spec.nodeIds,
    edge_ids: spec.edgeIds,
    direction: spec.direction,
    depth: spec.depth,
    confidence: spec.confidence,
    kind: "probable_structural",
    termination: spec.termination,
    cycle_edge_id: spec.cycleEdgeId ?? null,
    evidence_ids: evidenceIds,
    drill_down: evidenceDrillDown(evidenceIds)
  };
};

const basePathSpecs = (): PathSpec[] => [
  {
    id: "checkout:environment",
    rootId: checkoutNodeId,
    terminalId: awsEnvironmentNodeId,
    nodeIds: [checkoutNodeId, awsEnvironmentNodeId],
    edgeIds: [edgeId(edgeEnvContainsCheckout)],
    direction: "upstream",
    depth: 1,
    confidence: 1,
    termination: "leaf"
  },
  {
    id: "checkout:selected-pod",
    rootId: checkoutNodeId,
    terminalId: checkoutPodNodeId,
    nodeIds: [checkoutNodeId, checkoutPodNodeId],
    edgeIds: [edgeId(edgeServiceSelectsPod)],
    direction: "downstream",
    depth: 1,
    confidence: 0.9,
    termination: "leaf"
  },
  {
    id: "checkout:owned-pod",
    rootId: checkoutNodeId,
    terminalId: checkoutPodNodeId,
    nodeIds: [checkoutNodeId, checkoutApiNodeId, checkoutPodNodeId],
    edgeIds: [edgeId(edgeCheckoutDependsOnApi), edgeId(edgeWorkloadOwnsPod)],
    direction: "downstream",
    depth: 2,
    confidence: 0.8,
    termination: "leaf"
  },
  {
    id: "checkout:orders-topic",
    rootId: checkoutNodeId,
    terminalId: topicNodeId,
    nodeIds: [checkoutNodeId, checkoutApiNodeId, rdsNodeId, topicNodeId],
    edgeIds: [
      edgeId(edgeCheckoutDependsOnApi),
      edgeId(edgeApiDependsOnRds),
      edgeId(edgeRdsDependsOnTopic)
    ],
    direction: "downstream",
    depth: 3,
    confidence: 0.5,
    termination: "depth_limit"
  },
  {
    id: "checkout:worker-cycle",
    rootId: checkoutNodeId,
    terminalId: workerNodeId,
    nodeIds: [checkoutNodeId, checkoutApiNodeId, workerNodeId],
    edgeIds: [edgeId(edgeCheckoutDependsOnApi), edgeId(edgeApiDependsOnWorker)],
    direction: "downstream",
    depth: 2,
    confidence: 0.6,
    termination: "cycle_detected",
    cycleEdgeId: edgeId(edgeWorkerDependsOnApi)
  }
];

const freshSource = (sourceKey: string) => ({
  source_key: sourceKey,
  state: "fresh" as const,
  reason: null,
  detail: null,
  observed_at: generatedAt,
  evidence_ids: []
});

const allSourceStatus = () => [
  freshSource("kubernetes:env-aws-prod"),
  freshSource("cloud:env-aws-prod"),
  freshSource("kubernetes:env-gcp-staging"),
  freshSource("operations:incident-queue")
];

const collectEvidence = (nodes: TopologyNode[], edges: TopologyEdge[], paths: TopologyPath[]) => {
  const excerpts = new Map<string, string>(
    nodes.map((node) => [node.evidence_ids[0], `fixture ${node.kind} ${node.name}`])
  );
  for (const edge of edges) {
    excerpts.set(edge.evidence_ids[0], `fixture ${edge.kind} relationship ${edge.id}`);
  }
  for (const node of nodes) {
    if (node.metric) {
      excerpts.set(
        node.metric.evidence_ids[0],
        `fixture metric ${node.metric.key} for ${node.name}`
      );
    }
  }
  const ids = [
    ...new Set([
      ...nodes.flatMap((node) => [
        ...node.evidence_ids,
        ...(node.metric?.evidence_ids ?? []),
        ...node.ownership.evidence_ids
      ]),
      ...edges.flatMap((edge) => edge.evidence_ids),
      ...paths.flatMap((path) => path.evidence_ids)
    ])
  ].sort();
  return ids.map((id) => evidenceFor(id, excerpts.get(id) ?? `fixture topology evidence ${id}`));
};

const summaryMetric = (
  key: string,
  value: number,
  evidenceIds: ConsoleEvidenceId[]
): TopologyMetric => ({
  key,
  value,
  unit: "count",
  evidence_ids: evidenceIds,
  drill_down: evidenceDrillDown(evidenceIds),
  drill_down_reference: reference(evidenceIds)
});

const buildSnapshot = ({
  nodes,
  edges,
  paths,
  incidentId,
  focusNodeId,
  sourceStatus
}: {
  nodes: TopologyNode[];
  edges: TopologyEdge[];
  paths: TopologyPath[];
  incidentId: string | null;
  focusNodeId: string | null;
  sourceStatus: TopologySnapshot["source_status"];
}): TopologySnapshot => ({
  generated_at: generatedAt,
  scope,
  filter: { environment_ids: [], team_ids: [], incident_id: incidentId },
  focus_node_id: focusNodeId,
  traversal: { direction: "both", max_depth: 3 },
  summary: {
    visible_nodes: summaryMetric(
      "visible_nodes",
      nodes.length,
      nodes.flatMap((node) => node.evidence_ids)
    ),
    visible_edges: summaryMetric(
      "visible_edges",
      edges.length,
      edges.flatMap((edge) => edge.evidence_ids)
    ),
    affected_nodes: summaryMetric(
      "affected_nodes",
      nodes.filter((node) => node.affected_by_incident).length,
      nodes.filter((node) => node.affected_by_incident).flatMap((node) => node.evidence_ids)
    ),
    probable_paths: summaryMetric(
      "probable_paths",
      paths.length,
      paths.flatMap((path) => path.evidence_ids)
    )
  },
  nodes,
  edges,
  paths,
  source_status: sourceStatus,
  evidence: collectEvidence(nodes, edges, paths)
});

const healthyGraph = () => {
  const nodes = baseNodeSpecs().map(buildNode);
  const edges = baseEdgeSpecs().map(buildEdge);
  const paths = basePathSpecs().map((spec) => buildPath(spec, nodes, edges));
  return { nodes, edges, paths };
};

/** Healthy two-environment graph focused on the checkout service. */
export const topologySnapshotFixture: TopologySnapshot = buildSnapshot({
  ...healthyGraph(),
  incidentId: null,
  focusNodeId: checkoutNodeId,
  sourceStatus: allSourceStatus()
});

/** Same graph with the Sprint 11 queue item selected: checkout is affected. */
export const topologyIncidentSnapshotFixture: TopologySnapshot = (() => {
  const graph = healthyGraph();
  const nodes = graph.nodes.map((node) =>
    node.id === checkoutNodeId ? { ...node, affected_by_incident: true } : node
  );
  return buildSnapshot({
    nodes,
    edges: graph.edges,
    paths: graph.paths,
    incidentId: "alert-checkout-s1",
    focusNodeId: checkoutNodeId,
    sourceStatus: allSourceStatus()
  });
})();

/** AWS sources healthy, the GCP staging source unavailable: a partial view. */
export const topologyDegradedSnapshotFixture: TopologySnapshot = (() => {
  const graph = healthyGraph();
  const nodes = graph.nodes.filter((node) => node.environment_id !== "env-gcp-staging");
  const visibleIds = new Set(nodes.map((node) => node.id));
  const edges = graph.edges.filter(
    (edge) => visibleIds.has(edge.upstream_node_id) && visibleIds.has(edge.downstream_node_id)
  );
  const paths = graph.paths.filter((path) => visibleIds.has(path.root_node_id));
  return buildSnapshot({
    nodes,
    edges,
    paths,
    incidentId: null,
    focusNodeId: checkoutNodeId,
    sourceStatus: [
      freshSource("kubernetes:env-aws-prod"),
      freshSource("cloud:env-aws-prod"),
      {
        source_key: "kubernetes:env-gcp-staging",
        state: "unavailable",
        reason: "unreachable",
        detail: "fixture gcp staging session unavailable",
        observed_at: generatedAt,
        evidence_ids: []
      },
      freshSource("operations:incident-queue")
    ]
  });
})();

/** No admitted graph facts at all. */
export const topologyEmptySnapshotFixture: TopologySnapshot = (() => {
  const emptyEvidenceId = "evidence-topology-summary-empty";
  const snapshot = buildSnapshot({
    nodes: [],
    edges: [],
    paths: [],
    incidentId: null,
    focusNodeId: null,
    sourceStatus: [freshSource("operations:incident-queue")]
  });
  return {
    ...snapshot,
    summary: {
      visible_nodes: summaryMetric("visible_nodes", 0, [emptyEvidenceId]),
      visible_edges: summaryMetric("visible_edges", 0, [emptyEvidenceId]),
      affected_nodes: summaryMetric("affected_nodes", 0, [emptyEvidenceId]),
      probable_paths: summaryMetric("probable_paths", 0, [emptyEvidenceId])
    },
    evidence: [evidenceFor(emptyEvidenceId, "fixture empty topology snapshot")]
  };
})();

/**
 * A snapshot whose upstream path references a node and edge that are not
 * present, and whose evidence set drops that node's evidence: per-path and
 * evidence error states must surface without blanking the rest of the view.
 */
export const topologyBrokenPathSnapshotFixture: TopologySnapshot = (() => {
  const graph = healthyGraph();
  const nodes = graph.nodes.filter((node) => node.id !== awsEnvironmentNodeId);
  const visibleIds = new Set(nodes.map((node) => node.id));
  const edges = graph.edges.filter(
    (edge) => visibleIds.has(edge.upstream_node_id) && visibleIds.has(edge.downstream_node_id)
  );
  const downstreamPaths = graph.paths.filter((path) => path.direction !== "upstream");
  const snapshot = buildSnapshot({
    nodes,
    edges,
    paths: downstreamPaths,
    incidentId: null,
    focusNodeId: checkoutNodeId,
    sourceStatus: allSourceStatus()
  });
  const upstreamPath = graph.paths[0];
  return {
    ...snapshot,
    paths: [...downstreamPaths, upstreamPath],
    evidence: snapshot.evidence.filter(
      (item) => item.id !== "evidence-topology-environment-AWS production"
    )
  };
})();

/** Sprint 11 queue items available for the Incident filter. */
export const topologyIncidentsFixture: IncidentQueueItem[] = [
  {
    id: "alert-checkout-s1",
    title: "Checkout latency breach",
    source_kind: "alert",
    source_id: "alertmanager:checkout-latency",
    severity: "S1",
    priority: "P1",
    status: "investigating",
    business_impact: {
      level: "critical",
      summary: "Checkout is failing for a subset of customers",
      customer_scope: "partial",
      service_criticality: "revenue-critical",
      trajectory: "expanding"
    },
    scope,
    detected_at: generatedAt,
    opened_at: generatedAt,
    last_update: generatedAt,
    affected_scope: { environment_id: "env-aws-prod", resource_ids: [] },
    evidence_ids: ["evidence-topology-service-checkout"],
    drill_down: {
      destination: "topology",
      evidence_ids: ["evidence-topology-service-checkout"],
      filter_key: checkoutNodeId
    },
    drill_down_reference: reference(["evidence-topology-service-checkout"])
  }
];

export const topologyFixtureNodeIds = {
  checkout: checkoutNodeId,
  checkoutApi: checkoutApiNodeId,
  checkoutPod: checkoutPodNodeId,
  worker: workerNodeId,
  rds: rdsNodeId,
  topic: topicNodeId,
  payments: paymentsNodeId,
  awsEnvironment: awsEnvironmentNodeId,
  gcpEnvironment: gcpEnvironmentNodeId,
  stagingOrders: stagingOrdersNodeId,
  stagingApi: stagingApiNodeId
};
