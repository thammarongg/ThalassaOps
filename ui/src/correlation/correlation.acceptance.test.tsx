import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type { CommandEnvelope, EvidenceRef, Invoke } from "../../contracts/ipc";
import { I18nProvider, i18n } from "../i18n";
import { correlationFixtureSnapshot } from "./correlation-fixtures";
import { CorrelationWorkspace } from "./CorrelationWorkspace";

afterEach(() => {
  cleanup();
  localStorage.clear();
  void i18n.changeLanguage("en");
});

it("keeps mixed operational and security signals evidence reachable without an Incident route", async () => {
  const invoke = vi
    .fn()
    .mockImplementation((name: string, args: { envelope: CommandEnvelope<unknown> }) => {
      if (name === "correlation_snapshot") {
        return Promise.resolve({ ok: true, value: correlationFixtureSnapshot });
      }
      if (name === "correlation_evidence") {
        const ids = (args.envelope.payload as { evidence_ids: string[] }).evidence_ids;
        const evidence = correlationFixtureSnapshot.evidence.filter((item) =>
          ids.includes(item.id)
        );
        return Promise.resolve({ ok: true, value: evidence as EvidenceRef[] });
      }
      return Promise.resolve({ ok: true, value: {} });
    }) as unknown as Invoke;

  const user = userEvent.setup();
  render(
    <I18nProvider>
      <CorrelationWorkspace invoke={invoke} />
    </I18nProvider>
  );

  expect(await screen.findByText("Trivy")).toBeInTheDocument();
  expect(screen.getByText("Falco")).toBeInTheDocument();
  expect(screen.getByText("Kyverno")).toBeInTheDocument();
  expect(screen.getByText("OPA Gatekeeper")).toBeInTheDocument();
  expect(screen.queryByText(/root cause|caused by|confirmed dependency/i)).not.toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: /candidate-checkout/i }));
  await user.click(screen.getByRole("button", { name: "Open evidence" }));
  expect(await screen.findByText("evidence-correlation-alert")).toBeInTheDocument();
  expect(invoke).not.toHaveBeenCalledWith("incident.write", expect.anything());
});
