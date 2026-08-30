// SPDX-License-Identifier: Apache-2.0

/** Runtime guards shared by every IPC contract consumer. */

import type {
  CandidateStatus,
  ChangeAssociation,
  ChangeEvent,
  ChangeKind,
  ChangeMetric,
  ChangeMetricKey,
  ChangeSnapshot,
  CorrelationCandidate,
  CorrelationMetric,
  CorrelationMetricKey,
  CorrelationReason,
  CorrelationReasonKind,
  CorrelationSnapshot,
  CorrelationWindowState,
  ConsoleEvidenceId,
  DrillDownDestination,
  DrillDownReference,
  DrillDownTarget,
  EvidenceRef,
  EvidenceSourceKind,
  FindingAssetKind,
  FindingSeverity,
  ResourceScope,
  Signal,
  SignalKind,
  SignalPayload,
  SignalState,
  SignalTarget,
  SignalTargetKind,
  NumberUnit,
  SourceRecordRef,
  SourceStatus,
  StatusReason,
  TimeWindow,
  VulnerabilityFinding
} from "./ipc";

type UnknownRecord = Record<string, unknown>;

export const isRecord = (value: unknown): value is UnknownRecord =>
  typeof value === "object" && value !== null && !Array.isArray(value);

export const isString = (value: unknown): value is string => typeof value === "string";

export const isNonEmptyString = (value: unknown): value is string =>
  isString(value) && value.trim() !== "";

export const isNullableString = (value: unknown): value is string | null =>
  value === null || isString(value);

export const isNullableNonEmptyString = (value: unknown): value is string | null =>
  value === null || isNonEmptyString(value);

export const isBoolean = (value: unknown): value is boolean => typeof value === "boolean";

export const isStringArray = (value: unknown): value is string[] =>
  Array.isArray(value) && value.every((item) => isString(item) && item.trim() !== "");

export const isEnum = <T extends string>(value: unknown, values: readonly T[]): value is T =>
  isString(value) && values.includes(value as T);

const evidenceSources: EvidenceSourceKind[] = [
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

export const changeSources: EvidenceSourceKind[] = ["github", "gitlab", "argo_cd"];
const securityEvidenceSources: EvidenceSourceKind[] = [
  "trivy",
  "falco",
  "kyverno",
  "opa_gatekeeper"
];

const sensitiveUrlMarkers = [
  "password",
  "passwd",
  "secret",
  "token",
  "credential",
  "credential_reference",
  "authorization",
  "bearer",
  "api_key",
  "access_key",
  "private_key",
  "account",
  "account_id",
  "account-id",
  "project",
  "project_id",
  "project-id",
  "subscription",
  "subscription_id",
  "subscription-id",
  "cursor",
  "arn:",
  "/subscriptions/",
  "projects/",
  "pagination_cursor",
  "next_link",
  "nextlink",
  "sk-live-"
] as const;

const containsSensitiveValue = (value: string) => {
  const lower = value.toLowerCase();
  const isUuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value);
  const isOpaqueGeneratedValue =
    lower.startsWith("sha256:") ||
    lower.startsWith("dedup:v1:") ||
    lower.startsWith("candidate:v1:");
  return (
    sensitiveUrlMarkers.some((marker) => lower.includes(marker)) ||
    (!isUuid && !isOpaqueGeneratedValue && /\d{12,}/.test(value))
  );
};

export const isSafeDisplayText = (value: unknown): value is string =>
  isNonEmptyString(value) &&
  ![...value].some((character) => {
    const code = character.charCodeAt(0);
    return code <= 0x1f || code === 0x7f;
  }) &&
  !containsSensitiveValue(value);

const isUuid = (value: unknown): value is string =>
  isString(value) &&
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value);

const isNonNilUuid = (value: unknown): value is string =>
  isUuid(value) && value.toLowerCase() !== "00000000-0000-0000-0000-000000000000";

export const isNullableSafeDisplayText = (value: unknown): value is string | null =>
  value === null || isSafeDisplayText(value);

export const isSafeStringArray = (value: unknown): value is string[] =>
  Array.isArray(value) && value.every(isSafeDisplayText);

const isSortedUniqueSafeStringArray = (value: unknown): value is string[] =>
  isSafeStringArray(value) && value.every((item, index, items) => index === 0 || items[index - 1] < item);

const isSortedUniqueUuidArray = (value: unknown): value is string[] =>
  Array.isArray(value) &&
  value.every(isNonNilUuid) &&
  value.every((item, index, items) => index === 0 || items[index - 1] < item);

const isSuppressionKindConsistent = (
  kind: unknown,
  ruleIds: unknown,
  maintenanceWindowIds: unknown
) => {
  if (
    !isEnum(kind, [
      "not_suppressed",
      "rule",
      "maintenance_window",
      "rule_and_maintenance_window"
    ]) ||
    !isSortedUniqueSafeStringArray(ruleIds) ||
    !isSortedUniqueSafeStringArray(maintenanceWindowIds)
  ) {
    return false;
  }
  const hasRules = ruleIds.length > 0;
  const hasMaintenanceWindows = maintenanceWindowIds.length > 0;
  const expected =
    hasRules && hasMaintenanceWindows
      ? "rule_and_maintenance_window"
      : hasRules
        ? "rule"
        : hasMaintenanceWindows
          ? "maintenance_window"
          : "not_suppressed";
  return kind === expected;
};

const destinations: DrillDownDestination[] = [
  "evidence",
  "incident_queue",
  "signal_summary",
  "change_stream",
  "environment_status",
  "topology"
];

export const statusReasons: StatusReason[] = [
  "not_configured",
  "unreachable",
  "timed_out",
  "policy_denied",
  "no_data_in_window",
  "unknown"
];

export const isTrustedNativeUrl = (value: unknown): value is string => {
  if (!isSafeDisplayText(value)) return false;
  try {
    const url = new URL(value);
    return url.protocol === "https:" && url.hostname !== "" && !url.username && !url.password;
  } catch {
    return false;
  }
};

export const isScope = (value: unknown): value is ResourceScope => {
  if (!isRecord(value) || !isStringArray(value.resource_ids)) return false;
  return ["organization_id", "team_id", "workspace_id", "environment_id"].every(
    (key) => value[key] === undefined || isNullableNonEmptyString(value[key])
  );
};

const isCorrelationScope = (value: unknown): value is ResourceScope => {
  if (!isScope(value)) return false;
  return (
    [value.organization_id, value.team_id, value.workspace_id, value.environment_id].every(
      (id) => id === undefined || id === null || isUuid(id)
    ) && value.resource_ids.every(isUuid)
  );
};

const scopeContains = (parent: ResourceScope, child: ResourceScope) =>
  (parent.organization_id === undefined ||
    parent.organization_id === null ||
    child.organization_id === parent.organization_id) &&
  (parent.team_id === undefined || parent.team_id === null || child.team_id === parent.team_id) &&
  (parent.workspace_id === undefined ||
    parent.workspace_id === null ||
    child.workspace_id === parent.workspace_id) &&
  (parent.environment_id === undefined ||
    parent.environment_id === null ||
    child.environment_id === parent.environment_id) &&
  (parent.resource_ids.length === 0 ||
    (child.resource_ids.length > 0 &&
      child.resource_ids.every((id) => parent.resource_ids.includes(id))));

const sameSignalTarget = (left: SignalTarget, right: SignalTarget) =>
  left.kind === right.kind && left.id === right.id;

const isTimestamp = (value: unknown): value is string => {
  if (!isSafeDisplayText(value)) return false;
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d+)?(Z|[+-](\d{2}):(\d{2}))$/.exec(
    value
  );
  if (!match) return false;
  const [, yearText, monthText, dayText, hourText, minuteText, secondText, offset, offsetHourText, offsetMinuteText] =
    match;
  const year = Number(yearText);
  const month = Number(monthText);
  const day = Number(dayText);
  const hour = Number(hourText);
  const minute = Number(minuteText);
  const second = Number(secondText);
  const offsetHour = offset === "Z" ? 0 : Number(offsetHourText);
  const offsetMinute = offset === "Z" ? 0 : Number(offsetMinuteText);
  const leapYear = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
  const daysInMonth = [31, leapYear ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  return (
    month >= 1 &&
    month <= 12 &&
    day >= 1 &&
    day <= daysInMonth[month - 1] &&
    hour <= 23 &&
    minute <= 59 &&
    second <= 59 &&
    offsetHour <= 23 &&
    offsetMinute <= 59 &&
    !Number.isNaN(Date.parse(value))
  );
};

export const isTimeWindow = (value: unknown): value is TimeWindow =>
  isRecord(value) &&
  isTimestamp(value.start) &&
  isTimestamp(value.end) &&
  Date.parse(value.start) < Date.parse(value.end);

export const isDrillDownTarget = (value: unknown): value is DrillDownTarget =>
  isRecord(value) &&
  isEnum(value.destination, destinations) &&
  isStringArray(value.evidence_ids) &&
  (value.filter_key === null || isString(value.filter_key));

export const isDrillDownReference = (value: unknown): value is DrillDownReference =>
  isRecord(value) &&
  isNonEmptyString(value.source_query) &&
  isScope(value.scope) &&
  (value.time_window === null || isTimeWindow(value.time_window)) &&
  isStringArray(value.evidence_ids);

export const isSourceStatus = (value: unknown): value is SourceStatus =>
  isRecord(value) &&
  isSafeDisplayText(value.source_key) &&
  isEnum(value.state, ["fresh", "stale", "unavailable", "unverified"]) &&
  (value.reason === null || isEnum(value.reason, statusReasons)) &&
  isNullableSafeDisplayText(value.detail) &&
  isNullableSafeDisplayText(value.observed_at) &&
  Array.isArray(value.evidence_ids) &&
  value.evidence_ids.every(isSafeDisplayText);

export const isEvidence = (value: unknown): value is EvidenceRef =>
  isRecord(value) &&
  isSafeDisplayText(value.id) &&
  isEnum(value.source_kind, evidenceSources) &&
  (value.connector_id === null || isSafeDisplayText(value.connector_id)) &&
  isScope(value.scope) &&
  isSafeDisplayText(value.endpoint) &&
  (value.query === null || isSafeDisplayText(value.query)) &&
  isSafeDisplayText(value.observed_at) &&
  isSafeDisplayText(value.excerpt) &&
  (value.native_url === null || isTrustedNativeUrl(value.native_url)) &&
  isRecord(value.redaction) &&
  isBoolean(value.redaction.classification_verified) &&
  isBoolean(value.redaction.redaction_verified) &&
  isBoolean(value.redaction.masked) &&
  isBoolean(value.redaction.unparsed) &&
  value.redaction.classification_verified &&
  value.redaction.redaction_verified &&
  (!value.redaction.unparsed || !value.redaction.masked);

/**
 * Evidence responses are all-or-nothing: every requested id must be present
 * exactly once, mirroring the backend's admitted-evidence contract.
 */
export const isEvidenceResponse = (
  value: unknown,
  expectedIds: ConsoleEvidenceId[]
): value is EvidenceRef[] => {
  if (!Array.isArray(value) || !value.every(isEvidence)) return false;
  const returnedIds = new Set(value.map((item) => item.id));
  const requestedIds = new Set(expectedIds);
  return (
    returnedIds.size === value.length &&
    requestedIds.size === expectedIds.length &&
    returnedIds.size === requestedIds.size &&
    expectedIds.every((id) => returnedIds.has(id))
  );
};

const signalKinds: SignalKind[] = ["alert", "anomaly", "security_finding", "health_check"];
const signalStates: SignalState[] = ["active", "cleared", "observed", "unknown"];
const signalTargetKinds: SignalTargetKind[] = ["resource", "service", "deployment", "topology"];
const findingAssetKinds: FindingAssetKind[] = [
  "container_image",
  "runtime_resource",
  "kubernetes_resource",
  "host",
  "policy_subject"
];
const findingSeverities: FindingSeverity[] = [
  "critical",
  "high",
  "medium",
  "low",
  "negligible",
  "unknown"
];
const correlationReasonKinds: CorrelationReasonKind[] = [
  "shared_resource",
  "shared_service",
  "shared_deployment",
  "topology_relation"
];
const correlationMetricKeys: CorrelationMetricKey[] = [
  "normalized_signals",
  "active_candidates",
  "suppressed_candidates",
  "uncorrelated_signals"
];
const correlationWindowStates: CorrelationWindowState[] = [
  "open",
  "ready_to_finalize",
  "finalized",
  "reopened"
];
const candidateStatuses: CandidateStatus[] = ["active", "provisional", "suppressed"];

const isFiniteNumber = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value);

const isFiniteNonNegativeInteger = (value: unknown): value is number =>
  typeof value === "number" && Number.isSafeInteger(value) && value >= 0;

const isSignalTarget = (value: unknown): value is SignalTarget =>
  isRecord(value) &&
  isEnum(value.kind, signalTargetKinds) &&
  isSafeDisplayText(value.id);

const isCorrelationRequest = (value: unknown) =>
  isRecord(value) &&
  isTimeWindow(value.window) &&
  isTimestamp(value.window.start) &&
  isTimestamp(value.window.end) &&
  Date.parse(value.window.start) < Date.parse(value.window.end) &&
  Date.parse(value.window.end) - Date.parse(value.window.start) <= 86_400_000 &&
  isTimestamp(value.evaluated_at) &&
  Date.parse(value.evaluated_at) >= Date.parse(value.window.start) &&
  isFiniteNonNegativeInteger(value.allowed_lateness_seconds) &&
  value.allowed_lateness_seconds <= 21_600;

const isCorrelationWindow = (value: unknown): value is CorrelationSnapshot["window"] =>
  isRecord(value) &&
  isTimeWindow(value.range) &&
  isTimestamp(value.range.start) &&
  isTimestamp(value.range.end) &&
  Date.parse(value.range.start) < Date.parse(value.range.end) &&
  Date.parse(value.range.end) - Date.parse(value.range.start) <= 86_400_000 &&
  isTimestamp(value.evaluated_at) &&
  isTimestamp(value.watermark) &&
  isFiniteNonNegativeInteger(value.allowed_lateness_seconds) &&
  value.allowed_lateness_seconds <= 21_600 &&
  isEnum(value.state, correlationWindowStates) &&
  Date.parse(value.watermark) ===
    Date.parse(value.evaluated_at) - value.allowed_lateness_seconds * 1_000 &&
  (value.state === "reopened"
    ? Date.parse(value.evaluated_at) >=
      Date.parse(value.range.end) + value.allowed_lateness_seconds * 1_000
    : value.state ===
      (Date.parse(value.evaluated_at) < Date.parse(value.range.end)
        ? "open"
        : Date.parse(value.evaluated_at) <
            Date.parse(value.range.end) + value.allowed_lateness_seconds * 1_000
          ? "ready_to_finalize"
          : "finalized"));

const isAnomalyCondition = (value: unknown): boolean => {
  if (!isRecord(value)) return false;
  if (isRecord(value.threshold)) {
    return (
      Object.keys(value).length === 1 &&
      isEnum(value.threshold.operator, ["gt", "gte", "lt", "lte"]) &&
      isSafeDisplayText(value.threshold.threshold)
    );
  }
  if (isRecord(value.rate_of_change)) {
    return (
      Object.keys(value).length === 1 &&
      isEnum(value.rate_of_change.direction, ["increase", "decrease", "absolute"]) &&
      isSafeDisplayText(value.rate_of_change.threshold_per_second) &&
      isFiniteNonNegativeInteger(value.rate_of_change.window_seconds)
    );
  }
  return false;
};

const isSourceRecord = (value: unknown): value is SourceRecordRef =>
  isRecord(value) &&
  isEnum(value.source_kind, evidenceSources) &&
  isNullableSafeDisplayText(value.native_id) &&
  isNullableSafeDisplayText(value.revision) &&
  isSafeDisplayText(value.content_digest) &&
  isSortedUniqueSafeStringArray(value.evidence_ids) &&
  value.evidence_ids.length > 0;

const isSecurityPayload = (value: unknown): boolean => {
  if (!isRecord(value) || !isRecord(value.security_finding)) return false;
  if (Object.keys(value).length !== 1) return false;
  const finding = value.security_finding.finding;
  if (!isRecord(finding) || !isEnum(finding.source, evidenceSources)) return false;
  if (!isRecord(finding.asset)) return false;
  return (
    isEnum(finding.asset.kind, findingAssetKinds) &&
    isSignalTarget(finding.asset.target) &&
    isNullableSafeDisplayText(finding.asset.display_name) &&
    isNullableSafeDisplayText(finding.asset.artifact_digest) &&
    (finding.severity === null || isEnum(finding.severity, findingSeverities)) &&
    (finding.exploitability === null ||
      isEnum(finding.exploitability, [
        "exploited",
        "known_exploit",
        "probable",
        "possible",
        "unlikely",
        "none",
        "unknown"
      ])) &&
    (finding.cvss_score === null ||
      (isFiniteNumber(finding.cvss_score) && finding.cvss_score >= 0 && finding.cvss_score <= 10)) &&
    isSortedUniqueSafeStringArray(finding.evidence_ids) &&
    finding.evidence_ids.length > 0
  );
};

const isSignalPayload = (value: unknown): value is SignalPayload => {
  if (value === "alert") return true;
  if (!isRecord(value)) return false;
  if (isRecord(value.anomaly) && Object.keys(value).length === 1) {
    return (
      isFiniteNumber(value.anomaly.observed_value) &&
      isFiniteNumber(value.anomaly.comparison_value) &&
      isAnomalyCondition(value.anomaly.condition)
    );
  }
  if (isSecurityPayload(value)) return true;
  if (isRecord(value.health_check) && Object.keys(value).length === 1) {
    return isEnum(value.health_check.outcome, [
      "healthy",
      "degraded",
      "unavailable",
      "timed_out",
      "skipped_not_due",
      "skipped_cooldown",
      "skipped_disabled"
    ]);
  }
  return false;
};

const signalPayloadKindMatches = (kind: SignalKind, payload: SignalPayload) => {
  if (kind === "alert") return payload === "alert";
  if (typeof payload !== "object") return false;
  if (kind === "anomaly") return "anomaly" in payload;
  if (kind === "security_finding") return "security_finding" in payload;
  return "health_check" in payload;
};

const isEvidenceDrillDownForCorrelation = (value: unknown, evidenceIds: string[]) =>
  isDrillDownTarget(value) &&
  value.destination === "evidence" &&
  (value.filter_key === null || isSafeDisplayText(value.filter_key)) &&
  isSortedUniqueSafeStringArray(value.evidence_ids) &&
  value.evidence_ids.length > 0 &&
  value.evidence_ids.every((id) => evidenceIds.includes(id));

const isCorrelationSignal = (value: unknown): value is Signal => {
  if (
    !isRecord(value) ||
    !isNonNilUuid(value.id) ||
    !isEnum(value.kind, signalKinds) ||
    !isEnum(value.source, evidenceSources) ||
    !isEnum(value.state, signalStates) ||
    (value.observed_at !== null && !isTimestamp(value.observed_at)) ||
    (value.ingested_at !== null && !isTimestamp(value.ingested_at)) ||
    !isCorrelationScope(value.scope) ||
    !Array.isArray(value.targets) ||
    !value.targets.every(isSignalTarget) ||
    (value.business_severity !== null &&
      !isEnum(value.business_severity, ["S1", "S2", "S3", "S4", "S5"])) ||
    !isSignalPayload(value.payload) ||
    !signalPayloadKindMatches(value.kind, value.payload) ||
    !isSourceRecord(value.source_record) ||
    value.source_record.source_kind !== value.source ||
    (value.dedup_key !== null && !isSafeDisplayText(value.dedup_key)) ||
    !isRecord(value.suppression) ||
    !isEnum(value.suppression.kind, [
      "not_suppressed",
      "rule",
      "maintenance_window",
      "rule_and_maintenance_window"
    ]) ||
    !isSortedUniqueSafeStringArray(value.suppression.rule_ids) ||
    !isSortedUniqueSafeStringArray(value.suppression.maintenance_window_ids) ||
    !isSuppressionKindConsistent(
      value.suppression.kind,
      value.suppression.rule_ids,
      value.suppression.maintenance_window_ids
    ) ||
    !isTimestamp(value.suppression.evaluated_at) ||
    !isFiniteNonNegativeInteger(value.suppression.policy_version) ||
    !isSortedUniqueSafeStringArray(value.evidence_ids) ||
    value.evidence_ids.length === 0 ||
    new Set(value.evidence_ids).size !== value.evidence_ids.length ||
    !value.source_record.evidence_ids.every((id) =>
      (value.evidence_ids as string[]).includes(id)
    ) ||
    !isDrillDownTarget(value.drill_down) ||
    !isEvidenceDrillDownForCorrelation(value.drill_down, value.evidence_ids) ||
    !isDrillDownReference(value.drill_down_reference) ||
    !isSortedUniqueSafeStringArray(value.drill_down_reference.evidence_ids) ||
    value.drill_down_reference.evidence_ids.length === 0 ||
    !value.drill_down_reference.evidence_ids.every((id) =>
      (value.evidence_ids as string[]).includes(id)
    ) ||
    !isCorrelationScope(value.drill_down_reference.scope)
  ) {
    return false;
  }

  if (value.kind === "security_finding") {
    const finding = (
      value.payload as { security_finding: { finding: VulnerabilityFinding } }
    ).security_finding.finding;
    if (
      typeof value.payload !== "object" ||
      !("security_finding" in value.payload) ||
      !securityEvidenceSources.includes(finding.source) ||
      finding.source !== value.source ||
      !value.targets.some(
        (target) =>
          target.kind === finding.asset.target.kind && target.id === finding.asset.target.id
      ) ||
      !finding.evidence_ids.every((id) => (value.evidence_ids as string[]).includes(id))
    ) {
      return false;
    }
  }
  return true;
};

const isCorrelationReason = (value: unknown): value is CorrelationReason => {
  if (
    !isRecord(value) ||
    !isEnum(value.kind, correlationReasonKinds) ||
    !isEnum(value.qualification, ["exact_association", "probable_structural"]) ||
    !isSortedUniqueUuidArray(value.signal_ids) ||
    value.signal_ids.length < 2 ||
    (value.target !== null && !isSignalTarget(value.target)) ||
    !isSortedUniqueSafeStringArray(value.topology_path_ids) ||
    !isSortedUniqueSafeStringArray(value.evidence_ids) ||
    value.evidence_ids.length === 0
  ) {
    return false;
  }
  if (value.kind === "topology_relation") {
    return (
      value.target === null &&
      value.topology_path_ids.length > 0 &&
      value.qualification === "probable_structural"
    );
  }
  const targetKind =
    value.kind === "shared_resource"
      ? "resource"
      : value.kind === "shared_service"
        ? "service"
        : "deployment";
  return (
    value.target !== null &&
    value.target.kind === targetKind &&
    value.topology_path_ids.length === 0 &&
    value.qualification === "exact_association"
  );
};

const isCorrelationMetric = (value: unknown): value is CorrelationMetric =>
  isRecord(value) &&
  isEnum(value.key, correlationMetricKeys) &&
  isFiniteNumber(value.value) &&
  value.value >= 0 &&
  value.unit === "count" &&
  isSortedUniqueSafeStringArray(value.evidence_ids) &&
  value.evidence_ids.length > 0 &&
  isEvidenceDrillDownForCorrelation(value.drill_down, value.evidence_ids) &&
  isDrillDownReference(value.drill_down_reference) &&
  isCorrelationScope(value.drill_down_reference.scope) &&
  isSortedUniqueSafeStringArray(value.drill_down_reference.evidence_ids) &&
  value.drill_down_reference.evidence_ids.length > 0 &&
  value.drill_down_reference.evidence_ids.every((id) =>
    (value.evidence_ids as string[]).includes(id)
  );

const isCorrelationCandidate = (value: unknown): value is CorrelationCandidate =>
  isRecord(value) &&
  isSafeDisplayText(value.id) &&
  isCorrelationScope(value.scope) &&
  isCorrelationWindow(value.window) &&
  isSortedUniqueUuidArray(value.signal_ids) &&
  value.signal_ids.length >= 2 &&
  Array.isArray(value.grouping_targets) &&
  value.grouping_targets.every(isSignalTarget) &&
  Array.isArray(value.reasons) &&
  value.reasons.length > 0 &&
  value.reasons.every(isCorrelationReason) &&
  isEnum(value.status, candidateStatuses) &&
  isSortedUniqueUuidArray(value.late_signal_ids) &&
  isSortedUniqueSafeStringArray(value.evidence_ids) &&
  value.evidence_ids.length > 0 &&
  isEvidenceDrillDownForCorrelation(value.drill_down, value.evidence_ids) &&
  isDrillDownReference(value.drill_down_reference) &&
  isCorrelationScope(value.drill_down_reference.scope) &&
  isSortedUniqueSafeStringArray(value.drill_down_reference.evidence_ids) &&
  value.drill_down_reference.evidence_ids.length > 0 &&
  value.drill_down_reference.evidence_ids.every((id) =>
    (value.evidence_ids as string[]).includes(id)
  );

const sameWindow = (
  left: CorrelationSnapshot["window"],
  right: CorrelationSnapshot["window"]
) =>
  left.range.start === right.range.start &&
  left.range.end === right.range.end &&
  left.evaluated_at === right.evaluated_at &&
  left.watermark === right.watermark &&
  left.allowed_lateness_seconds === right.allowed_lateness_seconds &&
  left.state === right.state;

/**
 * Runtime guard for the source-preserving `correlation.snapshot` response.
 * Every reference is closed over the issued evidence set before React uses
 * the projection; malformed or partial snapshots are rejected as a whole.
 */
export const isCorrelationSnapshot = (value: unknown): value is CorrelationSnapshot => {
  if (
    !isRecord(value) ||
    !isTimestamp(value.generated_at) ||
    !isCorrelationScope(value.scope) ||
    !isCorrelationRequest(value.request) ||
    !isCorrelationWindow(value.window) ||
    !isRecord(value.summary) ||
    !Array.isArray(value.summary.metrics) ||
    !value.summary.metrics.every(isCorrelationMetric) ||
    !Array.isArray(value.signals) ||
    !value.signals.every(isCorrelationSignal) ||
    !Array.isArray(value.candidates) ||
    !value.candidates.every(isCorrelationCandidate) ||
    !Array.isArray(value.topology_paths) ||
    !Array.isArray(value.source_status) ||
    !value.source_status.every(isSourceStatus) ||
    !Array.isArray(value.evidence) ||
    !value.evidence.every(isEvidence)
  ) {
    return false;
  }

  const snapshot = value as CorrelationSnapshot;
  if (
    snapshot.request.window.start !== snapshot.window.range.start ||
    snapshot.request.window.end !== snapshot.window.range.end ||
    snapshot.request.evaluated_at !== snapshot.window.evaluated_at ||
    snapshot.request.allowed_lateness_seconds !== snapshot.window.allowed_lateness_seconds ||
    new Set(snapshot.evidence.map((item) => item.id)).size !== snapshot.evidence.length ||
    new Set(snapshot.signals.map((signal) => signal.id)).size !== snapshot.signals.length ||
    new Set(snapshot.candidates.map((candidate) => candidate.id)).size !==
      snapshot.candidates.length ||
    new Set(snapshot.source_status.map((status) => status.source_key)).size !==
      snapshot.source_status.length ||
    new Set(snapshot.summary.metrics.map((metric) => metric.key)).size !==
      snapshot.summary.metrics.length
  ) {
    return false;
  }

  const evidenceIds = new Set(snapshot.evidence.map((item) => item.id));
  const signalIds = new Set(snapshot.signals.map((signal) => signal.id));
  const evidenceById = new Map(snapshot.evidence.map((item) => [item.id, item]));
  if (
    snapshot.evidence.some(
      (item) => !isCorrelationScope(item.scope) || !scopeContains(snapshot.scope, item.scope)
    )
  ) {
    return false;
  }
  const pathIds = new Set<string>();
  for (const path of snapshot.topology_paths) {
    if (
      !isRecord(path) ||
      !isSafeDisplayText(path.id) ||
      !isSafeDisplayText(path.root_node_id) ||
      !isSafeDisplayText(path.terminal_node_id) ||
      !isSafeStringArray(path.node_ids) ||
      !isSafeStringArray(path.edge_ids) ||
      !isEnum(path.direction, ["upstream", "downstream", "both"]) ||
      !isFiniteNonNegativeInteger(path.depth) ||
      !isFiniteNumber(path.confidence) ||
      path.confidence < 0 ||
      path.confidence > 1 ||
      path.kind !== "probable_structural" ||
      !isEnum(path.termination, ["leaf", "cycle_detected", "depth_limit"]) ||
      (path.cycle_edge_id !== null && !isSafeDisplayText(path.cycle_edge_id)) ||
      !isSortedUniqueSafeStringArray(path.evidence_ids) ||
      path.evidence_ids.length === 0 ||
      !isEvidenceDrillDownForCorrelation(path.drill_down, path.evidence_ids) ||
      path.evidence_ids.some((id) => !evidenceIds.has(id))
    ) {
      return false;
    }
    if (pathIds.has(path.id)) return false;
    pathIds.add(path.id);
  }

  if (
    snapshot.summary.metrics.some(
      (metric) =>
        metric.evidence_ids.some((id) => !evidenceIds.has(id)) ||
        metric.drill_down.evidence_ids.some((id) => !evidenceIds.has(id)) ||
        metric.drill_down_reference.evidence_ids.some((id) => !evidenceIds.has(id)) ||
        !scopeContains(snapshot.scope, metric.drill_down_reference.scope)
    )
  ) {
    return false;
  }

  if (snapshot.source_status.some((status) => status.evidence_ids.some((id) => !evidenceIds.has(id)))) {
    return false;
  }

  for (const signal of snapshot.signals) {
    if (
      signal.evidence_ids.some((id) => !evidenceIds.has(id)) ||
      signal.drill_down.evidence_ids.some((id) => !evidenceIds.has(id)) ||
      signal.drill_down_reference.evidence_ids.some((id) => !evidenceIds.has(id)) ||
      signal.source_record.evidence_ids.some((id) => !evidenceIds.has(id)) ||
      !scopeContains(snapshot.scope, signal.scope) ||
      !scopeContains(signal.scope, signal.drill_down_reference.scope) ||
      (typeof signal.payload === "object" &&
        "security_finding" in signal.payload &&
        signal.payload.security_finding.finding.evidence_ids.some((id) => !evidenceIds.has(id)))
      ||
      [...new Set([...signal.evidence_ids, ...signal.source_record.evidence_ids])].some((id) => {
        const evidence = evidenceById.get(id);
        return (
          !evidence ||
          evidence.source_kind !== signal.source ||
          !scopeContains(signal.scope, evidence.scope)
        );
      })
    ) {
      return false;
    }
  }

  for (const candidate of snapshot.candidates) {
    const candidateSignalIds = new Set(candidate.signal_ids);
    const explainedSignalIds = new Set<string>();
    const memberSignals = candidate.signal_ids.map((id) =>
      snapshot.signals.find((signal) => signal.id === id)
    );
    const allSuppressed =
      memberSignals.length === candidate.signal_ids.length &&
      memberSignals.every(
        (signal) => signal !== undefined && signal.suppression.kind !== "not_suppressed"
      );
    const expectedStatus = allSuppressed
      ? "suppressed"
      : candidate.late_signal_ids.length > 0 || snapshot.window.state === "reopened"
        ? "provisional"
        : "active";
    if (
      candidate.scope.workspace_id !== snapshot.scope.workspace_id ||
      candidate.scope.team_id !== snapshot.scope.team_id ||
      candidate.scope.organization_id !== snapshot.scope.organization_id ||
      candidate.scope.environment_id !== snapshot.scope.environment_id ||
      candidate.scope.resource_ids.length !== snapshot.scope.resource_ids.length ||
      candidate.scope.resource_ids.some((id) => !snapshot.scope.resource_ids.includes(id)) ||
      !sameWindow(candidate.window, snapshot.window) ||
      candidate.signal_ids.some((id) => !signalIds.has(id)) ||
      candidate.late_signal_ids.some((id) => !signalIds.has(id)) ||
      candidate.evidence_ids.some((id) => !evidenceIds.has(id)) ||
      candidate.drill_down.evidence_ids.some((id) => !evidenceIds.has(id)) ||
      candidate.drill_down_reference.evidence_ids.some((id) => !evidenceIds.has(id)) ||
      !scopeContains(candidate.scope, candidate.drill_down_reference.scope) ||
      candidate.status !== expectedStatus ||
      candidate.grouping_targets.some(
        (target) =>
          !candidate.reasons.some(
            (reason) => reason.target !== null && sameSignalTarget(reason.target, target)
          )
      ) ||
      candidate.signal_ids.some((id) => {
        const signal = snapshot.signals.find((item) => item.id === id);
        return !signal || signal.evidence_ids.some((evidenceId) => !candidate.evidence_ids.includes(evidenceId));
      }) ||
      candidate.reasons.some(
        (reason) =>
          reason.signal_ids.some((id) => !candidateSignalIds.has(id)) ||
          reason.topology_path_ids.some((id) => !pathIds.has(id)) ||
          reason.evidence_ids.some((id) => !evidenceIds.has(id)) ||
          reason.evidence_ids.some((id) => !candidate.evidence_ids.includes(id)) ||
          reason.signal_ids.some((id) => {
            const signal = snapshot.signals.find((item) => item.id === id);
            return !signal || signal.evidence_ids.some((evidenceId) => !reason.evidence_ids.includes(evidenceId));
          }) ||
          (reason.target !== null &&
            ( !candidate.grouping_targets.some((target) => sameSignalTarget(target, reason.target!)) ||
              reason.signal_ids.some((id) => {
                const signal = snapshot.signals.find((item) => item.id === id);
                return !signal || !signal.targets.some((target) => sameSignalTarget(target, reason.target!));
              }) )) ||
          (reason.target === null &&
            reason.signal_ids.some((id) => {
              const signal = snapshot.signals.find((item) => item.id === id);
              return (
                !signal ||
                !reason.topology_path_ids.some((pathId) => {
                  const path = snapshot.topology_paths.find((item) => item.id === pathId);
                  return (
                    path !== undefined &&
                    signal.targets.some((target) => path.node_ids.includes(target.id))
                  );
                })
              );
            })) ||
          reason.topology_path_ids.some((id) => {
            const path = snapshot.topology_paths.find((item) => item.id === id);
            return !path || path.evidence_ids.some((evidenceId) => !reason.evidence_ids.includes(evidenceId));
          })
      )
    ) {
      return false;
    }
    for (const reason of candidate.reasons) {
      reason.signal_ids.forEach((id) => explainedSignalIds.add(id));
    }
    if (
      explainedSignalIds.size !== candidateSignalIds.size ||
      [...candidateSignalIds].some((id) => !explainedSignalIds.has(id))
    ) {
      return false;
    }
  }

  return true;
};

const changeKinds: ChangeKind[] = [
  "deployment",
  "configuration",
  "maintenance",
  "connector",
  "code_commit",
  "code_merge",
  "sync",
  "rollback"
];

const changeMetricKeys: ChangeMetricKey[] = [
  "changes_in_window",
  "associated_changes",
  "changes_by_source"
];

const numberUnits: NumberUnit[] = ["count", "percentage", "milliseconds", "seconds"];

const isChangeActor = (value: unknown): boolean =>
  isRecord(value) &&
  isEnum(value.kind, ["human", "automation", "unknown"]) &&
  isNullableSafeDisplayText(value.handle);

const isChangeDiffStat = (value: unknown): boolean =>
  value === null ||
  (isRecord(value) &&
    isFiniteNonNegativeInteger(value.files_changed) &&
    isFiniteNonNegativeInteger(value.insertions) &&
    isFiniteNonNegativeInteger(value.deletions) &&
    value.unit === "count");

const isChangeRevision = (value: unknown): boolean =>
  value === null ||
  (isRecord(value) &&
    isSafeDisplayText(value.id) &&
    isNullableSafeDisplayText(value.short_id) &&
    isSafeStringArray(value.parent_ids));

const isChangeRepository = (value: unknown): boolean =>
  value === null ||
  (isRecord(value) &&
    isSafeDisplayText(value.host) &&
    isNullableSafeDisplayText(value.namespace) &&
    isSafeDisplayText(value.name) &&
    isNullableSafeDisplayText(value.reference));

const isChangeSourceLink = (value: unknown): boolean =>
  value === null ||
  (isRecord(value) &&
    isEnum(value.kind, ["commit", "pull_request", "compare", "deployment", "application"]) &&
    isTrustedNativeUrl(value.url));

const isChangeEvent = (value: unknown): value is ChangeEvent =>
  isRecord(value) &&
  isNonNilUuid(value.id) &&
  isEnum(value.source, changeSources) &&
  isEnum(value.kind, changeKinds) &&
  isEnum(value.outcome, ["succeeded", "failed", "in_progress", "reverted", "unknown"]) &&
  isTimestamp(value.occurred_at) &&
  (value.ingested_at === null || isTimestamp(value.ingested_at)) &&
  isScope(value.scope) &&
  Array.isArray(value.targets) &&
  value.targets.every(isSignalTarget) &&
  isChangeRevision(value.revision) &&
  isChangeActor(value.actor) &&
  isChangeRepository(value.repository) &&
  isNullableSafeDisplayText(value.environment) &&
  isChangeDiffStat(value.diff_stat) &&
  isSafeStringArray(value.changed_paths) &&
  isChangeSourceLink(value.source_link) &&
  isSourceRecord(value.source_record) &&
  isStringArray(value.evidence_ids) &&
  value.evidence_ids.length > 0 &&
  isDrillDownTarget(value.drill_down) &&
  isDrillDownReference(value.drill_down_reference);

const isChangeAssociation = (value: unknown): value is ChangeAssociation =>
  isRecord(value) &&
  isNonNilUuid(value.change_id) &&
  isSafeDisplayText(value.candidate_id) &&
  isEnum(value.qualification, ["exact_association", "probable_structural"]) &&
  isFiniteNumber(value.lead_time_seconds) &&
  value.lead_time_seconds >= 0 &&
  (value.target === null || isSignalTarget(value.target)) &&
  isSafeStringArray(value.topology_path_ids) &&
  isStringArray(value.evidence_ids) &&
  value.evidence_ids.length > 0;

const isChangeMetric = (value: unknown): value is ChangeMetric =>
  isRecord(value) &&
  isEnum(value.key, changeMetricKeys) &&
  (value.source === null || isEnum(value.source, changeSources)) &&
  (value.key === "changes_by_source") === (value.source !== null) &&
  isFiniteNumber(value.value) &&
  value.value >= 0 &&
  isEnum(value.unit, numberUnits) &&
  value.unit === "count" &&
  isStringArray(value.evidence_ids) &&
  value.evidence_ids.length > 0 &&
  isDrillDownTarget(value.drill_down) &&
  isDrillDownReference(value.drill_down_reference);

/**
 * Runtime guard for the read-only `change.snapshot` response.
 *
 * The snapshot is accepted only as a whole: the timeline must reference known
 * events in `(occurred_at, id)` order, every association must name an event
 * inside the snapshot, and every evidence reference must resolve against the
 * retained source records. A change the backend could not fully attribute is
 * rejected rather than rendered without its source.
 */
export const isChangeSnapshot = (value: unknown): value is ChangeSnapshot => {
  if (
    !isRecord(value) ||
    !isTimestamp(value.generated_at) ||
    !isScope(value.scope) ||
    !isTimeWindow(value.request_window) ||
    !isFiniteNonNegativeInteger(value.lookback_seconds) ||
    value.lookback_seconds > 86_400 ||
    !Array.isArray(value.events) ||
    !value.events.every(isChangeEvent) ||
    !isRecord(value.timeline) ||
    !isTimeWindow(value.timeline.window) ||
    !Array.isArray(value.timeline.entry_ids) ||
    !isBoolean(value.timeline.truncated) ||
    !Array.isArray(value.associations) ||
    !value.associations.every(isChangeAssociation) ||
    !Array.isArray(value.metrics) ||
    !value.metrics.every(isChangeMetric) ||
    !Array.isArray(value.source_statuses) ||
    !value.source_statuses.every(isSourceStatus)
  ) {
    return false;
  }

  const snapshot = value as ChangeSnapshot;
  if (
    snapshot.timeline.window.start !== snapshot.request_window.start ||
    snapshot.timeline.window.end !== snapshot.request_window.end ||
    new Set(snapshot.events.map((event) => event.id)).size !== snapshot.events.length ||
    new Set(snapshot.timeline.entry_ids).size !== snapshot.timeline.entry_ids.length ||
    new Set(snapshot.source_statuses.map((status) => status.source_key)).size !==
      snapshot.source_statuses.length
  ) {
    return false;
  }

  const eventById = new Map(snapshot.events.map((event) => [event.id, event]));
  const knownEvidenceIds = new Set(
    snapshot.events.flatMap((event) => event.source_record.evidence_ids)
  );
  const closesOverEvidence = (ids: readonly string[]) =>
    ids.every((id) => knownEvidenceIds.has(id));

  let previousKey: [number, string] | null = null;
  for (const entryId of snapshot.timeline.entry_ids) {
    const event = eventById.get(entryId);
    if (!event) return false;
    const occurredAt = Date.parse(event.occurred_at);
    const windowStart = Date.parse(snapshot.timeline.window.start);
    const windowEnd = Date.parse(snapshot.timeline.window.end);
    if (occurredAt < windowStart || occurredAt >= windowEnd) return false;
    const key: [number, string] = [occurredAt, event.id];
    if (
      previousKey &&
      (previousKey[0] > key[0] || (previousKey[0] === key[0] && previousKey[1] > key[1]))
    ) {
      return false;
    }
    previousKey = key;
  }

  for (const event of snapshot.events) {
    if (
      !scopeContains(snapshot.scope, event.scope) ||
      !closesOverEvidence(event.evidence_ids) ||
      !closesOverEvidence(event.drill_down.evidence_ids) ||
      !closesOverEvidence(event.drill_down_reference.evidence_ids)
    ) {
      return false;
    }
  }

  const associationKeys = new Set<string>();
  for (const association of snapshot.associations) {
    const key = association.candidate_id + "|" + association.change_id;
    if (associationKeys.has(key)) return false;
    associationKeys.add(key);
    if (
      !eventById.has(association.change_id) ||
      association.lead_time_seconds > snapshot.lookback_seconds ||
      !closesOverEvidence(association.evidence_ids)
    ) {
      return false;
    }
  }

  const metricIdentities = new Set<string>();
  for (const metric of snapshot.metrics) {
    const identity = metric.key + "|" + (metric.source ?? "");
    if (metricIdentities.has(identity)) return false;
    metricIdentities.add(identity);
    if (
      !closesOverEvidence(metric.evidence_ids) ||
      !closesOverEvidence(metric.drill_down.evidence_ids) ||
      !closesOverEvidence(metric.drill_down_reference.evidence_ids)
    ) {
      return false;
    }
  }

  return snapshot.source_statuses.every((status) => closesOverEvidence(status.evidence_ids));
};
