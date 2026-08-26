import { useEffect, useState } from "react";
import type { ConnectorSummary, Invoke, NormalizedAlert } from "../contracts/ipc";
import { command } from "../contracts/ipc";
import { EmptyState } from "./design-system/components";
import { useTranslation } from "./i18n";
import { AlertsPanel } from "./observability/AlertsPanel";
import { GrafanaPanel } from "./observability/GrafanaPanel";
import { MetricsPanel } from "./observability/MetricsPanel";
import { TimeRangeControl } from "./observability/TimeRangeControl";
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

  const [error, setError] = useState("");

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
              (c) => ["prometheus", "alertmanager", "grafana"].includes(c.kind) && c.enabled
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

  const selectAlert = (alert: NormalizedAlert) => {
    setSelectedAlert(alert);
    setTimeContext(timeContextFromAlert(alert, new Date()));
  };

  return (
    <div className="observability-workspace">
      {timeContext && (
        <div>
          <TimeRangeControl timeContext={timeContext} onChange={setTimeContext} />
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
            onMetricContext={setMetricContext}
            selectedAlert={selectedAlert}
            timeContext={timeContext}
            onTimeContext={setTimeContext}
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
          />
        ))}
      </section>
    </div>
  );
}
