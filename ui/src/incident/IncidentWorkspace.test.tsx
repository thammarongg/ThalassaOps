// SPDX-License-Identifier: Apache-2.0

import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type { CommandEnvelope, IncidentTimelineEvent, Invoke } from "../../contracts/ipc";
import { I18nProvider, i18n } from "../i18n";
import en from "../locales/en";
import { INCIDENT_TIMELINE_LIMIT } from "./incident-envelope";
import { incidentFixturePage, incidentFixtureTimeline } from "./incident-fixtures";
import { IncidentWorkspace } from "./IncidentWorkspace";

/*
 * The shell must never hand the narrative the previous incident's events, not
 * even for the one commit between a selection change and the effect that
 * refetches. That frame is gone before any `await` can inspect the DOM, so the
 * real narrative is wrapped to record the events it receives on every render.
 * It still renders the real component: the DOM assertions elsewhere in this
 * file keep their meaning.
 */
const narrative = vi.hoisted(() => ({ renders: [] as IncidentTimelineEvent[][] }));

vi.mock("./IncidentNarrative", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./IncidentNarrative")>();
  return {
    IncidentNarrative: (props: { events: IncidentTimelineEvent[] }) => {
      narrative.renders.push([...props.events]);
      return actual.IncidentNarrative(props);
    }
  };
});

afterEach(() => {
  cleanup();
  narrative.renders.length = 0;
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

it("renders the selected incident's narrative from its timeline", async () => {
  const invoke = incidentInvokeMock();
  renderShell(invoke);

  const narrative = await screen.findByRole("table", {
    name: en.incident.narrative.caption
  });
  /*
   * One row per lifecycle event plus the header. The fixture's comment is not
   * among them: the narrative records what the system did.
   */
  const lifecycle = incidentFixtureTimeline.events.filter(
    (event) => event.payload.kind !== "commented"
  );
  expect(within(narrative).getAllByRole("row")).toHaveLength(lifecycle.length + 1);
});

/*
 * Every incident has at least its creation event, so "no lifecycle events yet"
 * is never true of a real one. Rendering the narrative before its timeline
 * arrives would put that sentence on an audit surface for the length of the
 * read.
 */
it("does not claim the incident has no lifecycle record while the timeline loads", async () => {
  const invoke = vi.fn((name: string) =>
    name === "incident_list"
      ? Promise.resolve({ ok: true, value: incidentFixturePage })
      : new Promise(() => {})
  ) as unknown as InvokeMock;
  renderShell(invoke);

  expect(await screen.findByText(en.incident.narrative.loading)).toBeInTheDocument();
  expect(screen.queryByText(en.incident.narrative.empty)).not.toBeInTheDocument();
});

/*
 * The same finding, caught one frame earlier: the DOM assertion above passes
 * once the effect-driven loading state lands, so it cannot tell a shell that
 * never renders the narrative early from one that renders it for a single
 * commit. The render log can.
 */
it("never renders the narrative before the first timeline page arrives", async () => {
  const invoke = vi.fn((name: string) =>
    name === "incident_list"
      ? Promise.resolve({ ok: true, value: incidentFixturePage })
      : new Promise(() => {})
  ) as unknown as InvokeMock;
  renderShell(invoke);

  await waitFor(() =>
    expect(
      within(screen.getByRole("listbox")).getByRole("option", { selected: true })
    ).toHaveAccessibleName(/checkout/i)
  );
  expect(narrative.renders).toEqual([]);
});

/*
 * A failed first read is not evidence of an empty record. The translated
 * error is already on the alert above the detail; rendering the narrative's
 * empty state beside it would assert a false no-record on an audit surface,
 * and this is a persistent state, not a transient frame.
 */
it("shows a translated timeline error without the false no-record state", async () => {
  const invoke = vi.fn((name: string) =>
    name === "incident_list"
      ? Promise.resolve({ ok: true, value: incidentFixturePage })
      : Promise.resolve({
          ok: false,
          error: { code: "NOT_FOUND", message: "raw wire text", details: {} }
        })
  ) as unknown as InvokeMock;
  renderShell(invoke);

  await waitFor(() =>
    expect(screen.getByRole("alert")).toHaveTextContent(en.incident.errors.notFound)
  );
  expect(screen.queryByText(en.incident.narrative.empty)).not.toBeInTheDocument();
  expect(
    screen.queryByRole("table", { name: en.incident.narrative.caption })
  ).not.toBeInTheDocument();
});

/*
 * Switching selection while the new incident's timeline is still pending: the
 * checkout record must not appear under the search summary, not even for the
 * commit between the click and the refetch effect.
 */
it("does not render the previous incident's record under a new selection", async () => {
  let timelineCalls = 0;
  const invoke = vi.fn((name: string) => {
    if (name === "incident_list") {
      return Promise.resolve({ ok: true, value: incidentFixturePage });
    }
    timelineCalls += 1;
    return timelineCalls === 1
      ? Promise.resolve({ ok: true, value: incidentFixtureTimeline })
      : new Promise(() => {});
  }) as unknown as InvokeMock;
  renderShell(invoke);

  await screen.findByRole("table", { name: en.incident.narrative.caption });
  const renderedUnderCheckout = narrative.renders.length;

  await userEvent.click(
    within(screen.getByRole("listbox")).getByRole("option", { name: /search/i })
  );
  await waitFor(() =>
    expect(
      within(screen.getByRole("listbox")).getByRole("option", { selected: true })
    ).toHaveAccessibleName(/search/i)
  );
  expect(screen.getByText(en.incident.narrative.loading)).toBeInTheDocument();

  const underSearch = narrative.renders.slice(renderedUnderCheckout);
  underSearch.forEach((events) => {
    expect(events.map((event) => event.incident_id)).not.toContain(incidentFixturePage.items[0].id);
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
