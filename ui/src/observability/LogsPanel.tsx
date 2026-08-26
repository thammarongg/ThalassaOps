import { useEffect, useState } from "react";
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

const logQueryFromAlert = (alert?: NormalizedAlert) => {
  if (!alert) return "";
  const namespace = alert.labels.namespace;
  if (!namespace) return "";
  const workloadKey = workloadLabelKeys.find((key) => alert.labels[key]);
  if (!workloadKey) return "";
  return `{namespace="${escapeLogLabelValue(namespace)}", ${workloadKey}="${escapeLogLabelValue(alert.labels[workloadKey])}"}`;
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
  onTraceSelect
}: {
  connector: ConnectorSummary;
  invoke: Invoke;
  selectedAlert?: NormalizedAlert;
  timeContext?: TimeContext;
  onTraceIdsChange?: (traceIds: string[] | null) => void;
  onTraceSelect?: (traceId: string) => void;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState(() => logQueryFromAlert(selectedAlert));
  const [result, setResult] = useState<LokiQueryResult>();
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    setQuery(logQueryFromAlert(selectedAlert));
    setResult(undefined);
    setError("");
    onTraceIdsChange?.(null);
  }, [onTraceIdsChange, selectedAlert]);

  const run = async () => {
    setError("");
    setResult(undefined);
    onTraceIdsChange?.(null);
    if (!timeContext) {
      setError(t("observability.selectAlertFirst"));
      return;
    }
    if (!query.trim()) {
      setError(t("observability.queryRequired"));
      return;
    }

    setLoading(true);
    try {
      const payload: LokiQueryRangeRequest = {
        connector_id: connector.id,
        query,
        start: timeContext.start,
        end: timeContext.end,
        limit: LOG_LIMIT
      };
      const response = await invoke<LokiQueryRangeRequest, LokiQueryResult>(
        "loki_query_range",
        {
          envelope: {
            request_id: crypto.randomUUID(),
            command: command("loki", "query_range"),
            capability: "ResourceRead",
            scope: { resource_ids: [] },
            payload
          }
        }
      );
      if (response.ok) {
        setResult(response.value);
        onTraceIdsChange?.(uniqueTraceIds(response.value));
      } else {
        setError(mapIpcError(response.error, t));
      }
    } catch (err: unknown) {
      setError(mapIpcError(err, t));
    } finally {
      setLoading(false);
    }
  };

  const hasEntries = result?.streams.some((stream) => stream.entries.length > 0) ?? false;

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
          <button type="button" onClick={run} disabled={loading || !timeContext}>
            {t("observability.runQuery")}
          </button>
        </div>
        {loading && <p role="status">{t("integrations.loading")}</p>}
        {!loading && error && (
          <p role="status" className="error">
            {error}
          </p>
        )}
        {!loading && !error && result && result.unparsed_count > 0 && (
          <p role="status" className="logs-panel__unparsed">
            {t("observability.unparsedWarning", { count: result.unparsed_count })}
          </p>
        )}
        {!loading && !error && result && !hasEntries && (
          <p role="status">{t("observability.noData")}</p>
        )}
        {!loading && !error && result && hasEntries && (
          <div className="logs-panel__results">
            {result.streams.map((stream, streamIndex) => (
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
                        onClick={() => onTraceSelect?.(entry.trace_id as string)}
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
