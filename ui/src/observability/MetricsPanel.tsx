import { useEffect, useRef, useState } from "react";
import type {
  ConnectorSummary,
  Invoke,
  NormalizedAlert,
  PrometheusQueryRangeRequest,
  PrometheusQueryRequest,
  PrometheusQueryResult,
  ResourceReference
} from "../../contracts/ipc";
import { command } from "../../contracts/ipc";
import { Card, Table } from "../design-system/components";
import { useTranslation } from "../i18n";
import type { TimeContext } from "./timeContext";

const mapIpcError = (err: unknown, t: (key: string) => string) => {
  const e = err as Record<string, unknown>;
  const code = e?.code;
  if (code === "CONNECTOR_UNAVAILABLE") return t("observability.unavailable");
  if (code === "POLICY_DENIED") return t("observability.denied");
  if (code === "MALFORMED_RESPONSE") return t("observability.malformed");
  return t("observability.unknownError");
};

export function MetricsPanel({
  connector,
  invoke,
  onMetricContext,
  selectedAlert,
  timeContext,
  resetKey
}: {
  connector: ConnectorSummary;
  invoke: Invoke;
  onMetricContext: (
    resetKey: number,
    ctx: { query: string; type: string; start?: string; end?: string }
  ) => void;
  selectedAlert?: NormalizedAlert;
  timeContext?: TimeContext;
  resetKey: number;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [result, setResult] = useState<PrometheusQueryResult>();
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [type, setType] = useState("instant");
  const resetKeyRef = useRef(resetKey);
  resetKeyRef.current = resetKey;
  const [resultKey, setResultKey] = useState<number>();
  const [errorKey, setErrorKey] = useState<number>();
  const [loadingKey, setLoadingKey] = useState<number>();

  useEffect(() => {
    setResult(undefined);
    setResultKey(undefined);
    setError("");
    setErrorKey(undefined);
    setLoading(false);
    setLoadingKey(undefined);
  }, [resetKey]);

  useEffect(() => {
    if (selectedAlert) {
      const match = Object.entries(selectedAlert.labels)
        .map(
          ([k, v]) =>
            `${k}="${v.replace(/\\/g, "\\\\").replace(/"/g, '\\"').replace(/\n/g, "\\n")}"`
        )
        .join(",");
      setQuery(`{${match}}`);
    }
  }, [selectedAlert]);

  const run = async () => {
    const requestKey = resetKey;
    setError("");
    setErrorKey(undefined);
    setLoading(true);
    setLoadingKey(requestKey);
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
          if (requestKey !== resetKeyRef.current) return;
          setResult(res.value);
          setResultKey(requestKey);
          onMetricContext(requestKey, { query, type: "instant" });
        } else {
          if (requestKey !== resetKeyRef.current) return;
          setError(mapIpcError(res.error, t));
          setErrorKey(requestKey);
        }
      } else {
        if (!timeContext) {
          setError(t("observability.selectAlertFirst"));
          setErrorKey(requestKey);
          setLoading(false);
          return;
        }
        const rangeContext = timeContext;
        const payload: PrometheusQueryRangeRequest = {
          connector_id: connector.id,
          query,
          start: rangeContext.start,
          end: rangeContext.end,
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
          if (requestKey !== resetKeyRef.current) return;
          setResult(res.value);
          setResultKey(requestKey);
          onMetricContext(requestKey, {
            query,
            type: "range",
            start: rangeContext.start,
            end: rangeContext.end
          });
        } else {
          if (requestKey !== resetKeyRef.current) return;
          setError(mapIpcError(res.error, t));
          setErrorKey(requestKey);
        }
      }
    } catch (err: unknown) {
      if (requestKey !== resetKeyRef.current) return;
      setError(mapIpcError(err, t));
      setErrorKey(requestKey);
    } finally {
      if (requestKey === resetKeyRef.current) {
        setLoading(false);
      }
    }
  };

  const visibleResult = resultKey === resetKey ? result : undefined;
  const visibleError = errorKey === resetKey ? error : "";
  const visibleLoading = loadingKey === resetKey && loading;

  const renderResource = (ref: ResourceReference) => {
    if ("resolved" in ref) {
      const r = ref.resolved;
      return `${r.kind} ${r.namespace}/${r.name}`;
    }
    return t("observability.unresolved", { reason: ref.unresolved.reason });
  };

  return (
    <Card titleKey="observability.prometheus">
      <h3>{connector.display_name}</h3>
      {selectedAlert && (
        <div style={{ marginBottom: "1rem", padding: "0.5rem", background: "#f5f5f5" }}>
          <strong>{t("observability.context")}: </strong>
          <span>{renderResource(selectedAlert.resource_reference)}</span>
          <br />
          <small>
            {Object.entries(selectedAlert.labels)
              .map(([k, v]) => `${k}=${v}`)
              .join(", ")}
          </small>
        </div>
      )}
      <div className="query-builder">
        <label>
          {t("observability.queryType")}:
          <select value={type} onChange={(e) => setType(e.target.value)}>
            <option value="instant">{t("observability.instant")}</option>
            <option value="range">{t("observability.range")}</option>
          </select>
        </label>
        <label>
          {t("observability.promqlQuery")}:
          <input value={query} onChange={(e) => setQuery(e.target.value)} />
        </label>
        <button type="button" onClick={run} disabled={visibleLoading}>
          {t("observability.runQuery")}
        </button>
      </div>
      {visibleLoading && <p role="status">{t("integrations.loading")}</p>}
      {!visibleLoading && visibleError && (
        <p role="status" className="error">
          {visibleError}
        </p>
      )}
      {!visibleLoading && !visibleError && visibleResult && visibleResult.series.length === 0 && (
        <p role="status">{t("observability.noData")}</p>
      )}
      {!visibleLoading && !visibleError && visibleResult && visibleResult.series.length > 0 && (
        <div>
          <p>{visibleResult.source.endpoint}</p>
          {visibleResult.series.map((s, i) => (
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
                  timestamp: new Date(samp.timestamp * 1000).toLocaleString(),
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
