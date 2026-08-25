import React, { useState, useEffect } from "react";
import type {
  ConnectorSummary,
  IpcResult,
  NormalizedAlert,
  PrometheusQueryResult,
  GrafanaHealth,
  GrafanaLinkResult
} from "../contracts/ipc";
import { command } from "../contracts/ipc";
import { Card, EmptyState, Table } from "./design-system/components";
import { useTranslation } from "./i18n";
import { open } from "@tauri-apps/plugin-shell";

type Invoke = (command: string, args: Record<string, unknown>) => Promise<IpcResult<unknown>>;

export function ObservabilityWorkspace({ invoke }: { invoke: Invoke }) {
  const { t } = useTranslation();
  const [connectors, setConnectors] = useState<ConnectorSummary[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    invoke("connector_list", {
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
          const all = result.value as ConnectorSummary[];
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
          <AlertmanagerPanel key={c.id} connector={c} invoke={invoke} />
        ))}
      </section>
      <section aria-label={t("observability.prometheus")}>
        <h2>{t("observability.prometheus")}</h2>
        {prom.map((c) => (
          <PrometheusPanel key={c.id} connector={c} invoke={invoke} />
        ))}
      </section>
      <section aria-label={t("observability.grafana")}>
        <h2>{t("observability.grafana")}</h2>
        {graf.map((c) => (
          <GrafanaPanel key={c.id} connector={c} invoke={invoke} />
        ))}
      </section>
    </div>
  );
}

function AlertmanagerPanel({ connector, invoke }: { connector: ConnectorSummary; invoke: Invoke }) {
  const [alerts, setAlerts] = useState<NormalizedAlert[]>([]);
  const [error, setError] = useState("");

  useEffect(() => {
    invoke("alertmanager_alerts", {
      envelope: {
        request_id: crypto.randomUUID(),
        command: command("alertmanager", "alerts"),
        capability: "ResourceRead",
        scope: { resource_ids: [] },
        payload: { connector_id: connector.id }
      }
    })
      .then((res) => {
        if (res.ok) setAlerts(res.value as NormalizedAlert[]);
        else setError(res.error.message);
      })
      .catch((err) => setError(String(err)));
  }, [connector, invoke]);

  return (
    <Card titleKey="observability.alertmanager">
      <h3>{connector.display_name}</h3>
      {error && (
        <p role="status" className="error">
          {error}
        </p>
      )}
      <Table
        captionKey="observability.alerts"
        columns={[
          { key: "state", headerKey: "observability.state" },
          { key: "fingerprint", headerKey: "observability.fingerprint" },
          { key: "labels", headerKey: "observability.labels" }
        ]}
        rows={alerts.map((a) => ({
          id: a.fingerprint,
          state: a.state,
          fingerprint: a.fingerprint,
          labels: JSON.stringify(a.labels)
        }))}
      />
    </Card>
  );
}

function PrometheusPanel({ connector, invoke }: { connector: ConnectorSummary; invoke: Invoke }) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [result, setResult] = useState<PrometheusQueryResult>();
  const [error, setError] = useState("");
  const [type, setType] = useState("instant");

  const run = async () => {
    setError("");
    const cmd = type === "instant" ? "prometheus_query" : "prometheus_query_range";
    const payload: Record<string, string> = { connector_id: connector.id, query };
    if (type === "instant") {
      payload.time = new Date().toISOString();
    } else {
      const end = new Date();
      const start = new Date(end.getTime() - 3600000);
      payload.start = start.toISOString();
      payload.end = end.toISOString();
      payload.step = "60s";
    }
    try {
      const res = await invoke(cmd, {
        envelope: {
          request_id: crypto.randomUUID(),
          command: command("prometheus", type === "instant" ? "query" : "query_range"),
          capability: "ResourceRead",
          scope: { resource_ids: [] },
          payload
        }
      });
      if (res.ok) setResult(res.value as PrometheusQueryResult);
      else setError(res.error.message);
    } catch (err: unknown) {
      setError(String(err));
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
          <ul>
            {result.series.map((s, i) => (
              <li key={i}>
                {JSON.stringify(s.labels)}:{" "}
                {t("observability.samples", { count: String(s.samples.length) })}
              </li>
            ))}
          </ul>
        </div>
      )}
    </Card>
  );
}

function GrafanaPanel({ connector, invoke }: { connector: ConnectorSummary; invoke: Invoke }) {
  const { t } = useTranslation();
  const [health, setHealth] = useState<GrafanaHealth>();
  const [error, setError] = useState("");

  useEffect(() => {
    invoke("grafana_health", {
      envelope: {
        request_id: crypto.randomUUID(),
        command: command("grafana", "health"),
        capability: "ResourceRead",
        scope: { resource_ids: [] },
        payload: { id: connector.id }
      }
    })
      .then((res) => {
        if (res.ok) setHealth(res.value as GrafanaHealth);
        else setError(res.error.message);
      })
      .catch((err) => setError(String(err)));
  }, [connector, invoke]);

  const openLink = async (target: string) => {
    try {
      const end = new Date();
      const start = new Date(end.getTime() - 3600000);
      const payload = {
        connector_id: connector.id,
        target,
        query: "up",
        start: start.toISOString(),
        end: end.toISOString()
      };
      const res = await invoke("grafana_link", {
        envelope: {
          request_id: crypto.randomUUID(),
          command: command("grafana", "link"),
          capability: "ResourceRead",
          scope: { resource_ids: [] },
          payload
        }
      });
      if (res.ok) {
        const url = (res.value as GrafanaLinkResult).url;
        await open(url);
      } else {
        setError(res.error.message);
      }
    } catch (err: unknown) {
      setError(String(err));
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
      <button type="button" onClick={() => openLink("dashboard")}>
        {t("observability.openDashboard")}
      </button>
      <button type="button" onClick={() => openLink("explore")}>
        {t("observability.openExplore")}
      </button>
    </Card>
  );
}
