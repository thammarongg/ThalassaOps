import { describe, expect, it } from "vitest";
import type {
  CorrelationSnapshot,
  EvidenceSourceKind,
  Signal,
  SignalPayload
} from "../../contracts/ipc";
import {
  correlationFixtureSnapshot,
  correlationFixtureSourceKinds,
  SPRINT_13_FIXTURE_CLOCK
} from "./correlation-fixtures";
import { isCorrelationSnapshot } from "../../contracts/guards";
import en from "../locales/en";
import th from "../locales/th";

describe("Signal correlation IPC contracts", () => {
  it("keeps all four security source wire values stable", () => {
    const expected: EvidenceSourceKind[] = ["trivy", "falco", "kyverno", "opa_gatekeeper"];
    expect(correlationFixtureSourceKinds).toEqual(expected);
    expect(SPRINT_13_FIXTURE_CLOCK).toBe("2026-08-28T09:00:00Z");
  });

  it("keeps the copied fixture source-preserving and evidence closed", () => {
    const snapshot: CorrelationSnapshot = correlationFixtureSnapshot;
    expect(snapshot.signals).toHaveLength(6);
    expect(snapshot.signals.every((signal) => signal.source_record.content_digest !== "")).toBe(
      true
    );
    expect(snapshot.signals.every((signal) => signal.source_record.evidence_ids.length > 0)).toBe(
      true
    );
    expect(snapshot.candidates[0].signal_ids).toEqual(
      expect.arrayContaining(snapshot.signals.slice(0, 2).map((signal) => signal.id))
    );
  });

  it("accepts only evidence-closed correlation snapshots", () => {
    expect(isCorrelationSnapshot(correlationFixtureSnapshot)).toBe(true);
    const malformed = structuredClone(correlationFixtureSnapshot);
    malformed.candidates[0].evidence_ids = ["evidence-not-issued"];
    expect(isCorrelationSnapshot(malformed)).toBe(false);
  });

  it("rejects nil UUIDs in correlation signal identities", () => {
    const malformed = structuredClone(correlationFixtureSnapshot);
    malformed.signals[0].id = "00000000-0000-0000-0000-000000000000";
    expect(isCorrelationSnapshot(malformed)).toBe(false);
  });

  it("accepts backend-issued evidence filters on correlation drill-downs", () => {
    const snapshot = structuredClone(correlationFixtureSnapshot);
    snapshot.signals[0].drill_down.filter_key = snapshot.signals[0].source_record.content_digest;
    snapshot.candidates[0].drill_down.filter_key = snapshot.candidates[0].id;
    snapshot.summary.metrics[0].drill_down.filter_key = "metric:normalized_signals";
    expect(isCorrelationSnapshot(snapshot)).toBe(true);
  });

  it("accepts generated opaque keys without treating their digits as account IDs", () => {
    const snapshot = structuredClone(correlationFixtureSnapshot);
    snapshot.signals[0].source_record.content_digest = "sha256:123456789012";
    snapshot.signals[0].dedup_key = "dedup:v1:alert:alert:123456789012";
    snapshot.candidates[0].id = "candidate:v1:123456789012";
    expect(isCorrelationSnapshot(snapshot)).toBe(true);
  });

  it("rejects correlation snapshots that violate backend contract bounds or closure", () => {
    const wide = structuredClone(correlationFixtureSnapshot);
    wide.request.window.end = "2026-08-30T09:05:00Z";
    wide.window.range.end = "2026-08-30T09:05:00Z";
    expect(isCorrelationSnapshot(wide)).toBe(false);

    const badWatermark = structuredClone(correlationFixtureSnapshot);
    badWatermark.window.watermark = badWatermark.window.evaluated_at;
    expect(isCorrelationSnapshot(badWatermark)).toBe(false);

    const missingCandidateEvidence = structuredClone(correlationFixtureSnapshot);
    missingCandidateEvidence.candidates[0].evidence_ids = ["evidence-correlation-alert"];
    missingCandidateEvidence.candidates[0].drill_down.evidence_ids = ["evidence-correlation-alert"];
    missingCandidateEvidence.candidates[0].drill_down_reference.evidence_ids = [
      "evidence-correlation-alert"
    ];
    expect(isCorrelationSnapshot(missingCandidateEvidence)).toBe(false);

    const badMetricUnit = structuredClone(correlationFixtureSnapshot);
    badMetricUnit.summary.metrics[0].unit = "percentage";
    expect(isCorrelationSnapshot(badMetricUnit)).toBe(false);

    const badStatusEvidence = structuredClone(correlationFixtureSnapshot);
    badStatusEvidence.source_status = [
      {
        source_key: "trivy",
        state: "unverified",
        reason: "unknown",
        detail: null,
        observed_at: null,
        evidence_ids: ["evidence-not-issued"]
      }
    ];
    expect(isCorrelationSnapshot(badStatusEvidence)).toBe(false);

    const malformedTimestamp = structuredClone(correlationFixtureSnapshot);
    malformedTimestamp.signals[1].drill_down_reference.time_window!.start = "2026-08-28";
    expect(isCorrelationSnapshot(malformedTimestamp)).toBe(false);

    const invalidCalendarTimestamp = structuredClone(correlationFixtureSnapshot);
    invalidCalendarTimestamp.signals[1].drill_down_reference.time_window!.start =
      "2026-02-31T09:00:00Z";
    expect(isCorrelationSnapshot(invalidCalendarTimestamp)).toBe(false);

    const candidateReferenceOutsideEvidence = structuredClone(correlationFixtureSnapshot);
    candidateReferenceOutsideEvidence.candidates[0].drill_down_reference.evidence_ids = [
      "evidence-correlation-alert",
      "evidence-correlation-trivy"
    ];
    expect(isCorrelationSnapshot(candidateReferenceOutsideEvidence)).toBe(false);

    const candidateReasonOutsideEvidence = structuredClone(correlationFixtureSnapshot);
    candidateReasonOutsideEvidence.candidates[0].reasons[0].evidence_ids = [
      "evidence-correlation-alert",
      "evidence-correlation-anomaly",
      "evidence-correlation-trivy"
    ];
    expect(isCorrelationSnapshot(candidateReasonOutsideEvidence)).toBe(false);

    const mismatchedEvidenceSource = structuredClone(correlationFixtureSnapshot);
    mismatchedEvidenceSource.evidence[0].source_kind = "prometheus";
    expect(isCorrelationSnapshot(mismatchedEvidenceSource)).toBe(false);

    const mismatchedSuppression = structuredClone(correlationFixtureSnapshot);
    mismatchedSuppression.signals[0].suppression.kind = "rule";
    expect(isCorrelationSnapshot(mismatchedSuppression)).toBe(false);

    const mismatchedCandidateStatus = structuredClone(correlationFixtureSnapshot);
    mismatchedCandidateStatus.candidates[0].status = "suppressed";
    expect(isCorrelationSnapshot(mismatchedCandidateStatus)).toBe(false);

    const mismatchedReasonTarget = structuredClone(correlationFixtureSnapshot);
    mismatchedReasonTarget.candidates[0].grouping_targets[0] = {
      kind: "service",
      id: "service/other"
    };
    mismatchedReasonTarget.candidates[0].reasons[0].target = {
      kind: "service",
      id: "service/other"
    };
    expect(isCorrelationSnapshot(mismatchedReasonTarget)).toBe(false);

    const accountIdentity = structuredClone(correlationFixtureSnapshot);
    accountIdentity.signals[0].source_record.native_id = "123456789012";
    expect(isCorrelationSnapshot(accountIdentity)).toBe(false);
  });

  it("uses null for absent optional source values and number for numeric facts", () => {
    const signal: Signal = correlationFixtureSnapshot.signals[0];
    expect(signal.observed_at).toBeNull();
    expect(signal.ingested_at).toBeNull();
    expect(signal.source_record.native_id).toBeNull();
    expect(signal.source_record.revision).toBeNull();

    const payload: SignalPayload = correlationFixtureSnapshot.signals[1].payload;
    if (typeof payload === "object" && "anomaly" in payload) {
      expect(typeof payload.anomaly.observed_value).toBe("number");
      expect(Number.isFinite(payload.anomaly.observed_value)).toBe(true);
      expect(typeof payload.anomaly.comparison_value).toBe("number");
    } else {
      throw new Error("fixture must contain an anomaly payload");
    }
  });

  it("keeps suppression, maintenance and reason values typed", () => {
    expect(correlationFixtureSnapshot.signals[0].suppression.kind).toBe("maintenance_window");
    expect(correlationFixtureSnapshot.candidates[0].reasons[0].qualification).toBe(
      "exact_association"
    );
    expect(correlationFixtureSnapshot.summary.metrics[0].unit).toBe("count");
  });

  it("keeps the correlation catalog structurally identical in English and Thai", () => {
    const keyPaths = (value: unknown, prefix = ""): string[] =>
      Object.entries(value as Record<string, unknown>).flatMap(([key, inner]) =>
        typeof inner === "object" && inner !== null
          ? keyPaths(inner, `${prefix}${key}.`)
          : [`${prefix}${key}`]
      );
    expect(keyPaths(th.correlation).sort()).toEqual(keyPaths(en.correlation).sort());
  });
});
