// SPDX-License-Identifier: Apache-2.0

import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type { CommandEnvelope, IncidentTimelineEvent, Invoke } from "../../contracts/ipc";
import { I18nProvider, i18n } from "../i18n";
import en from "../locales/en";
import { INCIDENT_TIMELINE_LIMIT } from "./incidentEnvelope";
import {
  incidentFixtureEvidence,
  incidentFixturePage,
  incidentFixtureTimeline
} from "./incident-fixtures";
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
        : name === "correlation_evidence"
          ? { ok: true, value: incidentFixtureEvidence }
          : name === "incident_add_comment"
            ? {
                ok: true,
                value: { incident: incidentFixturePage.items[0], events: [] }
              }
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

it("resolves the selected incident's evidence once through correlation_evidence", async () => {
  const invoke = incidentInvokeMock();
  renderShell(invoke);

  await waitFor(() =>
    expect(invoke.mock.calls.filter((call) => call[0] === "correlation_evidence")).toHaveLength(1)
  );

  const evidenceCall = invoke.mock.calls.find((call) => call[0] === "correlation_evidence");
  expect(evidenceCall?.[1].envelope.command).toBe("correlation.evidence");
  expect(evidenceCall?.[1].envelope.capability).toBe("ResourceRead");
  expect(evidenceCall?.[1].envelope.payload).toEqual({
    evidence_ids: incidentFixturePage.items[0].evidence_ids
  });
});

it("renders the incident summary card for the selected incident", async () => {
  const invoke = incidentInvokeMock();
  renderShell(invoke);

  expect(
    await screen.findByRole("heading", { name: /incident summary card/i })
  ).toBeInTheDocument();
});

it("submits a comment for the selected incident through incident_add_comment", async () => {
  const user = userEvent.setup();
  const invoke = incidentInvokeMock();
  renderShell(invoke);

  const comments = await screen.findByRole("region", { name: en.incident.comments.title });
  const composer = within(comments).getByRole("textbox");
  await user.type(composer, "paged the on-call");
  await user.click(within(comments).getByRole("button", { name: /add comment/i }));

  await waitFor(() =>
    expect(invoke.mock.calls.filter((call) => call[0] === "incident_add_comment")).toHaveLength(1)
  );
  const commentCall = invoke.mock.calls.find((call) => call[0] === "incident_add_comment");
  expect(commentCall?.[1].envelope.command).toBe("incident.add_comment");
  expect(commentCall?.[1].envelope.capability).toBe("IncidentWrite");
  expect(commentCall?.[1].envelope.payload).toEqual({
    incident_id: incidentFixturePage.items[0].id,
    body: "paged the on-call"
  });
});

it("reloads the incident and newest timeline event after a version conflict", async () => {
  const user = userEvent.setup();
  const actor = "99999999-9999-4999-8999-999999999999";
  const occurredAt = "2026-08-28T09:15:00Z";
  const latestIncident = {
    ...incidentFixturePage.items[0],
    status: "mitigating" as const,
    version: incidentFixturePage.items[0].version + 1,
    updated_at: occurredAt
  };
  const latestEvent: IncidentTimelineEvent = {
    ...incidentFixtureTimeline.events[5],
    id: "1a000000-0000-4000-8000-000000000007",
    sequence: 7,
    kind: "status_transitioned",
    actor_id: actor,
    occurred_at: occurredAt,
    payload: {
      kind: "status_transitioned",
      data: {
        from: "investigating",
        to: "mitigating",
        transition: {
          target: "mitigating",
          context: {
            action_description: "Restarted the checkout gateway",
            executor: actor,
            expected_impact: "Card payments recover"
          }
        }
      }
    }
  };
  const latestTimeline = {
    ...incidentFixtureTimeline,
    events: [...incidentFixtureTimeline.events, latestEvent],
    next_sequence: 7
  };
  let timelineCalls = 0;
  const invoke = vi.fn((name: string) => {
    if (name === "incident_list") return Promise.resolve({ ok: true, value: incidentFixturePage });
    if (name === "correlation_evidence") {
      return Promise.resolve({ ok: true, value: incidentFixtureEvidence });
    }
    if (name === "incident_transition") {
      return Promise.resolve({
        ok: false,
        error: {
          code: "INVALID_REQUEST",
          message: "incident request was rejected",
          details: { reason: "incident_version_conflict" }
        }
      });
    }
    if (name === "incident_get") return Promise.resolve({ ok: true, value: latestIncident });
    timelineCalls += 1;
    return Promise.resolve({
      ok: true,
      value: timelineCalls === 1 ? incidentFixtureTimeline : latestTimeline
    });
  }) as unknown as InvokeMock;
  renderShell(invoke);

  await screen.findByRole("table", { name: en.incident.narrative.caption });
  await user.click(await screen.findByRole("button", { name: /mitigating/i }));
  const transitionForm = screen.getByRole("form", { name: /mitigating/i });
  await user.type(
    within(transitionForm).getByLabelText(/action description/i),
    "Restarted the checkout gateway"
  );
  await user.type(
    within(transitionForm).getByLabelText(/expected impact/i),
    "Card payments recover"
  );
  await user.click(within(transitionForm).getByRole("button", { name: /submit transition/i }));
  await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent(actor));

  expect(invoke.mock.calls.filter((call) => call[0] === "incident_transition")).toHaveLength(1);
  const getCall = invoke.mock.calls.find((call) => call[0] === "incident_get");
  expect(getCall?.[1].envelope.command).toBe("incident.get");
  expect(getCall?.[1].envelope.capability).toBe("IncidentRead");
  expect(getCall?.[1].envelope.payload).toEqual({
    incident_id: incidentFixturePage.items[0].id
  });
  expect(invoke.mock.calls.filter((call) => call[0] === "incident_timeline")).toHaveLength(2);
  expect(screen.getByRole("alert")).toHaveTextContent(occurredAt);
  expect(screen.getByRole("alert")).toHaveTextContent(/not applied/i);
});

it("sends the exact versioned payload for status, severity, and role actions", async () => {
  const user = userEvent.setup();
  const invoke = incidentInvokeMock();
  const incident = incidentFixturePage.items[0];
  const principal = "12345678-1234-4123-8123-123456789012";
  renderShell(invoke);

  await screen.findByRole("table", { name: en.incident.narrative.caption });

  await user.clear(screen.getByLabelText(en.incident.actions.principalLabel));
  await user.type(screen.getByLabelText(en.incident.actions.principalLabel), principal);
  await user.click(screen.getByRole("button", { name: /mitigating/i }));
  const transitionForm = screen.getByRole("form", { name: /mitigating/i });
  await user.type(
    within(transitionForm).getByLabelText(/action description/i),
    "Restarted the checkout gateway"
  );
  await user.type(
    within(transitionForm).getByLabelText(/expected impact/i),
    "Card payments recover"
  );
  await user.click(within(transitionForm).getByRole("button", { name: /submit transition/i }));
  await waitFor(() =>
    expect(invoke.mock.calls.filter((call) => call[0] === "incident_transition")).toHaveLength(1)
  );
  expect(
    invoke.mock.calls.find((call) => call[0] === "incident_transition")?.[1].envelope.payload
  ).toEqual({
    incident_id: incident.id,
    expected_version: incident.version,
    transition: {
      target: "mitigating",
      context: {
        action_description: "Restarted the checkout gateway",
        executor: principal,
        expected_impact: "Card payments recover"
      }
    }
  });

  await user.click(screen.getByRole("button", { name: "Set S2" }));
  const severityForm = screen.getByRole("form", { name: /severity/i });
  await user.type(
    within(severityForm).getByLabelText(/reason/i),
    "Traffic remains above the error budget"
  );
  await user.click(within(severityForm).getByRole("button", { name: /submit severity/i }));
  await waitFor(() =>
    expect(invoke.mock.calls.filter((call) => call[0] === "incident_set_severity")).toHaveLength(1)
  );
  expect(
    invoke.mock.calls.find((call) => call[0] === "incident_set_severity")?.[1].envelope.payload
  ).toEqual({
    incident_id: incident.id,
    expected_version: incident.version,
    command: {
      action: "override",
      details: {
        selected: "S2",
        reason: "Traffic remains above the error budget",
        evidence_ids: incident.evidence_ids
      }
    }
  });

  await user.selectOptions(
    screen.getByLabelText(en.incident.actions.roleLabel),
    "incident_commander"
  );
  await user.clear(screen.getByLabelText(en.incident.actions.principalLabel));
  await user.type(screen.getByLabelText(en.incident.actions.principalLabel), principal);
  await user.click(screen.getByRole("button", { name: en.incident.actions.assign }));
  await waitFor(() =>
    expect(invoke.mock.calls.filter((call) => call[0] === "incident_assign_role")).toHaveLength(1)
  );
  expect(
    invoke.mock.calls.find((call) => call[0] === "incident_assign_role")?.[1].envelope.payload
  ).toEqual({
    incident_id: incident.id,
    expected_version: incident.version,
    command: {
      action: "assign",
      details: { role: "incident_commander", principal_id: principal }
    }
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
      : name === "correlation_evidence"
        ? Promise.resolve({ ok: true, value: incidentFixtureEvidence })
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
