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
  TopologyOwnershipSource,
  TopologyPath,
  TopologyPathTermination,
  TopologySnapshot,
  TopologySourceKind
} from "../../contracts/ipc";

/**
 * Deterministic Sprint 12 topology fixtures.
 *
 * The graph below is the TypeScript mirror of `topology_fixture_input` in
 * `src-tauri/src/topology/fixtures.rs`. IDs, evidence IDs, ownership and
 * relationships intentionally stay source-qualified so UI tests cannot pass
 * with a fabricated graph that differs from the Rust projection.
 */

const generatedAt = "2026-08-28T09:00:00Z";
const observedAt = "2026-08-28T09:00:00+00:00";
const scope: ResourceScope = {
  organization_id: "00000000-0000-0000-0000-000000000014",
  team_id: "00000000-0000-0000-0000-000000000013",
  workspace_id: "00000000-0000-0000-0000-000000000012",
  environment_id: null,
  resource_ids: []
};
const teamPlatform = "00000000-0000-0000-0000-000000000013";
const checkoutResourceId = "00000000-0000-0000-0000-000000000101";

const awsEnvironmentNodeId = "node:fixture:env-aws-prod:environment:env-aws-prod";
const gcpEnvironmentNodeId = "node:fixture:env-gcp-staging:environment:env-gcp-staging";
const namespaceProdNodeId = "node:kubernetes:env-aws-prod:namespace:uid-namespace-prod";
const namespaceStagingNodeId = "node:kubernetes:env-gcp-staging:namespace:uid-namespace-staging";
const workerNodeId = "node:kubernetes:env-aws-prod:node:uid-node-worker-a";
const checkoutPodNodeId = "node:kubernetes:env-aws-prod:pod:uid-pod-checkout-api-0";
const catalogPodNodeId = "node:kubernetes:env-gcp-staging:pod:uid-pod-catalog-api-0";
const checkoutNodeId = "node:kubernetes:env-aws-prod:service:uid-service-checkout";
const catalogNodeId = "node:kubernetes:env-gcp-staging:service:uid-service-catalog";
const checkoutApiNodeId = "node:kubernetes:env-aws-prod:workload:uid-workload-checkout-api";
const unassignedWorkerNodeId =
  "node:kubernetes:env-aws-prod:workload:uid-workload-unassigned-worker";
const catalogApiNodeId = "node:kubernetes:env-gcp-staging:workload:uid-workload-catalog-api";
const rdsNodeId = "node:cloud:env-aws-prod:cloud_resource:checkout-rds";
const replicaNodeId = "node:cloud:env-aws-prod:cloud_resource:checkout-rds-replica";
const catalogClusterNodeId = "node:cloud:env-gcp-staging:cluster:catalog-cluster";

const evidence = {
  environmentAws: "evidence-topology-environment-aws",
  environmentGcp: "evidence-topology-environment-gcp",
  cloudRds: "evidence-topology-cloud-checkout-rds",
  cloudReplica: "evidence-topology-cloud-checkout-rds-replica",
  cloudCatalogCluster: "evidence-topology-cloud-catalog-cluster",
  namespaceProd: "evidence-topology-k8s-namespace-prod",
  namespaceStaging: "evidence-topology-k8s-namespace-staging",
  nodeWorker: "evidence-topology-k8s-node-worker-a",
  podCheckout: "evidence-topology-k8s-pod-checkout-api-0",
  podCatalog: "evidence-topology-k8s-pod-catalog-api-0",
  serviceCheckout: "evidence-topology-k8s-service-checkout",
  serviceCatalog: "evidence-topology-k8s-service-catalog",
  workloadCheckout: "evidence-topology-k8s-workload-checkout-api",
  workloadUnassigned: "evidence-topology-k8s-workload-unassigned-worker",
  workloadCatalog: "evidence-topology-k8s-workload-catalog-api",
  alertCheckout: "evidence-topology-alert-checkout",
  metricCheckout: "evidence-topology-metric-checkout",
  edgeCheckoutApi: "evidence-topology-edge-checkout-api",
  edgeApiRds: "evidence-topology-edge-api-rds",
  edgeRdsReplica: "evidence-topology-edge-rds-replica",
  edgeReplicaRds: "evidence-topology-edge-replica-rds",
  ownershipPlatform: "evidence-topology-ownership-platform",
  ownershipEnvironment: "evidence-topology-ownership-environment",
  ownershipUnassigned: "evidence-topology-ownership-unassigned",
  incidentCheckout: "evidence-topology-incident-checkout",
  summary: "evidence-topology-summary"
} as const;

const sortedUnique = (ids: ConsoleEvidenceId[]) => [...new Set(ids)].sort();

type EvidenceSpec = {
  id: ConsoleEvidenceId;
  source_kind: EvidenceRef["source_kind"];
  excerpt: string;
};

const evidenceCatalog: EvidenceSpec[] = [
  {
    id: evidence.environmentAws,
    source_kind: "cloud",
    excerpt: "AWS production environment status"
  },
  { id: evidence.environmentGcp, source_kind: "cloud", excerpt: "GCP staging environment status" },
  { id: evidence.cloudRds, source_kind: "cloud", excerpt: "checkout database resource" },
  {
    id: evidence.cloudReplica,
    source_kind: "cloud",
    excerpt: "checkout database replica resource"
  },
  {
    id: evidence.cloudCatalogCluster,
    source_kind: "cloud",
    excerpt: "catalog cluster resource"
  },
  { id: evidence.namespaceProd, source_kind: "kubernetes", excerpt: "production namespace" },
  { id: evidence.namespaceStaging, source_kind: "kubernetes", excerpt: "staging namespace" },
  { id: evidence.nodeWorker, source_kind: "kubernetes", excerpt: "worker node" },
  { id: evidence.podCheckout, source_kind: "kubernetes", excerpt: "checkout API pod" },
  { id: evidence.podCatalog, source_kind: "kubernetes", excerpt: "catalog API pod" },
  { id: evidence.serviceCheckout, source_kind: "kubernetes", excerpt: "checkout service" },
  { id: evidence.serviceCatalog, source_kind: "kubernetes", excerpt: "catalog service" },
  { id: evidence.workloadCheckout, source_kind: "kubernetes", excerpt: "checkout API workload" },
  {
    id: evidence.workloadUnassigned,
    source_kind: "kubernetes",
    excerpt: "unassigned worker workload"
  },
  { id: evidence.workloadCatalog, source_kind: "kubernetes", excerpt: "catalog API workload" },
  { id: evidence.alertCheckout, source_kind: "alertmanager", excerpt: "checkout alert is firing" },
  { id: evidence.metricCheckout, source_kind: "prometheus", excerpt: "checkout request metric" },
  {
    id: evidence.edgeCheckoutApi,
    source_kind: "fixture",
    excerpt: "checkout to API structural dependency"
  },
  {
    id: evidence.edgeApiRds,
    source_kind: "fixture",
    excerpt: "API to database structural dependency"
  },
  {
    id: evidence.edgeRdsReplica,
    source_kind: "fixture",
    excerpt: "database to replica structural dependency"
  },
  {
    id: evidence.edgeReplicaRds,
    source_kind: "fixture",
    excerpt: "replica to database cycle edge"
  },
  {
    id: evidence.ownershipPlatform,
    source_kind: "fixture",
    excerpt: "platform ownership mapping"
  },
  {
    id: evidence.ownershipEnvironment,
    source_kind: "fixture",
    excerpt: "environment ownership mapping"
  },
  {
    id: evidence.ownershipUnassigned,
    source_kind: "fixture",
    excerpt: "unassigned ownership mapping"
  },
  {
    id: evidence.incidentCheckout,
    source_kind: "fixture",
    excerpt: "queue item affected checkout root"
  },
  { id: evidence.summary, source_kind: "fixture", excerpt: "topology summary counts" }
];

const fixtureEvidence = (): EvidenceRef[] =>
  evidenceCatalog
    .map((item) => ({
      id: item.id,
      source_kind: item.source_kind,
      connector_id: "fixture-topology",
      scope,
      endpoint: "fixture://topology",
      query: item.id,
      observed_at: observedAt,
      excerpt: item.excerpt,
      native_url: null,
      redaction: {
        classification_verified: true,
        redaction_verified: true,
        masked: false,
        unparsed: false
      }
    }))
    .sort((left, right) => left.id.localeCompare(right.id));

const topologyDrillDown = (
  filterKey: string | null,
  evidenceIds: ConsoleEvidenceId[]
): DrillDownTarget => ({
  destination: "topology",
  evidence_ids: sortedUnique(evidenceIds),
  filter_key: filterKey
});

const evidenceDrillDown = (evidenceIds: ConsoleEvidenceId[]): DrillDownTarget => ({
  destination: "evidence",
  evidence_ids: sortedUnique(evidenceIds),
  filter_key: null
});

const reference = (sourceQuery: string, evidenceIds: ConsoleEvidenceId[]): DrillDownReference => ({
  source_query: sourceQuery,
  scope,
  time_window: null,
  evidence_ids: sortedUnique(evidenceIds)
});

type NodeSpec = {
  id: string;
  kind: TopologyNodeKind;
  name: string;
  nativeKind: string;
  nativeId: string;
  environmentId: string;
  provider: string;
  status: ConsoleHealthState;
  labels: Record<string, string>;
  ownershipSource: TopologyOwnershipSource;
  evidenceIds: ConsoleEvidenceId[];
  ownershipEvidenceIds: ConsoleEvidenceId[];
  metric?: { key: string; value: number; sourceQuery: string };
};

const assigned = (
  source: TopologyOwnershipSource,
  evidenceIds: ConsoleEvidenceId[]
): Pick<NodeSpec, "ownershipSource" | "ownershipEvidenceIds"> => ({
  ownershipSource: source,
  ownershipEvidenceIds: evidenceIds
});

const resourceScopeOwnershipEvidence = (nodeEvidenceId: ConsoleEvidenceId) => [
  nodeEvidenceId,
  evidence.ownershipEnvironment,
  evidence.ownershipPlatform
];

const buildNode = (spec: NodeSpec): TopologyNode => {
  const evidenceIds = sortedUnique(spec.evidenceIds);
  const ownership =
    spec.ownershipSource === "unassigned"
      ? {
          team_id: null,
          team_name: null,
          source: "unassigned" as const,
          evidence_ids: []
        }
      : {
          team_id: teamPlatform,
          team_name: "Platform",
          source: spec.ownershipSource,
          evidence_ids: sortedUnique(spec.ownershipEvidenceIds)
        };
  const metric: TopologyMetric | null = spec.metric
    ? {
        key: spec.metric.key,
        value: spec.metric.value,
        unit: "count",
        evidence_ids: evidenceIds,
        drill_down: topologyDrillDown(null, evidenceIds),
        drill_down_reference: reference(spec.metric.sourceQuery, evidenceIds)
      }
    : null;
  return {
    id: spec.id,
    kind: spec.kind,
    name: spec.name,
    native_kind: spec.nativeKind,
    native_id: spec.nativeId,
    environment_id: spec.environmentId,
    provider: spec.provider,
    scope,
    status: spec.status,
    labels: spec.labels,
    ownership,
    metric,
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
    nativeKind: "EnvironmentStatus",
    nativeId: "env-aws-prod",
    environmentId: "env-aws-prod",
    provider: "aws",
    status: "degraded",
    labels: {},
    ...assigned("resource_scope", resourceScopeOwnershipEvidence(evidence.environmentAws)),
    evidenceIds: [evidence.environmentAws],
    metric: {
      key: "environment.env-aws-prod.resource_count",
      value: 4,
      sourceQuery: "environment:env-aws-prod"
    }
  },
  {
    id: gcpEnvironmentNodeId,
    kind: "environment",
    name: "GCP staging",
    nativeKind: "EnvironmentStatus",
    nativeId: "env-gcp-staging",
    environmentId: "env-gcp-staging",
    provider: "gcp",
    status: "healthy",
    labels: {},
    ...assigned("resource_scope", resourceScopeOwnershipEvidence(evidence.environmentGcp)),
    evidenceIds: [evidence.environmentGcp],
    metric: {
      key: "environment.env-gcp-staging.resource_count",
      value: 4,
      sourceQuery: "environment:env-gcp-staging"
    }
  },
  {
    id: namespaceProdNodeId,
    kind: "namespace",
    name: "prod",
    nativeKind: "Namespace",
    nativeId: "uid-namespace-prod",
    environmentId: "env-aws-prod",
    provider: "kubernetes",
    status: "unknown",
    labels: {},
    ...assigned("resource_scope", resourceScopeOwnershipEvidence(evidence.namespaceProd)),
    evidenceIds: [evidence.namespaceProd]
  },
  {
    id: namespaceStagingNodeId,
    kind: "namespace",
    name: "staging",
    nativeKind: "Namespace",
    nativeId: "uid-namespace-staging",
    environmentId: "env-gcp-staging",
    provider: "kubernetes",
    status: "unknown",
    labels: {},
    ...assigned("resource_scope", resourceScopeOwnershipEvidence(evidence.namespaceStaging)),
    evidenceIds: [evidence.namespaceStaging]
  },
  {
    id: workerNodeId,
    kind: "node",
    name: "worker-a",
    nativeKind: "Node",
    nativeId: "uid-node-worker-a",
    environmentId: "env-aws-prod",
    provider: "kubernetes",
    status: "healthy",
    labels: { role: "worker" },
    ...assigned("resource_scope", resourceScopeOwnershipEvidence(evidence.nodeWorker)),
    evidenceIds: [evidence.nodeWorker]
  },
  {
    id: checkoutPodNodeId,
    kind: "pod",
    name: "checkout-api-0",
    nativeKind: "Pod",
    nativeId: "uid-pod-checkout-api-0",
    environmentId: "env-aws-prod",
    provider: "kubernetes",
    status: "healthy",
    labels: { app: "checkout-api" },
    ...assigned("resource_scope", resourceScopeOwnershipEvidence(evidence.podCheckout)),
    evidenceIds: [evidence.podCheckout]
  },
  {
    id: catalogPodNodeId,
    kind: "pod",
    name: "catalog-api-0",
    nativeKind: "Pod",
    nativeId: "uid-pod-catalog-api-0",
    environmentId: "env-gcp-staging",
    provider: "kubernetes",
    status: "healthy",
    labels: { app: "catalog-api" },
    ...assigned("resource_scope", resourceScopeOwnershipEvidence(evidence.podCatalog)),
    evidenceIds: [evidence.podCatalog]
  },
  {
    id: checkoutNodeId,
    kind: "service",
    name: "checkout",
    nativeKind: "Service",
    nativeId: "uid-service-checkout",
    environmentId: "env-aws-prod",
    provider: "kubernetes",
    status: "unknown",
    labels: { app: "checkout", team: "platform" },
    ...assigned("explicit_label", [evidence.ownershipPlatform]),
    evidenceIds: [evidence.serviceCheckout, evidence.alertCheckout, evidence.metricCheckout]
  },
  {
    id: catalogNodeId,
    kind: "service",
    name: "catalog",
    nativeKind: "Service",
    nativeId: "uid-service-catalog",
    environmentId: "env-gcp-staging",
    provider: "kubernetes",
    status: "unknown",
    labels: { app: "catalog-api" },
    ...assigned("resource_scope", resourceScopeOwnershipEvidence(evidence.serviceCatalog)),
    evidenceIds: [evidence.serviceCatalog]
  },
  {
    id: checkoutApiNodeId,
    kind: "workload",
    name: "checkout-api",
    nativeKind: "Deployment",
    nativeId: "uid-workload-checkout-api",
    environmentId: "env-aws-prod",
    provider: "kubernetes",
    status: "healthy",
    labels: { app: "checkout-api" },
    ...assigned("resource_scope", resourceScopeOwnershipEvidence(evidence.workloadCheckout)),
    evidenceIds: [evidence.workloadCheckout],
    metric: {
      key: "ready_replicas:checkout-api",
      value: 3,
      sourceQuery: "kubernetes:env-aws-prod:checkout-api"
    }
  },
  {
    id: unassignedWorkerNodeId,
    kind: "workload",
    name: "unassigned-worker",
    nativeKind: "Deployment",
    nativeId: "uid-workload-unassigned-worker",
    environmentId: "env-aws-prod",
    provider: "kubernetes",
    status: "healthy",
    labels: { app: "unassigned-worker" },
    ...assigned("unassigned", []),
    evidenceIds: [evidence.workloadUnassigned],
    metric: {
      key: "ready_replicas:unassigned-worker",
      value: 1,
      sourceQuery: "kubernetes:env-aws-prod:unassigned-worker"
    }
  },
  {
    id: catalogApiNodeId,
    kind: "workload",
    name: "catalog-api",
    nativeKind: "Deployment",
    nativeId: "uid-workload-catalog-api",
    environmentId: "env-gcp-staging",
    provider: "kubernetes",
    status: "healthy",
    labels: { app: "catalog-api" },
    ...assigned("resource_scope", resourceScopeOwnershipEvidence(evidence.workloadCatalog)),
    evidenceIds: [evidence.workloadCatalog],
    metric: {
      key: "ready_replicas:catalog-api",
      value: 2,
      sourceQuery: "kubernetes:env-gcp-staging:catalog-api"
    }
  },
  {
    id: rdsNodeId,
    kind: "cloud_resource",
    name: "checkout-rds",
    nativeKind: "compute_instance",
    nativeId: "checkout-rds",
    environmentId: "env-aws-prod",
    provider: "aws",
    status: "degraded",
    labels: { location: "us-east-1", resource_type: "compute_instance", status: "degraded" },
    ...assigned("resource_scope", resourceScopeOwnershipEvidence(evidence.cloudRds)),
    evidenceIds: [evidence.cloudRds]
  },
  {
    id: replicaNodeId,
    kind: "cloud_resource",
    name: "checkout-rds-replica",
    nativeKind: "compute_instance",
    nativeId: "checkout-rds-replica",
    environmentId: "env-aws-prod",
    provider: "aws",
    status: "healthy",
    labels: { location: "us-east-1", resource_type: "compute_instance", status: "healthy" },
    ...assigned("resource_scope", resourceScopeOwnershipEvidence(evidence.cloudReplica)),
    evidenceIds: [evidence.cloudReplica]
  },
  {
    id: catalogClusterNodeId,
    kind: "cluster",
    name: "catalog-cluster",
    nativeKind: "kubernetes_cluster",
    nativeId: "catalog-cluster",
    environmentId: "env-gcp-staging",
    provider: "gcp",
    status: "healthy",
    labels: {
      location: "us-central1",
      resource_type: "kubernetes_cluster",
      status: "healthy"
    },
    ...assigned("resource_scope", resourceScopeOwnershipEvidence(evidence.cloudCatalogCluster)),
    evidenceIds: [evidence.cloudCatalogCluster]
  }
];

type EdgeSpec = {
  id: string;
  upstream: string;
  downstream: string;
  kind: TopologyEdgeKind;
  source: TopologySourceKind;
  sourceKey: string;
  confidence: number;
  metadata: Record<string, string>;
  evidenceIds: ConsoleEvidenceId[];
};

const edge = (
  id: string,
  upstream: string,
  downstream: string,
  kind: TopologyEdgeKind,
  source: TopologySourceKind,
  sourceKey: string,
  confidence: number,
  evidenceIds: ConsoleEvidenceId[]
): EdgeSpec => ({
  id,
  upstream,
  downstream,
  kind,
  source,
  sourceKey,
  confidence,
  metadata: { relationship: kind },
  evidenceIds
});

const edgeEnvContainsRds = edge(
  `edge:contains:${awsEnvironmentNodeId}:${rdsNodeId}:cloud`,
  awsEnvironmentNodeId,
  rdsNodeId,
  "contains",
  "cloud",
  "cloud",
  1,
  [evidence.environmentAws, evidence.cloudRds]
);
const edgeEnvContainsReplica = edge(
  `edge:contains:${awsEnvironmentNodeId}:${replicaNodeId}:cloud`,
  awsEnvironmentNodeId,
  replicaNodeId,
  "contains",
  "cloud",
  "cloud",
  1,
  [evidence.environmentAws, evidence.cloudReplica]
);
const edgeEnvContainsCluster = edge(
  `edge:contains:${gcpEnvironmentNodeId}:${catalogClusterNodeId}:cloud`,
  gcpEnvironmentNodeId,
  catalogClusterNodeId,
  "contains",
  "cloud",
  "cloud",
  1,
  [evidence.environmentGcp, evidence.cloudCatalogCluster]
);
const edgeEnvContainsProdNamespace = edge(
  `edge:contains:${awsEnvironmentNodeId}:${namespaceProdNodeId}:kubernetes:env-aws-prod`,
  awsEnvironmentNodeId,
  namespaceProdNodeId,
  "contains",
  "kubernetes",
  "kubernetes:env-aws-prod",
  1,
  [evidence.environmentAws, evidence.namespaceProd]
);
const edgeEnvContainsStagingNamespace = edge(
  `edge:contains:${gcpEnvironmentNodeId}:${namespaceStagingNodeId}:kubernetes:env-gcp-staging`,
  gcpEnvironmentNodeId,
  namespaceStagingNodeId,
  "contains",
  "kubernetes",
  "kubernetes:env-gcp-staging",
  1,
  [evidence.environmentGcp, evidence.namespaceStaging]
);
const edgeNamespaceContainsCheckout = edge(
  `edge:contains:${namespaceProdNodeId}:${checkoutNodeId}:kubernetes:env-aws-prod`,
  namespaceProdNodeId,
  checkoutNodeId,
  "contains",
  "kubernetes",
  "kubernetes:env-aws-prod",
  1,
  [evidence.namespaceProd, evidence.serviceCheckout]
);
const edgeNamespaceContainsCheckoutApi = edge(
  `edge:contains:${namespaceProdNodeId}:${checkoutApiNodeId}:kubernetes:env-aws-prod`,
  namespaceProdNodeId,
  checkoutApiNodeId,
  "contains",
  "kubernetes",
  "kubernetes:env-aws-prod",
  1,
  [evidence.namespaceProd, evidence.workloadCheckout]
);
const edgeNamespaceContainsCheckoutPod = edge(
  `edge:contains:${namespaceProdNodeId}:${checkoutPodNodeId}:kubernetes:env-aws-prod`,
  namespaceProdNodeId,
  checkoutPodNodeId,
  "contains",
  "kubernetes",
  "kubernetes:env-aws-prod",
  1,
  [evidence.namespaceProd, evidence.podCheckout]
);
const edgeNamespaceContainsUnassigned = edge(
  `edge:contains:${namespaceProdNodeId}:${unassignedWorkerNodeId}:kubernetes:env-aws-prod`,
  namespaceProdNodeId,
  unassignedWorkerNodeId,
  "contains",
  "kubernetes",
  "kubernetes:env-aws-prod",
  1,
  [evidence.namespaceProd, evidence.workloadUnassigned]
);
const edgeNamespaceContainsCatalog = edge(
  `edge:contains:${namespaceStagingNodeId}:${catalogNodeId}:kubernetes:env-gcp-staging`,
  namespaceStagingNodeId,
  catalogNodeId,
  "contains",
  "kubernetes",
  "kubernetes:env-gcp-staging",
  1,
  [evidence.namespaceStaging, evidence.serviceCatalog]
);
const edgeNamespaceContainsCatalogApi = edge(
  `edge:contains:${namespaceStagingNodeId}:${catalogApiNodeId}:kubernetes:env-gcp-staging`,
  namespaceStagingNodeId,
  catalogApiNodeId,
  "contains",
  "kubernetes",
  "kubernetes:env-gcp-staging",
  1,
  [evidence.namespaceStaging, evidence.workloadCatalog]
);
const edgeNamespaceContainsCatalogPod = edge(
  `edge:contains:${namespaceStagingNodeId}:${catalogPodNodeId}:kubernetes:env-gcp-staging`,
  namespaceStagingNodeId,
  catalogPodNodeId,
  "contains",
  "kubernetes",
  "kubernetes:env-gcp-staging",
  1,
  [evidence.namespaceStaging, evidence.podCatalog]
);
const edgeWorkloadOwnsCheckoutPod = edge(
  `edge:owns:${checkoutApiNodeId}:${checkoutPodNodeId}:kubernetes:env-aws-prod`,
  checkoutApiNodeId,
  checkoutPodNodeId,
  "owns",
  "kubernetes",
  "kubernetes:env-aws-prod",
  1,
  [evidence.workloadCheckout, evidence.podCheckout]
);
const edgeServiceSelectsCheckoutPod = edge(
  `edge:selects:${checkoutNodeId}:${checkoutPodNodeId}:kubernetes:env-aws-prod`,
  checkoutNodeId,
  checkoutPodNodeId,
  "selects",
  "kubernetes",
  "kubernetes:env-aws-prod",
  0.9,
  [evidence.serviceCheckout, evidence.podCheckout]
);
const edgeWorkloadOwnsCatalogPod = edge(
  `edge:owns:${catalogApiNodeId}:${catalogPodNodeId}:kubernetes:env-gcp-staging`,
  catalogApiNodeId,
  catalogPodNodeId,
  "owns",
  "kubernetes",
  "kubernetes:env-gcp-staging",
  1,
  [evidence.workloadCatalog, evidence.podCatalog]
);
const edgeServiceSelectsCatalogPod = edge(
  `edge:selects:${catalogNodeId}:${catalogPodNodeId}:kubernetes:env-gcp-staging`,
  catalogNodeId,
  catalogPodNodeId,
  "selects",
  "kubernetes",
  "kubernetes:env-gcp-staging",
  0.9,
  [evidence.serviceCatalog, evidence.podCatalog]
);
const edgeCheckoutDependsOnApi = edge(
  "edge:fixture:checkout-depends-on-api",
  checkoutNodeId,
  checkoutApiNodeId,
  "depends_on",
  "fixture",
  "fixture:topology",
  0.8,
  [evidence.edgeCheckoutApi]
);
const edgeApiDependsOnRds = edge(
  "edge:fixture:api-depends-on-rds",
  checkoutApiNodeId,
  rdsNodeId,
  "depends_on",
  "fixture",
  "fixture:topology",
  0.85,
  [evidence.edgeApiRds]
);
const edgeRdsDependsOnReplica = edge(
  "edge:fixture:rds-depends-on-replica",
  rdsNodeId,
  replicaNodeId,
  "depends_on",
  "fixture",
  "fixture:topology",
  0.7,
  [evidence.edgeRdsReplica]
);
const edgeReplicaDependsOnRds = edge(
  "edge:fixture:replica-depends-on-rds",
  replicaNodeId,
  rdsNodeId,
  "depends_on",
  "fixture",
  "fixture:topology",
  0.7,
  [evidence.edgeReplicaRds]
);

const baseEdgeSpecs = (): EdgeSpec[] => [
  edgeEnvContainsRds,
  edgeEnvContainsReplica,
  edgeEnvContainsCluster,
  edgeEnvContainsProdNamespace,
  edgeEnvContainsStagingNamespace,
  edgeNamespaceContainsCheckout,
  edgeNamespaceContainsCheckoutApi,
  edgeNamespaceContainsCheckoutPod,
  edgeNamespaceContainsUnassigned,
  edgeNamespaceContainsCatalog,
  edgeNamespaceContainsCatalogApi,
  edgeNamespaceContainsCatalogPod,
  edgeWorkloadOwnsCheckoutPod,
  edgeServiceSelectsCheckoutPod,
  edgeWorkloadOwnsCatalogPod,
  edgeServiceSelectsCatalogPod,
  edgeCheckoutDependsOnApi,
  edgeApiDependsOnRds,
  edgeRdsDependsOnReplica,
  edgeReplicaDependsOnRds
];

const buildEdge = (spec: EdgeSpec): TopologyEdge => {
  const evidenceIds = sortedUnique(spec.evidenceIds);
  return {
    id: spec.id,
    upstream_node_id: spec.upstream,
    downstream_node_id: spec.downstream,
    kind: spec.kind,
    provenance: [{ source: spec.source, source_key: spec.sourceKey, observed_at: observedAt }],
    confidence: spec.confidence,
    metadata: spec.metadata,
    evidence_ids: evidenceIds,
    drill_down: evidenceDrillDown(evidenceIds)
  };
};

type PathSpec = {
  rootId: string;
  terminalId: string;
  nodeIds: string[];
  edgeIds: string[];
  direction: TopologyDirection;
  confidence: number;
  termination: TopologyPathTermination;
  cycleEdgeId?: string;
};

const buildPath = (spec: PathSpec, nodes: TopologyNode[], edges: TopologyEdge[]): TopologyPath => {
  const nodesById = new Map(nodes.map((node) => [node.id, node]));
  const edgesById = new Map(edges.map((edge) => [edge.id, edge]));
  const evidenceIds = sortedUnique([
    ...spec.nodeIds.flatMap((id) => nodesById.get(id)?.evidence_ids ?? []),
    ...spec.edgeIds.flatMap((id) => edgesById.get(id)?.evidence_ids ?? []),
    ...(spec.cycleEdgeId ? (edgesById.get(spec.cycleEdgeId)?.evidence_ids ?? []) : [])
  ]);
  const depth = spec.edgeIds.length;
  const cycleKey = spec.cycleEdgeId ?? "none";
  return {
    id: `path:${spec.direction}:${spec.rootId}:${spec.edgeIds.join(",")}:${spec.termination}:${cycleKey}`,
    root_node_id: spec.rootId,
    terminal_node_id: spec.terminalId,
    node_ids: spec.nodeIds,
    edge_ids: spec.edgeIds,
    direction: spec.direction,
    depth,
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
    rootId: checkoutNodeId,
    terminalId: awsEnvironmentNodeId,
    nodeIds: [checkoutNodeId, namespaceProdNodeId, awsEnvironmentNodeId],
    edgeIds: [edgeNamespaceContainsCheckout.id, edgeEnvContainsProdNamespace.id],
    direction: "upstream",
    confidence: 1,
    termination: "leaf"
  },
  {
    rootId: checkoutNodeId,
    terminalId: checkoutPodNodeId,
    nodeIds: [checkoutNodeId, checkoutPodNodeId],
    edgeIds: [edgeServiceSelectsCheckoutPod.id],
    direction: "downstream",
    confidence: 0.9,
    termination: "leaf"
  },
  {
    rootId: checkoutNodeId,
    terminalId: checkoutPodNodeId,
    nodeIds: [checkoutNodeId, checkoutApiNodeId, checkoutPodNodeId],
    edgeIds: [edgeCheckoutDependsOnApi.id, edgeWorkloadOwnsCheckoutPod.id],
    direction: "downstream",
    confidence: 0.8,
    termination: "leaf"
  },
  {
    rootId: checkoutNodeId,
    terminalId: replicaNodeId,
    nodeIds: [checkoutNodeId, checkoutApiNodeId, rdsNodeId, replicaNodeId],
    edgeIds: [edgeCheckoutDependsOnApi.id, edgeApiDependsOnRds.id, edgeRdsDependsOnReplica.id],
    direction: "downstream",
    confidence: 0.7,
    termination: "cycle_detected",
    cycleEdgeId: edgeReplicaDependsOnRds.id
  }
];

const freshSource = (sourceKey: string, evidenceIds: ConsoleEvidenceId[]) => ({
  source_key: sourceKey,
  state: "fresh" as const,
  reason: null,
  detail: null,
  observed_at: observedAt,
  evidence_ids: sortedUnique(evidenceIds)
});

const allSourceStatus = () => [
  freshSource("cloud", [
    evidence.environmentAws,
    evidence.environmentGcp,
    evidence.cloudRds,
    evidence.cloudReplica,
    evidence.cloudCatalogCluster
  ]),
  freshSource("fixtures", [
    evidence.edgeApiRds,
    evidence.edgeCheckoutApi,
    evidence.edgeRdsReplica,
    evidence.edgeReplicaRds,
    evidence.ownershipPlatform,
    evidence.ownershipEnvironment,
    evidence.ownershipUnassigned,
    evidence.incidentCheckout,
    evidence.summary
  ]),
  freshSource("kubernetes:env-aws-prod", [
    evidence.namespaceProd,
    evidence.nodeWorker,
    evidence.podCheckout,
    evidence.serviceCheckout,
    evidence.workloadCheckout,
    evidence.workloadUnassigned
  ]),
  freshSource("kubernetes:env-gcp-staging", [
    evidence.namespaceStaging,
    evidence.podCatalog,
    evidence.serviceCatalog,
    evidence.workloadCatalog
  ]),
  freshSource("observability", [evidence.alertCheckout, evidence.metricCheckout])
];

const summaryMetric = (
  key: string,
  value: number,
  evidenceIds: ConsoleEvidenceId[]
): TopologyMetric => {
  const ids = sortedUnique(evidenceIds);
  return {
    key,
    value,
    unit: "count",
    evidence_ids: ids,
    drill_down: evidenceDrillDown(ids),
    drill_down_reference: reference(`topology:${key}`, ids)
  };
};

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
  nodes: nodes.slice().sort((left, right) => left.id.localeCompare(right.id)),
  edges: edges.slice().sort((left, right) => left.id.localeCompare(right.id)),
  paths: paths.slice().sort((left, right) => {
    const directionOrder = { upstream: 0, downstream: 1, both: 2 } as const;
    const directionDifference = directionOrder[left.direction] - directionOrder[right.direction];
    if (directionDifference !== 0) return directionDifference;
    if (left.depth !== right.depth) return left.depth - right.depth;
    if (left.termination !== right.termination) {
      return left.termination.localeCompare(right.termination);
    }
    const edgeDifference = left.edge_ids.join(",").localeCompare(right.edge_ids.join(","));
    if (edgeDifference !== 0) return edgeDifference;
    return left.id.localeCompare(right.id);
  }),
  source_status: sourceStatus
    .slice()
    .sort((left, right) => left.source_key.localeCompare(right.source_key)),
  evidence: fixtureEvidence()
});

const healthyGraph = () => {
  const nodes = baseNodeSpecs()
    .map(buildNode)
    .sort((left, right) => left.id.localeCompare(right.id));
  const edges = baseEdgeSpecs()
    .map(buildEdge)
    .sort((left, right) => left.id.localeCompare(right.id));
  const paths = basePathSpecs().map((spec) => buildPath(spec, nodes, edges));
  return { nodes, edges, paths };
};

/** Rust fixture graph focused on the checkout service. */
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

/** AWS sources healthy, the GCP Kubernetes source unavailable: a partial view. */
export const topologyDegradedSnapshotFixture: TopologySnapshot = (() => {
  const graph = healthyGraph();
  const nodes = graph.nodes.filter((node) => node.environment_id !== "env-gcp-staging");
  const visibleIds = new Set(nodes.map((node) => node.id));
  const edges = graph.edges.filter(
    (item) => visibleIds.has(item.upstream_node_id) && visibleIds.has(item.downstream_node_id)
  );
  const paths = graph.paths.filter((path) =>
    path.node_ids.every((nodeId) => visibleIds.has(nodeId))
  );
  return buildSnapshot({
    nodes,
    edges,
    paths,
    incidentId: null,
    focusNodeId: checkoutNodeId,
    sourceStatus: [
      freshSource("cloud", [
        evidence.environmentAws,
        evidence.environmentGcp,
        evidence.cloudRds,
        evidence.cloudReplica,
        evidence.cloudCatalogCluster
      ]),
      freshSource("fixtures", [
        evidence.edgeApiRds,
        evidence.edgeCheckoutApi,
        evidence.edgeRdsReplica,
        evidence.edgeReplicaRds,
        evidence.ownershipPlatform,
        evidence.ownershipEnvironment,
        evidence.ownershipUnassigned,
        evidence.incidentCheckout,
        evidence.summary
      ]),
      freshSource("kubernetes:env-aws-prod", [
        evidence.namespaceProd,
        evidence.nodeWorker,
        evidence.podCheckout,
        evidence.serviceCheckout,
        evidence.workloadCheckout,
        evidence.workloadUnassigned
      ]),
      {
        source_key: "kubernetes:env-gcp-staging",
        state: "unavailable",
        reason: "unreachable",
        detail: "fixture gcp staging session unavailable",
        observed_at: observedAt,
        evidence_ids: []
      },
      freshSource("observability", [evidence.alertCheckout, evidence.metricCheckout])
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
    sourceStatus: [freshSource("fixtures", [])]
  });
  return {
    ...snapshot,
    summary: {
      visible_nodes: summaryMetric("visible_nodes", 0, []),
      visible_edges: summaryMetric("visible_edges", 0, []),
      affected_nodes: summaryMetric("affected_nodes", 0, []),
      probable_paths: summaryMetric("probable_paths", 0, [])
    },
    evidence: [
      {
        id: emptyEvidenceId,
        source_kind: "fixture",
        connector_id: "fixture-topology",
        scope,
        endpoint: "fixture://topology",
        query: emptyEvidenceId,
        observed_at: observedAt,
        excerpt: "empty topology records",
        native_url: null,
        redaction: {
          classification_verified: true,
          redaction_verified: true,
          masked: false,
          unparsed: false
        }
      }
    ]
  };
})();

/** Deliberately malformed response used to exercise the UI error boundary. */
export const topologyBrokenPathSnapshotFixture: TopologySnapshot = (() => {
  const graph = healthyGraph();
  const nodes = graph.nodes.filter((node) => node.id !== awsEnvironmentNodeId);
  const visibleIds = new Set(nodes.map((node) => node.id));
  const edges = graph.edges.filter(
    (item) => visibleIds.has(item.upstream_node_id) && visibleIds.has(item.downstream_node_id)
  );
  const downstreamPaths = graph.paths.filter((path) => path.direction === "downstream");
  return {
    ...buildSnapshot({
      nodes,
      edges,
      paths: downstreamPaths,
      incidentId: null,
      focusNodeId: checkoutNodeId,
      sourceStatus: allSourceStatus()
    }),
    paths: [...downstreamPaths, graph.paths[0]]
  };
})();

/** Sprint 11 queue item available to the Incident filter. */
export const topologyIncidentsFixture: IncidentQueueItem[] = [
  {
    id: "alert-checkout-s1",
    title: "Checkout unavailable",
    source_kind: "alert",
    source_id: "alert-checkout-s1",
    severity: "S1",
    priority: "P1",
    status: "detected",
    business_impact: {
      level: "critical",
      summary: "Checkout requests are failing",
      customer_scope: "production checkout customers",
      service_criticality: "tier-0",
      trajectory: "expanding"
    },
    scope,
    detected_at: "2026-08-28T08:55:00Z",
    opened_at: "2026-08-28T08:55:00Z",
    last_update: "2026-08-28T08:59:00Z",
    affected_scope: { ...scope, resource_ids: [checkoutResourceId] },
    evidence_ids: [evidence.incidentCheckout],
    drill_down: {
      destination: "incident_queue",
      evidence_ids: [evidence.incidentCheckout],
      filter_key: "alert-checkout-s1"
    },
    drill_down_reference: reference("incident:alert-checkout-s1", [evidence.incidentCheckout])
  }
];

export const topologyFixtureNodeIds = {
  checkout: checkoutNodeId,
  checkoutApi: checkoutApiNodeId,
  checkoutPod: checkoutPodNodeId,
  unassignedWorker: unassignedWorkerNodeId,
  worker: workerNodeId,
  rds: rdsNodeId,
  replica: replicaNodeId,
  catalog: catalogNodeId,
  catalogApi: catalogApiNodeId,
  catalogPod: catalogPodNodeId,
  catalogCluster: catalogClusterNodeId,
  namespaceProd: namespaceProdNodeId,
  namespaceStaging: namespaceStagingNodeId,
  awsEnvironment: awsEnvironmentNodeId,
  gcpEnvironment: gcpEnvironmentNodeId
};
