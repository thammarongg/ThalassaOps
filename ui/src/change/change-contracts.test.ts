import { describe, expect, it } from "vitest";
import {
  changeKindWireValues,
  changeSnapshotFixture,
  changeSourceKinds,
  precedingChangeReason
} from "./change-fixtures";

describe("change intelligence IPC contract", () => {
  it("keeps the three source wire values and eight change kind values", () => {
    expect(changeSourceKinds).toEqual(["github", "gitlab", "argo_cd"]);
    expect(changeKindWireValues).toEqual([
      "deployment",
      "configuration",
      "maintenance",
      "connector",
      "code_commit",
      "code_merge",
      "sync",
      "rollback"
    ]);
  });

  it("uses the structural preceding-change reason", () => {
    expect(precedingChangeReason).toBe("preceding_change");
  });

  it("preserves optional change fields as null in the copied fixture", () => {
    const event = changeSnapshotFixture.events[0];
    expect(event.environment).toBeNull();
    expect(event.repository).toBeNull();
    expect(event.revision).toBeNull();
    expect(event.diff_stat).toBeNull();
    expect(event.source_link).toBeNull();
  });

  it("keeps every numeric contract value finite", () => {
    const numericValues = [
      changeSnapshotFixture.lookback_seconds,
      ...changeSnapshotFixture.events.flatMap((event) =>
        event.diff_stat
          ? [event.diff_stat.files_changed, event.diff_stat.insertions, event.diff_stat.deletions]
          : []
      ),
      ...changeSnapshotFixture.associations.map((association) => association.lead_time_seconds),
      ...changeSnapshotFixture.metrics.map((metric) => metric.value)
    ];

    expect(numericValues.every((value) => typeof value === "number" && Number.isFinite(value))).toBe(
      true
    );
  });
});
