import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type { Invoke } from "../../contracts/ipc";
import { evidenceIds, operationsSnapshotFixture } from "../dev/operations-fixture";
import { I18nProvider, i18n } from "../i18n";
import { OperationsConsole } from "../OperationsConsole";

vi.mock("@tauri-apps/plugin-shell", () => ({ open: vi.fn().mockResolvedValue(undefined) }));

afterEach(() => {
  cleanup();
  localStorage.clear();
  void i18n.changeLanguage("en");
});

it("lets an operator identify attention and open evidence without provider or mutation calls", async () => {
  const user = userEvent.setup();
  const snapshot = operationsSnapshotFixture();
  const invoke = vi
    .fn()
    .mockImplementation(
      (name: string, args: { envelope: { payload: { evidence_ids: string[] } } }) => {
        if (name === "operations_snapshot") return Promise.resolve({ ok: true, value: snapshot });
        if (name === "operations_evidence") {
          const ids = args.envelope.payload.evidence_ids;
          return Promise.resolve({
            ok: true,
            value: snapshot.evidence.filter((item) => ids.includes(item.id))
          });
        }
        throw new Error(`unexpected command: ${name}`);
      }
    );

  const { container } = render(
    <I18nProvider>
      <OperationsConsole invoke={invoke as Invoke} />
    </I18nProvider>
  );

  const headline = await screen.findByRole("heading", { name: "Checkout is affecting customers" });
  expect(headline).toBeInTheDocument();
  expect(screen.getByText("Checkout API failing")).toBeInTheDocument();
  expect(screen.getByText("AWS production")).toBeInTheDocument();
  expect(screen.getByText("GCP staging")).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Open evidence for active alerts (1)" })
  ).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Open evidence for active anomalies (1)" })
  ).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Open evidence for checks due (2)" })
  ).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Open evidence for timed out (1)" })
  ).toBeInTheDocument();

  const widgets = [...container.querySelectorAll<HTMLElement>("[data-widget-id]")];
  expect(widgets.slice(0, 2).map((widget) => widget.dataset.widgetId)).toEqual([
    "health_summary",
    "incident_queue"
  ]);
  expect(within(widgets[0]).getByRole("heading", { name: "Health summary" })).toBeInTheDocument();
  expect(
    within(widgets[1]).getByRole("heading", { name: "Active incident queue" })
  ).toBeInTheDocument();

  const numberButtons = [
    ...container.querySelectorAll<HTMLButtonElement>(
      '[data-testid="operations-critical-number"] button'
    )
  ];
  expect(numberButtons.length).toBeGreaterThan(10);
  for (const button of numberButtons) await user.click(button);

  const evidenceCalls = invoke.mock.calls.filter(([name]) => name === "operations_evidence");
  expect(evidenceCalls).toHaveLength(numberButtons.length);
  for (const [, args] of evidenceCalls) {
    expect(args.envelope.capability).toBe("ResourceRead");
    expect(args.envelope.payload.evidence_ids).toEqual(evidenceIds);
  }
  expect(invoke.mock.calls.map(([name]) => name)).toEqual([
    "operations_snapshot",
    ...Array.from({ length: numberButtons.length }, () => "operations_evidence")
  ]);
  expect(await screen.findByText(/Sensitive fields masked/)).toBeInTheDocument();
  expect(screen.getByText(/Unparsed source/)).toBeInTheDocument();
  expect(screen.getAllByText("fixture://operations")).not.toHaveLength(0);
  expect(screen.getAllByText("operations:snapshot")).not.toHaveLength(0);
});
