import type {
  ConsoleEvidenceId,
  TopologyEdge,
  TopologyNode,
  TopologyPath,
  TopologyPathTermination
} from "../../contracts/ipc";
import { EmptyState } from "../design-system/components";
import { useTranslation } from "../i18n";

/** Decorative termination glyphs; the typed text label carries the meaning. */
const terminationSymbol: Record<TopologyPathTermination, string> = {
  leaf: "⏹",
  cycle_detected: "↻",
  depth_limit: "…"
};

export function TopologyPathList({
  paths,
  nodesById,
  edgesById,
  focusName,
  incidentMode,
  onOpenEvidence
}: {
  paths: TopologyPath[];
  nodesById: Map<string, TopologyNode>;
  edgesById: Map<string, TopologyEdge>;
  focusName: string | null;
  incidentMode: boolean;
  onOpenEvidence: (evidenceIds: ConsoleEvidenceId[], subject: string) => void;
}) {
  const { t } = useTranslation();
  const upstream = paths.filter((path) => path.direction === "upstream");
  const downstream = paths.filter((path) => path.direction === "downstream");

  const title = incidentMode
    ? t("topology.paths.fromIncident")
    : focusName
      ? t("topology.paths.fromSelection", { name: focusName })
      : t("topology.paths.title");

  const isRenderable = (path: TopologyPath) =>
    path.node_ids.every((id) => nodesById.has(id)) &&
    path.edge_ids.every((id) => edgesById.has(id)) &&
    (path.cycle_edge_id === null || edgesById.has(path.cycle_edge_id));

  const renderPath = (path: TopologyPath) => {
    const names = path.node_ids.map((id) => nodesById.get(id)?.name ?? id);
    const subject = names.join(" → ");
    if (!isRenderable(path)) {
      return (
        <li key={path.id} className="topology-path topology-path--error">
          <span role="alert">{t("topology.paths.error")}</span>
          <button
            type="button"
            aria-label={t("topology.paths.viewEvidence")}
            onClick={() => onOpenEvidence(path.evidence_ids, path.id)}
          >
            {t("topology.graph.evidence")}
          </button>
        </li>
      );
    }
    return (
      <li key={path.id} className="topology-path">
        <div className="topology-path__summary">
          <span className="topology-path__kind">{t("topology.paths.probable")}</span>
          <span className="topology-path__sequence">{subject}</span>
          <span
            className={`topology-path__termination topology-path__termination--${path.termination}`}
          >
            <span aria-hidden="true">{terminationSymbol[path.termination]}</span>{" "}
            {t(`topology.paths.termination_${path.termination}`)}
          </span>
        </div>
        <dl className="topology-path__facts">
          <div>
            <dt>{t("topology.paths.depth")}</dt>
            <dd>{path.depth}</dd>
          </div>
          <div>
            <dt>{t("topology.paths.confidence")}</dt>
            <dd>
              {t("topology.graph.confidenceValue", { value: Math.round(path.confidence * 100) })}
            </dd>
          </div>
        </dl>
        {path.termination === "depth_limit" && (
          <p className="topology-path__note">{t("topology.paths.depthLimitNote")}</p>
        )}
        <button
          type="button"
          aria-label={t("topology.paths.viewEvidenceFor", { subject })}
          onClick={() => onOpenEvidence(path.evidence_ids, subject)}
        >
          {t("topology.graph.evidence")}
        </button>
      </li>
    );
  };

  return (
    <section className="topology-pathlist" aria-label={t("topology.paths.title")}>
      <h2>{title}</h2>
      {paths.length === 0 ? (
        <EmptyState titleKey="topology.paths.empty" />
      ) : (
        <>
          {upstream.length > 0 && (
            <div className="topology-pathlist__group">
              <h3>{t("topology.paths.upstream")}</h3>
              <ul>{upstream.map(renderPath)}</ul>
            </div>
          )}
          {downstream.length > 0 && (
            <div className="topology-pathlist__group">
              <h3>{t("topology.paths.downstream")}</h3>
              <ul>{downstream.map(renderPath)}</ul>
            </div>
          )}
        </>
      )}
    </section>
  );
}
