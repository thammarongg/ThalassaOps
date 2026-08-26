import { useEffect, useState } from "react";
import type {
  ConnectorSummary,
  Invoke,
  SpanSummary,
  TempoTraceRequest,
  TraceResult
} from "../../contracts/ipc";
import { command } from "../../contracts/ipc";
import { Card, Table } from "../design-system/components";
import { useTranslation } from "../i18n";
import type { TimeContext } from "./timeContext";

const ALLOWED_SPAN_ATTRIBUTES = [
  "http.status_code",
  "http.method",
  "http.route",
  "rpc.service",
  "rpc.method",
  "db.system",
  "exception.type",
  "otel.status_description"
] as const;

const mapIpcError = (err: unknown, t: (key: string) => string) => {
  const e = err as Record<string, unknown>;
  const code = e?.code;
  if (code === "CONNECTOR_UNAVAILABLE") return t("observability.unavailable");
  if (code === "POLICY_DENIED") return t("observability.denied");
  if (code === "MALFORMED_RESPONSE") return t("observability.malformed");
  return t("observability.unknownError");
};

const orderSpans = (spans: SpanSummary[]) => {
  const knownSpanIds = new Set(spans.map((span) => span.span_id));
  const children = new Map<string, SpanSummary[]>();
  const roots: SpanSummary[] = [];

  for (const span of spans) {
    if (!span.parent_span_id || !knownSpanIds.has(span.parent_span_id)) {
      roots.push(span);
      continue;
    }
    const siblings = children.get(span.parent_span_id) ?? [];
    siblings.push(span);
    children.set(span.parent_span_id, siblings);
  }

  const ordered: Array<{ span: SpanSummary; depth: number }> = [];
  const visited = new Set<string>();
  const visit = (span: SpanSummary, depth: number) => {
    if (visited.has(span.span_id)) return;
    visited.add(span.span_id);
    ordered.push({ span, depth });
    for (const child of children.get(span.span_id) ?? []) visit(child, depth + 1);
  };

  roots.forEach((span) => visit(span, 0));
  spans.forEach((span) => visit(span, 0));
  return ordered;
};

const renderAttributes = (span: SpanSummary) => {
  const attributes = Object.entries(span.attributes).filter(([key]) =>
    ALLOWED_SPAN_ATTRIBUTES.includes(key as (typeof ALLOWED_SPAN_ATTRIBUTES)[number])
  );
  if (!attributes.length) return <span aria-hidden="true">—</span>;
  return (
    <ul className="trace-panel__attributes">
      {attributes.map(([key, value]) => (
        <li key={key}>
          <code>{key}</code>: {value}
        </li>
      ))}
    </ul>
  );
};

export function TracePanel({
  connector,
  invoke,
  timeContext,
  traceId,
  traceIds
}: {
  connector: ConnectorSummary;
  invoke: Invoke;
  timeContext?: TimeContext;
  traceId?: string;
  traceIds?: string[] | null;
}) {
  const { t } = useTranslation();
  const [result, setResult] = useState<TraceResult>();
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!traceId) {
      setResult(undefined);
      setError("");
      setLoading(false);
      return;
    }

    setResult(undefined);
    setError("");
    setLoading(true);
    const payload: TempoTraceRequest = { connector_id: connector.id, trace_id: traceId };
    invoke<TempoTraceRequest, TraceResult>("tempo_trace", {
      envelope: {
        request_id: crypto.randomUUID(),
        command: command("tempo", "trace"),
        capability: "ResourceRead",
        scope: { resource_ids: [] },
        payload
      }
    })
      .then((response) => {
        if (response.ok) setResult(response.value);
        else setError(mapIpcError(response.error, t));
      })
      .catch((err: unknown) => setError(mapIpcError(err, t)))
      .finally(() => setLoading(false));
  }, [connector.id, invoke, t, traceId]);

  const orderedSpans = result ? orderSpans(result.spans) : [];
  const hasQueriedLogs = traceIds !== null && traceIds !== undefined;
  const hasTraceId = (traceIds?.length ?? 0) > 0;

  return (
    <Card titleKey="observability.tempo">
      <div className="trace-panel">
        <h3>{connector.display_name}</h3>
        {timeContext && (
          <p
            className="trace-panel__window"
            data-start={timeContext.start}
            data-end={timeContext.end}
          >
            {t("observability.traceWindow", {
              start: timeContext.start,
              end: timeContext.end
            })}
          </p>
        )}
        {!hasQueriedLogs && !traceId && <p>{t("observability.traceAwaitingLogs")}</p>}
        {hasQueriedLogs && !hasTraceId && !traceId && (
          <p role="status">{t("observability.noTraceId")}</p>
        )}
        {hasQueriedLogs && hasTraceId && !traceId && (
          <p>{t("observability.selectTrace")}</p>
        )}
        {loading && <p role="status">{t("integrations.loading")}</p>}
        {!loading && error && (
          <p role="status" className="error">
            {error}
          </p>
        )}
        {!loading && !error && result && result.spans.length === 0 && (
          <p role="status">{t("observability.noData")}</p>
        )}
        {!loading && !error && result && result.spans.length > 0 && (
          <Table
            captionKey="observability.traceSpans"
            columns={[
              { key: "depth", headerKey: "observability.spanDepth" },
              { key: "name", headerKey: "observability.spanName" },
              { key: "service", headerKey: "observability.serviceName" },
              { key: "duration", headerKey: "observability.duration" },
              { key: "status", headerKey: "observability.spanStatus" },
              { key: "attributes", headerKey: "observability.attributes" }
            ]}
            rows={orderedSpans.map(({ span, depth }) => ({
              id: span.span_id,
              depth: <span className="trace-panel__depth">{depth}</span>,
              name: (
                <span
                  className="trace-panel__span-name"
                  style={{ paddingInlineStart: `${depth * 0.75}rem` }}
                >
                  {span.name}
                </span>
              ),
              service: span.service_name,
              duration: (
                <>
                  <code>{span.duration_nano}</code>{" "}
                  <small>{t("observability.durationUnit")}</small>
                </>
              ),
              status: span.status,
              attributes: renderAttributes(span)
            }))}
          />
        )}
      </div>
    </Card>
  );
}
