import type {
  CorrelationSnapshot,
  EvidenceRef,
  EvidenceSourceKind,
  ResourceScope,
  Signal
} from "../../contracts/ipc";

export const SPRINT_13_FIXTURE_CLOCK = "2026-08-28T09:00:00Z" as const;

export const correlationFixtureSourceKinds: EvidenceSourceKind[] = [
  "trivy",
  "falco",
  "kyverno",
  "opa_gatekeeper"
];

/** Typed copies of the four committed, synthetic security records. */
export const securityFixtureRecords = {
  trivy: {
    SchemaVersion: 2,
    ArtifactName: "checkout:2026.08.28.1",
    ArtifactType: "container_image",
    Results: [
      {
        VulnerabilityID: "CVE-2024-1234",
        PkgName: "libcheckout",
        InstalledVersion: "1.2.3",
        Severity: "HIGH",
        CVSS: { nvd: { V3Score: 8.1 } },
        vendor_extension: { capture: "synthetic", revision_hint: "fixture-1" }
      }
    ]
  },
  falco: {
    source: "falco-fixture",
    event_id: "falco-event-1",
    rule: "Write below binary directory",
    priority: "Critical",
    time: "2026-08-28T08:58:30Z",
    target: { namespace: "prod", pod: "checkout-7d9c", container: "checkout" },
    vendor_extension: { capture: "synthetic", revision_hint: "fixture-1" }
  },
  kyverno: {
    apiVersion: "wgpolicyk8s.io/v1alpha2",
    policy: "disallow-host-path",
    rule: "host-path",
    result: "fail",
    severity: "high",
    resource: { namespace: "prod", kind: "Deployment", name: "checkout" },
    violation_path: "spec.template.spec.volumes[0].hostPath",
    vendor_extension: { capture: "synthetic", revision_hint: "fixture-1" }
  },
  opa_gatekeeper: {
    apiVersion: "constraints.gatekeeper.sh/v1beta1",
    constraint_template: "k8srequiredlabels",
    constraint: "checkout-required-labels",
    result: "violation",
    severity: "medium",
    resource: { namespace: "prod", kind: "Deployment", name: "checkout" },
    violation_path: "metadata.labels.service-tier",
    vendor_extension: { capture: "synthetic", revision_hint: "fixture-1" }
  }
} as const;

const scope: ResourceScope = {
  organization_id: "00000000-0000-0000-0000-000000000014",
  team_id: "00000000-0000-0000-0000-000000000013",
  workspace_id: "00000000-0000-0000-0000-000000000012",
  environment_id: "00000000-0000-0000-0000-000000000011",
  resource_ids: []
};

const evidence = (
  id: string,
  source_kind: EvidenceSourceKind,
  observed_at: string
): EvidenceRef => ({
  id,
  source_kind,
  connector_id: "fixture-catalog",
  scope,
  endpoint: `fixture://correlation/${source_kind}`,
  query: "recorded fixture",
  observed_at,
  excerpt: "synthetic source record",
  native_url: null,
  redaction: {
    classification_verified: true,
    redaction_verified: true,
    masked: false,
    unparsed: false
  }
});

const drillDown = (evidence_ids: string[]) => ({
  destination: "evidence" as const,
  evidence_ids,
  filter_key: null
});

const drillDownReference = (evidence_ids: string[]) => ({
  source_query: "recorded fixture",
  scope,
  time_window: {
    start: "2026-08-28T08:55:00Z",
    end: "2026-08-28T09:05:00Z"
  },
  evidence_ids
});

const window = {
  range: {
    start: "2026-08-28T08:55:00Z",
    end: "2026-08-28T09:05:00Z"
  },
  evaluated_at: SPRINT_13_FIXTURE_CLOCK,
  watermark: "2026-08-28T08:55:00Z",
  allowed_lateness_seconds: 300,
  state: "open" as const
};

const alertSignal: Signal = {
  id: "00000000-0000-0000-0000-000000000101",
  kind: "alert",
  source: "alertmanager",
  state: "observed",
  observed_at: null,
  ingested_at: null,
  scope,
  targets: [{ kind: "service", id: "service/checkout" }],
  business_severity: null,
  payload: "alert",
  source_record: {
    source_kind: "alertmanager",
    native_id: null,
    revision: null,
    content_digest: "sha256:fixture-alert-1",
    evidence_ids: ["evidence-correlation-alert"]
  },
  dedup_key: null,
  suppression: {
    kind: "maintenance_window",
    rule_ids: [],
    maintenance_window_ids: ["maintenance-checkout-release"],
    evaluated_at: SPRINT_13_FIXTURE_CLOCK,
    policy_version: 13
  },
  evidence_ids: ["evidence-correlation-alert"],
  drill_down: drillDown(["evidence-correlation-alert"]),
  drill_down_reference: drillDownReference(["evidence-correlation-alert"])
};

const anomalySignal: Signal = {
  id: "00000000-0000-0000-0000-000000000102",
  kind: "anomaly",
  source: "prometheus",
  state: "active",
  observed_at: "2026-08-28T08:59:00Z",
  ingested_at: SPRINT_13_FIXTURE_CLOCK,
  scope,
  targets: [{ kind: "service", id: "service/checkout" }],
  business_severity: "S2",
  payload: {
    anomaly: {
      observed_value: 0.08,
      comparison_value: 0.05,
      condition: {
        threshold: { operator: "gte", threshold: "0.05" }
      }
    }
  },
  source_record: {
    source_kind: "prometheus",
    native_id: "rule-checkout-errors",
    revision: "revision-1",
    content_digest: "sha256:fixture-anomaly-1",
    evidence_ids: ["evidence-correlation-anomaly"]
  },
  dedup_key: "dedup:v1:prometheus:anomaly:fixture-checkout",
  suppression: {
    kind: "not_suppressed",
    rule_ids: [],
    maintenance_window_ids: [],
    evaluated_at: SPRINT_13_FIXTURE_CLOCK,
    policy_version: 13
  },
  evidence_ids: ["evidence-correlation-anomaly"],
  drill_down: drillDown(["evidence-correlation-anomaly"]),
  drill_down_reference: drillDownReference(["evidence-correlation-anomaly"])
};

const securitySignal = (
  id: string,
  source: "trivy" | "falco" | "kyverno" | "opa_gatekeeper",
  target: { kind: "resource" | "service" | "deployment" | "topology"; id: string },
  assetKind:
    "container_image" | "runtime_resource" | "kubernetes_resource" | "host" | "policy_subject",
  severity: "critical" | "high" | "medium" | "low" | "negligible" | "unknown" | null,
  evidenceId: string,
  observedAt: string,
  nativeId: string
): Signal => ({
  id,
  kind: "security_finding",
  source,
  state: "observed",
  observed_at: observedAt,
  ingested_at: SPRINT_13_FIXTURE_CLOCK,
  scope,
  targets: [target],
  business_severity: null,
  payload: {
    security_finding: {
      finding: {
        source,
        asset: {
          kind: assetKind,
          target,
          display_name: null,
          artifact_digest: source === "trivy" ? "sha256:fixture-checkout-1" : null
        },
        severity,
        exploitability: null,
        cvss_score: source === "trivy" ? 8.1 : null,
        evidence_ids: [evidenceId]
      }
    }
  },
  source_record: {
    source_kind: source,
    native_id: nativeId,
    revision: "fixture-1",
    content_digest: `sha256:fixture-${source}`,
    evidence_ids: [evidenceId]
  },
  dedup_key: `dedup:v1:${source}:security_finding:${nativeId}`,
  suppression: {
    kind: "not_suppressed",
    rule_ids: [],
    maintenance_window_ids: [],
    evaluated_at: SPRINT_13_FIXTURE_CLOCK,
    policy_version: 13
  },
  evidence_ids: [evidenceId],
  drill_down: drillDown([evidenceId]),
  drill_down_reference: drillDownReference([evidenceId])
});

const trivySignal = securitySignal(
  "00000000-0000-0000-0000-000000000103",
  "trivy",
  { kind: "deployment", id: "deployment/checkout" },
  "container_image",
  "high",
  "evidence-correlation-trivy",
  "2026-08-28T08:57:00Z",
  "CVE-2024-1234"
);
const falcoSignal = securitySignal(
  "00000000-0000-0000-0000-000000000104",
  "falco",
  { kind: "resource", id: "pod/prod/checkout-7d9c" },
  "runtime_resource",
  "critical",
  "evidence-correlation-falco",
  "2026-08-28T08:58:30Z",
  "falco-event-1"
);
const kyvernoSignal = securitySignal(
  "00000000-0000-0000-0000-000000000105",
  "kyverno",
  { kind: "deployment", id: "deployment/prod/checkout" },
  "policy_subject",
  "high",
  "evidence-correlation-kyverno",
  "2026-08-28T08:59:00Z",
  "disallow-host-path:host-path"
);
const gatekeeperSignal = securitySignal(
  "00000000-0000-0000-0000-000000000106",
  "opa_gatekeeper",
  { kind: "deployment", id: "deployment/prod/checkout" },
  "policy_subject",
  "medium",
  "evidence-correlation-gatekeeper",
  "2026-08-28T08:59:15Z",
  "checkout-required-labels"
);

export const correlationFixtureSnapshot: CorrelationSnapshot = {
  generated_at: SPRINT_13_FIXTURE_CLOCK,
  scope,
  request: {
    window: window.range,
    evaluated_at: window.evaluated_at,
    allowed_lateness_seconds: window.allowed_lateness_seconds
  },
  window,
  summary: {
    metrics: [
      {
        key: "normalized_signals",
        value: 6,
        unit: "count",
        evidence_ids: [
          "evidence-correlation-alert",
          "evidence-correlation-anomaly",
          "evidence-correlation-falco",
          "evidence-correlation-gatekeeper",
          "evidence-correlation-kyverno",
          "evidence-correlation-trivy"
        ],
        drill_down: drillDown([
          "evidence-correlation-alert",
          "evidence-correlation-anomaly",
          "evidence-correlation-falco",
          "evidence-correlation-gatekeeper",
          "evidence-correlation-kyverno",
          "evidence-correlation-trivy"
        ]),
        drill_down_reference: drillDownReference([
          "evidence-correlation-alert",
          "evidence-correlation-anomaly",
          "evidence-correlation-falco",
          "evidence-correlation-gatekeeper",
          "evidence-correlation-kyverno",
          "evidence-correlation-trivy"
        ])
      }
    ]
  },
  signals: [alertSignal, anomalySignal, trivySignal, falcoSignal, kyvernoSignal, gatekeeperSignal],
  candidates: [
    {
      id: "candidate-checkout",
      scope,
      window,
      signal_ids: [alertSignal.id, anomalySignal.id],
      grouping_targets: [{ kind: "service", id: "service/checkout" }],
      reasons: [
        {
          kind: "shared_service",
          qualification: "exact_association",
          signal_ids: [alertSignal.id, anomalySignal.id],
          target: { kind: "service", id: "service/checkout" },
          topology_path_ids: [],
          evidence_ids: ["evidence-correlation-alert", "evidence-correlation-anomaly"]
        }
      ],
      status: "active",
      late_signal_ids: [],
      evidence_ids: ["evidence-correlation-alert", "evidence-correlation-anomaly"],
      drill_down: drillDown(["evidence-correlation-alert", "evidence-correlation-anomaly"]),
      drill_down_reference: drillDownReference([
        "evidence-correlation-alert",
        "evidence-correlation-anomaly"
      ])
    }
  ],
  topology_paths: [],
  source_status: [],
  evidence: [
    evidence("evidence-correlation-alert", "alertmanager", "2026-08-28T08:55:00Z"),
    evidence("evidence-correlation-anomaly", "prometheus", "2026-08-28T08:59:00Z"),
    evidence("evidence-correlation-trivy", "trivy", "2026-08-28T08:57:00Z"),
    evidence("evidence-correlation-falco", "falco", "2026-08-28T08:58:30Z"),
    evidence("evidence-correlation-kyverno", "kyverno", "2026-08-28T08:59:00Z"),
    evidence("evidence-correlation-gatekeeper", "opa_gatekeeper", "2026-08-28T08:59:15Z")
  ]
};

export const correlationFixtureSignals: Signal[] = correlationFixtureSnapshot.signals;
