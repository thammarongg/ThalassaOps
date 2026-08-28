import { describe, expect, it } from "vitest";
import type {
  CommandEnvelope,
  EvidenceRef,
  OperationsEvidenceRequest,
  OperationsSnapshot,
  OperationsSnapshotRequest
} from "../../contracts/ipc";

const scope = { resource_ids: [] };
const evidence: EvidenceRef = {
  id: "evidence-1",
  source_kind: "fixture",
  connector_id: null,
  scope,
  endpoint: "fixture://operations",
  query: "operations:snapshot",
  observed_at: "2026-08-28T09:00:00Z",
  excerpt: "fixture evidence",
  native_url: null,
  redaction: {
    classification_verified: true,
    redaction_verified: true,
    masked: false,
    unparsed: false
  }
};

const number = {
  key: "count",
  value: "1",
  unit: "count" as const,
  evidence_ids: [evidence.id],
  drill_down: {
    destination: "evidence" as const,
    evidence_ids: [evidence.id],
    filter_key: null
  },
  drill_down_reference: {
    source_query: "operations:snapshot",
    scope,
    time_window: null,
    evidence_ids: [evidence.id]
  }
};

const operationsSnapshotFixture: OperationsSnapshot = {
  generated_at: "2026-08-28T09:00:00Z",
  scope,
  source_status: [
    {
      source_key: "fixture",
      state: "fresh",
      reason: null,
      detail: null,
      observed_at: "2026-08-28T09:00:00Z",
      evidence_ids: [evidence.id]
    }
  ],
  health_summary: {
    state: "healthy",
    headline: {
      level: "none",
      summary: "No active business impact",
      customer_scope: "none",
      service_criticality: "none",
      trajectory: "improving"
    },
    attention: number,
    impacted_services: number,
    active_by_severity: [number],
    environments_by_state: [number],
    contributing_scopes: [{ scope, impact: "none", summary: "none", evidence_ids: [evidence.id] }]
  },
  incident_queue: [
    {
      id: "queue-1",
      title: "Fixture attention",
      source_kind: "alert",
      source_id: "alert-1",
      severity: "S1",
      priority: null,
      status: "detected",
      business_impact: {
        level: "critical",
        summary: "Fixture attention",
        customer_scope: "fixture customers",
        service_criticality: "tier-0",
        trajectory: "unknown"
      },
      scope,
      detected_at: "2026-08-28T09:00:00Z",
      opened_at: "2026-08-28T09:00:00Z",
      last_update: "2026-08-28T09:00:00Z",
      affected_scope: scope,
      evidence_ids: [evidence.id],
      drill_down: number.drill_down,
      drill_down_reference: number.drill_down_reference
    }
  ],
  signal_summary: {
    active_alerts: number,
    active_anomalies: number,
    checks_due: number,
    checks_timed_out: number,
    by_source: [{ source_kind: "alert", count: number }]
  },
  changes: [
    {
      id: "change-1",
      source: "fixture",
      occurred_at: "2026-08-28T09:00:00Z",
      kind: "deployment",
      summary: "Fixture deployment",
      actor: null,
      target_resource: null,
      native_link: null,
      scope,
      evidence_ids: [evidence.id],
      drill_down: number.drill_down
    }
  ],
  change_stream_status: { state: "available", reason: null, detail: null },
  environments: [
    {
      environment_id: "environment-1",
      name: "Fixture",
      provider: "fixture",
      health: "healthy",
      status_detail: "healthy",
      resource_count: number,
      last_observed_at: "2026-08-28T09:00:00Z",
      evidence_ids: [evidence.id],
      drill_down: number.drill_down
    }
  ],
  evidence: [evidence],
  widget_registry: [
    {
      id: "health_summary",
      title_key: "operations.health_summary",
      default_order: 0,
      default_size: "wide",
      required: true
    }
  ]
};

const snapshotRequest: CommandEnvelope<OperationsSnapshotRequest> = {
  request_id: "00000000-0000-0000-0000-000000000001",
  command: "operations.snapshot",
  capability: "WorkspaceRead",
  scope,
  payload: null
};

const evidenceRequest: CommandEnvelope<OperationsEvidenceRequest> = {
  request_id: "00000000-0000-0000-0000-000000000002",
  command: "operations.evidence",
  capability: "ResourceRead",
  scope,
  payload: { evidence_ids: [evidence.id] }
};

describe("Operations Console IPC contract", () => {
  it("keeps the Rust-shaped snapshot fixture evidence-backed", () => {
    expect(operationsSnapshotFixture.health_summary.attention.evidence_ids).toHaveLength(1);
    expect(operationsSnapshotFixture.widget_registry[0].id).toBe("health_summary");
  });

  it("uses the shared envelope for both read-only operations commands", () => {
    expect(snapshotRequest).toMatchObject({
      command: "operations.snapshot",
      capability: "WorkspaceRead",
      payload: null
    });
    expect(evidenceRequest).toMatchObject({
      command: "operations.evidence",
      capability: "ResourceRead",
      payload: { evidence_ids: [evidence.id] }
    });
  });
});
