import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type {
  CriticalNumber,
  DrillDownTarget,
  EvidenceRef,
  Invoke,
  OperationsSnapshot
} from "../../contracts/ipc";
import { I18nProvider, i18n } from "../i18n";
import { OperationsConsole } from "../OperationsConsole";

vi.mock("@tauri-apps/plugin-shell", () => ({ open: vi.fn().mockResolvedValue(undefined) }));

const scope = { resource_ids: [] };
const observedAt = "2026-08-28T09:00:00Z";
const evidenceIds = ["evidence-masked", "evidence-unparsed"];

const evidenceFor = (
  id: string,
  redaction: Pick<EvidenceRef["redaction"], "masked" | "unparsed">
): EvidenceRef => ({
  id,
  source_kind: "fixture",
  connector_id: "fixture-connector",
  scope,
  endpoint: "fixture://operations",
  query: "operations:snapshot",
  observed_at: observedAt,
  excerpt: `${id} source evidence`,
  native_url: null,
  redaction: {
    classification_verified: true,
    redaction_verified: true,
    ...redaction
  }
});

const drillDownFor = (destination: DrillDownTarget["destination"]): DrillDownTarget => ({
  destination,
  evidence_ids: evidenceIds,
  filter_key: null
});

const numberFor = (
  key: string,
  value: string,
  destination: DrillDownTarget["destination"] = "evidence"
): CriticalNumber => ({
  key,
  value,
  unit: "count",
  evidence_ids: evidenceIds,
  drill_down: drillDownFor(destination),
  drill_down_reference: {
    source_query: "operations:snapshot",
    scope,
    time_window: null,
    evidence_ids: evidenceIds
  }
});

const operationsSnapshotFixture = (): OperationsSnapshot => {
  const evidence = [
    evidenceFor("evidence-masked", { masked: true, unparsed: false }),
    evidenceFor("evidence-unparsed", { masked: false, unparsed: true })
  ];
  const attention = numberFor("attention", "2", "incident_queue");
  const impactedServices = numberFor("impacted_services", "2", "incident_queue");
  const activeS1 = numberFor("active_by_severity.S1", "1", "incident_queue");
  const activeS2 = numberFor("active_by_severity.S2", "1", "incident_queue");
  const environmentCount = numberFor("environments_by_state.Critical", "1", "environment_status");
  const healthyEnvironmentCount = numberFor(
    "environments_by_state.Healthy",
    "1",
    "environment_status"
  );
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
        evidence_ids: evidenceIds
      },
      {
        source_key: "prometheus",
        state: "fresh",
        reason: null,
        detail: null,
        observed_at: observedAt,
        evidence_ids: evidenceIds
      },
      {
        source_key: "health_checks",
        state: "fresh",
        reason: null,
        detail: null,
        observed_at: observedAt,
        evidence_ids: evidenceIds
      },
      {
        source_key: "changes",
        state: "fresh",
        reason: null,
        detail: null,
        observed_at: observedAt,
        evidence_ids: evidenceIds
      },
      {
        source_key: "cloud:aws-prod",
        state: "stale",
        reason: "unreachable",
        detail: "AWS production has one degraded service",
        observed_at: observedAt,
        evidence_ids: evidenceIds
      }
    ],
    health_summary: {
      state: "critical",
      headline: {
        level: "critical",
        summary: "Checkout is affecting customers",
        customer_scope: "Checkout customers",
        service_criticality: "tier-0",
        trajectory: "expanding",
        dimensions: {
          availability: "critical",
          customer_reach: "none",
          business_criticality: "none",
          data_integrity: "none",
          security_privacy: "none",
          financial_contractual: "none",
          trajectory: "expanding",
          production: true
        },
        evidence_ids: evidenceIds
      },
      attention,
      impacted_services: impactedServices,
      active_by_severity: [activeS1, activeS2],
      environments_by_state: [environmentCount, healthyEnvironmentCount],
      contributing_scopes: []
    },
    incident_queue: [
      {
        id: "incident-checkout",
        title: "Checkout API failing",
        source_kind: "alert",
        source_id: "alert-checkout",
        severity: "S1",
        priority: "P1",
        status: "investigating",
        business_impact: {
          level: "critical",
          summary: "Checkout is affecting customers",
          customer_scope: "Checkout customers",
          service_criticality: "tier-0",
          trajectory: "expanding",
          dimensions: {
            availability: "critical",
            customer_reach: "none",
            business_criticality: "none",
            data_integrity: "none",
            security_privacy: "none",
            financial_contractual: "none",
            trajectory: "expanding",
            production: true
          },
          evidence_ids: evidenceIds
        },
        scope,
        detected_at: observedAt,
        opened_at: observedAt,
        last_update: observedAt,
        affected_scope: scope,
        evidence_ids: evidenceIds,
        drill_down: drillDownFor("incident_queue"),
        drill_down_reference: {
          source_query: "operations:incident_queue",
          scope,
          time_window: null,
          evidence_ids: evidenceIds
        }
      },
      {
        id: "anomaly-checkout-rate",
        title: "Checkout error rate rising",
        source_kind: "anomaly",
        source_id: "anomaly-checkout-rate",
        severity: "S2",
        priority: null,
        status: "detected",
        business_impact: {
          level: "high",
          summary: "Checkout error rate is rising",
          customer_scope: "Checkout customers",
          service_criticality: "tier-1",
          trajectory: "expanding",
          dimensions: {
            availability: "high",
            customer_reach: "none",
            business_criticality: "none",
            data_integrity: "none",
            security_privacy: "none",
            financial_contractual: "none",
            trajectory: "expanding",
            production: true
          },
          evidence_ids: evidenceIds
        },
        scope,
        detected_at: observedAt,
        opened_at: observedAt,
        last_update: observedAt,
        affected_scope: scope,
        evidence_ids: evidenceIds,
        drill_down: drillDownFor("incident_queue"),
        drill_down_reference: {
          source_query: "operations:incident_queue",
          scope,
          time_window: null,
          evidence_ids: evidenceIds
        }
      }
    ],
    signal_summary: {
      active_alerts: numberFor("active_alerts", "1", "signal_summary"),
      active_anomalies: numberFor("active_anomalies", "1", "signal_summary"),
      checks_due: numberFor("checks_due", "2", "signal_summary"),
      checks_timed_out: numberFor("checks_timed_out", "1", "signal_summary"),
      by_source: [
        { source_kind: "alert", count: numberFor("signals.alert", "1", "signal_summary") },
        { source_kind: "anomaly", count: numberFor("signals.anomaly", "1", "signal_summary") },
        {
          source_kind: "scheduled_health_check",
          count: numberFor("signals.scheduled_health_check", "2", "signal_summary")
        }
      ]
    },
    changes: [
      {
        id: "change-checkout",
        source: "fixture",
        occurred_at: observedAt,
        kind: "deployment",
        summary: "Checkout deployment completed",
        actor: "release-bot",
        target_resource: "checkout-api",
        native_link: null,
        scope,
        evidence_ids: evidenceIds,
        drill_down: drillDownFor("change_stream")
      }
    ],
    change_stream_status: { state: "available", reason: null, detail: null },
    environments: [
      {
        environment_id: "aws-prod",
        name: "AWS production",
        provider: "aws",
        health: "critical",
        status_detail: "One service is unavailable",
        resource_count: numberFor("aws_resources", "3", "environment_status"),
        last_observed_at: observedAt,
        evidence_ids: evidenceIds,
        drill_down: drillDownFor("environment_status")
      },
      {
        environment_id: "gcp-staging",
        name: "GCP staging",
        provider: "gcp",
        health: "healthy",
        status_detail: "All services responding",
        resource_count: numberFor("gcp_resources", "2", "environment_status"),
        last_observed_at: observedAt,
        evidence_ids: evidenceIds,
        drill_down: drillDownFor("environment_status")
      }
    ],
    evidence,
    widget_registry: [
      ["health_summary", "wide", true],
      ["incident_queue", "wide", true],
      ["signal_summary", "standard", false],
      ["change_stream", "standard", false],
      ["environment_status", "wide", false]
    ].map(([id, size, required], default_order) => ({
      id,
      title_key: `operations.${id}`,
      default_order,
      default_size: size,
      required
    })) as OperationsSnapshot["widget_registry"]
  };
};

afterEach(() => {
  cleanup();
  localStorage.clear();
  void i18n.changeLanguage("en");
});

it("lets an operator identify attention and open evidence without provider or mutation calls", async () => {
  const user = userEvent.setup();
  const snapshot = operationsSnapshotFixture();
  const invoke = vi
    .fn()
    .mockImplementation(
      (name: string, args: { envelope: { payload: { evidence_ids: string[] } } }) => {
        if (name === "operations_snapshot") return Promise.resolve({ ok: true, value: snapshot });
        if (name === "operations_evidence") {
          const ids = args.envelope.payload.evidence_ids;
          return Promise.resolve({
            ok: true,
            value: snapshot.evidence.filter((item) => ids.includes(item.id))
          });
        }
        throw new Error(`unexpected command: ${name}`);
      }
    );

  const { container } = render(
    <I18nProvider>
      <OperationsConsole invoke={invoke as Invoke} />
    </I18nProvider>
  );

  const headline = await screen.findByRole("heading", { name: "Checkout is affecting customers" });
  expect(headline).toBeInTheDocument();
  expect(screen.getByText("Checkout API failing")).toBeInTheDocument();
  expect(screen.getByText("AWS production")).toBeInTheDocument();
  expect(screen.getByText("GCP staging")).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Open evidence for active alerts (1)" })
  ).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Open evidence for active anomalies (1)" })
  ).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Open evidence for checks due (2)" })
  ).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Open evidence for timed out (1)" })
  ).toBeInTheDocument();

  const widgets = [...container.querySelectorAll<HTMLElement>("[data-widget-id]")];
  expect(widgets.slice(0, 2).map((widget) => widget.dataset.widgetId)).toEqual([
    "health_summary",
    "incident_queue"
  ]);
  expect(within(widgets[0]).getByRole("heading", { name: "Health summary" })).toBeInTheDocument();
  expect(
    within(widgets[1]).getByRole("heading", { name: "Active incident queue" })
  ).toBeInTheDocument();

  const numberButtons = [
    ...container.querySelectorAll<HTMLButtonElement>(
      '[data-testid="operations-critical-number"] button'
    )
  ];
  expect(numberButtons.length).toBeGreaterThan(10);
  for (const button of numberButtons) await user.click(button);

  const evidenceCalls = invoke.mock.calls.filter(([name]) => name === "operations_evidence");
  expect(evidenceCalls).toHaveLength(numberButtons.length);
  for (const [, args] of evidenceCalls) {
    expect(args.envelope.capability).toBe("ResourceRead");
    expect(args.envelope.payload.evidence_ids).toEqual(evidenceIds);
  }
  expect(invoke.mock.calls.map(([name]) => name)).toEqual([
    "operations_snapshot",
    ...Array.from({ length: numberButtons.length }, () => "operations_evidence")
  ]);
  expect(await screen.findByText(/Sensitive fields masked/)).toBeInTheDocument();
  expect(screen.getByText(/Unparsed source/)).toBeInTheDocument();
  expect(screen.getAllByText("fixture://operations")).not.toHaveLength(0);
  expect(screen.getAllByText("operations:snapshot")).not.toHaveLength(0);
});
