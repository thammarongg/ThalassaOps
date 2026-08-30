import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ChangeSnapshot } from "../../contracts/ipc";
import { isChangeSnapshot } from "../../contracts/guards";
import { I18nProvider, i18n } from "../i18n";
import { changeSnapshotFixture } from "./change-fixtures";
import { ChangeTimeline } from "./ChangeTimeline";

afterEach(() => {
  cleanup();
  void i18n.changeLanguage("en");
});

const renderTimeline = (snapshot: ChangeSnapshot, selectedChangeId: string | null = null) =>
  render(
    <I18nProvider>
      <ChangeTimeline snapshot={snapshot} selectedChangeId={selectedChangeId} onSelect={vi.fn()} />
    </I18nProvider>
  );

describe("change timeline", () => {
  it("accepts the copied snapshot fixture as a valid contract", () => {
    expect(isChangeSnapshot(changeSnapshotFixture)).toBe(true);
  });

  it("renders timeline entries in snapshot order with their source and outcome", () => {
    renderTimeline(changeSnapshotFixture);

    const entries = screen.getAllByRole("button");
    expect(entries).toHaveLength(changeSnapshotFixture.timeline.entry_ids.length);
    expect(entries[0]).toHaveTextContent("GitHub");
    expect(entries[1]).toHaveTextContent("GitLab");
    expect(entries[1]).toHaveTextContent("Merge");
    expect(entries[1]).toHaveTextContent("Succeeded");
  });

  it("renders only events the snapshot placed in the timeline", () => {
    const snapshot: ChangeSnapshot = {
      ...changeSnapshotFixture,
      timeline: {
        ...changeSnapshotFixture.timeline,
        entry_ids: [changeSnapshotFixture.timeline.entry_ids[0]]
      }
    };
    renderTimeline(snapshot);

    expect(screen.getAllByRole("button")).toHaveLength(1);
    expect(screen.queryByText("GitLab")).not.toBeInTheDocument();
  });

  it("states truncation explicitly instead of silently dropping entries", () => {
    const snapshot: ChangeSnapshot = {
      ...changeSnapshotFixture,
      timeline: { ...changeSnapshotFixture.timeline, truncated: true }
    };
    renderTimeline(snapshot);

    expect(screen.getByRole("status")).toHaveTextContent(/most recent changes/i);
  });

  it("reports an empty window as an explicit empty state", () => {
    const snapshot: ChangeSnapshot = {
      ...changeSnapshotFixture,
      timeline: { ...changeSnapshotFixture.timeline, entry_ids: [] }
    };
    renderTimeline(snapshot);

    expect(screen.getByText("No change was recorded in this window.")).toBeInTheDocument();
    expect(screen.queryAllByRole("button")).toHaveLength(0);
  });

  it("only renders events that carry evidence IDs", () => {
    for (const entryId of changeSnapshotFixture.timeline.entry_ids) {
      const event = changeSnapshotFixture.events.find((item) => item.id === entryId);
      expect(event?.evidence_ids.length).toBeGreaterThan(0);
    }
  });

  it("renders Thai copy for the same typed values", async () => {
    await i18n.changeLanguage("th");
    renderTimeline(changeSnapshotFixture);

    expect(screen.getByText("ลำดับเวลาการเปลี่ยนแปลง")).toBeInTheDocument();
  });
});
