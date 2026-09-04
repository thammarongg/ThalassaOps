// SPDX-License-Identifier: Apache-2.0

import "@testing-library/jest-dom/vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, expect, it, vi } from "vitest";
import type { IncidentSeverityCommand, IncidentTransition, IpcResult } from "../../contracts/ipc";
import { I18nProvider, i18n } from "../i18n";
import { incidentFixturePage } from "./incident-fixtures";
import { IncidentActions, type ActionResult, type IncidentActionsProps } from "./IncidentActions";

afterEach(() => {
  cleanup();
  void i18n.changeLanguage("en");
});

const incident = { ...incidentFixturePage.items[1] };

const renderActions = (overrides: Partial<IncidentActionsProps> = {}) =>
  render(
    <I18nProvider>
      <IncidentActions
        incident={overrides.incident ?? incident}
        onTransition={overrides.onTransition ?? vi.fn().mockResolvedValue({ ok: true })}
        onSeverity={overrides.onSeverity ?? vi.fn().mockResolvedValue({ ok: true })}
        onAssign={overrides.onAssign ?? vi.fn().mockResolvedValue({ ok: true })}
        pending={overrides.pending ?? false}
        conflict={overrides.conflict ?? null}
      />
    </I18nProvider>
  );

it("does not render a status change until the command resolves", async () => {
  const user = userEvent.setup();
  let resolve: (value: ActionResult) => void = () => undefined;
  const onTransition: IncidentActionsProps["onTransition"] = vi.fn(
    () =>
      new Promise<ActionResult>((result) => {
        resolve = result;
      })
  );
  renderActions({ onTransition });

  await user.click(screen.getByRole("button", { name: /investigating/i }));
  const form = screen.getByRole("form", { name: /investigating/i });
  await user.type(
    within(form).getByLabelText(/investigation note/i),
    "Reviewed the checkout trace"
  );
  await user.click(within(form).getByRole("button", { name: /submit transition/i }));
  expect(screen.getByTestId("incident-status")).toHaveTextContent(/triage/i);

  await act(async () => {
    resolve({ ok: true, value: undefined });
  });
  await waitFor(() =>
    expect(screen.getByTestId("incident-status")).toHaveTextContent(/investigating/i)
  );
});

it("waits for the required transition context before calling onTransition", async () => {
  const user = userEvent.setup();
  const checkout = incidentFixturePage.items[0];
  const onTransition = vi.fn().mockResolvedValue({ ok: true, value: undefined });
  renderActions({ incident: checkout, onTransition });

  await user.click(screen.getByRole("button", { name: /mitigating/i }));
  expect(onTransition).not.toHaveBeenCalled();

  const form = screen.getByRole("form", { name: /mitigating/i });
  expect(within(form).getByRole("button", { name: /submit transition/i })).toBeDisabled();

  await user.type(
    within(form).getByLabelText(/action description/i),
    "Restarted the checkout gateway"
  );
  await user.type(within(form).getByLabelText(/expected impact/i), "Card payments recover");
  await user.click(within(form).getByRole("button", { name: /submit transition/i }));

  await waitFor(() => expect(onTransition).toHaveBeenCalledTimes(1));
  expect(onTransition).toHaveBeenCalledWith({
    target: "mitigating",
    context: {
      action_description: "Restarted the checkout gateway",
      executor: checkout.roles[0].principal_id,
      expected_impact: "Card payments recover"
    }
  });
});

it("reports a real version conflict and does not resubmit without retry", async () => {
  const user = userEvent.setup();
  const conflictResult: IpcResult<unknown> = {
    ok: false,
    error: {
      code: "INVALID_REQUEST",
      message: "incident request was rejected",
      details: { reason: "incident_version_conflict" }
    }
  };
  const onTransition = vi.fn().mockResolvedValue(conflictResult);

  function ConflictHarness() {
    const [conflict, setConflict] = useState<{ actor: string; at: string } | null>(null);
    const submit = async (transition: IncidentTransition): Promise<IpcResult<unknown>> => {
      const result = await onTransition(transition);
      if (
        !result.ok &&
        result.error.code === "INVALID_REQUEST" &&
        result.error.details.reason === "incident_version_conflict"
      ) {
        setConflict({ actor: "actor-uuid-from-reloaded-event", at: "2026-08-28T09:15:00Z" });
      }
      return result;
    };
    return (
      <IncidentActions
        incident={incident}
        onTransition={submit}
        onSeverity={vi.fn()}
        onAssign={vi.fn()}
        pending={false}
        conflict={conflict}
      />
    );
  }

  render(
    <I18nProvider>
      <ConflictHarness />
    </I18nProvider>
  );

  await user.click(screen.getByRole("button", { name: /investigating/i }));
  const form = screen.getByRole("form", { name: /investigating/i });
  await user.type(
    within(form).getByLabelText(/investigation note/i),
    "Reviewed the checkout trace"
  );
  await user.click(within(form).getByRole("button", { name: /submit transition/i }));
  await waitFor(() => expect(screen.getByRole("alert")).toBeInTheDocument());
  expect(screen.getByRole("alert")).toHaveTextContent("actor-uuid-from-reloaded-event");
  expect(screen.getByRole("alert")).toHaveTextContent("2026-08-28T09:15:00Z");
  expect(screen.getByRole("alert")).toHaveTextContent(/not applied/i);
  expect(onTransition).toHaveBeenCalledTimes(1);

  await user.click(screen.getByRole("button", { name: /retry/i }));
  await waitFor(() => expect(onTransition).toHaveBeenCalledTimes(2));
});

it("reassesses when selecting the derived severity and blocks a role no-op", async () => {
  const user = userEvent.setup();
  const onSeverity = vi.fn().mockResolvedValue({ ok: true, value: undefined });
  renderActions({ onSeverity });

  expect(screen.getByRole("button", { name: /assign role/i })).toBeDisabled();
  await user.click(screen.getByRole("button", { name: "Set S2" }));
  const form = screen.getByRole("form", { name: /severity/i });
  await user.type(within(form).getByLabelText(/reason/i), "The impact assessment changed");
  await user.click(within(form).getByRole("button", { name: /submit severity/i }));

  await waitFor(() => expect(onSeverity).toHaveBeenCalledTimes(1));
  const expected: IncidentSeverityCommand = {
    action: "reassess",
    details: { business_impact: incident.business_impact, reason: "The impact assessment changed" }
  };
  expect(onSeverity).toHaveBeenCalledWith(expected);
});
