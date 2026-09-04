// SPDX-License-Identifier: Apache-2.0

import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import type { IncidentTimelineEvent } from "../../contracts/ipc";
import { I18nProvider, i18n } from "../i18n";
import { IncidentNarrative } from "./IncidentNarrative";
import { incidentFixtureTimeline } from "./incident-fixtures";

afterEach(() => {
  cleanup();
  void i18n.changeLanguage("en");
});

const events = incidentFixtureTimeline.events;
const lifecycle = events.filter((event) => event.payload.kind !== "commented");
const comment = events.find((event) => event.payload.kind === "commented");

const renderNarrative = (given: IncidentTimelineEvent[] = events) =>
  render(
    <I18nProvider>
      <IncidentNarrative events={given} />
    </I18nProvider>
  );

const bodyRows = () => screen.getAllByRole("row").slice(1);

it("renders lifecycle events as a record and excludes comments", () => {
  renderNarrative();

  expect(bodyRows()).toHaveLength(lifecycle.length);
  expect(screen.getByText(/investigating/i)).toBeInTheDocument();
  expect(comment).toBeDefined();
  expect(screen.queryByText(/regional outage/i)).not.toBeInTheDocument();
});

it("renders each row with a timestamp, actor, change and reason column", () => {
  renderNarrative();

  expect(within(bodyRows()[0]).getAllByRole("cell")).toHaveLength(4);
});

/*
 * The narrative is ordered by sequence rather than by arrival: a resumed page
 * that appends an earlier event must not read as if it happened last.
 */
it("orders rows by sequence whatever order they arrive in", () => {
  renderNarrative([...lifecycle].reverse());

  /*
   * The machine-readable `datetime` is asserted rather than the rendered text,
   * which `toLocaleString` formats differently on every host.
   */
  const stamps = bodyRows().map((row) => row.querySelector("time")?.getAttribute("datetime"));
  expect(stamps).toEqual(lifecycle.map((event) => event.occurred_at));
});

it("shows the reason a responder gave, and an empty cell when there is none", () => {
  const [created, attached] = lifecycle;
  renderNarrative([{ ...created, reason: "Paged by the on-call rotation" }, attached]);

  const [first, second] = bodyRows();
  expect(within(first).getAllByRole("cell")[3]).toHaveTextContent("Paged by the on-call rotation");
  expect(within(second).getAllByRole("cell")[3]).toBeEmptyDOMElement();
});

/*
 * `disposition_changed` and `role_changed` never occur on the fixture incident,
 * so without these two rows the switch that describes them would be untested
 * and could render a blank cell in production.
 */
it("describes a disposition change and a role change from their payloads", () => {
  const base = lifecycle[0];
  renderNarrative([
    {
      ...base,
      id: "1a000000-0000-4000-8000-000000000007",
      sequence: 7,
      kind: "disposition_changed",
      payload: {
        kind: "disposition_changed",
        data: { previous: null, current: "duplicate", duplicate_of_incident_id: null }
      }
    },
    {
      ...base,
      id: "1a000000-0000-4000-8000-000000000008",
      sequence: 8,
      kind: "role_changed",
      payload: {
        kind: "role_changed",
        data: {
          role: "incident_commander",
          previous_principal_ids: [],
          current_principal_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
        }
      }
    }
  ]);

  const [disposition, role] = bodyRows();
  expect(disposition).toHaveTextContent(/duplicate/i);
  expect(role).toHaveTextContent(/incident commander/i);
});

it("says the incident has no lifecycle record rather than rendering an empty table", () => {
  renderNarrative(comment ? [comment] : []);

  expect(screen.queryByRole("table")).not.toBeInTheDocument();
  expect(screen.getByText(i18n.t("incident.narrative.empty"))).toBeInTheDocument();
});
