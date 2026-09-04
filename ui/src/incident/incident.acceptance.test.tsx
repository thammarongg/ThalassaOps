// SPDX-License-Identifier: Apache-2.0

import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type { CommandEnvelope, IncidentPage, Invoke } from "../../contracts/ipc";
import { I18nProvider, i18n } from "../i18n";
import en from "../locales/en";
import {
  incidentFixtureEvidence,
  incidentFixturePage,
  incidentFixtureTimeline
} from "./incident-fixtures";
import { IncidentWorkspace } from "./IncidentWorkspace";

afterEach(() => {
  cleanup();
  void i18n.changeLanguage("en");
});

type InvokeMock = ReturnType<typeof vi.fn> & {
  mock: { calls: [string, { envelope: CommandEnvelope<unknown> }][] };
};

const createAcceptanceFixture = (): { page: IncidentPage; vulnerabilityId: string } => {
  const vulnerability = incidentFixtureEvidence.find((item) => item.source_kind === "trivy");
  if (!vulnerability) {
    throw new Error("The acceptance fixture must include a trivy evidence reference");
  }

  const incident = {
    ...incidentFixturePage.items[0],
    summary: "Checkout vulnerability requires review",
    status: "triage" as const,
    evidence_ids: [...new Set([...incidentFixturePage.items[0].evidence_ids, vulnerability.id])]
  };
  return {
    page: { ...incidentFixturePage, items: [incident], next_cursor: null },
    vulnerabilityId: vulnerability.id
  };
};

const incidentInvokeMock = (page: IncidentPage): InvokeMock =>
  vi.fn((name: string) => {
    if (name === "incident_list") return Promise.resolve({ ok: true, value: page });
    if (name === "correlation_evidence") {
      return Promise.resolve({ ok: true, value: incidentFixtureEvidence });
    }
    if (name === "incident_timeline") {
      return Promise.resolve({ ok: true, value: incidentFixtureTimeline });
    }
    if (name === "incident_add_comment") {
      return Promise.resolve({ ok: true, value: { incident: page.items[0], events: [] } });
    }
    if (
      name === "incident_assign_role" ||
      name === "incident_transition" ||
      name === "incident_set_severity"
    ) {
      return Promise.resolve({ ok: true, value: undefined });
    }
    return Promise.reject(new Error(`Unexpected command: ${name}`));
  }) as unknown as InvokeMock;

const renderWorkspace = (invoke: InvokeMock) =>
  render(
    <I18nProvider>
      <IncidentWorkspace invoke={invoke as unknown as Invoke} />
    </I18nProvider>
  );

it("lets a responder work a vulnerability incident from triage to resolved", async () => {
  const user = userEvent.setup();
  const { page, vulnerabilityId } = createAcceptanceFixture();
  const invoke = incidentInvokeMock(page);
  renderWorkspace(invoke);

  await user.click(await screen.findByRole("option", { name: /vulnerability/i }));
  expect(await screen.findByRole("heading", { name: /summary card/i })).toBeInTheDocument();

  const vulnerabilityTab = await screen.findByRole("tab", { name: /vulnerabilit/i });
  await user.click(vulnerabilityTab);
  expect(await screen.findByText(vulnerabilityId)).toBeInTheDocument();

  const evidenceCall = await waitFor(() => {
    const call = invoke.mock.calls.find((candidate) => candidate[0] === "correlation_evidence");
    expect(call).toBeDefined();
    return call;
  });
  expect(evidenceCall?.[1].envelope.payload).toEqual({ evidence_ids: page.items[0].evidence_ids });

  const comments = screen.getByRole("region", { name: en.incident.comments.title });
  await user.type(within(comments).getByRole("textbox"), "confirmed the vulnerability finding");
  await user.click(within(comments).getByRole("button", { name: /add comment/i }));
  expect(await screen.findByText("confirmed the vulnerability finding")).toBeInTheDocument();

  const actions = screen.getByRole("region", { name: en.incident.actions.title });
  const principal = "12345678-1234-4123-8123-123456789012";
  await user.selectOptions(
    within(actions).getByLabelText(en.incident.actions.roleLabel),
    "incident_commander"
  );
  await user.type(within(actions).getByLabelText(en.incident.actions.principalLabel), principal);
  await user.click(within(actions).getByRole("button", { name: en.incident.actions.assign }));
  await waitFor(() =>
    expect(invoke.mock.calls.filter((call) => call[0] === "incident_assign_role")).toHaveLength(1)
  );

  await user.click(within(actions).getByRole("button", { name: /move to investigating/i }));
  let form = within(actions).getByRole("form", { name: /investigating/i });
  await user.type(
    within(form).getByLabelText(/investigation note/i),
    "Confirmed the vulnerability finding"
  );
  await user.click(within(form).getByRole("button", { name: /submit transition/i }));
  await waitFor(() =>
    expect(within(actions).getByTestId("incident-status")).toHaveTextContent(/investigating/i)
  );

  await user.click(within(actions).getByRole("button", { name: /move to mitigating/i }));
  form = within(actions).getByRole("form", { name: /mitigating/i });
  await user.type(
    within(form).getByLabelText(/action description/i),
    "Rotate the affected checkout dependency"
  );
  await user.type(within(form).getByLabelText(/expected impact/i), "Checkout requests recover");
  await user.click(within(form).getByRole("button", { name: /submit transition/i }));
  await waitFor(() =>
    expect(within(actions).getByTestId("incident-status")).toHaveTextContent(/mitigating/i)
  );

  await user.click(within(actions).getByRole("button", { name: /move to monitoring/i }));
  form = within(actions).getByRole("form", { name: /monitoring/i });
  await user.type(within(form).getByLabelText(/verification window/i), "300");
  await user.type(
    within(form).getByLabelText(/success criteria/i),
    "Error rate returns to baseline"
  );
  await user.click(within(form).getByRole("button", { name: /submit transition/i }));
  await waitFor(() =>
    expect(within(actions).getByTestId("incident-status")).toHaveTextContent(/monitoring/i)
  );

  await user.click(within(actions).getByRole("button", { name: /move to resolved/i }));
  form = within(actions).getByRole("form", { name: /resolved/i });
  await user.type(
    within(form).getByLabelText(/resolution summary/i),
    "The affected dependency has recovered"
  );
  fireEvent.change(within(form).getByLabelText(/impact ended at/i), {
    target: { value: "2026-08-28T16:00" }
  });
  await user.click(within(form).getByRole("button", { name: /submit transition/i }));
  await waitFor(() =>
    expect(within(actions).getByTestId("incident-status")).toHaveTextContent(/resolved/i)
  );

  const calls = invoke.mock.calls.map(([tauriCommand, args]) => ({
    tauriCommand,
    envelopeCommand: args.envelope.command
  }));
  expect(calls).toEqual(
    expect.arrayContaining([
      { tauriCommand: "correlation_evidence", envelopeCommand: "correlation.evidence" },
      { tauriCommand: "incident_add_comment", envelopeCommand: "incident.add_comment" },
      { tauriCommand: "incident_assign_role", envelopeCommand: "incident.assign_role" },
      { tauriCommand: "incident_transition", envelopeCommand: "incident.transition" }
    ])
  );
});
