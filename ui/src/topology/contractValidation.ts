import type {
  ConsoleEvidenceId,
  EvidenceRef,
  TopologyEdge,
  TopologyEdgeKind,
  TopologyFilter,
  TopologyMetric,
  TopologyNode,
  TopologyNodeKind,
  TopologyOwnership,
  TopologyOwnershipSource,
  TopologyPath,
  TopologyPathTermination,
  TopologySnapshot,
  TopologySourceKind,
  TopologyTraversal
} from "../../contracts/ipc";
import {
  isBoolean,
  isDrillDownReference,
  isDrillDownTarget,
  isEnum,
  isEvidence,
  isNonEmptyString,
  isNullableString,
  isRecord,
  isScope,
  isSourceStatus,
  isStringArray
} from "../../contracts/guards";

const nodeKinds: TopologyNodeKind[] = [
  "environment",
  "cluster",
  "namespace",
  "workload",
  "service",
  "pod",
  "node",
  "cloud_resource",
  "observability_target"
];
const ownershipSources: TopologyOwnershipSource[] = [
  "explicit_label",
  "resource_scope",
  "environment_default",
  "fixture",
  "unassigned"
];
const edgeKinds: TopologyEdgeKind[] = [
  "contains",
  "owns",
  "selects",
  "routes_to",
  "runs_on",
  "depends_on"
];
const sourceKinds: TopologySourceKind[] = ["kubernetes", "cloud", "observability", "fixture"];
const terminations: TopologyPathTermination[] = ["leaf", "cycle_detected", "depth_limit"];
const numberUnits = ["count", "percentage", "milliseconds", "seconds"] as const;

const isConfidence = (value: unknown) =>
  typeof value === "number" && Number.isFinite(value) && value >= 0 && value <= 1;

const isOwnership = (value: unknown): value is TopologyOwnership =>
  isRecord(value) &&
  (value.team_id === null || isNonEmptyString(value.team_id)) &&
  (value.team_name === null || isNonEmptyString(value.team_name)) &&
  isEnum(value.source, ownershipSources) &&
  isStringArray(value.evidence_ids);

const isMetric = (value: unknown): value is TopologyMetric =>
  isRecord(value) &&
  isNonEmptyString(value.key) &&
  typeof value.value === "number" &&
  Number.isFinite(value.value) &&
  isEnum(value.unit, numberUnits) &&
  isStringArray(value.evidence_ids) &&
  isDrillDownTarget(value.drill_down) &&
  isDrillDownReference(value.drill_down_reference);

const isNode = (value: unknown): value is TopologyNode =>
  isRecord(value) &&
  isNonEmptyString(value.id) &&
  isEnum(value.kind, nodeKinds) &&
  isNonEmptyString(value.name) &&
  isNullableString(value.native_kind) &&
  isNullableString(value.native_id) &&
  isNullableString(value.environment_id) &&
  isNullableString(value.provider) &&
  isScope(value.scope) &&
  isEnum(value.status, ["healthy", "degraded", "critical", "unknown"]) &&
  isRecord(value.labels) &&
  Object.values(value.labels).every((label) => typeof label === "string") &&
  isOwnership(value.ownership) &&
  (value.metric === null || isMetric(value.metric)) &&
  isBoolean(value.affected_by_incident) &&
  isStringArray(value.evidence_ids) &&
  isDrillDownTarget(value.drill_down);

const isEdge = (value: unknown): value is TopologyEdge =>
  isRecord(value) &&
  isNonEmptyString(value.id) &&
  isNonEmptyString(value.upstream_node_id) &&
  isNonEmptyString(value.downstream_node_id) &&
  isEnum(value.kind, edgeKinds) &&
  Array.isArray(value.provenance) &&
  value.provenance.every(
    (item) =>
      isRecord(item) &&
      isEnum(item.source, sourceKinds) &&
      isNonEmptyString(item.source_key) &&
      (item.observed_at === null || isNonEmptyString(item.observed_at))
  ) &&
  isConfidence(value.confidence) &&
  isRecord(value.metadata) &&
  Object.values(value.metadata).every((entry) => typeof entry === "string") &&
  isStringArray(value.evidence_ids) &&
  isDrillDownTarget(value.drill_down);

const isPath = (value: unknown): value is TopologyPath =>
  isRecord(value) &&
  isNonEmptyString(value.id) &&
  isNonEmptyString(value.root_node_id) &&
  isNonEmptyString(value.terminal_node_id) &&
  isStringArray(value.node_ids) &&
  isStringArray(value.edge_ids) &&
  isEnum(value.direction, ["upstream", "downstream", "both"]) &&
  typeof value.depth === "number" &&
  Number.isSafeInteger(value.depth) &&
  value.depth >= 0 &&
  isConfidence(value.confidence) &&
  value.kind === "probable_structural" &&
  isEnum(value.termination, terminations) &&
  (value.cycle_edge_id === null || isNonEmptyString(value.cycle_edge_id)) &&
  isStringArray(value.evidence_ids) &&
  isDrillDownTarget(value.drill_down);

const isTraversal = (value: unknown): value is TopologyTraversal =>
  isRecord(value) &&
  isEnum(value.direction, ["upstream", "downstream", "both"]) &&
  typeof value.max_depth === "number" &&
  Number.isSafeInteger(value.max_depth) &&
  value.max_depth >= 0 &&
  value.max_depth <= 8;

const isFilter = (value: unknown): value is TopologyFilter =>
  isRecord(value) &&
  isStringArray(value.environment_ids) &&
  isStringArray(value.team_ids) &&
  (value.incident_id === null || isNonEmptyString(value.incident_id));

const isSummary = (value: unknown) =>
  isRecord(value) &&
  isMetric(value.visible_nodes) &&
  isMetric(value.visible_edges) &&
  isMetric(value.affected_nodes) &&
  isMetric(value.probable_paths);

const referencesIssuedEvidence = (
  snapshot: TopologySnapshot,
  evidenceIds: Set<ConsoleEvidenceId>
) => {
  const references = [
    ...snapshot.nodes.flatMap((node) => [
      node.evidence_ids,
      node.ownership.evidence_ids,
      ...(node.metric
        ? [
            node.metric.evidence_ids,
            node.metric.drill_down.evidence_ids,
            node.metric.drill_down_reference.evidence_ids
          ]
        : []),
      node.drill_down.evidence_ids
    ]),
    ...snapshot.edges.flatMap((edge) => [edge.evidence_ids, edge.drill_down.evidence_ids]),
    ...snapshot.paths.flatMap((path) => [path.evidence_ids, path.drill_down.evidence_ids]),
    ...snapshot.source_status.map((status) => status.evidence_ids),
    ...[
      snapshot.summary.visible_nodes,
      snapshot.summary.visible_edges,
      snapshot.summary.affected_nodes,
      snapshot.summary.probable_paths
    ].flatMap((metric) => [
      metric.evidence_ids,
      metric.drill_down.evidence_ids,
      metric.drill_down_reference.evidence_ids
    ])
  ];
  return references.every((ids) => ids.every((id) => evidenceIds.has(id)));
};

/**
 * Contract check for the `topology.snapshot` IPC response.  A response that
 * fails this check renders the localized error state, never a partial view.
 */
export const isTopologySnapshot = (value: unknown): value is TopologySnapshot => {
  if (!isRecord(value)) return false;
  if (
    !isNonEmptyString(value.generated_at) ||
    !isScope(value.scope) ||
    !isFilter(value.filter) ||
    (value.focus_node_id !== null && !isNonEmptyString(value.focus_node_id)) ||
    !isTraversal(value.traversal) ||
    !isSummary(value.summary) ||
    !Array.isArray(value.nodes) ||
    !value.nodes.every(isNode) ||
    !Array.isArray(value.edges) ||
    !value.edges.every(isEdge) ||
    !Array.isArray(value.paths) ||
    !value.paths.every(isPath) ||
    !Array.isArray(value.source_status) ||
    !value.source_status.every(isSourceStatus) ||
    !Array.isArray(value.evidence) ||
    !value.evidence.every(isEvidence)
  ) {
    return false;
  }

  const snapshot = value as TopologySnapshot;
  const evidenceIds = new Set(snapshot.evidence.map((item: EvidenceRef) => item.id));
  if (evidenceIds.size !== snapshot.evidence.length) return false;
  if (
    new Set(snapshot.nodes.map((node) => node.id)).size !== snapshot.nodes.length ||
    new Set(snapshot.edges.map((edge) => edge.id)).size !== snapshot.edges.length ||
    new Set(snapshot.paths.map((path) => path.id)).size !== snapshot.paths.length
  ) {
    return false;
  }
  return referencesIssuedEvidence(snapshot, evidenceIds);
};
