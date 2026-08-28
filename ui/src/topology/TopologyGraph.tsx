import type {
  ConsoleEvidenceId,
  TopologyEdge,
  TopologyEdgeKind,
  TopologyNode
} from "../../contracts/ipc";
import { EmptyState, StatusIndicator, Table } from "../design-system/components";
import { useTranslation } from "../i18n";

const healthIndicatorState = (state: TopologyNode["status"]) => {
  if (state === "healthy") return "healthy" as const;
  if (state === "critical") return "critical" as const;
  if (state === "degraded") return "degraded" as const;
  return "unavailable" as const;
};

/** Decorative relation glyphs; the typed text label carries the meaning. */
const relationSymbol: Record<TopologyEdgeKind, string> = {
  contains: "▣",
  owns: "◆",
  selects: "◈",
  routes_to: "➔",
  runs_on: "⬢",
  depends_on: "⇢"
};

export function TopologyGraph({
  nodes,
  edges,
  selectedNodeId,
  onSelectNode,
  onOpenEvidence
}: {
  nodes: TopologyNode[];
  edges: TopologyEdge[];
  selectedNodeId: string | null;
  onSelectNode: (nodeId: string) => void;
  onOpenEvidence: (evidenceIds: ConsoleEvidenceId[], subject: string) => void;
}) {
  const { t } = useTranslation();
  const nodesById = new Map(nodes.map((node) => [node.id, node]));
  const nodeName = (nodeId: string) => nodesById.get(nodeId)?.name ?? nodeId;

  return (
    <div className="topology-graph">
      <section className="topology-graph__nodes" aria-label={t("topology.graph.nodesTitle")}>
        <h2>{t("topology.graph.nodesTitle")}</h2>
        {nodes.length === 0 ? (
          <EmptyState titleKey="topology.graph.emptyNodes" />
        ) : (
          <ul className="topology-graph__node-list">
            {nodes.map((node) => {
              const indicatorState = healthIndicatorState(node.status);
              const ownerLabel = node.ownership.team_name ?? t("topology.graph.unassigned");
              const selectLabel = [
                node.name,
                t(`topology.kinds.${node.kind}`),
                t(`status.${indicatorState}`),
                node.affected_by_incident ? t("topology.graph.affected") : null,
                ownerLabel
              ]
                .filter(Boolean)
                .join(", ");
              return (
                <li key={node.id} className="topology-graph__node">
                  <button
                    type="button"
                    className="topology-graph__node-select"
                    aria-pressed={selectedNodeId === node.id}
                    aria-label={selectLabel}
                    onClick={() => onSelectNode(node.id)}
                  >
                    <span className="topology-graph__node-name">{node.name}</span>
                    <span className="topology-graph__node-kind">
                      {t(`topology.kinds.${node.kind}`)}
                    </span>
                    <StatusIndicator state={indicatorState} />
                    {node.affected_by_incident && (
                      <span className="topology-graph__node-affected">
                        <span aria-hidden="true">▲</span> {t("topology.graph.affected")}
                      </span>
                    )}
                    <span className="topology-graph__node-owner">{ownerLabel}</span>
                  </button>
                  <button
                    type="button"
                    className="topology-graph__node-evidence"
                    aria-label={t("topology.graph.viewEvidence", { name: node.name })}
                    onClick={() => onOpenEvidence(node.evidence_ids, node.name)}
                  >
                    {t("topology.graph.evidence")}
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </section>
      <section className="topology-graph__edges" aria-label={t("topology.graph.edgesTitle")}>
        <h2>{t("topology.graph.edgesTitle")}</h2>
        {edges.length === 0 ? (
          <EmptyState titleKey="topology.graph.emptyEdges" />
        ) : (
          <Table
            captionKey="topology.graph.edgesCaption"
            columns={[
              { key: "upstream", headerKey: "topology.graph.upstream" },
              { key: "relation", headerKey: "topology.graph.relation" },
              { key: "downstream", headerKey: "topology.graph.downstream" },
              { key: "confidence", headerKey: "topology.graph.confidence" },
              { key: "provenance", headerKey: "topology.graph.provenance" },
              { key: "evidence", headerKey: "topology.graph.evidenceColumn" }
            ]}
            rows={edges.map((edge) => ({
              id: edge.id,
              upstream: nodeName(edge.upstream_node_id),
              relation: (
                <span className="topology-graph__relation">
                  <span aria-hidden="true">{relationSymbol[edge.kind]}</span>{" "}
                  {t(`topology.relations.${edge.kind}`)}
                </span>
              ),
              downstream: nodeName(edge.downstream_node_id),
              confidence: t("topology.graph.confidenceValue", {
                value: Math.round(edge.confidence * 100)
              }),
              provenance:
                edge.provenance.length > 0
                  ? edge.provenance.map((item) => item.source_key).join(", ")
                  : t("topology.graph.provenanceUnavailable"),
              evidence: (
                <button
                  type="button"
                  aria-label={t("topology.graph.viewEdgeEvidence", {
                    upstream: nodeName(edge.upstream_node_id),
                    downstream: nodeName(edge.downstream_node_id)
                  })}
                  onClick={() =>
                    onOpenEvidence(
                      edge.evidence_ids,
                      `${nodeName(edge.upstream_node_id)} → ${nodeName(edge.downstream_node_id)}`
                    )
                  }
                >
                  {t("topology.graph.evidence")}
                </button>
              )
            }))}
          />
        )}
      </section>
    </div>
  );
}
