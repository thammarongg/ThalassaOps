import { useEffect, useRef, useState } from "react";
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
  timeContext,
  resetKey
}: {
  connector: ConnectorSummary;
  invoke: Invoke;
  metricContext?: { query: string; type: string; start?: string; end?: string };
  selectedAlert?: NormalizedAlert;
  timeContext?: TimeContext;
  resetKey: number;
}) {
  const { t } = useTranslation();
  const [health, setHealth] = useState<GrafanaHealth>();
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const resetKeyRef = useRef(resetKey);
  resetKeyRef.current = resetKey;
  const [errorKey, setErrorKey] = useState<number>();
  const [loadingKey, setLoadingKey] = useState<number>(resetKey);

  const config = connector.config_metadata as Record<string, unknown>;
  const hasDashboard = !!config.default_dashboard_uid;
  const hasExplore = !!config.datasource_uid;

  useEffect(() => {
    setHealth(undefined);
    setError("");
    setErrorKey(undefined);
    setLoading(false);
    setLoadingKey(undefined);
  }, [resetKey]);

  useEffect(() => {
    const requestKey = resetKey;
    setLoading(true);
    setLoadingKey(requestKey);
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
        if (requestKey !== resetKeyRef.current) return;
        if (res.ok) setHealth(res.value);
        else {
          setError(mapIpcError(res.error, t));
          setErrorKey(requestKey);
        }
      })
      .catch((err) => {
        if (requestKey !== resetKeyRef.current) return;
        setError(mapIpcError(err, t));
        setErrorKey(requestKey);
      })
      .finally(() => {
        if (requestKey === resetKeyRef.current) {
          setLoading(false);
        }
      });
  }, [connector, invoke, resetKey, t]);

  const openLink = async (target: string) => {
    const requestKey = resetKey;
    try {
      if (!timeContext) {
        setError(t("observability.selectAlertFirst"));
        setErrorKey(requestKey);
        return;
      }
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

      const payload: GrafanaLinkRequest = {
        connector_id: connector.id,
        target,
        query,
        start: timeContext.start,
        end: timeContext.end
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
        if (requestKey !== resetKeyRef.current) return;
        const url = res.value.url;
        await open(url);
      } else {
        if (requestKey !== resetKeyRef.current) return;
        setError(mapIpcError(res.error, t));
        setErrorKey(requestKey);
      }
    } catch (err: unknown) {
      if (requestKey !== resetKeyRef.current) return;
      setError(mapIpcError(err, t));
      setErrorKey(requestKey);
    }
  };

  const visibleError = errorKey === resetKey ? error : "";
  const visibleLoading = loadingKey === resetKey && loading;

  return (
    <Card titleKey="observability.grafana">
      <h3>{connector.display_name}</h3>
      {visibleLoading && <p role="status">{t("integrations.loading")}</p>}
      {!visibleLoading && visibleError && (
        <p role="status" className="error">
          {visibleError}
        </p>
      )}
      {!visibleLoading && health && (
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
          disabled={!timeContext || (!metricContext && !selectedAlert)}
        >
          {t("observability.openDashboard")}
        </button>
      )}
      {hasExplore && (
        <button
          type="button"
          onClick={() => openLink("explore")}
          disabled={!timeContext || (!metricContext && !selectedAlert)}
        >
          {t("observability.openExplore")}
        </button>
      )}
    </Card>
  );
}
