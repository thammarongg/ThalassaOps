import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type { CommandEnvelope, EvidenceRef, IpcResult, Invoke } from "../../contracts/ipc";
import { I18nProvider, i18n } from "../i18n";
import { correlationFixtureSnapshot } from "./correlation-fixtures";
import { CorrelationWorkspace } from "./CorrelationWorkspace";

afterEach(() => {
  cleanup();
  localStorage.clear();
  void i18n.changeLanguage("en");
});

type CorrelationInvokeMock = Invoke & {
  mock: { calls: [string, { envelope: CommandEnvelope<unknown> }][] };
};

const evidenceFor = (id: string): EvidenceRef => {
  const source = correlationFixtureSnapshot.evidence.find((item) => item.id === id);
  if (!source) throw new Error(`missing fixture evidence ${id}`);
  return source;
};

const correlationInvoke = (
  options: {
    snapshot?: typeof correlationFixtureSnapshot;
    evidenceResult?: (ids: string[]) => IpcResult<EvidenceRef[]>;
    snapshotResult?: IpcResult<typeof correlationFixtureSnapshot>;
  } = {}
) =>
  vi.fn().mockImplementation((name: string, args: { envelope: CommandEnvelope<unknown> }) => {
    if (name === "correlation_snapshot") {
      return Promise.resolve(
        options.snapshotResult ?? {
          ok: true,
          value: options.snapshot ?? correlationFixtureSnapshot
        }
      );
    }
    if (name === "correlation_evidence") {
      const ids = (args.envelope.payload as { evidence_ids: string[] }).evidence_ids;
      return Promise.resolve(
        options.evidenceResult?.(ids) ?? { ok: true, value: ids.map(evidenceFor) }
      );
    }
    return Promise.resolve({ ok: true, value: {} });
  }) as unknown as CorrelationInvokeMock;

const renderWorkspace = (invoke: CorrelationInvokeMock) =>
  render(
    <I18nProvider>
      <CorrelationWorkspace invoke={invoke} />
    </I18nProvider>
  );

it("sends an explicit workspace request and renders deterministic candidates", async () => {
  const invoke = correlationInvoke();
  renderWorkspace(invoke);

  expect(await screen.findByRole("heading", { name: "Signal correlation" })).toBeInTheDocument();
  expect(screen.getByText("candidate-checkout")).toBeInTheDocument();
  expect(screen.getByText("shared service")).toBeInTheDocument();

  const snapshotCall = invoke.mock.calls.find(([name]) => name === "correlation_snapshot");
  expect(snapshotCall?.[1].envelope).toMatchObject({
    command: "correlation.snapshot",
    capability: "WorkspaceRead",
    scope: { resource_ids: [] },
    payload: {
      window: {
        start: "2026-08-28T08:55:00Z",
        end: "2026-08-28T09:05:00Z"
      },
      evaluated_at: "2026-08-28T09:00:00Z",
      allowed_lateness_seconds: 300
    }
  });
});

it("expands a candidate to every contributing Signal and opens issued evidence only", async () => {
  const user = userEvent.setup();
  const invoke = correlationInvoke();
  renderWorkspace(invoke);

  const candidate = await screen.findByRole("button", { name: /candidate-checkout/i });
  await user.click(candidate);

  const details = await screen.findByRole("region", { name: "Candidate details" });
  expect(within(details).getByText(/alertmanager/i)).toBeInTheDocument();
  expect(within(details).getByText(/prometheus/i)).toBeInTheDocument();
  const evidenceButton = within(details).getByRole("button", { name: /evidence/i });
  await user.click(evidenceButton);

  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith(
      "correlation_evidence",
      expect.objectContaining({
        envelope: expect.objectContaining({
          command: "correlation.evidence",
          capability: "ResourceRead",
          payload: {
            evidence_ids: ["evidence-correlation-alert", "evidence-correlation-anomaly"]
          }
        })
      })
    )
  );
  expect(await screen.findAllByText("synthetic source record")).toHaveLength(2);
});

it("uses localized typed copy for IPC failures rather than rendering the backend message", async () => {
  const invoke = correlationInvoke({
    snapshotResult: {
      ok: false,
      error: { code: "POLICY_DENIED", message: "secret provider payload", details: {} }
    }
  });
  renderWorkspace(invoke);

  expect(await screen.findByRole("alert")).toHaveTextContent(
    "This correlation view is blocked by policy."
  );
  expect(screen.queryByText("secret provider payload")).not.toBeInTheDocument();
});

it("renders omitted summary metrics as unavailable instead of inventing zero", async () => {
  const snapshot = structuredClone(correlationFixtureSnapshot);
  snapshot.summary.metrics = [];
  const invoke = correlationInvoke({ snapshot });
  renderWorkspace(invoke);

  expect(
    await screen.findAllByText("Number unavailable; evidence could not be verified.")
  ).toHaveLength(4);
  expect(screen.queryByText(/^0$/)).not.toBeInTheDocument();
});

it("shows source status and suppression as text, independent of color", async () => {
  const snapshot = structuredClone(correlationFixtureSnapshot);
  snapshot.source_status = [
    {
      source_key: "falco",
      state: "unavailable",
      reason: "unreachable",
      detail: "do not render this backend detail",
      observed_at: null,
      evidence_ids: []
    }
  ];
  const invoke = correlationInvoke({ snapshot });
  renderWorkspace(invoke);

  expect(await screen.findByText("Falco is unavailable (unreachable).")).toBeInTheDocument();
  expect(screen.getByText(/Suppressed by maintenance window/)).toBeInTheDocument();
  expect(screen.queryByText("do not render this backend detail")).not.toBeInTheDocument();
});
