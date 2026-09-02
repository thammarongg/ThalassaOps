import type {
  ChangeStreamItem,
  ChangeStreamStatus,
  ConsoleHealthState,
  ConsolePriority,
  ConsoleSeverity,
  CriticalNumber,
  EnvironmentStatus,
  EvidenceSourceKind,
  HealthSummary,
  ImpactLevel,
  IncidentQueueItem,
  OperationsSnapshot,
  QueueItemSourceKind,
  QueueStatus,
  SignalCount,
  SignalSummary,
  WidgetDefinition,
  WidgetId,
  WidgetSize
} from "../../contracts/ipc";
import {
  isBoolean,
  isDrillDownReference,
  isDrillDownTarget,
  isEnum,
  isEvidence,
  isIncidentBusinessImpact,
  isNonEmptyString,
  isNullableString,
  isRecord,
  isScope,
  isSourceStatus,
  isStringArray,
  statusReasons
} from "../../contracts/guards";

const evidenceSourceKinds: EvidenceSourceKind[] = [
  "alertmanager",
  "prometheus",
  "kubernetes",
  "cloud",
  "health_check",
  "fixture",
  "trivy",
  "falco",
  "kyverno",
  "opa_gatekeeper",
  "github",
  "gitlab",
  "argo_cd"
];
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
const widgetIds: WidgetId[] = [
  "health_summary",
  "incident_queue",
  "signal_summary",
  "change_stream",
  "environment_status"
];
const widgetSizes: WidgetSize[] = ["compact", "standard", "wide"];
const numberUnits = ["count", "percentage", "milliseconds", "seconds"] as const;
const changeKinds = ["deployment", "configuration", "maintenance", "connector"] as const;

const finiteDecimalPattern = /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/;

const isFiniteDecimal = (value: unknown): value is string =>
  typeof value === "string" &&
  finiteDecimalPattern.test(value.trim()) &&
  Number.isFinite(Number(value));

const sharesEvidence = (left: string[], right: string[]) => left.some((id) => right.includes(id));

/*
 * Console projections carry the same structured assessment as incident
 * writes: typed dimensions whose highest confirmed dimension must equal the
 * impact level, plus non-empty unique safe evidence references.
 */
const isBusinessImpact = isIncidentBusinessImpact;

const isCriticalNumber = (value: unknown): value is CriticalNumber => {
  if (!isRecord(value)) return false;
  return (
    isNonEmptyString(value.key) &&
    isFiniteDecimal(value.value) &&
    isEnum(value.unit, numberUnits) &&
    isStringArray(value.evidence_ids) &&
    isDrillDownTarget(value.drill_down) &&
    isDrillDownReference(value.drill_down_reference) &&
    sharesEvidence(value.evidence_ids, value.drill_down.evidence_ids) &&
    sharesEvidence(value.evidence_ids, value.drill_down_reference.evidence_ids)
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
  // Sprint 14 derives this stream from canonical change events, so the source
  // is a typed wire value rather than a free-form string.
  isEnum(value.source, evidenceSourceKinds) &&
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
  isNullableString(value.detail) &&
  (value.state === "available" ? value.reason === null : value.reason !== null);

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
  Number.isSafeInteger(value.default_order) &&
  value.default_order >= 0 &&
  value.default_order <= 65535 &&
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
  if (
    new Set(snapshot.incident_queue.map((item) => item.id)).size !==
      snapshot.incident_queue.length ||
    new Set(snapshot.changes.map((change) => change.id)).size !== snapshot.changes.length ||
    new Set(snapshot.environments.map((environment) => environment.environment_id)).size !==
      snapshot.environments.length ||
    new Set(snapshot.widget_registry.map((widget) => widget.id)).size !==
      snapshot.widget_registry.length
  ) {
    return false;
  }
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
