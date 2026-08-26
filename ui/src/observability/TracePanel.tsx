import { useEffect, useRef, useState } from "react";
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
  traceIds,
  resetKey
}: {
  connector: ConnectorSummary;
  invoke: Invoke;
  timeContext?: TimeContext;
  traceId?: string;
  traceIds?: string[] | null;
  resetKey: number;
}) {
  const { t } = useTranslation();
  const [result, setResult] = useState<TraceResult>();
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const resetKeyRef = useRef(resetKey);
  resetKeyRef.current = resetKey;
  const traceIdRef = useRef(traceId);
  traceIdRef.current = traceId;
  const [resultKey, setResultKey] = useState<number>();
  const [errorKey, setErrorKey] = useState<number>();
  const [loadingKey, setLoadingKey] = useState<number>();

  useEffect(() => {
    if (!traceId) {
      setResult(undefined);
      setResultKey(undefined);
      setError("");
      setErrorKey(undefined);
      setLoading(false);
      setLoadingKey(undefined);
      return;
    }

    setResult(undefined);
    setResultKey(undefined);
    setError("");
    setErrorKey(undefined);
    setLoading(true);
    setLoadingKey(resetKey);
    const requestKey = resetKey;
    const requestTraceId = traceId;
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
        if (requestKey !== resetKeyRef.current || requestTraceId !== traceIdRef.current) return;
        if (response.ok) {
          setResult(response.value);
          setResultKey(requestKey);
        } else {
          setError(mapIpcError(response.error, t));
          setErrorKey(requestKey);
        }
      })
      .catch((err: unknown) => {
        if (requestKey !== resetKeyRef.current || requestTraceId !== traceIdRef.current) return;
        setError(mapIpcError(err, t));
        setErrorKey(requestKey);
      })
      .finally(() => {
        if (requestKey === resetKeyRef.current && requestTraceId === traceIdRef.current) {
          setLoading(false);
        }
      });
  }, [connector.id, invoke, resetKey, t, traceId]);

  const visibleResult = resultKey === resetKey ? result : undefined;
  const visibleError = errorKey === resetKey ? error : "";
  const visibleLoading = loadingKey === resetKey && loading;
  const orderedSpans = visibleResult ? orderSpans(visibleResult.spans) : [];
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
        {hasQueriedLogs && hasTraceId && !traceId && <p>{t("observability.selectTrace")}</p>}
        {visibleLoading && <p role="status">{t("integrations.loading")}</p>}
        {!visibleLoading && visibleError && (
          <p role="status" className="error">
            {visibleError}
          </p>
        )}
        {!visibleLoading && !visibleError && visibleResult && visibleResult.spans.length === 0 && (
          <p role="status">{t("observability.noData")}</p>
        )}
        {!visibleLoading && !visibleError && visibleResult && visibleResult.spans.length > 0 && (
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
                  <code>{span.duration_nano}</code> <small>{t("observability.durationUnit")}</small>
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
