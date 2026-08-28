// SPDX-License-Identifier: Apache-2.0

/** Runtime guards shared by every IPC contract consumer. */

import type {
  ConsoleEvidenceId,
  DrillDownDestination,
  DrillDownReference,
  DrillDownTarget,
  EvidenceRef,
  EvidenceSourceKind,
  ResourceScope,
  SourceStatus,
  StatusReason,
  TimeWindow
} from "./ipc";

type UnknownRecord = Record<string, unknown>;

export const isRecord = (value: unknown): value is UnknownRecord =>
  typeof value === "object" && value !== null && !Array.isArray(value);

export const isString = (value: unknown): value is string => typeof value === "string";

export const isNonEmptyString = (value: unknown): value is string =>
  isString(value) && value.trim() !== "";

export const isNullableString = (value: unknown): value is string | null =>
  value === null || isString(value);

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
  "fixture"
];

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
  if (!isNonEmptyString(value)) return false;
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
    (key) => value[key] === undefined || value[key] === null || isString(value[key])
  );
};

export const isTimeWindow = (value: unknown): value is TimeWindow =>
  isRecord(value) && isNonEmptyString(value.start) && isNonEmptyString(value.end);

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
