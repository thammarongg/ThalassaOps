import { useEffect, useRef, useState } from "react";
import type {
  ConnectorSummary,
  Invoke,
  LokiQueryRangeRequest,
  LokiQueryResult,
  NormalizedAlert
} from "../../contracts/ipc";
import { command } from "../../contracts/ipc";
import { Card, Table } from "../design-system/components";
import { useTranslation } from "../i18n";
import type { TimeContext } from "./timeContext";

const LOG_LIMIT = 200;
const workloadLabelKeys = ["pod", "service", "deployment"] as const;

const mapIpcError = (err: unknown, t: (key: string) => string) => {
  const e = err as Record<string, unknown>;
  const code = e?.code;
  if (code === "CONNECTOR_UNAVAILABLE") return t("observability.unavailable");
  if (code === "POLICY_DENIED") return t("observability.denied");
  if (code === "MALFORMED_RESPONSE") return t("observability.malformed");
  return t("observability.unknownError");
};

const escapeLogLabelValue = (value: string) =>
  value.replace(/\\/g, "\\\\").replace(/"/g, '\\"').replace(/\n/g, "\\n");

type LogQueryState = {
  query: string;
  errorKey?:
    | "observability.logQueryMissingNamespace"
    | "observability.logQueryMissingWorkload"
    | "observability.logQueryAmbiguousWorkload";
};

const logQueryFromAlert = (alert?: NormalizedAlert): LogQueryState => {
  if (!alert) return { query: "" };
  const namespace = alert.labels.namespace?.trim();
  if (!namespace) {
    return { query: "", errorKey: "observability.logQueryMissingNamespace" };
  }
  const workloadKeys = workloadLabelKeys.filter((key) => alert.labels[key]?.trim());
  if (workloadKeys.length === 0) {
    return { query: "", errorKey: "observability.logQueryMissingWorkload" };
  }
  if (workloadKeys.length > 1) {
    return { query: "", errorKey: "observability.logQueryAmbiguousWorkload" };
  }
  const workloadKey = workloadKeys[0];
  const workload = alert.labels[workloadKey].trim();
  return {
    query: `{namespace="${escapeLogLabelValue(namespace)}", ${workloadKey}="${escapeLogLabelValue(workload)}"}`
  };
};

const uniqueTraceIds = (result: LokiQueryResult) =>
  Array.from(
    new Set(
      result.streams.flatMap((stream) =>
        stream.entries.flatMap((entry) => (entry.trace_id ? [entry.trace_id] : []))
      )
    )
  );

export function LogsPanel({
  connector,
  invoke,
  selectedAlert,
  timeContext,
  onTraceIdsChange,
  onTraceSelect,
  resetKey
}: {
  connector: ConnectorSummary;
  invoke: Invoke;
  selectedAlert?: NormalizedAlert;
  timeContext?: TimeContext;
  onTraceIdsChange?: (resetKey: number, traceIds: string[] | null) => void;
  onTraceSelect?: (resetKey: number, traceId: string) => void;
  resetKey: number;
}) {
  const { t } = useTranslation();
  const alertQuery = logQueryFromAlert(selectedAlert);
  const [query, setQuery] = useState(() => alertQuery.query);
  const [result, setResult] = useState<LokiQueryResult>();
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const resetKeyRef = useRef(resetKey);
  resetKeyRef.current = resetKey;
  const [resultKey, setResultKey] = useState<number>();
  const [errorKey, setErrorKey] = useState<number>();
  const [loadingKey, setLoadingKey] = useState<number>();

  useEffect(() => {
    setQuery(logQueryFromAlert(selectedAlert).query);
  }, [selectedAlert]);

  useEffect(() => {
    setResult(undefined);
    setResultKey(undefined);
    setError("");
    setErrorKey(undefined);
    setLoading(false);
    setLoadingKey(undefined);
    onTraceIdsChange?.(resetKey, null);
  }, [onTraceIdsChange, resetKey]);

  const run = async () => {
    const requestKey = resetKey;
    setError("");
    setErrorKey(undefined);
    setResult(undefined);
    setResultKey(undefined);
    onTraceIdsChange?.(requestKey, null);
    if (!timeContext) {
      setError(t("observability.selectAlertFirst"));
      setErrorKey(requestKey);
      return;
    }
    if (!query.trim()) {
      setError(t("observability.queryRequired"));
      setErrorKey(requestKey);
      return;
    }

    setLoading(true);
    setLoadingKey(requestKey);
    try {
      const payload: LokiQueryRangeRequest = {
        connector_id: connector.id,
        query,
        start: timeContext.start,
        end: timeContext.end,
        limit: LOG_LIMIT
      };
      const response = await invoke<LokiQueryRangeRequest, LokiQueryResult>("loki_query_range", {
        envelope: {
          request_id: crypto.randomUUID(),
          command: command("loki", "query_range"),
          capability: "ResourceRead",
          scope: { resource_ids: [] },
          payload
        }
      });
      if (response.ok) {
        if (requestKey !== resetKeyRef.current) return;
        setResult(response.value);
        setResultKey(requestKey);
        onTraceIdsChange?.(requestKey, uniqueTraceIds(response.value));
      } else {
        if (requestKey !== resetKeyRef.current) return;
        setError(mapIpcError(response.error, t));
        setErrorKey(requestKey);
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
  const hasEntries = visibleResult?.streams.some((stream) => stream.entries.length > 0) ?? false;

  return (
    <Card titleKey="observability.loki">
      <div className="logs-panel">
        <h3>{connector.display_name}</h3>
        <div className="logs-panel__query">
          <label htmlFor={`loki-query-${connector.id}`}>{t("observability.logqlQuery")}</label>
          <input
            id={`loki-query-${connector.id}`}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
          <button type="button" onClick={run} disabled={visibleLoading || !timeContext}>
            {t("observability.runQuery")}
          </button>
        </div>
        {alertQuery.errorKey && (
          <p role="status" className="logs-panel__query-warning">
            {t(alertQuery.errorKey)}
          </p>
        )}
        {visibleLoading && <p role="status">{t("integrations.loading")}</p>}
        {!visibleLoading && visibleError && (
          <p role="status" className="error">
            {visibleError}
          </p>
        )}
        {!visibleLoading && !visibleError && visibleResult && visibleResult.unparsed_count > 0 && (
          <p role="status" className="logs-panel__unparsed">
            {t("observability.unparsedWarning", { count: visibleResult.unparsed_count })}
          </p>
        )}
        {!visibleLoading && !visibleError && visibleResult && !hasEntries && (
          <p role="status">{t("observability.noData")}</p>
        )}
        {!visibleLoading && !visibleError && visibleResult && hasEntries && (
          <div className="logs-panel__results">
            {visibleResult.streams.map((stream, streamIndex) => (
              <section className="logs-panel__stream" key={`${connector.id}-${streamIndex}`}>
                <p className="logs-panel__labels">
                  <strong>{t("observability.streamLabels")}:</strong>{" "}
                  <code>
                    {Object.entries(stream.labels)
                      .map(([key, value]) => `${key}=${value}`)
                      .join(", ") || t("observability.noLabels")}
                  </code>
                </p>
                <Table
                  captionKey="observability.logEntries"
                  columns={[
                    { key: "timestamp", headerKey: "observability.timestamp" },
                    { key: "line", headerKey: "observability.logLine" },
                    { key: "trace", headerKey: "observability.trace" }
                  ]}
                  rows={stream.entries.map((entry, entryIndex) => ({
                    id: `${streamIndex}-${entryIndex}-${entry.timestamp_ns}`,
                    timestamp: <code>{entry.timestamp_ns}</code>,
                    line: <pre className="logs-panel__line">{entry.line}</pre>,
                    trace: entry.trace_id ? (
                      <button
                        type="button"
                        className="logs-panel__trace-control"
                        onClick={() => onTraceSelect?.(resetKey, entry.trace_id as string)}
                        aria-label={t("observability.openTrace", { traceId: entry.trace_id })}
                      >
                        {t("observability.openTraceShort")}
                      </button>
                    ) : (
                      <span aria-hidden="true">—</span>
                    )
                  }))}
                />
              </section>
            ))}
          </div>
        )}
      </div>
    </Card>
  );
}
