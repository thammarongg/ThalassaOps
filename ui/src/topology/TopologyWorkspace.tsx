import { useMemo, useState } from "react";
import type {
  ConsoleEvidenceId,
  ConsoleHealthState,
  IncidentQueueItem,
  SourceStatus,
  StatusReason,
  TopologyNode,
  TopologySnapshot
} from "../../contracts/ipc";
import { Drawer, EmptyState, StatusIndicator } from "../design-system/components";
import { useTranslation } from "../i18n";
import { TopologyFilters } from "./TopologyFilters";
import { TopologyGraph } from "./TopologyGraph";
import { TopologyPathList } from "./TopologyPathList";
import { TopologyEvidencePanel } from "./TopologyEvidencePanel";
import "./topology.css";

const ALL = "all";
const NO_SELECTION = "";

const healthIndicatorState = (state: ConsoleHealthState) => {
  if (state === "healthy") return "healthy" as const;
  if (state === "critical") return "critical" as const;
  if (state === "degraded") return "degraded" as const;
  return "unavailable" as const;
};

const statusReasonKey = (reason: StatusReason | null) =>
  reason ? `topology.reasons.${reason}` : "topology.reasons.unknown";

function SourceNotice({ source }: { source: SourceStatus }) {
  const { t } = useTranslation();
  const state = source.state === "stale" ? "degraded" : "unavailable";
  const role = source.state === "unavailable" || source.state === "unverified" ? "alert" : "status";
  return (
    <p className="topology-source-notice" role={role}>
      <StatusIndicator state={state} />{" "}
      <span>
        {t("topology.sourceNotice", {
          source: source.source_key,
          state: t(`topology.sourceStates.${source.state}`),
          reason: t(statusReasonKey(source.reason))
        })}
      </span>
    </p>
  );
}

function NodeDetail({
  node,
  environmentName,
  onOpenEvidence
}: {
  node: TopologyNode;
  environmentName: string;
  onOpenEvidence: (evidenceIds: ConsoleEvidenceId[], subject: string) => void;
}) {
  const { t } = useTranslation();
  const indicatorState = healthIndicatorState(node.status);
  return (
    <aside className="topology-detail" aria-label={t("topology.detail.title")}>
      <h2>{node.name}</h2>
      <dl>
        <div>
          <dt>{t("topology.detail.kind")}</dt>
          <dd>{t(`topology.kinds.${node.kind}`)}</dd>
        </div>
        <div>
          <dt>{t("topology.detail.environment")}</dt>
          <dd>{environmentName}</dd>
        </div>
        <div>
          <dt>{t("topology.detail.provider")}</dt>
          <dd>{node.provider ?? t("topology.detail.none")}</dd>
        </div>
        {node.native_id && (
          <div>
            <dt>{t("topology.detail.nativeId")}</dt>
            <dd>{node.native_id}</dd>
          </div>
        )}
        <div>
          <dt>{t("topology.detail.health")}</dt>
          <dd>
            <StatusIndicator state={indicatorState} />
          </dd>
        </div>
        <div>
          <dt>{t("topology.detail.owner")}</dt>
          <dd>
            {node.ownership.team_name ?? t("topology.detail.ownershipUnassigned")} (
            {t(`topology.ownershipSources.${node.ownership.source}`)})
          </dd>
        </div>
        <div>
          <dt>{t("topology.detail.labels")}</dt>
          <dd>
            {Object.keys(node.labels).length === 0 ? (
              t("topology.detail.noLabels")
            ) : (
              <ul className="topology-detail__labels">
                {Object.entries(node.labels).map(([key, value]) => (
                  <li key={key}>{`${key}: ${value}`}</li>
                ))}
              </ul>
            )}
          </dd>
        </div>
        <div>
          <dt>{t("topology.detail.metric")}</dt>
          <dd>
            {node.metric
              ? `${node.metric.key}: ${node.metric.value}${t(`topology.units.${node.metric.unit}`)}`
              : t("topology.detail.noMetric")}
          </dd>
        </div>
        <div>
          <dt>{t("topology.detail.affected")}</dt>
          <dd>
            {node.affected_by_incident
              ? t("topology.detail.affectedYes")
              : t("topology.detail.affectedNo")}
          </dd>
        </div>
      </dl>
      <button
        type="button"
        aria-label={t("topology.graph.viewEvidence", { name: node.name })}
        onClick={() => onOpenEvidence(node.evidence_ids, node.name)}
      >
        {t("topology.graph.evidence")}
      </button>
    </aside>
  );
}

export function TopologyWorkspace({
  snapshot,
  incidents = []
}: {
  snapshot: TopologySnapshot | null;
  incidents?: IncidentQueueItem[];
}) {
  const { t } = useTranslation();
  const [environment, setEnvironment] = useState(ALL);
  const [team, setTeam] = useState(ALL);
  const [incident, setIncident] = useState(snapshot?.filter.incident_id ?? NO_SELECTION);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [evidenceRequest, setEvidenceRequest] = useState<{
    subject: string;
    ids: ConsoleEvidenceId[];
  } | null>(null);

  const nodesById = useMemo(
    () => new Map((snapshot?.nodes ?? []).map((node) => [node.id, node])),
    [snapshot]
  );
  const edgesById = useMemo(
    () => new Map((snapshot?.edges ?? []).map((edge) => [edge.id, edge])),
    [snapshot]
  );

  const environmentOptions = useMemo(() => {
    const byId = new Map<string, string>();
    for (const node of snapshot?.nodes ?? []) {
      if (node.kind === "environment" && node.environment_id) {
        byId.set(node.environment_id, node.name);
      }
    }
    return [...byId.entries()].map(([id, name]) => ({ id, name }));
  }, [snapshot]);

  const teamOptions = useMemo(() => {
    const byId = new Map<string, string>();
    for (const node of snapshot?.nodes ?? []) {
      if (node.ownership.team_id && node.ownership.team_name) {
        byId.set(node.ownership.team_id, node.ownership.team_name);
      }
    }
    return [...byId.entries()].map(([id, name]) => ({ id, name }));
  }, [snapshot]);

  const visibleNodes = useMemo(() => {
    if (!snapshot) return [];
    const affectedRoots = snapshot.nodes.filter((node) => node.affected_by_incident);
    const contextIds = new Set<string>();
    if (incident !== NO_SELECTION) {
      const rootIds = new Set(affectedRoots.map((node) => node.id));
      for (const path of snapshot.paths) {
        if (rootIds.has(path.root_node_id)) {
          for (const id of path.node_ids) contextIds.add(id);
        }
      }
    }
    return snapshot.nodes.filter((node) => {
      if (incident !== NO_SELECTION) {
        const inBlastRadius = node.affected_by_incident || contextIds.has(node.id);
        if (!inBlastRadius) return false;
      }
      if (environment !== ALL && node.environment_id !== environment) return false;
      if (team !== ALL) {
        if (!node.ownership.team_id || node.ownership.team_id !== team) return false;
      }
      return true;
    });
  }, [snapshot, incident, environment, team]);

  const visibleNodeIds = useMemo(
    () => new Set(visibleNodes.map((node) => node.id)),
    [visibleNodes]
  );
  const visibleEdges = useMemo(() => {
    if (!snapshot) return [];
    return snapshot.edges.filter(
      (edge) =>
        visibleNodeIds.has(edge.upstream_node_id) && visibleNodeIds.has(edge.downstream_node_id)
    );
  }, [snapshot, visibleNodeIds]);

  const selectedNode =
    selectedNodeId && visibleNodeIds.has(selectedNodeId)
      ? nodesById.get(selectedNodeId)
      : undefined;

  const pathRoots = useMemo(() => {
    if (!snapshot) return new Set<string>();
    if (incident !== NO_SELECTION) {
      return new Set(
        snapshot.nodes.filter((node) => node.affected_by_incident).map((node) => node.id)
      );
    }
    if (selectedNodeId && visibleNodeIds.has(selectedNodeId)) return new Set([selectedNodeId]);
    if (snapshot.focus_node_id && visibleNodeIds.has(snapshot.focus_node_id)) {
      return new Set([snapshot.focus_node_id]);
    }
    return new Set<string>();
  }, [snapshot, incident, selectedNodeId, visibleNodeIds]);

  const visiblePaths = useMemo(() => {
    if (!snapshot) return [];
    return snapshot.paths.filter((path) => pathRoots.has(path.root_node_id));
  }, [snapshot, pathRoots]);

  const pathFocusName = useMemo(() => {
    if (!snapshot || incident !== NO_SELECTION) return null;
    const focusId =
      selectedNodeId && visibleNodeIds.has(selectedNodeId)
        ? selectedNodeId
        : snapshot.focus_node_id;
    const focusNode = focusId ? nodesById.get(focusId) : undefined;
    return focusNode?.name ?? null;
  }, [snapshot, incident, selectedNodeId, visibleNodeIds, nodesById]);

  const environmentNameById = useMemo(() => {
    const byId = new Map<string, string>();
    for (const node of snapshot?.nodes ?? []) {
      if (node.kind === "environment" && node.environment_id) {
        byId.set(node.environment_id, node.name);
      }
    }
    return byId;
  }, [snapshot]);

  const openEvidence = (ids: ConsoleEvidenceId[], subject: string) => {
    setEvidenceRequest({ subject, ids });
  };

  const sourceNotices = (snapshot?.source_status ?? []).filter(
    (source) => source.state !== "fresh"
  );

  return (
    <div className="topology-workspace">
      <header className="topology-workspace__header">
        <div>
          <p className="eyebrow">{t("topology.eyebrow")}</p>
          <h1>{t("topology.title")}</h1>
          <p className="topology-workspace__subtitle">{t("topology.subtitle")}</p>
        </div>
        {snapshot && (
          <p className="topology-workspace__sync">
            {t("topology.lastSync", { timestamp: snapshot.generated_at })}
          </p>
        )}
      </header>
      {sourceNotices.length > 0 && (
        <div className="topology-workspace__notices">
          {sourceNotices.map((source) => (
            <SourceNotice key={source.source_key} source={source} />
          ))}
        </div>
      )}
      <TopologyFilters
        environments={environmentOptions}
        teams={teamOptions}
        incidents={incidents}
        environment={environment}
        team={team}
        incident={incident}
        onEnvironmentChange={setEnvironment}
        onTeamChange={setTeam}
        onIncidentChange={setIncident}
      />
      <div className="topology-workspace__main">
        <div className="topology-workspace__graph">
          {!snapshot ? (
            <p role="status">{t("topology.loading")}</p>
          ) : snapshot.nodes.length === 0 ? (
            <EmptyState titleKey="topology.empty" />
          ) : (
            <TopologyGraph
              nodes={visibleNodes}
              edges={visibleEdges}
              selectedNodeId={selectedNodeId ?? null}
              onSelectNode={(nodeId) =>
                setSelectedNodeId((current) => (current === nodeId ? null : nodeId))
              }
              onOpenEvidence={openEvidence}
            />
          )}
        </div>
        {selectedNode && (
          <NodeDetail
            node={selectedNode}
            environmentName={
              (selectedNode.environment_id &&
                environmentNameById.get(selectedNode.environment_id)) ||
              selectedNode.environment_id ||
              t("topology.detail.none")
            }
            onOpenEvidence={openEvidence}
          />
        )}
      </div>
      {!snapshot ? (
        <p role="status">{t("topology.loading")}</p>
      ) : (
        <TopologyPathList
          paths={visiblePaths}
          nodesById={nodesById}
          edgesById={edgesById}
          focusName={pathFocusName}
          incidentMode={incident !== NO_SELECTION}
          onOpenEvidence={openEvidence}
        />
      )}
      <Drawer
        titleKey="topology.evidence.title"
        isOpen={evidenceRequest !== null}
        onClose={() => setEvidenceRequest(null)}
      >
        {evidenceRequest && (
          <TopologyEvidencePanel
            subject={evidenceRequest.subject}
            requestedIds={evidenceRequest.ids}
            evidence={snapshot?.evidence ?? []}
          />
        )}
      </Drawer>
    </div>
  );
}
