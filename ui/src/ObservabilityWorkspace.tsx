import { useCallback, useEffect, useRef, useState } from "react";
import type { ConnectorSummary, Invoke, NormalizedAlert } from "../contracts/ipc";
import { command } from "../contracts/ipc";
import { EmptyState } from "./design-system/components";
import { useTranslation } from "./i18n";
import { AlertsPanel } from "./observability/AlertsPanel";
import { GrafanaPanel } from "./observability/GrafanaPanel";
import { LogsPanel } from "./observability/LogsPanel";
import { MetricsPanel } from "./observability/MetricsPanel";
import { TimeRangeControl } from "./observability/TimeRangeControl";
import { TracePanel } from "./observability/TracePanel";
import { timeContextFromAlert, type TimeContext } from "./observability/timeContext";

const mapIpcError = (err: unknown, t: (key: string) => string) => {
  const e = err as Record<string, unknown>;
  const code = e?.code;
  if (code === "CONNECTOR_UNAVAILABLE") return t("observability.unavailable");
  if (code === "POLICY_DENIED") return t("observability.denied");
  if (code === "MALFORMED_RESPONSE") return t("observability.malformed");
  return t("observability.unknownError");
};

export function ObservabilityWorkspace({ invoke }: { invoke: Invoke }) {
  const { t } = useTranslation();
  const [connectors, setConnectors] = useState<ConnectorSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedAlert, setSelectedAlert] = useState<NormalizedAlert>();
  const [timeContext, setTimeContext] = useState<TimeContext>();
  const [metricContext, setMetricContext] = useState<{
    query: string;
    type: string;
    start?: string;
    end?: string;
  }>();
  const [logTraceIds, setLogTraceIds] = useState<string[] | null>(null);
  const [selectedTraceId, setSelectedTraceId] = useState<string>();
  const [investigationRevision, setInvestigationRevision] = useState(0);
  const investigationRevisionRef = useRef(0);

  const [error, setError] = useState("");
  const invalidateInvestigation = useCallback(() => {
    investigationRevisionRef.current += 1;
    setInvestigationRevision(investigationRevisionRef.current);
    setMetricContext(undefined);
    setLogTraceIds(null);
    setSelectedTraceId(undefined);
  }, []);

  const handleMetricContext = useCallback(
    (revision: number, metricContext: { query: string; type: string; start?: string; end?: string }) => {
      if (revision !== investigationRevisionRef.current) return;
      setMetricContext(metricContext);
    },
    []
  );

  const handleLogTraceIds = useCallback((revision: number, traceIds: string[] | null) => {
    if (revision !== investigationRevisionRef.current) return;
    setLogTraceIds(traceIds);
    setSelectedTraceId(undefined);
  }, []);

  const handleTraceSelect = useCallback((revision: number, traceId: string) => {
    if (revision !== investigationRevisionRef.current) return;
    setSelectedTraceId(traceId);
  }, []);

  useEffect(() => {
    invoke<null, ConnectorSummary[]>("connector_list", {
      envelope: {
        request_id: crypto.randomUUID(),
        command: command("connector", "list"),
        capability: "ConnectorRead",
        scope: { resource_ids: [] },
        payload: null
      }
    })
      .then((result) => {
        if (result.ok) {
          const all = result.value;
          setConnectors(
            all.filter(
              (c) =>
                ["prometheus", "alertmanager", "grafana", "loki", "tempo"].includes(c.kind) &&
                c.enabled
            )
          );
        } else {
          setError(mapIpcError(result.error, t));
        }
      })
      .catch((err) => setError(mapIpcError(err, t)))
      .finally(() => setLoading(false));
  }, [invoke, t]);

  if (loading) return <p role="status">{t("integrations.loading")}</p>;
  if (error)
    return (
      <p role="status" className="error">
        {error}
      </p>
    );
  if (!connectors.length) return <EmptyState titleKey="observability.empty" />;

  const am = connectors.filter((c) => c.kind === "alertmanager");
  const prom = connectors.filter((c) => c.kind === "prometheus");
  const graf = connectors.filter((c) => c.kind === "grafana");
  const loki = connectors.filter((c) => c.kind === "loki");
  const tempo = connectors.filter((c) => c.kind === "tempo");

  const selectAlert = (alert: NormalizedAlert) => {
    setSelectedAlert(alert);
    setTimeContext(timeContextFromAlert(alert, new Date()));
    invalidateInvestigation();
  };

  const handleTimeContextChange = (nextTimeContext: TimeContext) => {
    setTimeContext(nextTimeContext);
    invalidateInvestigation();
  };

  return (
    <div className="observability-workspace">
      {timeContext && (
        <div>
          <TimeRangeControl timeContext={timeContext} onChange={handleTimeContextChange} />
          {timeContext.source === "manual" && (
            <p role="status">{t("observability.manualTimeContext")}</p>
          )}
        </div>
      )}
      <section aria-label={t("observability.alertmanager")}>
        <h2>{t("observability.alertmanager")}</h2>
        {am.map((c) => (
          <AlertsPanel
            key={c.id}
            connector={c}
            invoke={invoke}
            selectedAlert={selectedAlert}
            onSelectAlert={selectAlert}
            timeContext={timeContext}
          />
        ))}
      </section>
      <section aria-label={t("observability.prometheus")}>
        <h2>{t("observability.prometheus")}</h2>
        {prom.map((c) => (
          <MetricsPanel
            key={c.id}
            connector={c}
            invoke={invoke}
            onMetricContext={handleMetricContext}
            selectedAlert={selectedAlert}
            timeContext={timeContext}
            resetKey={investigationRevision}
          />
        ))}
      </section>
      <section aria-label={t("observability.grafana")}>
        <h2>{t("observability.grafana")}</h2>
        {graf.map((c) => (
          <GrafanaPanel
            key={c.id}
            connector={c}
            invoke={invoke}
            metricContext={metricContext}
            selectedAlert={selectedAlert}
            timeContext={timeContext}
            resetKey={investigationRevision}
          />
        ))}
      </section>
      <section aria-label={t("observability.loki")}>
        <h2>{t("observability.loki")}</h2>
        {loki.map((c) => (
          <LogsPanel
            key={c.id}
            connector={c}
            invoke={invoke}
            selectedAlert={selectedAlert}
            timeContext={timeContext}
            onTraceIdsChange={handleLogTraceIds}
            onTraceSelect={handleTraceSelect}
            resetKey={investigationRevision}
          />
        ))}
      </section>
      <section aria-label={t("observability.tempo")}>
        <h2>{t("observability.tempo")}</h2>
        {tempo.map((c) => (
          <TracePanel
            key={c.id}
            connector={c}
            invoke={invoke}
            timeContext={timeContext}
            traceId={selectedTraceId}
            traceIds={logTraceIds}
            resetKey={investigationRevision}
          />
        ))}
      </section>
    </div>
  );
}
