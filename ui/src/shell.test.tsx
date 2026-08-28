import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { I18nProvider, i18n } from "./i18n";
import { Shell } from "./shell";
import { open } from "@tauri-apps/plugin-shell";
import type { CloudEnvironment, CloudResource } from "../contracts/ipc";

vi.mock("@tauri-apps/plugin-shell", () => ({
  open: vi.fn()
}));
const openMock = vi.mocked(open);

const context = {
  organization_name: "Local Organization",
  team_name: "Local Team",
  workspace_name: "Local Workspace",
  policy_version: 1
};
afterEach(() => {
  cleanup();
  localStorage.clear();
});

it("navigates product areas from the command palette with keyboard and closes it with Escape", async () => {
  const user = userEvent.setup();
  render(
    <I18nProvider>
      <Shell invoke={vi.fn().mockResolvedValue({ ok: true, value: context })} />
    </I18nProvider>
  );
  await user.keyboard("{Control>}k{/Control}");
  const input = await screen.findByRole("textbox", { name: "Command palette" });
  await user.type(input, "inc");
  await user.keyboard("{Enter}");
  expect(screen.getByRole("heading", { name: "Incidents" })).toBeInTheDocument();
  await user.keyboard("{Meta>}k{/Meta}{Escape}");
  expect(screen.queryByRole("dialog", { name: "Command palette" })).not.toBeInTheDocument();
});

it("pins a navigation item and opens the honest terminal placeholder", async () => {
  const user = userEvent.setup();
  render(
    <I18nProvider>
      <Shell invoke={vi.fn().mockResolvedValue({ ok: true, value: context })} />
    </I18nProvider>
  );
  await user.click(screen.getByRole("button", { name: "Pin Incidents" }));
  expect(screen.getByRole("navigation", { name: "Favorites" })).toHaveTextContent("Incidents");
  await user.click(screen.getByRole("button", { name: "Open terminal" }));
  expect(screen.getByRole("dialog", { name: "Terminal" })).toHaveTextContent("not yet available");
  await user.click(screen.getByRole("button", { name: "Open external terminal" }));
  expect(screen.getByRole("status")).toHaveTextContent("not yet available");
});

it("shows an unavailable policy indicator and context error when the context request is denied", async () => {
  render(
    <I18nProvider>
      <Shell
        invoke={vi.fn().mockResolvedValue({
          ok: false,
          error: {
            code: "POLICY_DENIED",
            message: "Policy denied the workspace context request.",
            details: {}
          }
        })}
      />
    </I18nProvider>
  );

  await screen.findByRole("button", {
    name: "Organization: Workspace context is unavailable."
  });
  const policyStatus = screen.getByText("Policy version …").parentElement;
  expect(policyStatus?.querySelector(".indicator")).toHaveClass("indicator--unavailable");
});

it("adds and tests a fixture connector through the integrations IPC commands", async () => {
  const user = userEvent.setup();
  const connector = {
    id: "fixture-1",
    kind: "fixture",
    display_name: "Fixture connector",
    enabled: true,
    config_metadata: {},
    credential_configured: false,
    health_state: "healthy"
  };
  const invoke = vi.fn().mockImplementation((name: string) => {
    if (name === "system_context") return Promise.resolve({ ok: true, value: context });
    if (name === "connector_list")
      return Promise.resolve({
        ok: true,
        value: invoke.mock.calls.some(([command]) => command === "connector_add") ? [connector] : []
      });
    if (name === "connector_add" || name === "connector_test")
      return Promise.resolve({ ok: true, value: connector });
    return Promise.resolve({
      ok: true,
      value: { connector, manifest: { capabilities: [] }, logs: [] }
    });
  });
  render(
    <I18nProvider>
      <Shell invoke={invoke} />
    </I18nProvider>
  );
  await user.click(screen.getByRole("button", { name: "Integrations" }));
  await user.click(await screen.findByRole("button", { name: "Add connector" }));
  await user.type(screen.getByRole("textbox", { name: "Connector" }), "My Fixture");
  await user.click(screen.getByRole("button", { name: "Save configuration" }));
  expect(invoke).toHaveBeenCalledWith(
    "connector_add",
    expect.objectContaining({
      envelope: expect.objectContaining({ command: "connector.add", capability: "ConnectorAct" })
    })
  );
  await user.click(await screen.findByRole("button", { name: "Test connection" }));
  expect(invoke).toHaveBeenCalledWith(
    "connector_test",
    expect.objectContaining({
      envelope: expect.objectContaining({ command: "connector.test", capability: "ConnectorAct" })
    })
  );
});

it("warns when an observability connector uses HTTP without blocking save", async () => {
  const user = userEvent.setup();
  const invoke = vi.fn().mockImplementation((name: string) => {
    if (name === "system_context") return Promise.resolve({ ok: true, value: context });
    if (name === "connector_list") return Promise.resolve({ ok: true, value: [] });
    if (name === "connector_add") return Promise.resolve({ ok: true, value: {} });
    return Promise.resolve({ ok: true, value: {} });
  });

  render(
    <I18nProvider>
      <Shell invoke={invoke} />
    </I18nProvider>
  );

  await user.click(screen.getByRole("button", { name: "Integrations" }));
  await user.click(await screen.findByRole("button", { name: "Add connector" }));
  await user.type(screen.getByLabelText("Connector"), "HTTP Prometheus");
  await user.selectOptions(screen.getByLabelText("Kind"), "prometheus");
  await user.type(screen.getByLabelText("Base URL"), "https://observability.example.test:9090");
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();

  await user.clear(screen.getByLabelText("Base URL"));
  await user.type(screen.getByLabelText("Base URL"), "http://observability.example.test:9090");
  expect(screen.getByRole("alert")).toHaveTextContent("HTTP");
  expect(screen.getByRole("alert")).toHaveTextContent("HTTPS");
  await i18n.changeLanguage("th");
  expect(screen.getByRole("alert")).toHaveTextContent("อนุญาต");
  await i18n.changeLanguage("en");

  await user.click(screen.getByRole("button", { name: "Save configuration" }));
  expect(invoke).toHaveBeenCalledWith("connector_add", expect.anything());
});

it("filters Kubernetes resources by health and shows a masked manifest banner", async () => {
  const user = userEvent.setup();
  const connector = {
    id: "k8s-1",
    kind: "kubernetes",
    display_name: "Cluster",
    enabled: true,
    config_metadata: { context_name: "test" },
    credential_configured: false,
    health_state: "healthy"
  };
  const inventory = {
    availability: [],
    topology: [],
    resources: [
      {
        resource: { kind: "Pod", name: "prod/crashing", labels: {} },
        conditions: [],
        containers: [],
        health: "crash_loop_back_off"
      },
      {
        resource: { kind: "Service", name: "stage/web", labels: {} },
        conditions: [],
        containers: [],
        health: "healthy"
      }
    ]
  };
  const invoke = vi.fn().mockImplementation((name: string) => {
    if (name === "system_context") return Promise.resolve({ ok: true, value: context });
    if (name === "connector_list") return Promise.resolve({ ok: true, value: [connector] });
    if (name === "kubernetes_inventory") return Promise.resolve({ ok: true, value: inventory });
    if (name === "kubernetes_pod_logs") return Promise.resolve({ ok: true, value: "logs" });
    if (name === "kubernetes_pod_events") return Promise.resolve({ ok: true, value: [] });
    if (name === "kubernetes_resource_manifest")
      return Promise.resolve({
        ok: true,
        value: { yaml: "token: <REDACTED>", masked: true, risk_class: "READ-ONLY" }
      });
    return Promise.resolve({ ok: true, value: {} });
  });
  render(
    <I18nProvider>
      <Shell invoke={invoke} />
    </I18nProvider>
  );
  await user.click(screen.getByRole("button", { name: "Integrations" }));
  await user.click(await screen.findByRole("button", { name: "Inspect cluster" }));
  expect(await screen.findByText("crash_loop_back_off")).toBeInTheDocument();
  await user.selectOptions(screen.getByLabelText("Health"), "crash_loop_back_off");
  expect(screen.getByText("Pod/prod/crashing")).toBeInTheDocument();
  expect(screen.queryByText("Service/stage/web")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Pod/prod/crashing" }));
  await user.click(screen.getByRole("button", { name: "View manifest" }));
  expect(await screen.findByRole("status")).toHaveTextContent("Sensitive fields redacted");
});

it("renders observability workspace, lists alerts, runs metric query, and handles context propagation", async () => {
  const user = userEvent.setup();
  const alertmanager = {
    id: "am-1",
    kind: "alertmanager",
    display_name: "AM",
    enabled: true,
    config_metadata: {},
    credential_configured: false,
    health_state: "healthy"
  };
  const prometheus = {
    id: "prom-1",
    kind: "prometheus",
    display_name: "PROM",
    enabled: true,
    config_metadata: {},
    credential_configured: false,
    health_state: "healthy"
  };
  const grafana = {
    id: "graf-1",
    kind: "grafana",
    display_name: "GRAF",
    enabled: true,
    config_metadata: { default_dashboard_uid: "dash1", datasource_uid: "ds1" },
    credential_configured: false,
    health_state: "healthy"
  };

  const invoke = vi.fn().mockImplementation((name: string) => {
    if (name === "system_context") return Promise.resolve({ ok: true, value: context });
    if (name === "connector_list")
      return Promise.resolve({ ok: true, value: [alertmanager, prometheus, grafana] });
    if (name === "alertmanager_alerts")
      return Promise.resolve({
        ok: true,
        value: [
          {
            fingerprint: "123",
            state: "firing",
            starts_at: "2024-01-01T00:00:00Z",
            ends_at: "2024-01-01T01:00:00Z",
            labels: { alertname: "HighCPU" },
            annotations: {},
            resource_reference: { unresolved: { reason: "test" } },
            source: { connector_id: "am-1", endpoint: "/api/v2/alerts" }
          },
          {
            fingerprint: "456",
            state: "firing",
            starts_at: "2024-01-01T00:00:00Z",
            ends_at: "2024-01-01T01:00:00Z",
            labels: { alertname: "LowMemory", pod: "api-server" },
            annotations: {},
            resource_reference: {
              resolved: { namespace: "prod", kind: "Pod", name: "api-server" }
            },
            source: { connector_id: "am-1", endpoint: "/api/v2/alerts" }
          }
        ]
      });
    if (name === "grafana_health")
      return Promise.resolve({ ok: true, value: { version: "10.0", database: "sqlite" } });
    if (name === "prometheus_query")
      return Promise.resolve({
        ok: true,
        value: {
          source: { connector_id: "prom-1", query: "up", endpoint: "/api/v1/query" },
          series: [
            { labels: { instance: "A" }, samples: [{ timestamp: 1700000000, value: "1.5" }] }
          ]
        }
      });
    if (name === "prometheus_query_range")
      return Promise.resolve({
        ok: true,
        value: {
          source: {
            connector_id: "prom-1",
            query: '{alertname="LowMemory",pod="api-server"}',
            endpoint: "/api/v1/query_range"
          },
          series: [
            { labels: { instance: "B" }, samples: [{ timestamp: 1700000000, value: "2.5" }] }
          ]
        }
      });
    if (name === "grafana_link")
      return Promise.resolve({
        ok: true,
        value: { url: "http://localhost/d/dash1?from=1700000000000&to=1700000060000" }
      });
    return Promise.resolve({ ok: true, value: {} });
  });

  render(
    <I18nProvider>
      <Shell invoke={invoke} />
    </I18nProvider>
  );

  await user.click(screen.getByRole("button", { name: "Observability" }));

  // Wait for AM panel
  expect(await screen.findByRole("heading", { name: "AM" })).toBeInTheDocument();

  // Select the unresolved alert using the radio button
  const radioUnresolved = screen.getByRole("radio", { name: "Select alert 123" });
  await user.click(radioUnresolved);

  // Check Prometheus panel got the context (the input should have the label)
  const queryInput = screen.getByRole("textbox");
  expect(queryInput).toHaveValue('{alertname="HighCPU"}');

  // Select the resolved alert
  const radioResolved = screen.getByRole("radio", { name: "Select alert 456" });
  await user.click(radioResolved);

  // Check the resolved context is rendered in Prometheus panel
  await import("@testing-library/react").then((m) =>
    m.waitFor(() => {
      expect(queryInput).toHaveValue('{alertname="LowMemory",pod="api-server"}');
    })
  );
  const elements = screen.getAllByText("Pod prod/api-server");
  expect(elements.length).toBeGreaterThan(0);
  // Run range query
  await user.selectOptions(screen.getByRole("combobox"), "range");
  await user.click(screen.getByRole("button", { name: "Run Query" }));
  expect(await screen.findByText("2.5")).toBeInTheDocument();

  expect(invoke).toHaveBeenCalledWith(
    "prometheus_query_range",
    expect.objectContaining({
      envelope: expect.objectContaining({
        payload: expect.objectContaining({
          query: '{alertname="LowMemory",pod="api-server"}',
          step_seconds: 60
        })
      })
    })
  );
  const rangeCall = invoke.mock.calls.find((c) => c[0] === "prometheus_query_range");
  expect(rangeCall?.[1].envelope.payload.start).toBeTruthy();
  expect(rangeCall?.[1].envelope.payload.end).toBeTruthy();

  expect(openMock).not.toHaveBeenCalled();

  // Grafana open dashboard with context
  await user.click(screen.getByRole("button", { name: "Open Dashboard" }));
  expect(invoke).toHaveBeenCalledWith(
    "grafana_link",
    expect.objectContaining({
      envelope: expect.objectContaining({
        payload: expect.objectContaining({ query: '{alertname="LowMemory",pod="api-server"}' })
      })
    })
  );

  await import("@testing-library/react").then((m) =>
    m.waitFor(() => {
      expect(openMock).toHaveBeenCalledWith(
        "http://localhost/d/dash1?from=1700000000000&to=1700000060000"
      );
      expect(openMock.mock.calls[0]?.[0]).not.toContain("var-query");
    })
  );
});

it("follows an alert through masked Loki logs into an explicit Tempo trace", async () => {
  const user = userEvent.setup();
  const traceId = "4bf92f3577b34da6a3ce929d0e0e4736";
  const alertmanager = {
    id: "am-logs-1",
    kind: "alertmanager",
    display_name: "AM logs",
    enabled: true,
    config_metadata: {},
    credential_configured: false,
    health_state: "healthy"
  };
  const loki = {
    id: "loki-1",
    kind: "loki",
    display_name: "Loki fixture",
    enabled: true,
    config_metadata: {},
    credential_configured: false,
    health_state: "healthy"
  };
  const prometheus = {
    id: "prom-logs-1",
    kind: "prometheus",
    display_name: "Prometheus fixture",
    enabled: true,
    config_metadata: {},
    credential_configured: false,
    health_state: "healthy"
  };
  const tempo = {
    id: "tempo-1",
    kind: "tempo",
    display_name: "Tempo fixture",
    enabled: true,
    config_metadata: {},
    credential_configured: false,
    health_state: "healthy"
  };
  const alert = {
    fingerprint: "logs-123",
    state: "firing",
    starts_at: "2026-08-26T00:00:00Z",
    ends_at: "",
    labels: { alertname: "ApiError", namespace: "prod", pod: "api-0" },
    annotations: {},
    generator_url: null,
    source: { connector_id: "am-logs-1", endpoint: "/api/v2/alerts" },
    resource_reference: { resolved: { namespace: "prod", kind: "Pod", name: "api-0" } }
  };
  const logResult = {
    streams: [
      {
        labels: { namespace: "prod", pod: "api-0" },
        entries: [
          {
            timestamp_ns: "1735689600000000001",
            line: `{"msg":"boom","api_key":"<REDACTED>","trace_id":"${traceId}"}`,
            parsed: true,
            masked: true,
            fields: { api_key: "<REDACTED>", msg: "boom", trace_id: traceId },
            trace_id: traceId
          },
          {
            timestamp_ns: "1735689600000000002",
            line: "plain text line with api_key=<unparsed-fixture-value>",
            parsed: false,
            masked: false,
            fields: null,
            trace_id: null
          }
        ]
      }
    ],
    source: {
      connector_id: "loki-1",
      query: '{namespace="prod", pod="api-0"}',
      endpoint: "/loki/api/v1/query_range"
    },
    unparsed_count: 1
  };
  const traceResult = {
    trace_id: traceId,
    spans: [
      {
        trace_id: traceId,
        span_id: "0123456789abcdef",
        parent_span_id: null,
        name: "GET /orders",
        service_name: "api",
        start_time_unix_nano: "1735689600000000000",
        duration_nano: "123",
        status: "STATUS_CODE_OK",
        attributes: { "http.status_code": "200" }
      }
    ],
    source: { connector_id: "tempo-1", trace_id: traceId, endpoint: `/api/traces/${traceId}` }
  };
  const invoke = vi.fn().mockImplementation((name: string) => {
    if (name === "system_context") return Promise.resolve({ ok: true, value: context });
    if (name === "connector_list")
      return Promise.resolve({ ok: true, value: [alertmanager, prometheus, loki, tempo] });
    if (name === "alertmanager_alerts") return Promise.resolve({ ok: true, value: [alert] });
    if (name === "prometheus_query_range")
      return Promise.resolve({
        ok: true,
        value: {
          source: {
            connector_id: "prom-logs-1",
            query: '{alertname="ApiError",namespace="prod",pod="api-0"}',
            endpoint: "/api/v1/query_range"
          },
          series: [
            { labels: { instance: "api-0" }, samples: [{ timestamp: 1735689600, value: "1" }] }
          ]
        }
      });
    if (name === "loki_query_range") return Promise.resolve({ ok: true, value: logResult });
    if (name === "tempo_trace") return Promise.resolve({ ok: true, value: traceResult });
    return Promise.resolve({ ok: true, value: {} });
  });

  render(
    <I18nProvider>
      <Shell invoke={invoke} />
    </I18nProvider>
  );

  await user.click(screen.getByRole("button", { name: "Observability" }));
  await screen.findByRole("heading", { name: "AM logs" });
  await user.click(screen.getByRole("radio", { name: "Select alert logs-123" }));

  const logQuery = await screen.findByRole("textbox", { name: "LogQL query" });
  expect(logQuery).toHaveValue('{namespace="prod", pod="api-0"}');
  expect(screen.getByText(/Investigation window:/)).toHaveAttribute("data-start", alert.starts_at);
  const investigationWindow = screen.getByText(/Investigation window:/);
  const timeEnd = investigationWindow.getAttribute("data-end");

  await user.selectOptions(screen.getByLabelText(/Query type/), "range");
  await user.click(screen.getAllByRole("button", { name: "Run Query" })[0]);
  const metricCall = await waitFor(() => {
    const call = invoke.mock.calls.find(([name]) => name === "prometheus_query_range");
    if (!call) throw new Error("prometheus_query_range was not invoked");
    return call;
  });
  expect(metricCall[1].envelope.payload).toEqual(
    expect.objectContaining({ start: alert.starts_at, end: timeEnd })
  );

  await user.click(screen.getAllByRole("button", { name: "Run Query" })[1]);

  expect(await screen.findByText(/<REDACTED>/)).toBeInTheDocument();
  expect(screen.getByText(/could not be parsed.*not masked/i)).toHaveTextContent("1");
  const logCall = await waitFor(() => {
    const call = invoke.mock.calls.find(([name]) => name === "loki_query_range");
    if (!call) throw new Error("loki_query_range was not invoked");
    return call;
  });
  expect(logCall[1].envelope.payload).toEqual(
    expect.objectContaining({
      connector_id: "loki-1",
      query: '{namespace="prod", pod="api-0"}',
      start: "2026-08-26T00:00:00Z",
      end: timeEnd
    })
  );

  await user.click(screen.getByRole("button", { name: new RegExp(`Open trace.*${traceId}`) }));
  expect(await screen.findByText("api")).toBeInTheDocument();
  expect(screen.getByText("123")).toBeInTheDocument();
  expect(screen.getByText("http.status_code")).toBeInTheDocument();
  expect(invoke).toHaveBeenCalledWith(
    "tempo_trace",
    expect.objectContaining({
      envelope: expect.objectContaining({
        payload: { connector_id: "tempo-1", trace_id: traceId }
      })
    })
  );
});

it("clears prior observability evidence and Grafana context when switching alerts", async () => {
  const user = userEvent.setup();
  const alertmanager = {
    id: "am-stale",
    kind: "alertmanager",
    display_name: "AM stale",
    enabled: true,
    config_metadata: {},
    credential_configured: false,
    health_state: "healthy"
  };
  const prometheus = {
    id: "prom-stale",
    kind: "prometheus",
    display_name: "PROM stale",
    enabled: true,
    config_metadata: {},
    credential_configured: false,
    health_state: "healthy"
  };
  const grafana = {
    id: "graf-stale",
    kind: "grafana",
    display_name: "GRAF stale",
    enabled: true,
    config_metadata: { default_dashboard_uid: "dash-stale" },
    credential_configured: false,
    health_state: "healthy"
  };
  const loki = {
    id: "loki-stale",
    kind: "loki",
    display_name: "Loki stale",
    enabled: true,
    config_metadata: {},
    credential_configured: false,
    health_state: "healthy"
  };
  const tempo = {
    id: "tempo-stale",
    kind: "tempo",
    display_name: "Tempo stale",
    enabled: true,
    config_metadata: {},
    credential_configured: false,
    health_state: "healthy"
  };
  const alertA = {
    fingerprint: "stale-a",
    state: "resolved",
    starts_at: "2026-08-25T00:00:00Z",
    ends_at: "2026-08-25T00:10:00Z",
    labels: { alertname: "OldAlert", namespace: "prod", pod: "api-a" },
    annotations: {},
    generator_url: null,
    source: { connector_id: alertmanager.id, endpoint: "/api/v2/alerts" },
    resource_reference: { resolved: { namespace: "prod", kind: "Pod", name: "api-a" } }
  };
  const alertB = {
    fingerprint: "stale-b",
    state: "resolved",
    starts_at: "2026-08-26T00:00:00Z",
    ends_at: "2026-08-26T00:10:00Z",
    labels: { alertname: "NewAlert", namespace: "prod", pod: "api-b" },
    annotations: {},
    generator_url: null,
    source: { connector_id: alertmanager.id, endpoint: "/api/v2/alerts" },
    resource_reference: { resolved: { namespace: "prod", kind: "Pod", name: "api-b" } }
  };
  const traceA = "4bf92f3577b34da6a3ce929d0e0e4736";
  const traceB = "0af7651916cd43dd8448eb211c80319c";
  const logResult = (traceId: string, line: string, connectorId: string) => ({
    streams: [
      {
        labels: { namespace: "prod", pod: "fixture" },
        entries: [
          {
            timestamp_ns: "1735689600000000001",
            line,
            parsed: true,
            masked: false,
            fields: { message: line },
            trace_id: traceId
          }
        ]
      }
    ],
    source: {
      connector_id: connectorId,
      query: '{namespace="prod", pod="fixture"}',
      endpoint: "/loki/api/v1/query_range"
    },
    unparsed_count: 0
  });
  const traceResult = (traceId: string, serviceName: string) => ({
    trace_id: traceId,
    spans: [
      {
        trace_id: traceId,
        span_id: "0123456789abcdef",
        parent_span_id: null,
        name: `span-${serviceName}`,
        service_name: serviceName,
        start_time_unix_nano: "1735689600000000000",
        duration_nano: "123",
        status: "STATUS_CODE_OK",
        attributes: { "http.status_code": "200" }
      }
    ],
    source: { connector_id: tempo.id, trace_id: traceId, endpoint: `/api/traces/${traceId}` }
  });
  const invoke = vi
    .fn()
    .mockImplementation(
      (name: string, args?: { envelope?: { payload?: Record<string, unknown> } }) => {
        if (name === "system_context") return Promise.resolve({ ok: true, value: context });
        if (name === "connector_list")
          return Promise.resolve({
            ok: true,
            value: [alertmanager, prometheus, grafana, loki, tempo]
          });
        if (name === "alertmanager_alerts")
          return Promise.resolve({ ok: true, value: [alertA, alertB] });
        if (name === "grafana_health")
          return Promise.resolve({ ok: true, value: { version: "10.0", database: "sqlite" } });
        if (name === "prometheus_query_range") {
          const query = String(args?.envelope?.payload?.query ?? "");
          const alert = query.includes("api-a") ? alertA : alertB;
          return Promise.resolve({
            ok: true,
            value: {
              source: { connector_id: prometheus.id, query, endpoint: "/api/v1/query_range" },
              series: [
                {
                  labels: { instance: alert.labels.pod },
                  samples: [{ timestamp: 1700000000, value: `metric-${alert.labels.pod}` }]
                }
              ]
            }
          });
        }
        if (name === "loki_query_range") {
          const query = String(args?.envelope?.payload?.query ?? "");
          const alert = query.includes("api-a") ? alertA : alertB;
          return Promise.resolve({
            ok: true,
            value: logResult(alert === alertA ? traceA : traceB, `log-${alert.labels.pod}`, loki.id)
          });
        }
        if (name === "tempo_trace") {
          const traceId = String(args?.envelope?.payload?.trace_id ?? "");
          return Promise.resolve({
            ok: true,
            value: traceResult(traceId, traceId === traceA ? "service-a" : "service-b")
          });
        }
        if (name === "grafana_link")
          return Promise.resolve({ ok: true, value: { url: "http://localhost/d/dash-stale" } });
        return Promise.resolve({ ok: true, value: {} });
      }
    );

  render(
    <I18nProvider>
      <Shell invoke={invoke} />
    </I18nProvider>
  );

  await user.click(screen.getByRole("button", { name: "Observability" }));
  await screen.findByRole("heading", { name: "AM stale" });
  await user.click(screen.getByRole("radio", { name: "Select alert stale-a" }));
  await user.selectOptions(screen.getByLabelText(/Query type/), "range");
  await user.click(screen.getAllByRole("button", { name: "Run Query" })[0]);
  expect(await screen.findByText("metric-api-a")).toBeInTheDocument();
  await user.click(screen.getAllByRole("button", { name: "Run Query" })[1]);
  expect(await screen.findByText("log-api-a")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: new RegExp(`Open trace.*${traceA}`) }));
  expect(await screen.findByText("service-a")).toBeInTheDocument();

  await user.click(screen.getByRole("radio", { name: "Select alert stale-b" }));
  await waitFor(() => {
    expect(screen.queryByText("metric-api-a")).not.toBeInTheDocument();
    expect(screen.queryByText("log-api-a")).not.toBeInTheDocument();
    expect(screen.queryByText("service-a")).not.toBeInTheDocument();
  });
  expect(screen.getByText(/Investigation window:/)).toHaveAttribute("data-start", alertB.starts_at);

  await user.click(screen.getByRole("button", { name: "Open Dashboard" }));
  const grafanaCall = invoke.mock.calls.filter(([name]) => name === "grafana_link").at(-1);
  expect(grafanaCall?.[1].envelope.payload).toEqual(
    expect.objectContaining({
      query: expect.stringContaining("api-b"),
      start: alertB.starts_at,
      end: alertB.ends_at
    })
  );

  await user.click(screen.getAllByRole("button", { name: "Run Query" })[0]);
  expect(await screen.findByText("metric-api-b")).toBeInTheDocument();
  await user.click(screen.getAllByRole("button", { name: "Run Query" })[1]);
  expect(await screen.findByText("log-api-b")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: new RegExp(`Open trace.*${traceB}`) }));
  expect(await screen.findByText("service-b")).toBeInTheDocument();

  fireEvent.change(screen.getByLabelText("Start"), {
    target: { value: "2026-08-26T00:05" }
  });
  await waitFor(() => {
    expect(screen.queryByText("metric-api-b")).not.toBeInTheDocument();
    expect(screen.queryByText("log-api-b")).not.toBeInTheDocument();
    expect(screen.queryByText("service-b")).not.toBeInTheDocument();
  });
  expect(
    screen.getByText("This time window no longer follows the selected alert.")
  ).toBeInTheDocument();
});

it("states explicitly when the current Loki window has no trace ID", async () => {
  const user = userEvent.setup();
  const alertmanager = {
    id: "am-no-trace",
    kind: "alertmanager",
    display_name: "AM no trace",
    enabled: true,
    config_metadata: {},
    credential_configured: false,
    health_state: "healthy"
  };
  const loki = {
    id: "loki-no-trace",
    kind: "loki",
    display_name: "Loki no trace",
    enabled: true,
    config_metadata: {},
    credential_configured: false,
    health_state: "healthy"
  };
  const tempo = {
    id: "tempo-no-trace",
    kind: "tempo",
    display_name: "Tempo no trace",
    enabled: true,
    config_metadata: {},
    credential_configured: false,
    health_state: "healthy"
  };
  const alert = {
    fingerprint: "logs-no-trace",
    state: "firing",
    starts_at: "2026-08-26T00:00:00Z",
    ends_at: "",
    labels: { alertname: "NoTrace", namespace: "prod", service: "api" },
    annotations: {},
    generator_url: null,
    source: { connector_id: "am-no-trace", endpoint: "/api/v2/alerts" },
    resource_reference: { resolved: { namespace: "prod", kind: "Service", name: "api" } }
  };
  const invoke = vi.fn().mockImplementation((name: string) => {
    if (name === "system_context") return Promise.resolve({ ok: true, value: context });
    if (name === "connector_list")
      return Promise.resolve({ ok: true, value: [alertmanager, loki, tempo] });
    if (name === "alertmanager_alerts") return Promise.resolve({ ok: true, value: [alert] });
    if (name === "loki_query_range")
      return Promise.resolve({
        ok: true,
        value: {
          streams: [
            {
              labels: { namespace: "prod", service: "api" },
              entries: [
                {
                  timestamp_ns: "1735689600000000001",
                  line: '{"msg":"healthy"}',
                  parsed: true,
                  masked: false,
                  fields: { msg: "healthy" },
                  trace_id: null
                }
              ]
            }
          ],
          source: {
            connector_id: "loki-no-trace",
            query: '{namespace="prod", service="api"}',
            endpoint: "/loki/api/v1/query_range"
          },
          unparsed_count: 0
        }
      });
    return Promise.resolve({ ok: true, value: {} });
  });

  render(
    <I18nProvider>
      <Shell invoke={invoke} />
    </I18nProvider>
  );

  await user.click(screen.getByRole("button", { name: "Observability" }));
  await screen.findByRole("heading", { name: "AM no trace" });
  await user.click(screen.getByRole("radio", { name: "Select alert logs-no-trace" }));
  expect(await screen.findByRole("textbox", { name: "LogQL query" })).toHaveValue(
    '{namespace="prod", service="api"}'
  );
  await user.click(screen.getByRole("button", { name: "Run Query" }));

  expect(
    await screen.findByText("No trace ID was found in the current log window.")
  ).toBeInTheDocument();
  expect(invoke).not.toHaveBeenCalledWith("tempo_trace", expect.anything());
});

it("shows localized LogQL states for missing and ambiguous alert labels", async () => {
  const user = userEvent.setup();
  const alertmanager = {
    id: "am-logql-state",
    kind: "alertmanager",
    display_name: "AM LogQL states",
    enabled: true,
    config_metadata: {},
    credential_configured: false,
    health_state: "healthy"
  };
  const loki = {
    id: "loki-logql-state",
    kind: "loki",
    display_name: "Loki LogQL states",
    enabled: true,
    config_metadata: {},
    credential_configured: false,
    health_state: "healthy"
  };
  const alerts = [
    {
      fingerprint: "missing-namespace",
      state: "firing",
      starts_at: "2026-08-26T00:00:00Z",
      ends_at: "",
      labels: { alertname: "MissingNamespace", pod: "api-0" },
      annotations: {},
      generator_url: null,
      source: { connector_id: alertmanager.id, endpoint: "/api/v2/alerts" },
      resource_reference: { unresolved: { reason: "missing namespace label" } }
    },
    {
      fingerprint: "ambiguous-workload",
      state: "firing",
      starts_at: "2026-08-26T00:00:00Z",
      ends_at: "",
      labels: { alertname: "AmbiguousWorkload", namespace: "prod", pod: "api-0", service: "api" },
      annotations: {},
      generator_url: null,
      source: { connector_id: alertmanager.id, endpoint: "/api/v2/alerts" },
      resource_reference: { unresolved: { reason: "ambiguous resource reference" } }
    }
  ];
  const invoke = vi.fn().mockImplementation((name: string) => {
    if (name === "system_context") return Promise.resolve({ ok: true, value: context });
    if (name === "connector_list")
      return Promise.resolve({ ok: true, value: [alertmanager, loki] });
    if (name === "alertmanager_alerts") return Promise.resolve({ ok: true, value: alerts });
    return Promise.resolve({ ok: true, value: {} });
  });

  render(
    <I18nProvider>
      <Shell invoke={invoke} />
    </I18nProvider>
  );

  await user.click(screen.getByRole("button", { name: "Observability" }));
  await screen.findByRole("heading", { name: "AM LogQL states" });
  await user.click(screen.getByRole("radio", { name: "Select alert missing-namespace" }));
  expect(
    await screen.findByText("Alert must include a namespace label before logs can be queried.")
  ).toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: "LogQL query" })).toHaveValue("");

  await user.click(screen.getByRole("radio", { name: "Select alert ambiguous-workload" }));
  expect(
    await screen.findByText(
      "Alert includes multiple workload labels (pod, service or deployment); choose one before querying logs."
    )
  ).toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: "LogQL query" })).toHaveValue("");
});

it("renders loading state in Thai", async () => {
  const invoke = vi.fn().mockImplementation((name: string) => {
    if (name === "system_context") {
      return Promise.resolve({
        ok: true,
        value: {
          admin_email: "admin@test.com",
          workspace_id: "ws-1",
          workspace_name: "Test Workspace"
        }
      });
    }
    // For connector_list, return an unresolved promise to keep it in loading state
    return new Promise(() => {});
  });

  await import("./i18n").then((m) => m.i18n.changeLanguage("th"));

  render(
    <I18nProvider>
      <Shell invoke={invoke} />
    </I18nProvider>
  );

  const user = userEvent.setup();
  await user.click(await screen.findByRole("button", { name: "การสังเกตการณ์" }));

  expect(await screen.findByText("กำลังโหลดการเชื่อมต่อ…")).toBeInTheDocument();
  await import("./i18n").then((m) => m.i18n.changeLanguage("en"));
});

it("renders localized unavailable error for ObservabilityWorkspace", async () => {
  const invoke = vi.fn().mockImplementation((name: string) => {
    if (name === "system_context") {
      return Promise.resolve({
        ok: true,
        value: { admin_email: "test", workspace_id: "w1", workspace_name: "test" }
      });
    }
    if (name === "connector_list") {
      return Promise.resolve({
        ok: false,
        error: { code: "CONNECTOR_UNAVAILABLE" }
      });
    }
    return Promise.resolve({ ok: true, value: {} });
  });

  render(
    <I18nProvider>
      <Shell invoke={invoke} />
    </I18nProvider>
  );

  const user = userEvent.setup();
  await user.click(await screen.findByRole("button", { name: "Observability" }));

  expect(await screen.findByText("Connector is unavailable or disabled.")).toBeInTheDocument();
});

it("handles connector form submission securely", async () => {
  type ConnectorAddCall = { envelope: { payload: Record<string, unknown> } };
  const invoke = vi.fn().mockImplementation((name: string, args: ConnectorAddCall) => {
    if (name === "system_context") {
      return Promise.resolve({ ok: true, value: { admin_email: "test" } });
    }
    if (name === "connector_list") {
      return Promise.resolve({ ok: true, value: [] });
    }
    if (name === "connector_add") {
      if (args.envelope.payload.display_name === "Fail") {
        return Promise.resolve({ ok: false, error: { code: "INVALID_REQUEST" } });
      }
      return Promise.resolve({ ok: true, value: {} });
    }
    return Promise.resolve({ ok: true, value: {} });
  });

  render(
    <I18nProvider>
      <Shell invoke={invoke} />
    </I18nProvider>
  );

  const user = userEvent.setup();
  await user.click(await screen.findByRole("button", { name: "Integrations" }));
  await user.click(await screen.findByRole("button", { name: "Add connector" }));

  // Test none omits credential_value
  await user.type(screen.getByLabelText("Connector"), "TestNone");
  await user.selectOptions(screen.getByLabelText("Kind"), "prometheus");
  await user.selectOptions(screen.getByLabelText("Auth mode"), "none");
  await user.type(screen.getByLabelText("Base URL"), "http://localhost:9090");
  await user.click(screen.getByRole("button", { name: "Save configuration" }));

  expect(invoke).toHaveBeenCalledWith("connector_add", expect.anything());
  const connectorAddPayload = () => {
    const call = invoke.mock.calls.filter((call) => call[0] === "connector_add").at(-1);
    if (!call) throw new Error("connector_add was not invoked");
    return (call[1] as ConnectorAddCall).envelope.payload;
  };
  expect(connectorAddPayload().credential_value).toBeUndefined();

  // Test Basic sends exactly once and password DOM field clears
  await user.click(await screen.findByRole("button", { name: "Add connector" }));
  await user.type(screen.getByLabelText("Connector"), "TestBasic");
  await user.selectOptions(screen.getByLabelText("Kind"), "prometheus");
  await user.type(screen.getByLabelText("Base URL"), "http://localhost:9090");
  await user.selectOptions(screen.getByLabelText("Auth mode"), "basic");
  await user.type(screen.getByLabelText("Username"), "admin");
  await user.type(screen.getByLabelText("Credential"), "input-value-123");
  await user.click(screen.getByRole("button", { name: "Save configuration" }));

  expect(connectorAddPayload().credential_value).toBe("input-value-123");
  // Check password DOM field clears - credInput is still in DOM since form re-opened?
  // Actually we need to check credInput value after submit, but form closes on success.
  // We can test the error case for password clearing.

  await user.click(await screen.findByRole("button", { name: "Add connector" }));
  await user.type(screen.getByLabelText("Connector"), "Fail");
  await user.selectOptions(screen.getByLabelText("Kind"), "prometheus");
  await user.type(screen.getByLabelText("Base URL"), "http://localhost:9090");
  await user.selectOptions(screen.getByLabelText("Auth mode"), "bearer");
  await user.type(screen.getByLabelText("Credential"), "input-value-456");
  await user.click(screen.getByRole("button", { name: "Save configuration" }));

  expect(await screen.findByText("Invalid request data provided.")).toBeInTheDocument();
  // Password should clear
  expect(screen.getByLabelText("Credential")).toHaveValue("");
});

it("configures Loki and Tempo tenant metadata separately from credentials", async () => {
  const user = userEvent.setup();
  const invoke = vi.fn().mockImplementation((name: string) => {
    if (name === "system_context") return Promise.resolve({ ok: true, value: context });
    if (name === "connector_list") return Promise.resolve({ ok: true, value: [] });
    if (name === "connector_add") return Promise.resolve({ ok: true, value: {} });
    return Promise.resolve({ ok: true, value: {} });
  });

  render(
    <I18nProvider>
      <Shell invoke={invoke} />
    </I18nProvider>
  );

  await user.click(await screen.findByRole("button", { name: "Integrations" }));
  await user.click(await screen.findByRole("button", { name: "Add connector" }));
  await user.type(screen.getByLabelText("Connector"), "Loki tenant");
  await user.selectOptions(screen.getByLabelText("Kind"), "loki");
  await user.type(screen.getByLabelText("Base URL"), "https://loki.example.test");
  await user.type(screen.getByLabelText("Tenant ID"), "team-a");
  await user.click(screen.getByRole("button", { name: "Save configuration" }));

  const addPayload = () => {
    const call = invoke.mock.calls.filter((call) => call[0] === "connector_add").at(-1);
    if (!call) throw new Error("connector_add was not invoked");
    return (call[1] as { envelope: { payload: Record<string, unknown> } }).envelope.payload;
  };
  expect(addPayload().config_metadata).toEqual(
    expect.objectContaining({ base_url: "https://loki.example.test", tenant_id: "team-a" })
  );
  expect(addPayload().credential_value).toBeUndefined();

  await user.click(await screen.findByRole("button", { name: "Add connector" }));
  await user.type(screen.getByLabelText("Connector"), "Tempo tenant");
  await user.selectOptions(screen.getByLabelText("Kind"), "tempo");
  await user.type(screen.getByLabelText("Base URL"), "https://tempo.example.test");
  await user.type(screen.getByLabelText("Tenant ID"), "team-b");
  await user.click(screen.getByRole("button", { name: "Save configuration" }));
  expect(addPayload().config_metadata).toEqual(
    expect.objectContaining({ base_url: "https://tempo.example.test", tenant_id: "team-b" })
  );
  expect(addPayload().credential_value).toBeUndefined();
});

it("shows three cloud environments with provider boundaries and keeps healthy ones visible when one session expires", async () => {
  const user = userEvent.setup();
  const awsConnector = {
    id: "aws-1",
    kind: "aws",
    display_name: "AWS Production",
    enabled: true,
    config_metadata: { profile: "prod", region: "us-east-1" },
    credential_configured: false,
    health_state: "healthy"
  };
  const azureConnector = {
    id: "azure-1",
    kind: "azure",
    display_name: "Azure Production",
    enabled: true,
    config_metadata: { subscription_id: "sub-1", tenant_id: "tenant-1" },
    credential_configured: false,
    health_state: "healthy"
  };
  const gcpConnector = {
    id: "gcp-1",
    kind: "gcp",
    display_name: "GCP Production",
    enabled: true,
    config_metadata: { project: "prod-project" },
    credential_configured: false,
    health_state: "healthy"
  };
  const accessFixtures: Record<string, CloudEnvironment> = {
    "aws-1": {
      connector_id: "aws-1",
      provider: "aws",
      account_label: "prod",
      location: "us-east-1",
      access: "confirmed",
      remedy: ""
    },
    "azure-1": {
      connector_id: "azure-1",
      provider: "azure",
      account_label: "sub-1",
      location: "eastus",
      access: "no_credential",
      remedy: "az login --subscription sub-1"
    },
    "gcp-1": {
      connector_id: "gcp-1",
      provider: "gcp",
      account_label: "prod-project",
      location: "global",
      access: "confirmed",
      remedy: ""
    }
  };
  const inventoryFixtures: Record<string, CloudResource[]> = {
    "aws-1": [
      {
        provider: "aws",
        environment_id: "aws-1",
        resource_type: "kubernetes_cluster",
        id: "arn:aws:eks:us-east-1:123:cluster/prod-eks",
        name: "prod-eks",
        location: "us-east-1",
        health: "healthy",
        status_detail: "ACTIVE",
        console_url: "https://console.aws.amazon.com/eks/home#/clusters/prod-eks",
        cli_command: "aws eks describe-cluster --name prod-eks --profile prod --region us-east-1"
      },
      {
        provider: "aws",
        environment_id: "aws-1",
        resource_type: "compute_instance",
        id: "i-prod-ec2",
        name: "prod-ec2",
        location: "us-east-1",
        health: "healthy",
        status_detail: "running",
        console_url: "https://console.aws.amazon.com/ec2/home#/Instances:i-prod-ec2",
        cli_command:
          "aws ec2 describe-instances --instance-ids i-prod-ec2 --profile prod --region us-east-1"
      }
    ],
    "azure-1": [
      {
        provider: "azure",
        environment_id: "azure-1",
        resource_type: "kubernetes_cluster",
        id: "/subscriptions/sub-1/resourceGroups/prod/providers/Microsoft.ContainerService/managedClusters/prod-aks",
        name: "prod-aks",
        location: "eastus",
        health: "healthy",
        status_detail: "Succeeded",
        console_url: "https://portal.azure.com/#resource/prod-aks",
        cli_command: "az aks show --name prod-aks --subscription sub-1"
      }
    ],
    "gcp-1": [
      {
        provider: "gcp",
        environment_id: "gcp-1",
        resource_type: "kubernetes_cluster",
        id: "projects/prod-project/locations/asia-southeast1/clusters/prod-gke",
        name: "prod-gke",
        location: "asia-southeast1",
        health: "healthy",
        status_detail: "RUNNING",
        console_url:
          "https://console.cloud.google.com/kubernetes/clusters/details/asia-southeast1/prod-gke",
        cli_command:
          "gcloud container clusters describe prod-gke --project prod-project --region asia-southeast1"
      },
      {
        provider: "gcp",
        environment_id: "gcp-1",
        resource_type: "compute_instance",
        id: "projects/prod-project/zones/asia-southeast1-a/instances/prod-gce",
        name: "prod-gce",
        location: "asia-southeast1-a",
        health: "healthy",
        status_detail: "RUNNING",
        console_url:
          "https://console.cloud.google.com/compute/instancesDetail/zones/asia-southeast1-a/instances/prod-gce",
        cli_command:
          "gcloud compute instances describe prod-gce --project prod-project --zone asia-southeast1-a"
      }
    ]
  };
  const invoke = vi
    .fn()
    .mockImplementation(
      (name: string, args?: { envelope?: { payload?: { connector_id?: string } } }) => {
        if (name === "system_context") return Promise.resolve({ ok: true, value: context });
        if (name === "connector_list")
          return Promise.resolve({ ok: true, value: [awsConnector, azureConnector, gcpConnector] });
        if (name === "cloud_access_check") {
          const id = args?.envelope?.payload?.connector_id;
          return Promise.resolve({ ok: true, value: accessFixtures[id!] });
        }
        if (name === "cloud_inventory") {
          const id = args?.envelope?.payload?.connector_id;
          return Promise.resolve({ ok: true, value: inventoryFixtures[id!] ?? [] });
        }
        return Promise.resolve({ ok: true, value: {} });
      }
    );

  render(
    <I18nProvider>
      <Shell invoke={invoke} />
    </I18nProvider>
  );

  await user.click(screen.getByRole("button", { name: "Environments" }));

  expect(await screen.findByText("AWS")).toBeInTheDocument();
  expect(screen.getByText("Azure")).toBeInTheDocument();
  expect(screen.getByText("GCP")).toBeInTheDocument();
  expect(await screen.findByText("prod-eks")).toBeInTheDocument();
  expect(await screen.findByText("prod-gke")).toBeInTheDocument();
  expect(await screen.findByText("prod-ec2")).toBeInTheDocument();
  expect(await screen.findByText("prod-gce")).toBeInTheDocument();
  expect(screen.getByText(/az login/)).toBeInTheDocument();
  expect(screen.queryByText("prod-aks")).not.toBeInTheDocument();
});
