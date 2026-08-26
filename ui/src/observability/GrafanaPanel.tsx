import { useEffect, useState } from "react";
import type {
  ConnectorSummary,
  GrafanaHealth,
  GrafanaHealthRequest,
  GrafanaLinkRequest,
  GrafanaLinkResult,
  Invoke,
  NormalizedAlert
} from "../../contracts/ipc";
import { command } from "../../contracts/ipc";
import { Card } from "../design-system/components";
import { useTranslation } from "../i18n";
import { open } from "@tauri-apps/plugin-shell";
import type { TimeContext } from "./timeContext";

const mapIpcError = (err: unknown, t: (key: string) => string) => {
  const e = err as Record<string, unknown>;
  const code = e?.code;
  if (code === "CONNECTOR_UNAVAILABLE") return t("observability.unavailable");
  if (code === "POLICY_DENIED") return t("observability.denied");
  if (code === "MALFORMED_RESPONSE") return t("observability.malformed");
  return t("observability.unknownError");
};

export function GrafanaPanel({
  connector,
  invoke,
  metricContext,
  selectedAlert,
  timeContext
}: {
  connector: ConnectorSummary;
  invoke: Invoke;
  metricContext?: { query: string; type: string; start?: string; end?: string };
  selectedAlert?: NormalizedAlert;
  timeContext?: TimeContext;
}) {
  const { t } = useTranslation();
  const [health, setHealth] = useState<GrafanaHealth>();
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);

  const config = connector.config_metadata as Record<string, unknown>;
  const hasDashboard = !!config.default_dashboard_uid;
  const hasExplore = !!config.datasource_uid;

  useEffect(() => {
    setLoading(true);
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
      .catch((err) => setError(mapIpcError(err, t)))
      .finally(() => setLoading(false));
  }, [connector, invoke, t]);

  const openLink = async (target: string) => {
    try {
      if (!metricContext && !selectedAlert) return;
      let query = "";
      if (metricContext?.query) query = metricContext.query;
      else if (selectedAlert)
        query = `{${Object.entries(selectedAlert.labels)
          .map(
            ([k, v]) =>
              `${k}="${v.replace(/\\/g, "\\\\").replace(/"/g, '\\"').replace(/\n/g, "\\n")}"`
          )
          .join(",")}}`;

      let start = new Date(Date.now() - 3600000);
      let end = new Date();
      if (timeContext) {
        start = new Date(timeContext.start);
        end = new Date(timeContext.end);
      } else if (metricContext?.start && metricContext?.end) {
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
      {loading && <p role="status">{t("integrations.loading")}</p>}
      {!loading && error && (
        <p role="status" className="error">
          {error}
        </p>
      )}
      {!loading && health && (
        <p>
          {t("observability.grafanaVersion", {
            version: health.version,
            database: health.database
          })}
        </p>
      )}
      {hasDashboard && (
        <button
          type="button"
          onClick={() => openLink("dashboard")}
          disabled={!metricContext && !selectedAlert}
        >
          {t("observability.openDashboard")}
        </button>
      )}
      {hasExplore && (
        <button
          type="button"
          onClick={() => openLink("explore")}
          disabled={!metricContext && !selectedAlert}
        >
          {t("observability.openExplore")}
        </button>
      )}
    </Card>
  );
}
