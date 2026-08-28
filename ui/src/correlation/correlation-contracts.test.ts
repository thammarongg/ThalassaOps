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
