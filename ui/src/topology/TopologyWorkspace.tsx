import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  CommandEnvelope,
  ConsoleEvidenceId,
  ConsoleHealthState,
  EvidenceRef,
  IncidentQueueItem,
  Invoke,
  OperationsSnapshot,
  SourceStatus,
  StatusReason,
  TopologyEdge,
  TopologyNode,
  TopologyDirection,
  TopologyRequest,
  TopologySnapshot
} from "../../contracts/ipc";
import { command } from "../../contracts/ipc";
import { isEvidenceResponse } from "../../contracts/guards";
import { isOperationsSnapshot } from "../operations/contractValidation";
import { Drawer, EmptyState, StatusIndicator } from "../design-system/components";
import { useTranslation } from "../i18n";
import { TopologyFilters, type EnvironmentOption, type TeamOption } from "./TopologyFilters";
import { TopologyGraph } from "./TopologyGraph";
import { TopologyPathList } from "./TopologyPathList";
import { TopologyEvidencePanel, type TopologyEvidenceState } from "./TopologyEvidencePanel";
import { isTopologySnapshot } from "./contractValidation";
import "./topology.css";

const ALL = "all";
const NO_SELECTION = "";
const DEFAULT_MAX_DEPTH = 3;

type SnapshotState = "loading" | "ready" | "error";

const operationsSnapshotEnvelope = (): CommandEnvelope<null> => ({
  request_id: crypto.randomUUID(),
  command: command("operations", "snapshot"),
  capability: "WorkspaceRead",
  scope: { resource_ids: [] },
  payload: null
});

/** The unfiltered workspace graph is the source for filter dropdown options. */
const UNFILTERED_TOPOLOGY_REQUEST: TopologyRequest = {
  filter: { environment_ids: [], team_ids: [], incident_id: null },
  focus_node_id: null,
  traversal: { direction: "both", max_depth: DEFAULT_MAX_DEPTH }
};

const topologyEnvelope = <T,>(
  verb: "snapshot" | "evidence",
  capability: "WorkspaceRead" | "ResourceRead",
  payload: T
): CommandEnvelope<T> => ({
  request_id: crypto.randomUUID(),
  command: command("topology", verb),
  capability,
  scope: { resource_ids: [] },
  payload
});

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

function EdgeDetail({
  edge,
  nodesById,
  onOpenEvidence
}: {
  edge: TopologyEdge;
  nodesById: Map<string, TopologyNode>;
  onOpenEvidence: (evidenceIds: ConsoleEvidenceId[], subject: string) => void;
}) {
  const { t } = useTranslation();
  const upstream = nodesById.get(edge.upstream_node_id)?.name ?? edge.upstream_node_id;
  const downstream = nodesById.get(edge.downstream_node_id)?.name ?? edge.downstream_node_id;
  const subject = `${upstream} → ${downstream}`;
  return (
    <aside className="topology-detail" aria-label={t("topology.edgeDetail.title")}>
      <h2>{subject}</h2>
      <dl>
        <div>
          <dt>{t("topology.edgeDetail.relation")}</dt>
          <dd>{t(`topology.relations.${edge.kind}`)}</dd>
        </div>
        <div>
          <dt>{t("topology.edgeDetail.direction")}</dt>
          <dd>{t("topology.edgeDetail.upstreamToDownstream")}</dd>
        </div>
        <div>
          <dt>{t("topology.edgeDetail.confidence")}</dt>
          <dd>
            {t("topology.graph.confidenceValue", { value: Math.round(edge.confidence * 100) })}
          </dd>
        </div>
        <div>
          <dt>{t("topology.edgeDetail.provenance")}</dt>
          <dd>
            {edge.provenance.length > 0
              ? edge.provenance.map((item) => item.source_key).join(", ")
              : t("topology.graph.provenanceUnavailable")}
          </dd>
        </div>
      </dl>
      <button
        type="button"
        aria-label={t("topology.graph.viewEdgeEvidence", { upstream, downstream })}
        onClick={() => onOpenEvidence(edge.evidence_ids, subject)}
      >
        {t("topology.graph.evidence")}
      </button>
    </aside>
  );
}

const environmentOptionsFrom = (snapshot: TopologySnapshot): EnvironmentOption[] => {
  const byId = new Map<string, string>();
  for (const node of snapshot.nodes) {
    if (node.kind === "environment" && node.environment_id) {
      byId.set(node.environment_id, node.name);
    }
  }
  return [...byId.entries()].map(([id, name]) => ({ id, name }));
};

const teamOptionsFrom = (snapshot: TopologySnapshot): TeamOption[] => {
  const byId = new Map<string, string>();
  for (const node of snapshot.nodes) {
    if (node.ownership.team_id && node.ownership.team_name) {
      byId.set(node.ownership.team_id, node.ownership.team_name);
    }
  }
  return [...byId.entries()].map(([id, name]) => ({ id, name }));
};

export function TopologyWorkspace({
  invoke,
  initialIncidentId = null
}: {
  invoke: Invoke;
  initialIncidentId?: string | null;
}) {
  const { t } = useTranslation();
  const [snapshot, setSnapshot] = useState<TopologySnapshot>();
  const [snapshotState, setSnapshotState] = useState<SnapshotState>("loading");
  const [snapshotError, setSnapshotError] = useState("");
  const [incidents, setIncidents] = useState<IncidentQueueItem[]>([]);
  const [incidentsError, setIncidentsError] = useState(false);
  const [environmentOptions, setEnvironmentOptions] = useState<EnvironmentOption[]>([]);
  const [teamOptions, setTeamOptions] = useState<TeamOption[]>([]);
  const [environment, setEnvironment] = useState(ALL);
  const [team, setTeam] = useState(ALL);
  const [incident, setIncident] = useState(initialIncidentId ?? NO_SELECTION);
  const [direction, setDirection] = useState<TopologyDirection>("both");
  const [maxDepth, setMaxDepth] = useState(DEFAULT_MAX_DEPTH);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null);
  const [evidenceRequest, setEvidenceRequest] = useState<{
    subject: string;
    ids: ConsoleEvidenceId[];
  } | null>(null);
  const [evidenceState, setEvidenceState] = useState<TopologyEvidenceState>("idle");
  const [evidence, setEvidence] = useState<EvidenceRef[]>([]);
  const [evidenceError, setEvidenceError] = useState("");
  const snapshotRequestRef = useRef(0);
  const evidenceRequestRef = useRef(0);

  const issuedEvidenceIds = useMemo(
    () => new Set(snapshot?.evidence.map((item) => item.id) ?? []),
    [snapshot]
  );

  const nodesById = useMemo(
    () => new Map((snapshot?.nodes ?? []).map((node) => [node.id, node])),
    [snapshot]
  );
  const edgesById = useMemo(
    () => new Map((snapshot?.edges ?? []).map((edge) => [edge.id, edge])),
    [snapshot]
  );
  const selectedEdge = useMemo(
    () => (selectedEdgeId ? edgesById.get(selectedEdgeId) : undefined),
    [edgesById, selectedEdgeId]
  );

  // The Incident filter lists the workspace queue, the same projection the
  // Operations Console renders.  A failure here degrades only the dropdown.
  useEffect(() => {
    let active = true;
    void invoke<null, OperationsSnapshot>("operations_snapshot", {
      envelope: operationsSnapshotEnvelope()
    })
      .then((result) => {
        if (!active) return;
        if (result.ok && isOperationsSnapshot(result.value)) {
          setIncidents(result.value.incident_queue);
          setIncidentsError(false);
        } else {
          setIncidents([]);
          setIncidentsError(true);
        }
      })
      .catch(() => {
        if (!active) return;
        setIncidents([]);
        setIncidentsError(true);
      });
    return () => {
      active = false;
    };
  }, [invoke]);

  // Filter options describe the whole workspace graph, not the currently
  // filtered view, so filtering can be widened again after it is narrowed.
  useEffect(() => {
    let active = true;
    void invoke<TopologyRequest, TopologySnapshot>("topology_snapshot", {
      envelope: topologyEnvelope("snapshot", "WorkspaceRead", UNFILTERED_TOPOLOGY_REQUEST)
    })
      .then((result) => {
        if (!active || !result.ok || !isTopologySnapshot(result.value)) return;
        setEnvironmentOptions(environmentOptionsFrom(result.value));
        setTeamOptions(teamOptionsFrom(result.value));
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [invoke]);

  // The backend owns filtering and traversal: every Environment, Team,
  // Incident or focus change re-reads the projection through IPC.
  useEffect(() => {
    const requestId = ++snapshotRequestRef.current;
    const request: TopologyRequest = {
      filter: {
        environment_ids: environment === ALL ? [] : [environment],
        team_ids: team === ALL ? [] : [team],
        incident_id: incident === NO_SELECTION ? null : incident
      },
      focus_node_id: selectedNodeId,
      traversal: { direction, max_depth: maxDepth }
    };
    void invoke<TopologyRequest, TopologySnapshot>("topology_snapshot", {
      envelope: topologyEnvelope("snapshot", "WorkspaceRead", request)
    })
      .then((result) => {
        if (requestId !== snapshotRequestRef.current) return;
        if (result.ok && isTopologySnapshot(result.value)) {
          setSnapshot(result.value);
          setSnapshotState("ready");
        } else {
          setSnapshotState("error");
          setSnapshotError(t("topology.snapshotError"));
        }
      })
      .catch(() => {
        if (requestId !== snapshotRequestRef.current) return;
        setSnapshotState("error");
        setSnapshotError(t("topology.snapshotError"));
      });
  }, [invoke, t, environment, team, incident, selectedNodeId, direction, maxDepth]);

  const selectedNode = useMemo(
    () =>
      snapshot && selectedNodeId
        ? snapshot.nodes.find((node) => node.id === selectedNodeId)
        : undefined,
    [snapshot, selectedNodeId]
  );

  const pathFocusName = useMemo(() => {
    if (!snapshot || incident !== NO_SELECTION) return null;
    const focusNode = snapshot.focus_node_id ? nodesById.get(snapshot.focus_node_id) : undefined;
    return focusNode?.name ?? null;
  }, [snapshot, incident, nodesById]);

  const environmentNameById = useMemo(() => {
    const byId = new Map<string, string>();
    for (const node of snapshot?.nodes ?? []) {
      if (node.kind === "environment" && node.environment_id) {
        byId.set(node.environment_id, node.name);
      }
    }
    return byId;
  }, [snapshot]);

  const openEvidence = useCallback(
    (ids: ConsoleEvidenceId[], subject: string) => {
      const requestId = ++evidenceRequestRef.current;
      const admitted = [...new Set(ids.filter((id) => issuedEvidenceIds.has(id)))];
      setEvidenceRequest({ subject, ids: admitted });
      setEvidence([]);
      setEvidenceError("");
      if (!admitted.length) {
        setEvidenceState("error");
        setEvidenceError(t("topology.evidence.unavailable"));
        return;
      }
      setEvidenceState("loading");
      void invoke<{ evidence_ids: ConsoleEvidenceId[] }, EvidenceRef[]>("topology_evidence", {
        envelope: topologyEnvelope("evidence", "ResourceRead", { evidence_ids: admitted })
      })
        .then((result) => {
          if (requestId !== evidenceRequestRef.current) return;
          if (result.ok && isEvidenceResponse(result.value, admitted)) {
            setEvidence(result.value);
            setEvidenceState("ready");
          } else {
            setEvidenceState("error");
            setEvidenceError(t("topology.evidence.error"));
          }
        })
        .catch(() => {
          if (requestId !== evidenceRequestRef.current) return;
          setEvidenceState("error");
          setEvidenceError(t("topology.evidence.error"));
        });
    },
    [invoke, issuedEvidenceIds, t]
  );

  const sourceNotices = (snapshot?.source_status ?? []).filter(
    (source) => source.state !== "fresh"
  );

  const snapshotPlaceholder =
    snapshotState === "error" ? (
      <p className="topology-workspace__error" role="alert">
        {snapshotError}
      </p>
    ) : (
      <p role="status">{t("topology.loading")}</p>
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
      {snapshotState === "error" && snapshot && (
        <p className="topology-workspace__error" role="alert">
          {snapshotError}
        </p>
      )}
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
        direction={direction}
        maxDepth={maxDepth}
        onEnvironmentChange={setEnvironment}
        onTeamChange={setTeam}
        onIncidentChange={setIncident}
        onDirectionChange={setDirection}
        onMaxDepthChange={setMaxDepth}
      />
      {incidentsError && (
        <p className="topology-workspace__incidents-note" role="status">
          {t("topology.incidentsUnavailable")}
        </p>
      )}
      <div className="topology-workspace__main">
        <div className="topology-workspace__graph">
          {!snapshot ? (
            snapshotPlaceholder
          ) : snapshot.nodes.length === 0 ? (
            <EmptyState titleKey="topology.empty" />
          ) : (
            <TopologyGraph
              nodes={snapshot.nodes}
              edges={snapshot.edges}
              selectedNodeId={selectedNodeId ?? null}
              selectedEdgeId={selectedEdgeId}
              onSelectNode={(nodeId) => {
                setSelectedEdgeId(null);
                setSelectedNodeId((current) => (current === nodeId ? null : nodeId));
              }}
              onSelectEdge={(edgeId) => {
                setSelectedNodeId(null);
                setSelectedEdgeId((current) => (current === edgeId ? null : edgeId));
              }}
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
        {!selectedNode && selectedEdge && (
          <EdgeDetail edge={selectedEdge} nodesById={nodesById} onOpenEvidence={openEvidence} />
        )}
      </div>
      {!snapshot ? (
        snapshotPlaceholder
      ) : (
        <TopologyPathList
          paths={snapshot.paths}
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
            evidenceState={evidenceState}
            evidence={evidence}
            errorMessage={evidenceError}
          />
        )}
      </Drawer>
    </div>
  );
}
