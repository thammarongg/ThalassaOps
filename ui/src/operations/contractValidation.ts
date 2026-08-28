import type {
  BusinessImpact,
  ChangeStreamItem,
  ChangeStreamStatus,
  ConsoleHealthState,
  ConsolePriority,
  ConsoleSeverity,
  CriticalNumber,
  DrillDownReference,
  DrillDownTarget,
  EnvironmentStatus,
  EvidenceRef,
  HealthSummary,
  ImpactLevel,
  IncidentQueueItem,
  OperationsSnapshot,
  QueueItemSourceKind,
  QueueStatus,
  ResourceScope,
  SignalCount,
  SignalSummary,
  SourceStatus,
  StatusReason,
  TimeWindow,
  WidgetDefinition,
  WidgetId,
  WidgetSize
} from "../../contracts/ipc";

const healthStates: ConsoleHealthState[] = ["healthy", "degraded", "critical", "unknown"];
const impactLevels: ImpactLevel[] = ["critical", "high", "medium", "low", "none", "unknown"];
const severities: ConsoleSeverity[] = ["S1", "S2", "S3", "S4", "S5"];
const priorities: ConsolePriority[] = ["P1", "P2", "P3", "P4", "P5"];
const queueSources: QueueItemSourceKind[] = [
  "alert",
  "anomaly",
  "scheduled_health_check",
  "fixture_incident"
];
const queueStatuses: QueueStatus[] = [
  "detected",
  "triage",
  "investigating",
  "mitigating",
  "monitoring"
];
const statusReasons: StatusReason[] = [
  "not_configured",
  "unreachable",
  "timed_out",
  "policy_denied",
  "no_data_in_window",
  "unknown"
];
const widgetIds: WidgetId[] = [
  "health_summary",
  "incident_queue",
  "signal_summary",
  "change_stream",
  "environment_status"
];
const widgetSizes: WidgetSize[] = ["compact", "standard", "wide"];
const numberUnits = ["count", "percentage", "milliseconds", "seconds"] as const;
const evidenceSources = [
  "alertmanager",
  "prometheus",
  "kubernetes",
  "cloud",
  "health_check",
  "fixture"
] as const;
const destinations = [
  "evidence",
  "incident_queue",
  "signal_summary",
  "change_stream",
  "environment_status"
] as const;
const changeKinds = ["deployment", "configuration", "maintenance", "connector"] as const;

type UnknownRecord = Record<string, unknown>;

const isRecord = (value: unknown): value is UnknownRecord =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const isString = (value: unknown): value is string => typeof value === "string";

const isNonEmptyString = (value: unknown): value is string =>
  isString(value) && value.trim() !== "";

const isNullableString = (value: unknown): value is string | null =>
  value === null || isString(value);

export const isTrustedNativeUrl = (value: unknown): value is string => {
  if (!isNonEmptyString(value)) return false;
  try {
    const url = new URL(value);
    return url.protocol === "https:" && url.hostname !== "" && !url.username && !url.password;
  } catch {
    return false;
  }
};

const isBoolean = (value: unknown): value is boolean => typeof value === "boolean";

const isStringArray = (value: unknown): value is string[] =>
  Array.isArray(value) && value.every((item) => isString(item) && item.trim() !== "");

const sharesEvidence = (left: string[], right: string[]) => left.some((id) => right.includes(id));

const isEnum = <T extends string>(value: unknown, values: readonly T[]): value is T =>
  isString(value) && values.includes(value as T);

const isScope = (value: unknown): value is ResourceScope => {
  if (!isRecord(value) || !isStringArray(value.resource_ids)) return false;
  return ["organization_id", "team_id", "workspace_id", "environment_id"].every(
    (key) => value[key] === undefined || isString(value[key])
  );
};

const isBusinessImpact = (value: unknown): value is BusinessImpact => {
  if (!isRecord(value)) return false;
  return (
    isEnum(value.level, impactLevels) &&
    isNonEmptyString(value.summary) &&
    isNonEmptyString(value.customer_scope) &&
    isNonEmptyString(value.service_criticality) &&
    isEnum(value.trajectory, ["expanding", "stable", "improving", "unknown"])
  );
};

const isTimeWindow = (value: unknown): value is TimeWindow =>
  isRecord(value) && isNonEmptyString(value.start) && isNonEmptyString(value.end);

const isDrillDownTarget = (value: unknown): value is DrillDownTarget =>
  isRecord(value) &&
  isEnum(value.destination, destinations) &&
  isStringArray(value.evidence_ids) &&
  (value.filter_key === null || isString(value.filter_key));

const isDrillDownReference = (value: unknown): value is DrillDownReference =>
  isRecord(value) &&
  isNonEmptyString(value.source_query) &&
  isScope(value.scope) &&
  (value.time_window === null || isTimeWindow(value.time_window)) &&
  isStringArray(value.evidence_ids);

const isCriticalNumber = (value: unknown): value is CriticalNumber => {
  if (!isRecord(value)) return false;
  const numberValue = isString(value.value) ? Number(value.value) : Number.NaN;
  return (
    isNonEmptyString(value.key) &&
    isNonEmptyString(value.value) &&
    Number.isFinite(numberValue) &&
    isEnum(value.unit, numberUnits) &&
    isStringArray(value.evidence_ids) &&
    isDrillDownTarget(value.drill_down) &&
    isDrillDownReference(value.drill_down_reference) &&
    sharesEvidence(value.evidence_ids, value.drill_down.evidence_ids) &&
    sharesEvidence(value.evidence_ids, value.drill_down_reference.evidence_ids)
  );
};

const isSourceStatus = (value: unknown): value is SourceStatus =>
  isRecord(value) &&
  isNonEmptyString(value.source_key) &&
  isEnum(value.state, ["fresh", "stale", "unavailable", "unverified"]) &&
  (value.reason === null || isEnum(value.reason, statusReasons)) &&
  isNullableString(value.detail) &&
  (value.observed_at === null || isString(value.observed_at)) &&
  Array.isArray(value.evidence_ids) &&
  value.evidence_ids.every(isNonEmptyString);

export const isEvidence = (value: unknown): value is EvidenceRef =>
  isRecord(value) &&
  isNonEmptyString(value.id) &&
  isEnum(value.source_kind, evidenceSources) &&
  (value.connector_id === null || isNonEmptyString(value.connector_id)) &&
  isScope(value.scope) &&
  isNonEmptyString(value.endpoint) &&
  (value.query === null || isString(value.query)) &&
  isNonEmptyString(value.observed_at) &&
  isNonEmptyString(value.excerpt) &&
  (value.native_url === null || isTrustedNativeUrl(value.native_url)) &&
  isRecord(value.redaction) &&
  isBoolean(value.redaction.classification_verified) &&
  isBoolean(value.redaction.redaction_verified) &&
  isBoolean(value.redaction.masked) &&
  isBoolean(value.redaction.unparsed) &&
  value.redaction.classification_verified &&
  value.redaction.redaction_verified;

export const isEvidenceResponse = (
  value: unknown,
  expectedIds: string[]
): value is EvidenceRef[] => {
  if (!Array.isArray(value) || !value.every(isEvidence)) return false;
  const returnedIds = new Set(value.map((item) => item.id));
  return (
    returnedIds.size === value.length &&
    returnedIds.size === expectedIds.length &&
    expectedIds.every((id) => returnedIds.has(id))
  );
};

const isQueueItem = (value: unknown): value is IncidentQueueItem =>
  isRecord(value) &&
  isNonEmptyString(value.id) &&
  isNonEmptyString(value.title) &&
  isEnum(value.source_kind, queueSources) &&
  isNonEmptyString(value.source_id) &&
  isEnum(value.severity, severities) &&
  (value.priority === null || isEnum(value.priority, priorities)) &&
  isEnum(value.status, queueStatuses) &&
  isBusinessImpact(value.business_impact) &&
  isScope(value.scope) &&
  isNonEmptyString(value.detected_at) &&
  isNonEmptyString(value.opened_at) &&
  isNonEmptyString(value.last_update) &&
  isScope(value.affected_scope) &&
  isStringArray(value.evidence_ids) &&
  isDrillDownTarget(value.drill_down) &&
  isDrillDownReference(value.drill_down_reference) &&
  sharesEvidence(value.evidence_ids, value.drill_down.evidence_ids) &&
  sharesEvidence(value.evidence_ids, value.drill_down_reference.evidence_ids);

const isSignalCount = (value: unknown): value is SignalCount =>
  isRecord(value) && isEnum(value.source_kind, queueSources) && isCriticalNumber(value.count);

const isSignalSummary = (value: unknown): value is SignalSummary =>
  isRecord(value) &&
  isCriticalNumber(value.active_alerts) &&
  isCriticalNumber(value.active_anomalies) &&
  isCriticalNumber(value.checks_due) &&
  isCriticalNumber(value.checks_timed_out) &&
  Array.isArray(value.by_source) &&
  value.by_source.every(isSignalCount);

const isChange = (value: unknown): value is ChangeStreamItem =>
  isRecord(value) &&
  isNonEmptyString(value.id) &&
  isNullableString(value.source) &&
  isNonEmptyString(value.occurred_at) &&
  isEnum(value.kind, changeKinds) &&
  isNonEmptyString(value.summary) &&
  isNullableString(value.actor) &&
  isNullableString(value.target_resource) &&
  isNullableString(value.native_link) &&
  isScope(value.scope) &&
  isStringArray(value.evidence_ids) &&
  isDrillDownTarget(value.drill_down) &&
  sharesEvidence(value.evidence_ids, value.drill_down.evidence_ids);

const isChangeStatus = (value: unknown): value is ChangeStreamStatus =>
  isRecord(value) &&
  isEnum(value.state, ["available", "empty", "unavailable"]) &&
  (value.reason === null || isEnum(value.reason, statusReasons)) &&
  isNullableString(value.detail);

const isEnvironment = (value: unknown): value is EnvironmentStatus =>
  isRecord(value) &&
  isNonEmptyString(value.environment_id) &&
  isNonEmptyString(value.name) &&
  isNullableString(value.provider) &&
  isEnum(value.health, healthStates) &&
  isNonEmptyString(value.status_detail) &&
  isCriticalNumber(value.resource_count) &&
  isNonEmptyString(value.last_observed_at) &&
  isStringArray(value.evidence_ids) &&
  isDrillDownTarget(value.drill_down) &&
  sharesEvidence(value.evidence_ids, value.drill_down.evidence_ids) &&
  sharesEvidence(value.evidence_ids, value.resource_count.evidence_ids);

const isHealthSummary = (value: unknown): value is HealthSummary =>
  isRecord(value) &&
  isEnum(value.state, healthStates) &&
  isBusinessImpact(value.headline) &&
  isCriticalNumber(value.attention) &&
  isCriticalNumber(value.impacted_services) &&
  Array.isArray(value.active_by_severity) &&
  value.active_by_severity.every(isCriticalNumber) &&
  Array.isArray(value.environments_by_state) &&
  value.environments_by_state.every(isCriticalNumber) &&
  Array.isArray(value.contributing_scopes) &&
  value.contributing_scopes.every(
    (scope) =>
      isRecord(scope) &&
      isScope(scope.scope) &&
      isEnum(scope.impact, impactLevels) &&
      isNonEmptyString(scope.summary) &&
      isStringArray(scope.evidence_ids)
  );

const isWidget = (value: unknown): value is WidgetDefinition =>
  isRecord(value) &&
  isEnum(value.id, widgetIds) &&
  isNonEmptyString(value.title_key) &&
  typeof value.default_order === "number" &&
  Number.isInteger(value.default_order) &&
  value.default_order >= 0 &&
  isEnum(value.default_size, widgetSizes) &&
  isBoolean(value.required);

const allCriticalNumbers = (snapshot: OperationsSnapshot): CriticalNumber[] => [
  snapshot.health_summary.attention,
  snapshot.health_summary.impacted_services,
  snapshot.signal_summary.active_alerts,
  snapshot.signal_summary.active_anomalies,
  snapshot.signal_summary.checks_due,
  snapshot.signal_summary.checks_timed_out,
  ...snapshot.health_summary.active_by_severity,
  ...snapshot.health_summary.environments_by_state,
  ...snapshot.signal_summary.by_source.map((item) => item.count),
  ...snapshot.environments.map((item) => item.resource_count)
];

export const isOperationsSnapshot = (value: unknown): value is OperationsSnapshot => {
  if (!isRecord(value)) return false;
  if (
    !isNonEmptyString(value.generated_at) ||
    !isScope(value.scope) ||
    !Array.isArray(value.source_status) ||
    !value.source_status.every(isSourceStatus) ||
    !isHealthSummary(value.health_summary) ||
    !Array.isArray(value.incident_queue) ||
    !value.incident_queue.every(isQueueItem) ||
    !isSignalSummary(value.signal_summary) ||
    !Array.isArray(value.changes) ||
    !value.changes.every(isChange) ||
    !isChangeStatus(value.change_stream_status) ||
    !Array.isArray(value.environments) ||
    !value.environments.every(isEnvironment) ||
    !Array.isArray(value.evidence) ||
    !value.evidence.every(isEvidence) ||
    !Array.isArray(value.widget_registry) ||
    !value.widget_registry.every(isWidget)
  ) {
    return false;
  }

  const snapshot = value as OperationsSnapshot;
  const evidenceIds = new Set(snapshot.evidence.map((item) => item.id));
  if (evidenceIds.size !== snapshot.evidence.length) return false;
  const references = [
    ...allCriticalNumbers(snapshot).flatMap((number) => [
      number.evidence_ids,
      number.drill_down.evidence_ids,
      number.drill_down_reference.evidence_ids
    ]),
    ...snapshot.source_status.map((status) => status.evidence_ids),
    ...snapshot.incident_queue.flatMap((item) => [
      item.evidence_ids,
      item.drill_down.evidence_ids,
      item.drill_down_reference.evidence_ids
    ]),
    ...snapshot.changes.flatMap((change) => [change.evidence_ids, change.drill_down.evidence_ids]),
    ...snapshot.environments.flatMap((environment) => [
      environment.evidence_ids,
      environment.drill_down.evidence_ids
    ]),
    ...snapshot.health_summary.contributing_scopes.map((scope) => scope.evidence_ids)
  ];
  return references.every((ids) => ids.every((id) => evidenceIds.has(id)));
};
