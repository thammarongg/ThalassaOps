// SPDX-License-Identifier: Apache-2.0

import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type { CommandEnvelope, Invoke } from "../../contracts/ipc";
import { I18nProvider, i18n } from "../i18n";
import en from "../locales/en";
import { INCIDENT_TIMELINE_LIMIT } from "./incident-envelope";
import { incidentFixturePage, incidentFixtureTimeline } from "./incident-fixtures";
import { IncidentWorkspace } from "./IncidentWorkspace";

afterEach(() => {
  cleanup();
  void i18n.changeLanguage("en");
});

type InvokeMock = ReturnType<typeof vi.fn> & {
  mock: { calls: [string, { envelope: CommandEnvelope<unknown> }][] };
};

/*
 * `Invoke` takes the Tauri command name positionally and the envelope second,
 * so the mock routes on the name. Asserting a single object argument would
 * pass against a shell that never reached IPC at all.
 */
const incidentInvokeMock = () =>
  vi.fn((name: string) =>
    Promise.resolve(
      name === "incident_list"
        ? { ok: true, value: incidentFixturePage }
        : { ok: true, value: incidentFixtureTimeline }
    )
  ) as unknown as InvokeMock;

const renderShell = (invoke: InvokeMock) =>
  render(
    <I18nProvider>
      <IncidentWorkspace invoke={invoke as unknown as Invoke} />
    </I18nProvider>
  );

it("selects the first incident and loads its timeline", async () => {
  const invoke = incidentInvokeMock();
  renderShell(invoke);

  await waitFor(() =>
    expect(
      within(screen.getByRole("listbox")).getByRole("option", { selected: true })
    ).toHaveAccessibleName(/checkout/i)
  );

  const timeline = invoke.mock.calls.find((call) => call[0] === "incident_timeline");
  expect(timeline).toBeDefined();
  expect(timeline?.[1].envelope.command).toBe("incident.timeline");
  expect(timeline?.[1].envelope.capability).toBe("IncidentRead");
  expect(timeline?.[1].envelope.payload).toEqual({
    incident_id: incidentFixturePage.items[0].id,
    after_sequence: null,
    limit: INCIDENT_TIMELINE_LIMIT
  });
});

it("translates a list error code rather than printing it", async () => {
  const invoke = vi.fn().mockResolvedValue({
    ok: false,
    error: { code: "PERMISSION_DENIED", message: "raw wire text", details: {} }
  }) as unknown as InvokeMock;
  renderShell(invoke);

  await waitFor(() =>
    expect(screen.getByRole("alert")).toHaveTextContent(en.incident.errors.permissionDenied)
  );
  expect(screen.queryByText("raw wire text")).not.toBeInTheDocument();
});

it("keeps the reader's selection when a further page arrives", async () => {
  const secondPage = {
    items: [
      {
        ...incidentFixturePage.items[0],
        id: "11111111-1111-4111-8111-111111111111",
        summary: "Notification fan-out backlog"
      }
    ],
    next_cursor: null
  };
  let listCalls = 0;
  const invoke = vi.fn((name: string) => {
    if (name === "incident_list") {
      listCalls += 1;
      return Promise.resolve({
        ok: true,
        value: listCalls === 1 ? incidentFixturePage : secondPage
      });
    }
    return Promise.resolve({ ok: true, value: incidentFixtureTimeline });
  }) as unknown as InvokeMock;

  renderShell(invoke);
  const listbox = await screen.findByRole("listbox");
  await waitFor(() =>
    expect(within(listbox).getByRole("option", { selected: true })).toHaveAccessibleName(
      /checkout/i
    )
  );

  /*
   * The reader moves off the auto-selected first row. Paging must not drag
   * them back: an effect that selects `incidents[0]` on every page change
   * passes an append-only assertion but fails this one.
   */
  await userEvent.click(within(listbox).getByRole("option", { name: /search/i }));
  await userEvent.click(screen.getByRole("button", { name: en.incident.loadMore }));

  await waitFor(() =>
    expect(within(listbox).getAllByRole("option")).toHaveLength(
      incidentFixturePage.items.length + secondPage.items.length
    )
  );
  const selected = within(listbox).getAllByRole("option", { selected: true });
  expect(selected).toHaveLength(1);
  expect(selected[0]).toHaveAccessibleName(/search/i);
});
