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
  isNullableSafeDisplayText,
  isRecord,
  isSafeDisplayText,
  isSafeStringArray,
  isScope,
  isSourceStatus
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

const isNonEmptySafeStringArray = (value: unknown): value is string[] =>
  isSafeStringArray(value) && value.length > 0;

const sharesEvidence = (left: string[], right: string[]) => left.some((id) => right.includes(id));

const isTopologyDrillDown = (value: unknown, evidenceIds: string[]) =>
  isDrillDownTarget(value) &&
  value.destination === "topology" &&
  (value.filter_key === null || isSafeDisplayText(value.filter_key)) &&
  isNonEmptySafeStringArray(value.evidence_ids) &&
  sharesEvidence(evidenceIds, value.evidence_ids);

const isNodeTopologyDrillDown = (value: unknown, evidenceIds: string[], nodeId: string) =>
  isTopologyDrillDown(value, evidenceIds) && value.filter_key === nodeId;

const isEvidenceDrillDown = (value: unknown, evidenceIds: string[]) =>
  isDrillDownTarget(value) &&
  value.destination === "evidence" &&
  value.filter_key === null &&
  isNonEmptySafeStringArray(value.evidence_ids) &&
  sharesEvidence(evidenceIds, value.evidence_ids);

const isOwnership = (value: unknown): value is TopologyOwnership => {
  if (
    !isRecord(value) ||
    !isEnum(value.source, ownershipSources) ||
    !Array.isArray(value.evidence_ids) ||
    !value.evidence_ids.every(isSafeDisplayText)
  ) {
    return false;
  }
  if (value.source === "unassigned") {
    return value.team_id === null && value.team_name === null;
  }
  return (
    isSafeDisplayText(value.team_id) &&
    isSafeDisplayText(value.team_name) &&
    isNonEmptySafeStringArray(value.evidence_ids)
  );
};

const isMetricFor = (
  value: unknown,
  destination: "topology" | "evidence"
): value is TopologyMetric => {
  if (
    !isRecord(value) ||
    !isSafeDisplayText(value.key) ||
    typeof value.value !== "number" ||
    !Number.isFinite(value.value) ||
    (value.unit === "count" && value.value < 0) ||
    !isEnum(value.unit, numberUnits) ||
    !isDrillDownReference(value.drill_down_reference) ||
    !isSafeDisplayText(value.drill_down_reference.source_query)
  ) {
    return false;
  }
  if (
    destination === "evidence" &&
    value.value === 0 &&
    Array.isArray(value.evidence_ids) &&
    value.evidence_ids.length === 0
  ) {
    return (
      isDrillDownTarget(value.drill_down) &&
      value.drill_down.destination === "evidence" &&
      value.drill_down.filter_key === null &&
      value.drill_down.evidence_ids.length === 0 &&
      value.drill_down_reference.evidence_ids.length === 0
    );
  }
  if (
    !isNonEmptySafeStringArray(value.evidence_ids) ||
    !(destination === "topology"
      ? isTopologyDrillDown(value.drill_down, value.evidence_ids)
      : isEvidenceDrillDown(value.drill_down, value.evidence_ids))
  ) {
    return false;
  }
  return (
    isNonEmptySafeStringArray(value.drill_down_reference.evidence_ids) &&
    sharesEvidence(value.evidence_ids, value.drill_down_reference.evidence_ids)
  );
};

const isMetric = (value: unknown): value is TopologyMetric => isMetricFor(value, "topology");

const isSummaryMetric = (value: unknown): value is TopologyMetric => isMetricFor(value, "evidence");

const isNode = (value: unknown): value is TopologyNode => {
  if (
    !isRecord(value) ||
    !isSafeDisplayText(value.id) ||
    !isEnum(value.kind, nodeKinds) ||
    !isSafeDisplayText(value.name) ||
    !isNullableSafeDisplayText(value.native_kind) ||
    !isNullableSafeDisplayText(value.native_id) ||
    !isNullableSafeDisplayText(value.environment_id) ||
    !isNullableSafeDisplayText(value.provider) ||
    !isScope(value.scope) ||
    !isEnum(value.status, ["healthy", "degraded", "critical", "unknown"]) ||
    !isRecord(value.labels) ||
    !Object.entries(value.labels).every(
      ([key, label]) => isSafeDisplayText(key) && isSafeDisplayText(label)
    ) ||
    !isOwnership(value.ownership) ||
    (value.metric !== null && !isMetric(value.metric)) ||
    !isBoolean(value.affected_by_incident) ||
    !isNonEmptySafeStringArray(value.evidence_ids)
  ) {
    return false;
  }
  return isNodeTopologyDrillDown(value.drill_down, value.evidence_ids, value.id);
};

const isEdge = (value: unknown): value is TopologyEdge => {
  if (
    !isRecord(value) ||
    !isSafeDisplayText(value.id) ||
    !isSafeDisplayText(value.upstream_node_id) ||
    !isSafeDisplayText(value.downstream_node_id) ||
    value.upstream_node_id === value.downstream_node_id ||
    !isEnum(value.kind, edgeKinds) ||
    !Array.isArray(value.provenance) ||
    value.provenance.length === 0 ||
    !value.provenance.every(
      (item) =>
        isRecord(item) &&
        isEnum(item.source, sourceKinds) &&
        isSafeDisplayText(item.source_key) &&
        (item.observed_at === null || isSafeDisplayText(item.observed_at))
    ) ||
    !isConfidence(value.confidence) ||
    !isRecord(value.metadata) ||
    !Object.entries(value.metadata).every(
      ([key, entry]) => isSafeDisplayText(key) && isSafeDisplayText(entry)
    ) ||
    !isNonEmptySafeStringArray(value.evidence_ids)
  ) {
    return false;
  }
  const provenance = value.provenance as Array<{ source: string; source_key: string }>;
  const provenanceIdentity = new Set(
    provenance.map((item) => JSON.stringify([item.source, item.source_key]))
  );
  if (provenanceIdentity.size !== provenance.length) return false;
  return isEvidenceDrillDown(value.drill_down, value.evidence_ids);
};

const isPath = (value: unknown): value is TopologyPath => {
  if (
    !isRecord(value) ||
    !isSafeDisplayText(value.id) ||
    !isSafeDisplayText(value.root_node_id) ||
    !isSafeDisplayText(value.terminal_node_id) ||
    !isNonEmptySafeStringArray(value.node_ids) ||
    !isSafeStringArray(value.edge_ids) ||
    !isEnum(value.direction, ["upstream", "downstream", "both"]) ||
    typeof value.depth !== "number" ||
    !Number.isSafeInteger(value.depth) ||
    value.depth < 0 ||
    value.depth > 8 ||
    value.node_ids.length !== value.edge_ids.length + 1 ||
    value.depth !== value.edge_ids.length ||
    value.node_ids[0] !== value.root_node_id ||
    value.node_ids[value.node_ids.length - 1] !== value.terminal_node_id ||
    new Set(value.node_ids).size !== value.node_ids.length ||
    !isConfidence(value.confidence) ||
    value.kind !== "probable_structural" ||
    !isEnum(value.termination, terminations) ||
    (value.termination === "cycle_detected"
      ? !isSafeDisplayText(value.cycle_edge_id)
      : value.cycle_edge_id !== null) ||
    !isNonEmptySafeStringArray(value.evidence_ids)
  ) {
    return false;
  }
  return isEvidenceDrillDown(value.drill_down, value.evidence_ids);
};

const isTraversal = (value: unknown): value is TopologyTraversal =>
  isRecord(value) &&
  isEnum(value.direction, ["upstream", "downstream", "both"]) &&
  typeof value.max_depth === "number" &&
  Number.isSafeInteger(value.max_depth) &&
  value.max_depth >= 0 &&
  value.max_depth <= 8;

const isFilter = (value: unknown): value is TopologyFilter =>
  isRecord(value) &&
  isSafeStringArray(value.environment_ids) &&
  isSafeStringArray(value.team_ids) &&
  (value.incident_id === null || isSafeDisplayText(value.incident_id));

const isSummary = (value: unknown) =>
  isRecord(value) &&
  isSummaryMetric(value.visible_nodes) &&
  isSummaryMetric(value.visible_edges) &&
  isSummaryMetric(value.affected_nodes) &&
  isSummaryMetric(value.probable_paths);

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
  if (
    snapshot.summary.visible_nodes.value !== snapshot.nodes.length ||
    snapshot.summary.visible_edges.value !== snapshot.edges.length ||
    snapshot.summary.affected_nodes.value !==
      snapshot.nodes.filter((node) => node.affected_by_incident).length ||
    snapshot.summary.probable_paths.value !== snapshot.paths.length
  ) {
    return false;
  }
  const evidenceIds = new Set(snapshot.evidence.map((item: EvidenceRef) => item.id));
  if (evidenceIds.size !== snapshot.evidence.length) return false;
  const sourceStatusKeys = new Set(snapshot.source_status.map((status) => status.source_key));
  if (sourceStatusKeys.size !== snapshot.source_status.length) return false;
  if (
    new Set(snapshot.nodes.map((node) => node.id)).size !== snapshot.nodes.length ||
    new Set(snapshot.edges.map((edge) => edge.id)).size !== snapshot.edges.length ||
    new Set(snapshot.paths.map((path) => path.id)).size !== snapshot.paths.length
  ) {
    return false;
  }
  const nodeIds = new Set(snapshot.nodes.map((node) => node.id));
  const edgeById = new Map(snapshot.edges.map((edge) => [edge.id, edge]));
  if (snapshot.focus_node_id !== null && !nodeIds.has(snapshot.focus_node_id)) {
    return false;
  }
  if (
    snapshot.edges.some(
      (edge) => !nodeIds.has(edge.upstream_node_id) || !nodeIds.has(edge.downstream_node_id)
    )
  ) {
    return false;
  }
  if (
    snapshot.paths.some((path) => {
      if (
        path.node_ids.some((nodeId) => !nodeIds.has(nodeId)) ||
        path.edge_ids.some((edgeId) => !edgeById.has(edgeId))
      ) {
        return true;
      }
      const pathEdgeIds = [...path.edge_ids, ...(path.cycle_edge_id ? [path.cycle_edge_id] : [])];
      const expectedConfidence = pathEdgeIds.reduce(
        (minimum, edgeId) => Math.min(minimum, edgeById.get(edgeId)?.confidence ?? 1),
        1
      );
      if (path.confidence !== expectedConfidence) return true;
      const expectedEvidence = new Set<ConsoleEvidenceId>();
      for (const nodeId of path.node_ids) {
        const node = snapshot.nodes.find((candidate) => candidate.id === nodeId);
        if (!node) return true;
        node.evidence_ids.forEach((id) => expectedEvidence.add(id));
      }
      for (const edgeId of path.edge_ids) {
        const edge = edgeById.get(edgeId);
        if (!edge) return true;
        edge.evidence_ids.forEach((id) => expectedEvidence.add(id));
      }
      if (path.cycle_edge_id !== null) {
        const cycleEdge = edgeById.get(path.cycle_edge_id);
        if (!cycleEdge) return true;
        cycleEdge.evidence_ids.forEach((id) => expectedEvidence.add(id));
      }
      const actualEvidence = new Set(path.evidence_ids);
      if (
        actualEvidence.size !== path.evidence_ids.length ||
        actualEvidence.size !== expectedEvidence.size ||
        [...expectedEvidence].some((id) => !actualEvidence.has(id)) ||
        [...actualEvidence].some((id) => !expectedEvidence.has(id)) ||
        JSON.stringify([...path.evidence_ids]) !== JSON.stringify([...expectedEvidence].sort())
      ) {
        return true;
      }
      for (const [index, edgeId] of path.edge_ids.entries()) {
        const edge = edgeById.get(edgeId);
        if (!edge) return true;
        const from = path.node_ids[index];
        const to = path.node_ids[index + 1];
        const followsDirection =
          path.direction === "downstream"
            ? edge.upstream_node_id === from && edge.downstream_node_id === to
            : path.direction === "upstream"
              ? edge.downstream_node_id === from && edge.upstream_node_id === to
              : (edge.upstream_node_id === from && edge.downstream_node_id === to) ||
                (edge.downstream_node_id === from && edge.upstream_node_id === to);
        if (!followsDirection) return true;
      }
      if (path.cycle_edge_id === null) return false;
      const cycleEdge = edgeById.get(path.cycle_edge_id);
      if (!cycleEdge || path.edge_ids.includes(path.cycle_edge_id)) return true;
      const terminal = path.terminal_node_id;
      const closesCycle =
        path.direction === "downstream"
          ? cycleEdge.upstream_node_id === terminal &&
            path.node_ids.includes(cycleEdge.downstream_node_id)
          : path.direction === "upstream"
            ? cycleEdge.downstream_node_id === terminal &&
              path.node_ids.includes(cycleEdge.upstream_node_id)
            : (cycleEdge.upstream_node_id === terminal &&
                path.node_ids.includes(cycleEdge.downstream_node_id)) ||
              (cycleEdge.downstream_node_id === terminal &&
                path.node_ids.includes(cycleEdge.upstream_node_id));
      return !closesCycle;
    })
  ) {
    return false;
  }
  return referencesIssuedEvidence(snapshot, evidenceIds);
};
