// SPDX-License-Identifier: Apache-2.0

/** JSON contract shared by the Tauri Rust core and React UI. */

export type UUID = string;

export type ResourceScope = {
  organization_id?: UUID | null;
  team_id?: UUID | null;
  workspace_id?: UUID | null;
  environment_id?: UUID | null;
  resource_ids: UUID[];
};

export type Capability =
  | "WorkspaceRead"
  | "EnvironmentRead"
  | "ResourceRead"
  | "IncidentRead"
  | "IncidentWrite"
  | "PolicyEvaluate"
  | "PolicyManage"
  | "ConnectorRead"
  | "ConnectorAct";

export type Permission =
  | "Read"
  | "Investigate"
  | "RecommendAction"
  | "ExecuteAction"
  | "ManagePolicy"
  | "ManageMembership"
  | "ManageIncident"
  | "AuditRead";

export type CommandName = `${string}.${string}`;

export type CommandEnvelope<T> = {
  request_id: UUID;
  command: CommandName;
  capability: Capability;
  scope: ResourceScope;
  payload: T;
};

export type IpcErrorCode =
  | "INVALID_REQUEST"
  | "NOT_FOUND"
  | "PERMISSION_DENIED"
  | "POLICY_DENIED"
  | "CONNECTOR_UNAVAILABLE"
  | "MALFORMED_RESPONSE"
  | "INVALID_EVENT_SEQUENCE"
  | "INVALID_SEVERITY_OVERRIDE"
  | "INTERNAL_ERROR";

export type IpcError = {
  code: IpcErrorCode;
  message: string;
  details: Record<string, unknown>;
};

export type IpcResult<T> = { ok: true; value: T } | { ok: false; error: IpcError };

export type WorkspaceContext = {
  organization_name: string;
  team_name: string;
  workspace_name: string;
  policy_version: number;
};

export type CloudProvider = "aws" | "azure" | "gcp";
export type CloudResourceType = "kubernetes_cluster" | "compute_instance";
export type CloudHealthState = "healthy" | "degraded" | "unavailable" | "unknown";
export type CloudAccessState =
  | "confirmed"
  | "no_credential"
  | "session_expired"
  | "permission_denied"
  | "unavailable";
export type CloudEnvironment = {
  connector_id: string;
  provider: CloudProvider;
  account_label: string;
  location: string;
  access: CloudAccessState;
  remedy: string;
};
export type CloudResource = {
  provider: CloudProvider;
  environment_id: string;
  resource_type: CloudResourceType;
  id: string;
  name: string;
  location: string;
  health: CloudHealthState;
  status_detail: string;
  console_url: string;
  cli_command: string;
};

export type StatusState = "healthy" | "degraded" | "unavailable" | "warning" | "critical";
export type ConnectorCapability = { key: string; operation: "Read" | "Act"; resource_kinds: string[] };
export type ConnectorManifest = { id: string; display_name: string; version: string; capabilities: ConnectorCapability[] };
export type ConnectorSummary = {
  id: string; kind: string; display_name: string; enabled: boolean; config_metadata: Record<string, unknown>;
  credential_configured: boolean; health_state: StatusState; last_checked_at?: string; last_successful_sync_at?: string;
};
export type ConnectorLogEntry = { id: string; checked_at: string; outcome: StatusState; message: string };
export type ConnectorDiagnostics = { connector: ConnectorSummary; manifest: ConnectorManifest; logs: ConnectorLogEntry[] };
export type KubernetesCondition = { type_: string; status: string; reason?: string; message?: string };
export type KubernetesOwner = { kind: string; name: string; uid?: string };
export type KubernetesHealth = "healthy" | "degraded" | "crash_loop_back_off" | "oom_killed" | "pending" | "unknown";
export type KubernetesResource = { resource: { name: string; kind: string; labels: Record<string, string> }; status?: string; conditions: KubernetesCondition[]; owner?: KubernetesOwner; service_selector?: Record<string, string>; replicas?: { desired: number; ready: number; available?: number }; containers: { name: string; restart_count: number; waiting_reason?: string; terminated_reason?: string; last_terminated_reason?: string }[]; health: KubernetesHealth };
export type KubernetesEvent = { type_?: string; reason?: string; message?: string; involved_kind?: string; involved_name?: string };
export type KubernetesInventory = { resources: KubernetesResource[]; availability: { resource_kind: string; available: boolean; reason?: string }[]; topology: { from_kind: string; from_name: string; to_kind: string; to_name: string; relationship: string }[] };
export type KubernetesManifest = { yaml: string; masked: boolean; risk_class: string };
export type MetricSample = { timestamp: number; value: string };
export type MetricSeries = { labels: Record<string, string>; samples: MetricSample[] };
export type MetricSourceReference = { connector_id: string; query: string; endpoint: string };
export type PrometheusQueryResult = { series: MetricSeries[]; source: MetricSourceReference };

export type LogEntry = {
  timestamp_ns: string;
  line: string;
  parsed: boolean;
  masked: boolean;
  fields: Record<string, string> | null;
  trace_id: string | null;
};
export type LogStream = { labels: Record<string, string>; entries: LogEntry[] };
export type LogSourceReference = { connector_id: string; query: string; endpoint: string };
export type LokiQueryResult = { streams: LogStream[]; source: LogSourceReference; unparsed_count: number };

export type SpanSummary = {
  trace_id: string;
  span_id: string;
  parent_span_id: string | null;
  name: string;
  service_name: string;
  start_time_unix_nano: string;
  duration_nano: string;
  status: string;
  attributes: Record<string, string>;
};
export type TraceSourceReference = { connector_id: string; trace_id: string; endpoint: string };
export type TraceResult = { trace_id: string; spans: SpanSummary[]; source: TraceSourceReference };

export type ResourceReference =
  | { resolved: { namespace: string; kind: string; name: string } }
  | { unresolved: { reason: string } };
export type AlertSourceReference = { connector_id: string; endpoint: string };
export type NormalizedAlert = {
  fingerprint: string;
  state: string;
  starts_at: string;
  ends_at: string;
  labels: Record<string, string>;
  annotations: Record<string, string>;
  generator_url: string | null;
  source: AlertSourceReference;
  resource_reference: ResourceReference;
};

export type GrafanaHealth = { database: string; version: string };
export type GrafanaLinkResult = { url: string };

export type GrafanaLinkRequest = { connector_id: string; target: string; query: string; start: string; end: string };
export type AlertmanagerAlertsRequest = { connector_id: string };
export type PrometheusQueryRequest = { connector_id: string; query: string };
export type PrometheusQueryRangeRequest = { connector_id: string; query: string; start: string; end: string; step_seconds: number };
export type LokiQueryRangeRequest = { connector_id: string; query: string; start: string; end: string; limit: number };
export type TempoTraceRequest = { connector_id: string; trace_id: string };
export type TempoHealthRequest = { id: string };
export type GrafanaHealthRequest = { id: string };

export type ConsoleEvidenceId = string;
export type EvidenceSourceKind =
  | "alertmanager"
  | "prometheus"
  | "kubernetes"
  | "cloud"
  | "health_check"
  | "fixture"
  | "trivy"
  | "falco"
  | "kyverno"
  | "opa_gatekeeper"
  | "github"
  | "gitlab"
  | "argo_cd";
export type EvidenceRedaction = {
  classification_verified: boolean;
  redaction_verified: boolean;
  masked: boolean;
  unparsed: boolean;
};
export type EvidenceRef = {
  id: ConsoleEvidenceId;
  source_kind: EvidenceSourceKind;
  connector_id: string | null;
  scope: ResourceScope;
  endpoint: string;
  query: string | null;
  observed_at: string;
  excerpt: string;
  native_url: string | null;
  redaction: EvidenceRedaction;
};

export type DrillDownDestination =
  | "evidence"
  | "incident_queue"
  | "signal_summary"
  | "change_stream"
  | "environment_status"
  | "topology";
export type DrillDownTarget = {
  destination: DrillDownDestination;
  evidence_ids: ConsoleEvidenceId[];
  filter_key: string | null;
};
export type TimeWindow = { start: string; end: string };

export type SignalId = UUID;
export type SignalKind = "alert" | "anomaly" | "security_finding" | "health_check";
export type SignalState = "active" | "cleared" | "observed" | "unknown";
export type SignalTargetKind = "resource" | "service" | "deployment" | "topology";
export type SignalTarget = { kind: SignalTargetKind; id: string };
export type SourceRecordRef = {
  source_kind: EvidenceSourceKind;
  native_id: string | null;
  revision: string | null;
  content_digest: string;
  evidence_ids: ConsoleEvidenceId[];
};

export type FindingAssetKind =
  "container_image" | "runtime_resource" | "kubernetes_resource" | "host" | "policy_subject";
export type FindingAsset = {
  kind: FindingAssetKind;
  target: SignalTarget;
  display_name: string | null;
  artifact_digest: string | null;
};
export type FindingSeverity = "critical" | "high" | "medium" | "low" | "negligible" | "unknown";
export type Exploitability =
  "exploited" | "known_exploit" | "probable" | "possible" | "unlikely" | "none" | "unknown";
export type VulnerabilityFinding = {
  source: EvidenceSourceKind;
  asset: FindingAsset;
  severity: FindingSeverity | null;
  exploitability: Exploitability | null;
  cvss_score: number | null;
  evidence_ids: ConsoleEvidenceId[];
};

export type SignalPayload =
  | "alert"
  | {
      anomaly: {
        observed_value: number;
        comparison_value: number;
        condition: AnomalyCondition;
      };
    }
  | { security_finding: { finding: VulnerabilityFinding } }
  | { health_check: { outcome: HealthCheckOutcome } };
export type SuppressionKind =
  "not_suppressed" | "rule" | "maintenance_window" | "rule_and_maintenance_window";
export type SuppressionState = {
  kind: SuppressionKind;
  rule_ids: string[];
  maintenance_window_ids: string[];
  evaluated_at: string;
  policy_version: number;
};
export type Signal = {
  id: SignalId;
  kind: SignalKind;
  source: EvidenceSourceKind;
  state: SignalState;
  observed_at: string | null;
  ingested_at: string | null;
  scope: ResourceScope;
  targets: SignalTarget[];
  business_severity: ConsoleSeverity | null;
  payload: SignalPayload;
  source_record: SourceRecordRef;
  dedup_key: string | null;
  suppression: SuppressionState;
  evidence_ids: ConsoleEvidenceId[];
  drill_down: DrillDownTarget;
  drill_down_reference: DrillDownReference;
};

export type CorrelationRequest = {
  window: TimeWindow;
  evaluated_at: string;
  allowed_lateness_seconds: number;
};
export type CorrelationWindowState = "open" | "ready_to_finalize" | "finalized" | "reopened";
export type CorrelationWindow = {
  range: TimeWindow;
  evaluated_at: string;
  watermark: string;
  allowed_lateness_seconds: number;
  state: CorrelationWindowState;
};
export type CorrelationReasonKind =
  | "shared_resource"
  | "shared_service"
  | "shared_deployment"
  | "topology_relation"
  | "preceding_change";
export type CorrelationQualification = "exact_association" | "probable_structural";
export type CorrelationReason = {
  kind: CorrelationReasonKind;
  qualification: CorrelationQualification;
  signal_ids: SignalId[];
  target: SignalTarget | null;
  topology_path_ids: string[];
  evidence_ids: ConsoleEvidenceId[];
};
export type CandidateStatus = "active" | "provisional" | "suppressed";
export type CorrelationCandidate = {
  id: string;
  scope: ResourceScope;
  window: CorrelationWindow;
  signal_ids: SignalId[];
  grouping_targets: SignalTarget[];
  reasons: CorrelationReason[];
  status: CandidateStatus;
  late_signal_ids: SignalId[];
  evidence_ids: ConsoleEvidenceId[];
  drill_down: DrillDownTarget;
  drill_down_reference: DrillDownReference;
};
export type CorrelationMetricKey =
  "normalized_signals" | "active_candidates" | "suppressed_candidates" | "uncorrelated_signals";
export type CorrelationMetric = {
  key: CorrelationMetricKey;
  value: number;
  unit: NumberUnit;
  evidence_ids: ConsoleEvidenceId[];
  drill_down: DrillDownTarget;
  drill_down_reference: DrillDownReference;
};
export type CorrelationSummary = { metrics: CorrelationMetric[] };
export type CorrelationSnapshot = {
  generated_at: string;
  scope: ResourceScope;
  request: CorrelationRequest;
  window: CorrelationWindow;
  summary: CorrelationSummary;
  signals: Signal[];
  candidates: CorrelationCandidate[];
  topology_paths: TopologyPath[];
  source_status: SourceStatus[];
  evidence: EvidenceRef[];
};
export type CorrelationEvidenceRequest = { evidence_ids: ConsoleEvidenceId[] };
export type SuppressionRule = {
  id: string;
  enabled: boolean;
  scope: ResourceScope;
  source: EvidenceSourceKind | null;
  signal_kind: SignalKind | null;
  target: SignalTarget | null;
};
export type MaintenanceWindowReason =
  "planned_change" | "routine_maintenance" | "security_testing" | "unknown";
export type MaintenanceWindow = {
  id: string;
  enabled: boolean;
  scope: ResourceScope;
  target: SignalTarget | null;
  window: TimeWindow;
  reason: MaintenanceWindowReason;
  policy_version: number;
};
export type DrillDownReference = {
  source_query: string;
  scope: ResourceScope;
  time_window: TimeWindow | null;
  evidence_ids: ConsoleEvidenceId[];
};
export type NumberUnit = "count" | "percentage" | "milliseconds" | "seconds";
export type CriticalNumber = {
  key: string;
  value: string;
  unit: NumberUnit;
  evidence_ids: ConsoleEvidenceId[];
  drill_down: DrillDownTarget;
  drill_down_reference: DrillDownReference;
};

export type ConsoleHealthState = "healthy" | "degraded" | "critical" | "unknown";
export type ImpactLevel = "critical" | "high" | "medium" | "low" | "none" | "unknown";
export type ConsoleSeverity = "S1" | "S2" | "S3" | "S4" | "S5";
export type ConsolePriority = "P1" | "P2" | "P3" | "P4" | "P5";
export type ImpactTrajectory = "expanding" | "stable" | "improving" | "unknown";
export type BusinessImpact = {
  level: ImpactLevel;
  summary: string;
  customer_scope: string;
  service_criticality: string;
  trajectory: ImpactTrajectory;
  dimensions: ImpactDimensions;
  evidence_ids: ConsoleEvidenceId[];
};
export type ImpactDimensions = {
  availability: ImpactLevel;
  customer_reach: ImpactLevel;
  business_criticality: ImpactLevel;
  data_integrity: ImpactLevel;
  security_privacy: ImpactLevel;
  financial_contractual: ImpactLevel;
  trajectory: ImpactTrajectory;
  production: boolean;
};
export type ContributingScope = {
  scope: ResourceScope;
  impact: ImpactLevel;
  summary: string;
  evidence_ids: ConsoleEvidenceId[];
};
export type HealthSummary = {
  state: ConsoleHealthState;
  headline: BusinessImpact;
  attention: CriticalNumber;
  impacted_services: CriticalNumber;
  active_by_severity: CriticalNumber[];
  environments_by_state: CriticalNumber[];
  contributing_scopes: ContributingScope[];
};

export type QueueItemSourceKind = "alert" | "anomaly" | "scheduled_health_check" | "fixture_incident";
export type QueueStatus = "detected" | "triage" | "investigating" | "mitigating" | "monitoring";
export type IncidentQueueItem = {
  id: string;
  title: string;
  source_kind: QueueItemSourceKind;
  source_id: string;
  severity: ConsoleSeverity;
  priority: ConsolePriority | null;
  status: QueueStatus;
  business_impact: BusinessImpact;
  scope: ResourceScope;
  detected_at: string;
  opened_at: string;
  last_update: string;
  affected_scope: ResourceScope;
  evidence_ids: ConsoleEvidenceId[];
  drill_down: DrillDownTarget;
  drill_down_reference: DrillDownReference;
};
export type SignalCount = { source_kind: QueueItemSourceKind; count: CriticalNumber };
export type SignalSummary = {
  active_alerts: CriticalNumber;
  active_anomalies: CriticalNumber;
  checks_due: CriticalNumber;
  checks_timed_out: CriticalNumber;
  by_source: SignalCount[];
};
export type AlertSummary = { active: CriticalNumber; by_source: SignalCount[] };
export type AnomalySummary = { active: CriticalNumber; by_severity: CriticalNumber[] };

export type ChangeKind =
  | "deployment"
  | "configuration"
  | "maintenance"
  | "connector"
  | "code_commit"
  | "code_merge"
  | "sync"
  | "rollback";
export type ChangeStreamItem = {
  id: string;
  source: EvidenceSourceKind;
  occurred_at: string;
  kind: ChangeKind;
  summary: string;
  actor: string | null;
  target_resource: string | null;
  native_link: string | null;
  scope: ResourceScope;
  evidence_ids: ConsoleEvidenceId[];
  drill_down: DrillDownTarget;
};

export type ChangeEventId = UUID;
export type ChangeOutcome = "succeeded" | "failed" | "in_progress" | "reverted" | "unknown";
export type ChangeActorKind = "human" | "automation" | "unknown";
export type ChangeActor = { kind: ChangeActorKind; handle: string | null };
export type ChangeRevision = {
  id: string;
  short_id: string | null;
  parent_ids: string[];
};
export type ChangeRepositoryRef = {
  host: string;
  namespace: string;
  name: string;
  reference: string | null;
};
export type ChangeDiffStat = {
  files_changed: number;
  insertions: number;
  deletions: number;
  unit: NumberUnit;
};
export type ChangeLinkKind = "commit" | "pull_request" | "compare" | "deployment" | "application";
export type ChangeSourceLink = { kind: ChangeLinkKind; url: string };
export type ChangeEvent = {
  id: ChangeEventId;
  source: EvidenceSourceKind;
  kind: ChangeKind;
  outcome: ChangeOutcome;
  occurred_at: string;
  ingested_at: string | null;
  scope: ResourceScope;
  targets: SignalTarget[];
  revision: ChangeRevision | null;
  actor: ChangeActor;
  repository: ChangeRepositoryRef | null;
  environment: string | null;
  diff_stat: ChangeDiffStat | null;
  changed_paths: string[];
  source_link: ChangeSourceLink | null;
  source_record: SourceRecordRef;
  evidence_ids: ConsoleEvidenceId[];
  drill_down: DrillDownTarget;
  drill_down_reference: DrillDownReference;
};
export type ChangeTimeline = {
  window: TimeWindow;
  entry_ids: ChangeEventId[];
  truncated: boolean;
};
export type ChangeAssociation = {
  change_id: ChangeEventId;
  candidate_id: string;
  qualification: CorrelationQualification;
  lead_time_seconds: number;
  target: SignalTarget | null;
  topology_path_ids: string[];
  evidence_ids: ConsoleEvidenceId[];
};
export type ChangeMetricKey =
  | "changes_in_window"
  | "associated_changes"
  | "changes_by_source";
export type ChangeMetric = {
  key: ChangeMetricKey;
  source: EvidenceSourceKind | null;
  value: number;
  unit: NumberUnit;
  evidence_ids: ConsoleEvidenceId[];
  drill_down: DrillDownTarget;
  drill_down_reference: DrillDownReference;
};
export type ChangeRequest = {
  window: TimeWindow;
  evaluated_at: string;
  lookback_seconds: number;
  limit: number;
};
export type ChangeEvidenceRequest = { evidence_ids: ConsoleEvidenceId[] };
export type ChangeSnapshot = {
  generated_at: string;
  scope: ResourceScope;
  request_window: TimeWindow;
  lookback_seconds: number;
  events: ChangeEvent[];
  timeline: ChangeTimeline;
  associations: ChangeAssociation[];
  metrics: ChangeMetric[];
  source_statuses: SourceStatus[];
};
export type StatusReason =
  | "not_configured"
  | "unreachable"
  | "timed_out"
  | "policy_denied"
  | "no_data_in_window"
  | "unknown";
export type ChangeStreamState = "available" | "empty" | "unavailable";
export type ChangeStreamStatus = {
  state: ChangeStreamState;
  reason: StatusReason | null;
  detail: string | null;
};
export type EnvironmentStatus = {
  environment_id: string;
  name: string;
  provider: string | null;
  health: ConsoleHealthState;
  status_detail: string;
  resource_count: CriticalNumber;
  last_observed_at: string;
  evidence_ids: ConsoleEvidenceId[];
  drill_down: DrillDownTarget;
};
export type SourceState = "fresh" | "stale" | "unavailable" | "unverified";
export type SourceStatus = {
  source_key: string;
  state: SourceState;
  reason: StatusReason | null;
  detail: string | null;
  observed_at: string | null;
  evidence_ids: ConsoleEvidenceId[];
};

export type WidgetId =
  | "health_summary"
  | "incident_queue"
  | "signal_summary"
  | "change_stream"
  | "environment_status";
export type WidgetSize = "compact" | "standard" | "wide";
export type WidgetDefinition = {
  id: WidgetId;
  title_key: string;
  default_order: number;
  default_size: WidgetSize;
  required: boolean;
};
export type WidgetPreference = {
  id: WidgetId;
  visible: boolean;
  order: number;
  size: WidgetSize;
  collapsed: boolean;
};
export type WidgetKind = WidgetId;
export type WidgetOptions = Record<string, unknown>;
export type WidgetConfig = {
  id: WidgetId;
  kind: WidgetKind;
  visible: boolean;
  order: number;
  options: WidgetOptions;
};

export type AnomalyCondition =
  | { threshold: { operator: "gt" | "gte" | "lt" | "lte"; threshold: string } }
  | {
      rate_of_change: {
        direction: "increase" | "decrease" | "absolute";
        threshold_per_second: string;
        window_seconds: number;
      };
    };
export type AnomalyRule = {
  id: string;
  name: string;
  enabled: boolean;
  scope: ResourceScope;
  metric_key: string;
  condition: AnomalyCondition;
  severity: ConsoleSeverity;
  cooldown_seconds: number;
};
export type MetricFixtureSample = { timestamp_seconds: number; value: string };
export type MetricFixtureSource = { connector_id: string; query: string; endpoint: string };
export type MetricFixture = {
  key: string;
  scope: ResourceScope;
  labels: Record<string, string>;
  samples: MetricFixtureSample[];
  source: MetricFixtureSource;
};
export type AnomalySignal = {
  id: string;
  rule_id: string;
  metric_key: string;
  severity: ConsoleSeverity;
  observed_at: string;
  observed_value: number;
  comparison_value: number;
  condition: AnomalyCondition;
  scope: ResourceScope;
  evidence_id: ConsoleEvidenceId;
};
export type AnomalyEvaluationStatus = "triggered" | "not_triggered" | "insufficient_data";
export type AnomalyEvaluation = {
  rule_id: string;
  metric_key: string;
  status: AnomalyEvaluationStatus;
  signal: AnomalySignal | null;
};
export type HealthCheckOutcome =
  | "healthy"
  | "degraded"
  | "unavailable"
  | "timed_out"
  | "skipped_not_due"
  | "skipped_cooldown"
  | "skipped_disabled";
export type HealthCheckSource =
  | { connector: { connector_id: string; probe_key: string } }
  | { kubernetes: { connector_id: string; resource_key: string } }
  | { observability: { connector_id: string; probe_key: string } }
  | { fixture: { fixture_key: string } };
export type HealthCheckSchedule = {
  id: string;
  name: string;
  enabled: boolean;
  scope: ResourceScope;
  source: HealthCheckSource;
  interval_seconds: number;
  timeout_ms: number;
  cooldown_seconds: number;
  last_run_at: string | null;
  last_signal_at: string | null;
  defined_by: string | null;
  defined_at: string | null;
  last_outcome: HealthCheckOutcome | null;
};
export type FixtureHealthCheck = {
  outcome: HealthCheckOutcome;
  duration_ms: number;
  evidence_id: ConsoleEvidenceId | null;
};
export type HealthCheckAudit = {
  run_id: string;
  schedule_id: string;
  triggered_by: string;
  started_at: string;
  completed_at: string;
  duration_ms: number;
  scope: ResourceScope;
  source: HealthCheckSource;
  outcome: HealthCheckOutcome;
  cooldown_suppressed: boolean;
  policy_version: number;
};
export type HealthCheckResult = {
  schedule_id: string;
  outcome: HealthCheckOutcome;
  observed_at: string;
  evidence_id: ConsoleEvidenceId | null;
  audit: HealthCheckAudit;
};

export type OperationsSnapshot = {
  generated_at: string;
  scope: ResourceScope;
  source_status: SourceStatus[];
  health_summary: HealthSummary;
  incident_queue: IncidentQueueItem[];
  signal_summary: SignalSummary;
  changes: ChangeStreamItem[];
  change_stream_status: ChangeStreamStatus;
  environments: EnvironmentStatus[];
  evidence: EvidenceRef[];
  widget_registry: WidgetDefinition[];
};

export type OperationsSnapshotRequest = null;
export type OperationsEvidenceRequest = { evidence_ids: ConsoleEvidenceId[] };
export type OperationsSnapshotResponse = OperationsSnapshot;
export type OperationsEvidenceResponse = EvidenceRef[];

export type TopologyNodeKind =
  | "environment"
  | "cluster"
  | "namespace"
  | "workload"
  | "service"
  | "pod"
  | "node"
  | "cloud_resource"
  | "observability_target";

export type TopologyOwnershipSource =
  | "explicit_label"
  | "resource_scope"
  | "environment_default"
  | "fixture"
  | "unassigned";

export type TopologyOwnership = {
  team_id: UUID | null;
  team_name: string | null;
  source: TopologyOwnershipSource;
  evidence_ids: ConsoleEvidenceId[];
};

export type TopologyMetric = {
  key: string;
  value: number;
  unit: NumberUnit;
  evidence_ids: ConsoleEvidenceId[];
  drill_down: DrillDownTarget;
  drill_down_reference: DrillDownReference;
};

export type TopologyNode = {
  id: string;
  kind: TopologyNodeKind;
  name: string;
  native_kind: string | null;
  native_id: string | null;
  environment_id: string | null;
  provider: string | null;
  scope: ResourceScope;
  status: ConsoleHealthState;
  labels: Record<string, string>;
  ownership: TopologyOwnership;
  metric: TopologyMetric | null;
  affected_by_incident: boolean;
  evidence_ids: ConsoleEvidenceId[];
  drill_down: DrillDownTarget;
};

export type TopologyEdgeKind =
  | "contains"
  | "owns"
  | "selects"
  | "routes_to"
  | "runs_on"
  | "depends_on";

export type TopologySourceKind = "kubernetes" | "cloud" | "observability" | "fixture";

export type TopologyEdgeProvenance = {
  source: TopologySourceKind;
  source_key: string;
  observed_at: string | null;
};

export type TopologyEdge = {
  id: string;
  upstream_node_id: string;
  downstream_node_id: string;
  kind: TopologyEdgeKind;
  provenance: TopologyEdgeProvenance[];
  confidence: number;
  metadata: Record<string, string>;
  evidence_ids: ConsoleEvidenceId[];
  drill_down: DrillDownTarget;
};

export type TopologyDirection = "upstream" | "downstream" | "both";
export type TopologyPathKind = "probable_structural";
export type TopologyPathTermination = "leaf" | "cycle_detected" | "depth_limit";

export type TopologyPath = {
  id: string;
  root_node_id: string;
  terminal_node_id: string;
  node_ids: string[];
  edge_ids: string[];
  direction: TopologyDirection;
  depth: number;
  confidence: number;
  kind: TopologyPathKind;
  termination: TopologyPathTermination;
  cycle_edge_id: string | null;
  evidence_ids: ConsoleEvidenceId[];
  drill_down: DrillDownTarget;
};

export type TopologyTraversal = {
  direction: TopologyDirection;
  max_depth: number;
};

export type TopologyFilter = {
  environment_ids: string[];
  team_ids: UUID[];
  incident_id: string | null;
};

export type TopologyRequest = {
  filter: TopologyFilter;
  focus_node_id: string | null;
  traversal: TopologyTraversal;
};

export type TopologySummary = {
  visible_nodes: TopologyMetric;
  visible_edges: TopologyMetric;
  affected_nodes: TopologyMetric;
  probable_paths: TopologyMetric;
};

export type TopologySnapshot = {
  generated_at: string;
  scope: ResourceScope;
  filter: TopologyFilter;
  focus_node_id: string | null;
  traversal: TopologyTraversal;
  summary: TopologySummary;
  nodes: TopologyNode[];
  edges: TopologyEdge[];
  paths: TopologyPath[];
  source_status: SourceStatus[];
  evidence: EvidenceRef[];
};

export type TopologyEvidenceRequest = { evidence_ids: ConsoleEvidenceId[] };

/*
 * Sprint 15 incident write model.  These shapes mirror
 * `thalassa_domain::Incident`, its timeline events, and the request/page
 * contracts frozen for the `incident.*` command surface.
 */

export type IncidentStatus =
  | "detected"
  | "triage"
  | "investigating"
  | "mitigating"
  | "monitoring"
  | "resolved"
  | "closed"
  | "reopened";
export type IncidentDisposition =
  | "duplicate"
  | "false_positive"
  | "suppressed"
  | "cancelled"
  | "informational";
export type IncidentSeverity = ConsoleSeverity;
export type IncidentRole =
  | "owner"
  | "incident_commander"
  | "technical_lead"
  | "communications_lead"
  | "approver"
  | "change_owner"
  | "stakeholder";
export type IncidentSourceKind =
  | "alert"
  | "anomaly"
  | "user_report"
  | "scheduled_health_check"
  | "vulnerability_finding"
  | "manual_report";
export type IncidentEventKind =
  | "incident_created"
  | "triggers_attached"
  | "status_transitioned"
  | "severity_changed"
  | "disposition_changed"
  | "role_changed";

export type IncidentSeverityOverride = {
  derived: IncidentSeverity;
  selected: IncidentSeverity;
  actor_id: UUID;
  reason: string;
  evidence_ids: ConsoleEvidenceId[];
};

export type IncidentRoleAssignment = {
  role: IncidentRole;
  principal_id: UUID;
  assigned_by: UUID;
  assigned_at: string;
};

export type IncidentReport = { reporter_id: UUID | null; summary: string };

export type IncidentTrigger = {
  id: UUID;
  source_kind: IncidentSourceKind;
  source_id: string;
  source_record_digest: string | null;
  scope: ResourceScope;
  observed_at: string;
  signal_id: UUID | null;
  evidence_ids: ConsoleEvidenceId[];
  report: IncidentReport | null;
};

export type Incident = {
  id: UUID;
  summary: string;
  scope: ResourceScope;
  owning_team_id: UUID;
  business_impact: BusinessImpact;
  derived_severity: IncidentSeverity;
  severity_override: IncidentSeverityOverride | null;
  status: IncidentStatus;
  disposition: IncidentDisposition | null;
  duplicate_of_incident_id: UUID | null;
  trigger_ids: UUID[];
  signal_ids: UUID[];
  evidence_ids: ConsoleEvidenceId[];
  hypothesis_ids: UUID[];
  action_ids: UUID[];
  roles: IncidentRoleAssignment[];
  version: number;
  created_at: string;
  updated_at: string;
};

export type TriageContext = {
  business_impact: BusinessImpact;
  owner: UUID;
  duplicate_checked: boolean;
};
export type InvestigatingContext = {
  note: string;
  evidence_ids: ConsoleEvidenceId[];
};
export type MitigatingContext = {
  action_description: string;
  executor: UUID;
  expected_impact: string;
};
export type MonitoringContext = {
  verification_seconds: number;
  success_criteria: string;
  watch_owner: UUID;
};
export type ResolvedContext = {
  resolution_summary: string;
  evidence_ids: ConsoleEvidenceId[];
  impact_ended_at: string;
};
export type ClosedContext = { closure_notes: string; follow_up_ids: string[] };
export type ReopenedContext = {
  reason: string;
  evidence_ids: ConsoleEvidenceId[];
  recurrence_signal_id: UUID | null;
};

export type IncidentTransition =
  | { target: "triage"; context: TriageContext }
  | { target: "investigating"; context: InvestigatingContext }
  | { target: "mitigating"; context: MitigatingContext }
  | { target: "monitoring"; context: MonitoringContext }
  | { target: "resolved"; context: ResolvedContext }
  | { target: "closed"; context: ClosedContext }
  | { target: "reopened"; context: ReopenedContext };

export type CreatedPayload = {
  summary: string;
  scope: ResourceScope;
  owning_team_id: UUID;
  derived_severity: IncidentSeverity;
  trigger_ids: UUID[];
  initial_roles: IncidentRoleAssignment[];
};
export type TriggersAttachedPayload = { trigger_ids: UUID[] };
export type StatusTransitionedPayload = {
  from: IncidentStatus;
  to: IncidentStatus;
  transition: IncidentTransition;
};
export type SeverityChangedPayload = {
  previous_impact: BusinessImpact;
  current_impact: BusinessImpact;
  previous_severity: IncidentSeverity;
  current_severity: IncidentSeverity;
  previous_override: IncidentSeverityOverride | null;
  current_override: IncidentSeverityOverride | null;
};
export type DispositionChangedPayload = {
  previous: IncidentDisposition | null;
  current: IncidentDisposition | null;
  duplicate_of_incident_id: UUID | null;
};
export type RoleChangedPayload = {
  role: IncidentRole;
  previous_principal_ids: UUID[];
  current_principal_id: UUID | null;
};

export type IncidentTimelinePayload =
  | { kind: "created"; data: CreatedPayload }
  | { kind: "triggers_attached"; data: TriggersAttachedPayload }
  | { kind: "status_transitioned"; data: StatusTransitionedPayload }
  | { kind: "severity_changed"; data: SeverityChangedPayload }
  | { kind: "disposition_changed"; data: DispositionChangedPayload }
  | { kind: "role_changed"; data: RoleChangedPayload };

export type IncidentTimelineEvent = {
  id: UUID;
  incident_id: UUID;
  sequence: number;
  kind: IncidentEventKind;
  actor_id: UUID;
  reason: string | null;
  occurred_at: string;
  request_id: UUID;
  policy_version: number;
  payload: IncidentTimelinePayload;
};

export type IncidentTriggerInput =
  | { kind: "alert"; source_id: string }
  | { kind: "anomaly"; source_id: string }
  | { kind: "scheduled_health_check"; source_id: string }
  | { kind: "vulnerability_finding"; source_id: string }
  | {
      kind: "user_report";
      reporter_id: UUID;
      observed_at: string;
      summary: string;
      scope: ResourceScope;
    }
  | {
      kind: "manual_report";
      observed_at: string;
      summary: string;
      scope: ResourceScope;
    };

export type IncidentRoleAssignmentInput = {
  role: IncidentRole;
  principal_id: UUID;
};

export type IncidentCreateRequest = {
  summary: string;
  triggers: IncidentTriggerInput[];
  business_impact: BusinessImpact;
  initial_roles: IncidentRoleAssignmentInput[];
};
export type IncidentGetRequest = { incident_id: UUID };
export type IncidentListRequest = { cursor: string | null; limit: number };
export type IncidentTimelineRequest = {
  incident_id: UUID;
  after_sequence: number | null;
  limit: number;
};
export type IncidentTransitionRequest = {
  incident_id: UUID;
  expected_version: number;
  transition: IncidentTransition;
};
export type IncidentSeverityCommand =
  | {
      action: "reassess";
      details: { business_impact: BusinessImpact; reason: string };
    }
  | {
      action: "override";
      details: {
        selected: IncidentSeverity;
        reason: string;
        evidence_ids: ConsoleEvidenceId[];
      };
    };
export type IncidentSeverityRequest = {
  incident_id: UUID;
  expected_version: number;
  command: IncidentSeverityCommand;
};
export type IncidentDispositionCommand = {
  disposition: IncidentDisposition | null;
  duplicate_of_incident_id: UUID | null;
  reason: string;
};
export type IncidentDispositionRequest = {
  incident_id: UUID;
  expected_version: number;
  command: IncidentDispositionCommand;
};
export type IncidentRoleCommand =
  | { action: "assign"; details: { role: IncidentRole; principal_id: UUID } }
  | { action: "replace"; details: { role: IncidentRole; principal_id: UUID } }
  | { action: "release"; details: { role: IncidentRole; principal_id: UUID } };
export type IncidentRoleRequest = {
  incident_id: UUID;
  expected_version: number;
  command: IncidentRoleCommand;
};

export type IncidentPage = {
  items: Incident[];
  next_cursor: string | null;
};
export type IncidentTimelinePage = {
  incident_id: UUID;
  events: IncidentTimelineEvent[];
  next_sequence: number | null;
};

export type Invoke = <T, U>(command: string, args: { envelope: CommandEnvelope<T> }) => Promise<IpcResult<U>>;

/**
 * Tauri command names use lowercase resource.verb components. Commands must
 * be registered with an explicit capability and permission on the Rust side.
 */
export const command = <R extends string, V extends string>(resource: R, verb: V): `${R}.${V}` =>
  `${resource}.${verb}`;
