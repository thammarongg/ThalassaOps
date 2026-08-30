import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  ChangeEvent,
  ChangeRequest,
  ChangeSnapshot,
  CommandEnvelope,
  CorrelationMetric,
  CorrelationMetricKey,
  CorrelationRequest,
  CorrelationSnapshot,
  EvidenceRef,
  Invoke,
  IpcErrorCode,
  Signal,
  SourceStatus
} from "../../contracts/ipc";
import { command } from "../../contracts/ipc";
import {
  isChangeSnapshot,
  isCorrelationSnapshot,
  isEvidenceResponse
} from "../../contracts/guards";
import { ChangeDetail } from "../change/ChangeDetail";
import { ChangeTimeline } from "../change/ChangeTimeline";
import { Drawer, EmptyState, StatusIndicator } from "../design-system/components";
import { useTranslation } from "../i18n";
import { CandidateDetails } from "./CandidateDetails";
import { CandidateList } from "./CandidateList";
import {
  CorrelationEvidencePanel,
  type CorrelationEvidenceState
} from "./CorrelationEvidencePanel";
import "./correlation.css";

type SnapshotState = "loading" | "ready" | "error";

export const DEFAULT_CORRELATION_REQUEST: CorrelationRequest = {
  window: {
    start: "2026-08-28T08:55:00Z",
    end: "2026-08-28T09:05:00Z"
  },
  evaluated_at: "2026-08-28T09:00:00Z",
  allowed_lateness_seconds: 300
};

export const DEFAULT_CHANGE_REQUEST: ChangeRequest = {
  window: {
    start: "2026-08-28T08:00:00Z",
    end: "2026-08-28T09:00:00Z"
  },
  evaluated_at: "2026-08-28T09:00:00Z",
  lookback_seconds: 3600,
  limit: 50
};

const changeEnvelope = <T,>(
  verb: "snapshot" | "evidence",
  capability: "WorkspaceRead" | "ResourceRead",
  payload: T
): CommandEnvelope<T> => ({
  request_id: crypto.randomUUID(),
  command: command("change", verb),
  capability,
  scope: { resource_ids: [] },
  payload
});

const correlationEnvelope = <T,>(
  verb: "snapshot" | "evidence",
  capability: "WorkspaceRead" | "ResourceRead",
  payload: T
): CommandEnvelope<T> => ({
  request_id: crypto.randomUUID(),
  command: command("correlation", verb),
  capability,
  scope: { resource_ids: [] },
  payload
});

const sourceLabel = (source: SourceStatus["source_key"], t: (key: string) => string) => {
  const knownSources = [
    "alertmanager",
    "prometheus",
    "kubernetes",
    "cloud",
    "health_check",
    "fixture",
    "trivy",
    "falco",
    "kyverno",
    "opa_gatekeeper",
    "topology"
  ];
  return knownSources.includes(source)
    ? t("correlation.sources." + source)
    : t("correlation.sources.unknown");
};

const localizedErrorKey = (code: IpcErrorCode) => {
  switch (code) {
    case "INVALID_REQUEST":
      return "correlation.errors.invalidRequest";
    case "NOT_FOUND":
      return "correlation.errors.notFound";
    case "PERMISSION_DENIED":
      return "correlation.errors.permissionDenied";
    case "POLICY_DENIED":
      return "correlation.errors.policyDenied";
    case "CONNECTOR_UNAVAILABLE":
      return "correlation.errors.connectorUnavailable";
    case "MALFORMED_RESPONSE":
      return "correlation.errors.malformedResponse";
    case "INTERNAL_ERROR":
      return "correlation.errors.internalError";
    default:
      return "correlation.errors.internalError";
  }
};

const metricLabelKey = (metric: CorrelationMetric) => "correlation.metrics." + metric.key;
const metricUnitKey = (metric: CorrelationMetric) => "correlation.units." + metric.unit;
const CORRELATION_METRIC_KEYS: CorrelationMetricKey[] = [
  "normalized_signals",
  "active_candidates",
  "suppressed_candidates",
  "uncorrelated_signals"
];

function SourceNotices({ sources }: { sources: SourceStatus[] }) {
  const { t } = useTranslation();
  if (sources.length === 0) return null;
  return (
    <div className="correlation-source-notices">
      {sources.map((source) => {
        const state =
          source.state === "fresh"
            ? ("healthy" as const)
            : source.state === "stale"
              ? ("degraded" as const)
              : ("unavailable" as const);
        return (
          <p
            className="correlation-source-notice"
            role={
              source.state === "unavailable" || source.state === "unverified" ? "alert" : "status"
            }
            key={source.source_key + "-" + source.state + "-" + (source.reason ?? "unknown")}
          >
            <StatusIndicator state={state} />{" "}
            <span>
              {t("correlation.sourceNotice", {
                source: sourceLabel(source.source_key, t),
                state: t("correlation.sourceStates." + source.state),
                reason: t(
                  source.reason
                    ? "correlation.reasons." + source.reason
                    : "correlation.reasons.unknown"
                )
              })}
            </span>
          </p>
        );
      })}
    </div>
  );
}

function MetricCard({
  metric,
  issuedEvidenceIds,
  onOpen
}: {
  metric: CorrelationMetric | undefined;
  issuedEvidenceIds: Set<string>;
  onOpen: (metric: CorrelationMetric, evidenceIds: string[]) => void;
}) {
  const { t } = useTranslation();
  if (!metric) {
    return (
      <div className="correlation-metric correlation-metric--unavailable" role="status">
        <StatusIndicator state="unavailable" /> {t("correlation.metrics.unavailable")}
      </div>
    );
  }
  const evidenceIds = [...new Set(metric.evidence_ids.filter((id) => issuedEvidenceIds.has(id)))];
  if (evidenceIds.length === 0) {
    return (
      <div className="correlation-metric correlation-metric--unavailable" role="status">
        <StatusIndicator state="unavailable" /> {t("correlation.metrics.unavailable")}
      </div>
    );
  }
  const label = t(metricLabelKey(metric));
  return (
    <button
      type="button"
      className="correlation-metric"
      aria-label={t("correlation.metrics.openEvidence", { label, value: metric.value })}
      onClick={() => onOpen(metric, evidenceIds)}
    >
      <span className="correlation-metric__value">
        {metric.value}
        {metric.unit !== "count" && t(metricUnitKey(metric))}
      </span>
      <span className="correlation-metric__label">{label}</span>
      <span className="correlation-metric__affordance" aria-hidden="true">
        ↗
      </span>
    </button>
  );
}

function SourceSummary({ signals }: { signals: Signal[] }) {
  const { t } = useTranslation();
  const sources = [...new Set(signals.map((signal) => signal.source))];
  if (sources.length === 0) return null;
  return (
    <section className="correlation-sources" aria-labelledby="correlation-sources-title">
      <h2 id="correlation-sources-title">{t("correlation.sourcesTitle")}</h2>
      <ul>
        {sources.map((source) => (
          <li key={source}>{t("correlation.sources." + source)}</li>
        ))}
      </ul>
    </section>
  );
}

export function CorrelationWorkspace({ invoke }: { invoke: Invoke }) {
  const { t } = useTranslation();
  const [snapshot, setSnapshot] = useState<CorrelationSnapshot>();
  const [changeSnapshot, setChangeSnapshot] = useState<ChangeSnapshot>();
  const [selectedChangeId, setSelectedChangeId] = useState<string | null>(null);
  const [snapshotState, setSnapshotState] = useState<SnapshotState>("loading");
  const [snapshotError, setSnapshotError] = useState("");
  const [selectedCandidateId, setSelectedCandidateId] = useState<string | null>(null);
  const [evidenceSubject, setEvidenceSubject] = useState("");
  const [evidenceState, setEvidenceState] = useState<CorrelationEvidenceState>("idle");
  const [evidence, setEvidence] = useState<EvidenceRef[]>([]);
  const [evidenceError, setEvidenceError] = useState("");
  const snapshotRequestRef = useRef(0);
  const changeRequestRef = useRef(0);
  const evidenceRequestRef = useRef(0);

  const issuedEvidenceIds = useMemo(
    () => new Set(snapshot?.evidence.map((item) => item.id) ?? []),
    [snapshot]
  );
  const selectedCandidate = snapshot?.candidates.find(
    (candidate) => candidate.id === selectedCandidateId
  );

  useEffect(() => {
    const requestId = ++snapshotRequestRef.current;
    setSnapshotState("loading");
    setSnapshotError("");
    void invoke<CorrelationRequest, CorrelationSnapshot>("correlation_snapshot", {
      envelope: correlationEnvelope("snapshot", "WorkspaceRead", DEFAULT_CORRELATION_REQUEST)
    })
      .then((result) => {
        if (requestId !== snapshotRequestRef.current) return;
        if (result.ok && isCorrelationSnapshot(result.value)) {
          setSnapshot(result.value);
          setSelectedCandidateId(result.value.candidates[0]?.id ?? null);
          setSnapshotState("ready");
        } else {
          setSnapshot(undefined);
          setSelectedCandidateId(null);
          setSnapshotState("error");
          setSnapshotError(
            result.ok
              ? t("correlation.errors.malformedResponse")
              : t(localizedErrorKey(result.error.code))
          );
        }
      })
      .catch(() => {
        if (requestId !== snapshotRequestRef.current) return;
        setSnapshot(undefined);
        setSelectedCandidateId(null);
        setSnapshotState("error");
        setSnapshotError(t("correlation.errors.internalError"));
      });
    return () => {
      snapshotRequestRef.current += 1;
    };
  }, [invoke, t]);

  useEffect(() => {
    const requestId = ++changeRequestRef.current;
    void invoke<ChangeRequest, ChangeSnapshot>("change_snapshot", {
      envelope: changeEnvelope("snapshot", "WorkspaceRead", DEFAULT_CHANGE_REQUEST)
    })
      .then((result) => {
        if (requestId !== changeRequestRef.current) return;
        // A change view is context, never the reason a workspace fails to
        // render: a rejected change snapshot leaves the correlation view intact
        // and the change lane reports its own empty state.
        if (result.ok && isChangeSnapshot(result.value)) {
          setChangeSnapshot(result.value);
          setSelectedChangeId(result.value.timeline.entry_ids[0] ?? null);
        } else {
          setChangeSnapshot(undefined);
          setSelectedChangeId(null);
        }
      })
      .catch(() => {
        if (requestId !== changeRequestRef.current) return;
        setChangeSnapshot(undefined);
        setSelectedChangeId(null);
      });
    return () => {
      changeRequestRef.current += 1;
    };
  }, [invoke]);

  const openChangeEvidence = useCallback(
    (subject: string, evidenceIds: string[]) => {
      const requestId = ++evidenceRequestRef.current;
      const ids = [...new Set(evidenceIds)];
      setEvidenceSubject(subject);
      setEvidence([]);
      setEvidenceError("");
      setEvidenceState(ids.length > 0 ? "loading" : "error");
      if (ids.length === 0) {
        setEvidenceError(t("change.metrics.unavailable"));
        return;
      }
      void invoke<{ evidence_ids: string[] }, EvidenceRef[]>("change_evidence", {
        envelope: changeEnvelope("evidence", "ResourceRead", { evidence_ids: ids })
      })
        .then((result) => {
          if (requestId !== evidenceRequestRef.current) return;
          if (result.ok && isEvidenceResponse(result.value, ids)) {
            setEvidence(result.value);
            setEvidenceState("ready");
          } else {
            setEvidenceState("error");
            setEvidenceError(
              result.ok
                ? t("change.errors.malformedResponse")
                : t(localizedErrorKey(result.error.code))
            );
          }
        })
        .catch(() => {
          if (requestId !== evidenceRequestRef.current) return;
          setEvidenceState("error");
          setEvidenceError(t("change.errors.internalError"));
        });
    },
    [invoke, t]
  );

  const openEvidence = useCallback(
    (subject: string, evidenceIds: string[]) => {
      const requestId = ++evidenceRequestRef.current;
      const ids = [...new Set(evidenceIds.filter((id) => issuedEvidenceIds.has(id)))];
      setEvidenceSubject(subject);
      setEvidence([]);
      setEvidenceError("");
      setEvidenceState(ids.length > 0 ? "loading" : "error");
      if (ids.length === 0) {
        setEvidenceError(t("correlation.metrics.unavailable"));
        return;
      }
      void invoke<{ evidence_ids: string[] }, EvidenceRef[]>("correlation_evidence", {
        envelope: correlationEnvelope("evidence", "ResourceRead", { evidence_ids: ids })
      })
        .then((result) => {
          if (requestId !== evidenceRequestRef.current) return;
          if (result.ok && isEvidenceResponse(result.value, ids)) {
            setEvidence(result.value);
            setEvidenceState("ready");
          } else {
            setEvidenceState("error");
            setEvidenceError(
              result.ok
                ? t("correlation.errors.malformedResponse")
                : t(localizedErrorKey(result.error.code))
            );
          }
        })
        .catch(() => {
          if (requestId !== evidenceRequestRef.current) return;
          setEvidenceState("error");
          setEvidenceError(t("correlation.errors.internalError"));
        });
    },
    [invoke, issuedEvidenceIds, t]
  );

  const openMetricEvidence = useCallback(
    (metric: CorrelationMetric, evidenceIds: string[]) => {
      openEvidence(t(metricLabelKey(metric)), evidenceIds);
    },
    [openEvidence, t]
  );

  return (
    <section className="correlation-workspace">
      <header className="correlation-workspace__header">
        <div>
          <p className="eyebrow">{t("correlation.eyebrow")}</p>
          <h1>{t("correlation.title")}</h1>
          <p className="correlation-workspace__subtitle">{t("correlation.subtitle")}</p>
        </div>
        {snapshot && (
          <p className="correlation-workspace__sync">
            {t("correlation.lastSync", { timestamp: snapshot.generated_at })}
          </p>
        )}
      </header>

      {snapshotState === "loading" && (
        <p className="correlation-workspace__state" role="status">
          {t("correlation.loading")}
        </p>
      )}
      {snapshotState === "error" && (
        <p
          className="correlation-workspace__state correlation-workspace__state--error"
          role="alert"
        >
          {snapshotError}
        </p>
      )}
      {snapshotState === "ready" && snapshot && (
        <>
          <SourceNotices sources={snapshot.source_status} />
          <section className="correlation-summary" aria-labelledby="correlation-summary-title">
            <div className="correlation-section-heading">
              <div>
                <p className="eyebrow">{t("correlation.summary.eyebrow")}</p>
                <h2 id="correlation-summary-title">{t("correlation.summary.title")}</h2>
              </div>
              <p className="correlation-summary__window">
                {t("correlation.summary.window", {
                  start: snapshot.window.range.start,
                  end: snapshot.window.range.end
                })}
              </p>
            </div>
            <div className="correlation-metric-grid">
              {CORRELATION_METRIC_KEYS.map((key) => (
                <MetricCard
                  key={key}
                  metric={snapshot.summary.metrics.find((item) => item.key === key)}
                  issuedEvidenceIds={issuedEvidenceIds}
                  onOpen={openMetricEvidence}
                />
              ))}
            </div>
          </section>
          <SourceSummary signals={snapshot.signals} />
          <div className="correlation-workspace__main">
            <section
              className="correlation-candidates"
              aria-labelledby="correlation-candidates-title"
            >
              <div className="correlation-section-heading">
                <h2 id="correlation-candidates-title">{t("correlation.candidates.title")}</h2>
                <span>{t("correlation.candidates.description")}</span>
              </div>
              <CandidateList
                candidates={snapshot.candidates}
                selectedId={selectedCandidateId}
                onSelect={(candidate) => setSelectedCandidateId(candidate.id)}
              />
            </section>
            {selectedCandidate ? (
              <CandidateDetails
                candidate={selectedCandidate}
                signals={snapshot.signals}
                onOpenEvidence={openEvidence}
                changeAssociations={
                  changeSnapshot?.associations.filter(
                    (association) => association.candidate_id === selectedCandidate.id
                  ) ?? []
                }
                changeEvents={changeSnapshot?.events ?? []}
                onOpenChangeEvidence={openChangeEvidence}
              />
            ) : (
              <EmptyState titleKey="correlation.details.selectCandidate" />
            )}
          </div>
          {changeSnapshot && (
            <div className="correlation-workspace__changes">
              <ChangeTimeline
                snapshot={changeSnapshot}
                selectedChangeId={selectedChangeId}
                onSelect={(event: ChangeEvent) => setSelectedChangeId(event.id)}
              />
              {changeSnapshot.events
                .filter((event) => event.id === selectedChangeId)
                .map((event) => (
                  <ChangeDetail key={event.id} event={event} onOpenEvidence={openChangeEvidence} />
                ))}
            </div>
          )}
          <Drawer
            titleKey="correlation.evidence.title"
            isOpen={evidenceState !== "idle"}
            onClose={() => setEvidenceState("idle")}
          >
            <CorrelationEvidencePanel
              subject={evidenceSubject}
              evidenceState={evidenceState}
              evidence={evidence}
              errorMessage={evidenceError}
            />
          </Drawer>
        </>
      )}
    </section>
  );
}
