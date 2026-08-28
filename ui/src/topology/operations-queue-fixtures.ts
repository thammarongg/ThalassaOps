import type {
  CriticalNumber,
  EvidenceRef,
  IncidentQueueItem,
  OperationsSnapshot
} from "../../contracts/ipc";

/**
 * Test-only minimal Operations Console projection carrying an incident
 * queue.  It satisfies the full `operations.snapshot` contract so the
 * Operations Console and the topology Incident filter accept it.
 */

const scope = { resource_ids: [] };
const observedAt = "2026-08-28T09:00:00Z";

const evidenceFor = (id: string): EvidenceRef => ({
  id,
  source_kind: "fixture",
  connector_id: null,
  scope,
  endpoint: "fixture://operations",
  query: "operations:snapshot",
  observed_at: observedAt,
  excerpt: `${id} operations evidence`,
  native_url: null,
  redaction: {
    classification_verified: true,
    redaction_verified: true,
    masked: false,
    unparsed: false
  }
});

const numberFor = (key: string, evidenceId: string): CriticalNumber => ({
  key,
  value: "0",
  unit: "count",
  evidence_ids: [evidenceId],
  drill_down: { destination: "evidence", evidence_ids: [evidenceId], filter_key: null },
  drill_down_reference: {
    source_query: "operations:snapshot",
    scope,
    time_window: null,
    evidence_ids: [evidenceId]
  }
});

export const operationsSnapshotFor = (incidents: IncidentQueueItem[]): OperationsSnapshot => ({
  generated_at: observedAt,
  scope,
  source_status: [],
  health_summary: {
    state: "healthy",
    headline: {
      level: "none",
      summary: "No active business impact",
      customer_scope: "none",
      service_criticality: "none",
      trajectory: "improving"
    },
    attention: numberFor("attention", "evidence-attention"),
    impacted_services: numberFor("impacted_services", "evidence-services"),
    active_by_severity: [numberFor("active_by_severity.S1", "evidence-severity")],
    environments_by_state: [numberFor("healthy_environments", "evidence-environments")],
    contributing_scopes: []
  },
  incident_queue: incidents,
  signal_summary: {
    active_alerts: numberFor("active_alerts", "evidence-alerts"),
    active_anomalies: numberFor("active_anomalies", "evidence-anomalies"),
    checks_due: numberFor("checks_due", "evidence-due"),
    checks_timed_out: numberFor("checks_timed_out", "evidence-timeout"),
    by_source: []
  },
  changes: [],
  change_stream_status: { state: "available", reason: null, detail: null },
  environments: [],
  evidence: [
    evidenceFor("evidence-attention"),
    evidenceFor("evidence-services"),
    evidenceFor("evidence-severity"),
    evidenceFor("evidence-environments"),
    evidenceFor("evidence-alerts"),
    evidenceFor("evidence-anomalies"),
    evidenceFor("evidence-due"),
    evidenceFor("evidence-timeout"),
    ...incidents.flatMap((item) => item.evidence_ids.map(evidenceFor))
  ],
  widget_registry: []
});
