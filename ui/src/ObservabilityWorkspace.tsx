import React, { useState, useEffect } from "react";
import type {
  ConnectorSummary,
  NormalizedAlert,
  PrometheusQueryResult,
  GrafanaHealth,
  GrafanaLinkResult,
  Invoke,
  AlertmanagerAlertsRequest,
  PrometheusQueryRequest,
  PrometheusQueryRangeRequest,
  GrafanaHealthRequest,
  GrafanaLinkRequest
} from "../contracts/ipc";
import { command } from "../contracts/ipc";
import { Card, EmptyState, Table } from "./design-system/components";
import { useTranslation } from "./i18n";
import { open } from "@tauri-apps/plugin-shell";

const mapIpcError = (err: unknown, t: (key: string) => string) => {
  const e = err as Record<string, unknown>;
  const code = e?.code;
  const msg = typeof e?.message === "string" ? e.message : String(err);
  if (code === "ConnectorUnavailable" || msg.includes("ConnectorUnavailable"))
    return t("observability.unavailable");
  if (msg.includes("malformed") || code === "MalformedResponse")
    return t("observability.malformed");
  return msg;
};

export function ObservabilityWorkspace({ invoke }: { invoke: Invoke }) {
  const { t } = useTranslation();
  const [connectors, setConnectors] = useState<ConnectorSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedAlert, setSelectedAlert] = useState<NormalizedAlert>();
  const [metricContext, setMetricContext] = useState<{
    query: string;
    type: string;
    start?: string;
    end?: string;
  }>();

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
        }
      })
      .finally(() => setLoading(false));
  }, [invoke]);

  if (loading) return <p role="status">{t("integrations.loading")}</p>;
  if (!connectors.length) return <EmptyState titleKey="observability.empty" />;

  const am = connectors.filter((c) => c.kind === "alertmanager");
  const prom = connectors.filter((c) => c.kind === "prometheus");
  const graf = connectors.filter((c) => c.kind === "grafana");

  return (
    <div className="observability-workspace">
      <section aria-label={t("observability.alertmanager")}>
        <h2>{t("observability.alertmanager")}</h2>
        {am.map((c) => (
          <AlertmanagerPanel
            key={c.id}
            connector={c}
            invoke={invoke}
            selectedAlert={selectedAlert}
            onSelectAlert={setSelectedAlert}
          />
        ))}
      </section>
      <section aria-label={t("observability.prometheus")}>
        <h2>{t("observability.prometheus")}</h2>
        {prom.map((c) => (
          <PrometheusPanel
            key={c.id}
            connector={c}
            invoke={invoke}
            onMetricContext={setMetricContext}
            selectedAlert={selectedAlert}
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
          />
        ))}
      </section>
    </div>
  );
}

function AlertmanagerPanel({
  connector,
  invoke,
  selectedAlert,
  onSelectAlert
}: {
  connector: ConnectorSummary;
  invoke: Invoke;
  selectedAlert?: NormalizedAlert;
  onSelectAlert: (alert: NormalizedAlert) => void;
}) {
  const { t } = useTranslation();
  const [alerts, setAlerts] = useState<NormalizedAlert[]>([]);
  const [error, setError] = useState("");

  useEffect(() => {
    invoke<AlertmanagerAlertsRequest, NormalizedAlert[]>("alertmanager_alerts", {
      envelope: {
        request_id: crypto.randomUUID(),
        command: command("alertmanager", "alerts"),
        capability: "ResourceRead",
        scope: { resource_ids: [] },
        payload: { connector_id: connector.id }
      }
    })
      .then((res) => {
        if (res.ok) setAlerts(res.value);
        else setError(mapIpcError(res.error, t));
      })
      .catch((err) => setError(mapIpcError(err, t)));
  }, [connector, invoke, t]);

  const renderResource = (ref: Record<string, unknown>) => {
    if ("resolved" in ref) {
      const r = ref.resolved as Record<string, unknown>;
      return `${r.kind} ${r.namespace}/${r.name}`;
    }
    const unresolved = ref.unresolved as Record<string, unknown> | undefined;
    return t("observability.unresolved", { reason: unresolved?.reason || "" });
  };

  return (
    <Card titleKey="observability.alertmanager">
      <h3>{connector.display_name}</h3>
      {error && (
        <p role="status" className="error">
          {error}
        </p>
      )}
      {!error && alerts.length === 0 && <p>{t("observability.empty")}</p>}
      {alerts.length > 0 && (
        <Table
          captionKey="observability.alerts"
          columns={[
            { key: "select", headerKey: "observability.state" }, // Reuse state header space or something, wait we can just add a blank header or use 'state' for the first col
            { key: "state", headerKey: "observability.state" },
            { key: "timestamp", headerKey: "observability.timestamp" },
            { key: "labels", headerKey: "observability.labels" },
            { key: "resource", headerKey: "observability.resource" }
          ]}
          rows={alerts.map((a) => ({
            id: a.fingerprint,
            select: (
              <input
                type="radio"
                name="selectedAlert"
                aria-label={`Select alert ${a.fingerprint}`}
                checked={selectedAlert?.fingerprint === a.fingerprint}
                onChange={() => onSelectAlert(a)}
              />
            ),
            state: a.state,
            timestamp: new Date(a.starts_at).toLocaleString(),
            labels: Object.entries(a.labels)
              .map(([k, v]) => `${k}=${v}`)
              .join(", "),
            resource: renderResource(a.resource_reference)
          }))}
        />
      )}
    </Card>
  );
}

function PrometheusPanel({
  connector,
  invoke,
  onMetricContext,
  selectedAlert
}: {
  connector: ConnectorSummary;
  invoke: Invoke;
  onMetricContext: (ctx: { query: string; type: string; start?: string; end?: string }) => void;
  selectedAlert?: NormalizedAlert;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [result, setResult] = useState<PrometheusQueryResult>();
  const [error, setError] = useState("");
  const [type, setType] = useState("instant");

  useEffect(() => {
    if (selectedAlert) {
      const match = Object.entries(selectedAlert.labels)
        .map(([k, v]) => `${k}="${v}"`)
        .join(",");
      setQuery(`{${match}}`);
    }
  }, [selectedAlert]);

  const run = async () => {
    setError("");
    try {
      if (type === "instant") {
        const payload: PrometheusQueryRequest = { connector_id: connector.id, query };
        const res = await invoke<PrometheusQueryRequest, PrometheusQueryResult>(
          "prometheus_query",
          {
            envelope: {
              request_id: crypto.randomUUID(),
              command: command("prometheus", "query"),
              capability: "ResourceRead",
              scope: { resource_ids: [] },
              payload
            }
          }
        );
        if (res.ok) {
          setResult(res.value);
          onMetricContext({ query, type: "instant" });
        } else {
          setError(mapIpcError(res.error, t));
        }
      } else {
        const end = new Date();
        const start = new Date(end.getTime() - 3600000);
        const payload: PrometheusQueryRangeRequest = {
          connector_id: connector.id,
          query,
          start: start.toISOString(),
          end: end.toISOString(),
          step_seconds: 60
        };
        const res = await invoke<PrometheusQueryRangeRequest, PrometheusQueryResult>(
          "prometheus_query_range",
          {
            envelope: {
              request_id: crypto.randomUUID(),
              command: command("prometheus", "query_range"),
              capability: "ResourceRead",
              scope: { resource_ids: [] },
              payload
            }
          }
        );
        if (res.ok) {
          setResult(res.value);
          onMetricContext({ query, type: "range", start: payload.start, end: payload.end });
        } else {
          setError(mapIpcError(res.error, t));
        }
      }
    } catch (err: unknown) {
      setError(mapIpcError(err, t));
    }
  };

  return (
    <Card titleKey="observability.prometheus">
      <h3>{connector.display_name}</h3>
      <div>
        <select value={type} onChange={(e) => setType(e.target.value)}>
          <option value="instant">{t("observability.instant")}</option>
          <option value="range">{t("observability.range")}</option>
        </select>
        <input value={query} onChange={(e) => setQuery(e.target.value)} />
        <button type="button" onClick={run}>
          {t("observability.runQuery")}
        </button>
      </div>
      {error && (
        <p role="status" className="error">
          {error}
        </p>
      )}
      {result && (
        <div>
          <p>{result.source.endpoint}</p>
          {result.series.map((s, i) => (
            <div key={i}>
              <h4>
                {Object.entries(s.labels)
                  .map(([k, v]) => `${k}="${v}"`)
                  .join(", ")}
              </h4>
              <Table
                captionKey="observability.samples"
                columns={[
                  { key: "timestamp", headerKey: "observability.timestamp" },
                  { key: "value", headerKey: "observability.value" }
                ]}
                rows={s.samples.map((samp, j) => ({
                  id: `${i}-${j}`,
                  timestamp: new Date(samp.timestamp).toLocaleString(),
                  value: samp.value
                }))}
              />
            </div>
          ))}
        </div>
      )}
    </Card>
  );
}

function GrafanaPanel({
  connector,
  invoke,
  metricContext,
  selectedAlert
}: {
  connector: ConnectorSummary;
  invoke: Invoke;
  metricContext?: { query: string; type: string; start?: string; end?: string };
  selectedAlert?: NormalizedAlert;
}) {
  const { t } = useTranslation();
  const [health, setHealth] = useState<GrafanaHealth>();
  const [error, setError] = useState("");

  const config = connector.config_metadata as Record<string, unknown>;
  const hasDashboard = !!config.default_dashboard_uid;
  const hasExplore = !!config.datasource_uid;

  useEffect(() => {
    invoke<GrafanaHealthRequest, GrafanaHealth>("grafana_health", {
      envelope: {
        request_id: crypto.randomUUID(),
        command: command("grafana", "health"),
        capability: "ResourceRead",
        scope: { resource_ids: [] },
        payload: { id: connector.id }
      }
    })
      .then((res) => {
        if (res.ok) setHealth(res.value);
        else setError(mapIpcError(res.error, t));
      })
      .catch((err) => setError(mapIpcError(err, t)));
  }, [connector, invoke, t]);

  const openLink = async (target: string) => {
    try {
      let query = "up";
      if (metricContext?.query) query = metricContext.query;
      else if (selectedAlert)
        query = `{${Object.entries(selectedAlert.labels)
          .map(([k, v]) => `${k}="${v}"`)
          .join(",")}}`;

      let start = new Date(Date.now() - 3600000);
      let end = new Date();
      if (metricContext?.start && metricContext?.end) {
        start = new Date(metricContext.start);
        end = new Date(metricContext.end);
      } else if (selectedAlert) {
        start = new Date(selectedAlert.starts_at);
        if (selectedAlert.state === "resolved" && selectedAlert.ends_at) {
          end = new Date(selectedAlert.ends_at);
        }
      }

      const payload: GrafanaLinkRequest = {
        connector_id: connector.id,
        target,
        query,
        start: start.toISOString(),
        end: end.toISOString()
      };
      const res = await invoke<GrafanaLinkRequest, GrafanaLinkResult>("grafana_link", {
        envelope: {
          request_id: crypto.randomUUID(),
          command: command("grafana", "link"),
          capability: "ResourceRead",
          scope: { resource_ids: [] },
          payload
        }
      });
      if (res.ok) {
        const url = res.value.url;
        await open(url);
      } else {
        setError(mapIpcError(res.error, t));
      }
    } catch (err: unknown) {
      setError(mapIpcError(err, t));
    }
  };

  return (
    <Card titleKey="observability.grafana">
      <h3>{connector.display_name}</h3>
      {error && (
        <p role="status" className="error">
          {error}
        </p>
      )}
      {health && (
        <p>
          {t("observability.grafanaVersion", {
            version: health.version,
            database: health.database
          })}
        </p>
      )}
      {hasDashboard && (
        <button type="button" onClick={() => openLink("dashboard")}>
          {t("observability.openDashboard")}
        </button>
      )}
      {hasExplore && (
        <button type="button" onClick={() => openLink("explore")}>
          {t("observability.openExplore")}
        </button>
      )}
    </Card>
  );
}
