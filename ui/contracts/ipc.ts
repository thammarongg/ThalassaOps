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
  | "fixture";
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

export type ChangeKind = "deployment" | "configuration" | "maintenance" | "connector";
export type ChangeStreamItem = {
  id: string;
  source: string | null;
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

export type Invoke = <T, U>(command: string, args: { envelope: CommandEnvelope<T> }) => Promise<IpcResult<U>>;

/**
 * Tauri command names use lowercase resource.verb components. Commands must
 * be registered with an explicit capability and permission on the Rust side.
 */
export const command = <R extends string, V extends string>(resource: R, verb: V): `${R}.${V}` =>
  `${resource}.${verb}`;
