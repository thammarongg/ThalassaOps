import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { open } from "@tauri-apps/plugin-shell";
import type {
  BusinessImpact,
  CriticalNumber,
  DrillDownReference,
  DrillDownTarget,
  EvidenceRef,
  OperationsSnapshot
} from "../../contracts/ipc";
import { I18nProvider, i18n } from "../i18n";
import { OperationsConsole } from "../OperationsConsole";

vi.mock("@tauri-apps/plugin-shell", () => ({ open: vi.fn().mockResolvedValue(undefined) }));

const scope = { resource_ids: [] };
const observedAt = "2026-08-28T09:00:00Z";

const evidenceFor = (id: string, excerpt = `${id} evidence`): EvidenceRef => ({
  id,
  source_kind: "fixture",
  connector_id: null,
  scope,
  endpoint: "fixture://operations",
  query: "operations:snapshot",
  observed_at: observedAt,
  excerpt,
  native_url: null,
  redaction: {
    classification_verified: true,
    redaction_verified: true,
    masked: false,
    unparsed: false
  }
});

const drillDownFor = (
  evidenceId: string,
  destination: DrillDownTarget["destination"] = "evidence"
) => ({
  destination,
  evidence_ids: [evidenceId],
  filter_key: null
});

const referenceFor = (evidenceId: string): DrillDownReference => ({
  source_query: "operations:snapshot",
  scope,
  time_window: null,
  evidence_ids: [evidenceId]
});

const numberFor = (
  key: string,
  value: string,
  evidenceId: string,
  destination: DrillDownTarget["destination"] = "evidence"
): CriticalNumber => ({
  key,
  value,
  unit: "count",
  evidence_ids: [evidenceId],
  drill_down: drillDownFor(evidenceId, destination),
  drill_down_reference: referenceFor(evidenceId)
});

const impactFor = (overrides: Partial<BusinessImpact> = {}): BusinessImpact => ({
  level: "none",
  summary: "No active business impact",
  customer_scope: "none",
  service_criticality: "none",
  trajectory: "improving",
  ...overrides
});

const healthySnapshot = (): OperationsSnapshot => {
  const ids = [
    "evidence-health",
    "evidence-attention",
    "evidence-services",
    "evidence-severity",
    "evidence-environments",
    "evidence-alerts",
    "evidence-anomalies",
    "evidence-due",
    "evidence-timeout",
    "evidence-change",
    "evidence-environment"
  ];
  const evidence = ids.map((id) => evidenceFor(id));
  return {
    generated_at: observedAt,
    scope,
    source_status: [
      {
        source_key: "alertmanager",
        state: "fresh",
        reason: null,
        detail: null,
        observed_at: observedAt,
        evidence_ids: ["evidence-alerts"]
      },
      {
        source_key: "anomalies",
        state: "fresh",
        reason: null,
        detail: null,
        observed_at: observedAt,
        evidence_ids: ["evidence-anomalies"]
      },
      {
        source_key: "health_checks",
        state: "fresh",
        reason: null,
        detail: null,
        observed_at: observedAt,
        evidence_ids: ["evidence-due"]
      },
      {
        source_key: "changes",
        state: "fresh",
        reason: null,
        detail: null,
        observed_at: observedAt,
        evidence_ids: ["evidence-change"]
      },
      {
        source_key: "environment:prod",
        state: "fresh",
        reason: null,
        detail: null,
        observed_at: observedAt,
        evidence_ids: ["evidence-environment"]
      }
    ],
    health_summary: {
      state: "healthy",
      headline: impactFor(),
      attention: numberFor("attention", "0", "evidence-attention", "incident_queue"),
      impacted_services: numberFor("impacted_services", "0", "evidence-services"),
      active_by_severity: [
        numberFor("active_by_severity.S1", "0", "evidence-severity", "incident_queue")
      ],
      environments_by_state: [
        numberFor("healthy_environments", "1", "evidence-environments", "environment_status")
      ],
      contributing_scopes: []
    },
    incident_queue: [],
    signal_summary: {
      active_alerts: numberFor("active_alerts", "0", "evidence-alerts", "signal_summary"),
      active_anomalies: numberFor("active_anomalies", "0", "evidence-anomalies", "signal_summary"),
      checks_due: numberFor("checks_due", "0", "evidence-due", "signal_summary"),
      checks_timed_out: numberFor("checks_timed_out", "0", "evidence-timeout", "signal_summary"),
      by_source: []
    },
    changes: [
      {
        id: "change-1",
        source: "fixture",
        occurred_at: observedAt,
        kind: "deployment",
        summary: "Payment API deployed",
        actor: "release-bot",
        target_resource: "payment-api",
        native_link: null,
        scope,
        evidence_ids: ["evidence-change"],
        drill_down: drillDownFor("evidence-change", "change_stream")
      }
    ],
    change_stream_status: { state: "available", reason: null, detail: null },
    environments: [
      {
        environment_id: "prod",
        name: "Production",
        provider: "aws",
        health: "healthy",
        status_detail: "All services responding",
        resource_count: numberFor(
          "prod_resources",
          "12",
          "evidence-environment",
          "environment_status"
        ),
        last_observed_at: observedAt,
        evidence_ids: ["evidence-environment"],
        drill_down: drillDownFor("evidence-environment", "environment_status")
      }
    ],
    evidence,
    widget_registry: [
      {
        id: "health_summary",
        title_key: "operations.healthSummary",
        default_order: 0,
        default_size: "wide",
        required: true
      },
      {
        id: "incident_queue",
        title_key: "operations.incidentQueue",
        default_order: 1,
        default_size: "standard",
        required: true
      },
      {
        id: "signal_summary",
        title_key: "operations.signalSummary",
        default_order: 2,
        default_size: "standard",
        required: false
      },
      {
        id: "change_stream",
        title_key: "operations.changeStream",
        default_order: 3,
        default_size: "standard",
        required: false
      },
      {
        id: "environment_status",
        title_key: "operations.environmentStatus",
        default_order: 4,
        default_size: "wide",
        required: false
      }
    ]
  };
};

const anomalySnapshot = (): OperationsSnapshot => {
  const snapshot = structuredClone(healthySnapshot());
  snapshot.health_summary.state = "critical";
  snapshot.health_summary.headline = impactFor({
    level: "critical",
    summary: "Checkout API is affecting customers",
    customer_scope: "Checkout customers",
    service_criticality: "Tier 0",
    trajectory: "expanding"
  });
  snapshot.health_summary.attention = numberFor(
    "attention",
    "3",
    "evidence-attention",
    "incident_queue"
  );
  snapshot.health_summary.impacted_services = numberFor(
    "impacted_services",
    "2",
    "evidence-services"
  );
  snapshot.health_summary.active_by_severity = [
    numberFor("active_by_severity.S1", "1", "evidence-severity", "incident_queue"),
    numberFor("active_by_severity.S2", "2", "evidence-severity", "incident_queue")
  ];
  snapshot.signal_summary.active_alerts = numberFor(
    "active_alerts",
    "1",
    "evidence-alerts",
    "signal_summary"
  );
  snapshot.signal_summary.active_anomalies = numberFor(
    "active_anomalies",
    "2",
    "evidence-anomalies",
    "signal_summary"
  );
  snapshot.signal_summary.checks_due = numberFor(
    "checks_due",
    "1",
    "evidence-due",
    "signal_summary"
  );
  snapshot.signal_summary.checks_timed_out = numberFor(
    "checks_timed_out",
    "1",
    "evidence-timeout",
    "signal_summary"
  );
  snapshot.incident_queue = [
    {
      id: "incident-1",
      title: "Checkout API failing",
      source_kind: "alert",
      source_id: "alert-1",
      severity: "S1",
      priority: "P1",
      status: "investigating",
      business_impact: snapshot.health_summary.headline,
      scope,
      detected_at: observedAt,
      opened_at: observedAt,
      last_update: observedAt,
      affected_scope: scope,
      evidence_ids: ["evidence-alerts"],
      drill_down: drillDownFor("evidence-alerts", "incident_queue"),
      drill_down_reference: referenceFor("evidence-alerts")
    }
  ];
  return snapshot;
};

const renderConsole = (snapshot: OperationsSnapshot, invoke = vi.fn()) => {
  const actualInvoke = invoke as ReturnType<typeof vi.fn>;
  actualInvoke.mockResolvedValue({ ok: true, value: snapshot });
  return render(
    <I18nProvider>
      <OperationsConsole invoke={actualInvoke} />
    </I18nProvider>
  );
};

afterEach(() => {
  cleanup();
  localStorage.clear();
  void i18n.changeLanguage("en");
});

it("renders a healthy command center with a calm impact headline", async () => {
  renderConsole(healthySnapshot());

  expect(await screen.findByRole("heading", { name: "Operations Console" })).toBeInTheDocument();
  expect(screen.getByText("No active business impact")).toBeInTheDocument();
  expect(screen.getByText("Payment API deployed")).toBeInTheDocument();
  expect(screen.getByText("All services responding")).toBeInTheDocument();
});

it("puts anomalies and failing checks at the top of the attention narrative", async () => {
  renderConsole(anomalySnapshot());

  expect(
    await screen.findByRole("heading", { name: "Checkout API is affecting customers" })
  ).toBeInTheDocument();
  expect(screen.getByText("Checkout API failing")).toBeInTheDocument();
  expect(screen.getByText("S1 Critical")).toBeInTheDocument();
  expect(screen.getByText("S1 critical")).toBeInTheDocument();
  expect(screen.getByText("P1")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /timed out/ })).toBeInTheDocument();
});

it("keeps the rest of the console visible when a source is unavailable", async () => {
  const snapshot = healthySnapshot();
  snapshot.source_status = snapshot.source_status.map((status) =>
    status.source_key === "changes"
      ? { ...status, state: "unavailable", reason: "unreachable", detail: "Change API is offline" }
      : status
  );
  snapshot.change_stream_status = {
    state: "unavailable",
    reason: "unreachable",
    detail: "Change API is offline"
  };

  renderConsole(snapshot);

  expect(await screen.findAllByText("Change API is offline")).not.toHaveLength(0);
  expect(screen.getByText("Production")).toBeInTheDocument();
  expect(screen.getByText("No active business impact")).toBeInTheDocument();
});

it("rejects a malformed nested snapshot before any widget dereferences it", async () => {
  const snapshot = healthySnapshot();
  snapshot.health_summary = {} as OperationsSnapshot["health_summary"];

  renderConsole(snapshot);

  expect(await screen.findAllByText("The console snapshot is unavailable.")).toHaveLength(6);
});

it("labels a stale source as degraded with its typed reason", async () => {
  const snapshot = healthySnapshot();
  snapshot.source_status = snapshot.source_status.map((status) =>
    status.source_key === "alertmanager"
      ? { ...status, state: "stale", reason: "timed_out", detail: "Alertmanager timed out" }
      : status
  );

  renderConsole(snapshot);

  expect(await screen.findAllByText("alertmanager is degraded (timed out).")).not.toHaveLength(0);
  expect(screen.getAllByText("Alertmanager timed out")).not.toHaveLength(0);
});

it("keeps each widget explicit when the snapshot request fails", async () => {
  const invoke = vi.fn().mockRejectedValue(new Error("snapshot unavailable"));
  render(
    <I18nProvider>
      <OperationsConsole invoke={invoke} />
    </I18nProvider>
  );

  expect(await screen.findAllByText("The console snapshot is unavailable.")).toHaveLength(6);
});

it("renders a directed empty state when the change stream has no data", async () => {
  const snapshot = healthySnapshot();
  snapshot.changes = [];
  snapshot.change_stream_status = { state: "empty", reason: "no_data_in_window", detail: null };

  renderConsole(snapshot);

  expect(await screen.findByText("No recent changes in this window")).toBeInTheDocument();
  expect(screen.getByText("No active business impact")).toBeInTheDocument();
});

it("persists curated widget visibility and order while keeping required widgets visible", async () => {
  const user = userEvent.setup();
  renderConsole(healthySnapshot());

  await screen.findByRole("heading", { name: "Operations Console" });
  await user.click(screen.getByRole("button", { name: "Customize console" }));
  const settings = screen.getByRole("dialog", { name: "Customize console" });
  await user.click(within(settings).getByRole("checkbox", { name: "Show Alerts and anomalies" }));
  await user.click(within(settings).getByRole("button", { name: "Move Recent change stream up" }));

  const stored = JSON.parse(localStorage.getItem("thalassaops.operations.widgets.v1") ?? "null");
  expect(stored.version).toBe(1);
  expect(
    stored.preferences.find((preference: { id: string }) => preference.id === "signal_summary")
      .visible
  ).toBe(false);
  expect(
    stored.preferences.find((preference: { id: string }) => preference.id === "change_stream").order
  ).toBe(2);
  expect(within(settings).getByRole("checkbox", { name: "Show Health summary" })).toBeChecked();
  expect(
    within(settings).getByRole("checkbox", { name: "Show Active incident queue" })
  ).toBeChecked();
  expect(within(settings).getByRole("checkbox", { name: "Show Health summary" })).toBeDisabled();

  cleanup();
  renderConsole(healthySnapshot());
  await screen.findByRole("heading", { name: "Operations Console" });
  expect(screen.queryByRole("heading", { name: "Alerts and anomalies" })).not.toBeInTheDocument();
});

it("gives every rendered critical number a focusable affordance with issued evidence ids", async () => {
  renderConsole(anomalySnapshot());

  await screen.findByRole("heading", { name: "Checkout API is affecting customers" });
  const criticalNumbers = screen.getAllByTestId("operations-critical-number");
  expect(criticalNumbers.length).toBeGreaterThan(5);
  for (const number of criticalNumbers) {
    const button = within(number).getByRole("button");
    expect(button).toHaveAttribute("data-evidence-ids");
    expect(JSON.parse(button.getAttribute("data-evidence-ids") ?? "[]")).not.toHaveLength(0);
    expect(button).not.toHaveAttribute("tabindex", "-1");
  }
});

it("opens evidence for a critical number through the read-only evidence command", async () => {
  const user = userEvent.setup();
  const invoke = vi.fn();
  renderConsole(healthySnapshot(), invoke);

  await screen.findByText("No active business impact");
  await user.click(
    within(screen.getAllByTestId("operations-critical-number")[0]).getByRole("button")
  );

  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith(
      "operations_evidence",
      expect.objectContaining({
        envelope: expect.objectContaining({
          command: "operations.evidence",
          capability: "ResourceRead",
          payload: { evidence_ids: expect.any(Array) }
        })
      })
    )
  );
});

it("shows the evidence connector and opens only its trusted HTTPS source link", async () => {
  const user = userEvent.setup();
  const snapshot = healthySnapshot();
  const nativeEvidence = snapshot.evidence.find((item) => item.id === "evidence-attention");
  if (!nativeEvidence) throw new Error("attention evidence fixture is required");
  nativeEvidence.connector_id = "fixture-connector";
  nativeEvidence.native_url = "https://source.example/evidence";
  const invoke = vi
    .fn()
    .mockImplementation((name: string) =>
      name === "operations_snapshot"
        ? Promise.resolve({ ok: true, value: snapshot })
        : Promise.resolve({ ok: true, value: [nativeEvidence] })
    );

  render(
    <I18nProvider>
      <OperationsConsole invoke={invoke} />
    </I18nProvider>
  );

  await screen.findByText("No active business impact");
  await user.click(
    within(screen.getAllByTestId("operations-critical-number")[0]).getByRole("button")
  );

  expect(await screen.findByText("fixture-connector")).toBeInTheDocument();
  const openButton = await screen.findByRole("button", { name: "Open trusted source" });
  await user.click(openButton);
  expect(open).toHaveBeenCalledWith("https://source.example/evidence");
});

it("does not let an older drill-down response replace the latest selection", async () => {
  const user = userEvent.setup();
  const snapshot = healthySnapshot();
  const pending: Array<(result: unknown) => void> = [];
  const invoke = vi.fn().mockImplementation((name: string) => {
    if (name === "operations_snapshot") {
      return Promise.resolve({ ok: true, value: snapshot });
    }
    return new Promise((resolve) => pending.push(resolve));
  });

  render(
    <I18nProvider>
      <OperationsConsole invoke={invoke} />
    </I18nProvider>
  );

  await screen.findByText("No active business impact");
  const criticalNumbers = screen.getAllByTestId("operations-critical-number");
  await user.click(within(criticalNumbers[0]).getByRole("button"));
  await user.click(within(criticalNumbers[1]).getByRole("button"));
  expect(pending).toHaveLength(2);

  pending[1]({ ok: true, value: [snapshot.evidence[2]] });
  expect(await screen.findByText("evidence-services evidence")).toBeInTheDocument();
  pending[0]({ ok: true, value: [snapshot.evidence[1]] });
  await new Promise((resolve) => setTimeout(resolve, 0));
  await waitFor(() =>
    expect(screen.queryByText("evidence-attention evidence")).not.toBeInTheDocument()
  );
});

it("rejects a malformed nested evidence response before rendering it", async () => {
  const user = userEvent.setup();
  const snapshot = healthySnapshot();
  const invoke = vi
    .fn()
    .mockImplementation((name: string) =>
      name === "operations_snapshot"
        ? Promise.resolve({ ok: true, value: snapshot })
        : Promise.resolve({ ok: true, value: [{ id: "malformed-evidence" }] })
    );

  render(
    <I18nProvider>
      <OperationsConsole invoke={invoke} />
    </I18nProvider>
  );

  await screen.findByText("No active business impact");
  await user.click(
    within(screen.getAllByTestId("operations-critical-number")[0]).getByRole("button")
  );
  expect(
    await screen.findByText("Evidence is unavailable for this drill-down.")
  ).toBeInTheDocument();
});
