import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type { CommandEnvelope, EvidenceRef, Invoke } from "../../contracts/ipc";
import en from "../locales/en";
import th from "../locales/th";
import { correlationFixtureSnapshot } from "../correlation/correlation-fixtures";
import { CorrelationWorkspace } from "../correlation/CorrelationWorkspace";
import { I18nProvider, i18n } from "../i18n";
import { changeSnapshotFixture } from "./change-fixtures";

afterEach(() => {
  cleanup();
  void i18n.changeLanguage("en");
});

const changeEvidence: EvidenceRef[] = changeSnapshotFixture.events.map((event) => ({
  id: event.source_record.evidence_ids[0],
  source_kind: event.source,
  connector_id: null,
  scope: event.scope,
  endpoint: "fixture://change/" + event.source,
  query: null,
  observed_at: event.occurred_at,
  excerpt: "retained",
  native_url: null,
  redaction: {
    classification_verified: true,
    redaction_verified: true,
    masked: false,
    unparsed: false
  }
}));

const invokeWithChanges = () =>
  vi.fn().mockImplementation((name: string, args: { envelope: CommandEnvelope<unknown> }) => {
    if (name === "correlation_snapshot") {
      return Promise.resolve({ ok: true, value: correlationFixtureSnapshot });
    }
    if (name === "change_snapshot") {
      return Promise.resolve({ ok: true, value: changeSnapshotFixture });
    }
    if (name === "change_evidence") {
      const ids = (args.envelope.payload as { evidence_ids: string[] }).evidence_ids;
      return Promise.resolve({
        ok: true,
        value: changeEvidence.filter((item) => ids.includes(item.id))
      });
    }
    if (name === "correlation_evidence") {
      const ids = (args.envelope.payload as { evidence_ids: string[] }).evidence_ids;
      return Promise.resolve({
        ok: true,
        value: correlationFixtureSnapshot.evidence.filter((item) => ids.includes(item.id))
      });
    }
    return Promise.resolve({ ok: true, value: {} });
  }) as unknown as Invoke;

it("lets a responder see what changed before a candidate and reach the supporting source", async () => {
  const invoke = invokeWithChanges();
  const user = userEvent.setup();
  render(
    <I18nProvider>
      <CorrelationWorkspace invoke={invoke} />
    </I18nProvider>
  );

  await user.click(await screen.findByRole("button", { name: /candidate-checkout/i }));

  const changeSection = screen.getByRole("region", {
    name: "Changes before these signals"
  });
  expect(within(changeSection).getByText("Changed before")).toBeInTheDocument();
  expect(within(changeSection).getByText("GitLab")).toBeInTheDocument();
  expect(within(changeSection).getByText(/Preceded the first signal by 6 min/)).toBeInTheDocument();
  expect(within(changeSection).getByText("Shared target: deployment/checkout")).toBeInTheDocument();

  const sourceLink = within(changeSection).getByRole("link", { name: "Open at source" });
  expect(sourceLink).toHaveAttribute(
    "href",
    "https://gitlab.example/storefront/checkout/-/merge_requests/128"
  );

  await user.click(within(changeSection).getByRole("button", { name: "View change evidence" }));
  expect(await screen.findByText("evidence-change-merge-fixture")).toBeInTheDocument();

  expect(invoke).not.toHaveBeenCalledWith("change_ingest", expect.anything());
  expect(invoke).not.toHaveBeenCalledWith("change_write", expect.anything());
});

it("never renders diff content or an empty diff viewer in the change detail", async () => {
  const invoke = invokeWithChanges();
  render(
    <I18nProvider>
      <CorrelationWorkspace invoke={invoke} />
    </I18nProvider>
  );

  const detail = await screen.findByRole("region", { name: "Change detail" });
  expect(within(detail).getByText(/Diff content is read at the source/)).toBeInTheDocument();
  expect(within(detail).queryByText(/@@ -/)).not.toBeInTheDocument();
});

it("keeps every change locale value free of causal wording", () => {
  const causal = /caus|root cause|trigger/i;
  const collect = (value: unknown): string[] =>
    typeof value === "string"
      ? [value]
      : typeof value === "object" && value !== null
        ? Object.values(value).flatMap(collect)
        : [];

  for (const catalog of [en.change, th.change]) {
    for (const copy of collect(catalog)) {
      expect(copy).not.toMatch(causal);
    }
  }
});
