// SPDX-License-Identifier: Apache-2.0

import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type { EvidenceRef, Incident } from "../../contracts/ipc";
import { I18nProvider, i18n } from "../i18n";
import { IncidentSummaryCard, buildSummaryMarkdown } from "./IncidentSummaryCard";
import {
  incidentFixtureEvidence,
  incidentFixturePage,
  incidentFixtureTimeline
} from "./incident-fixtures";

afterEach(() => {
  cleanup();
  void i18n.changeLanguage("en");
});

const incident = (() => {
  const base = incidentFixturePage.items[0];
  const commanderPrincipal = "99999999-9999-4999-8999-999999999999";
  const evidence: EvidenceRef[] = [
    {
      ...incidentFixtureEvidence[0],
      id: "evidence-summary-card",
      excerpt: "The source excerpt contains AKIAIOSFODNN7EXAMPLE and must never be copied."
    }
  ];

  return {
    ...base,
    derived_severity: "S2" as const,
    severity_override: {
      derived: "S2" as const,
      selected: "S1" as const,
      actor_id: base.roles[0].principal_id,
      reason: "The customer reach increased",
      evidence_ids: base.evidence_ids
    },
    roles: [
      ...base.roles,
      {
        role: "incident_commander" as const,
        principal_id: commanderPrincipal,
        assigned_by: base.roles[0].principal_id,
        assigned_at: "2026-08-28T08:42:00Z"
      }
    ],
    evidence,
    timeline: incidentFixtureTimeline
  } satisfies Incident & { evidence: EvidenceRef[]; timeline: typeof incidentFixtureTimeline };
})();

it("copies only the explicit summary allowlist", () => {
  const markdown = buildSummaryMarkdown(incident);
  const comment = incident.timeline.events.find((event) => event.payload.kind === "commented");

  expect(incident.evidence[0].excerpt).toContain("AKIA");
  expect(comment?.payload.kind).toBe("commented");
  if (comment?.payload.kind === "commented") {
    expect(comment.payload.data.body).toBe(
      "Payment provider confirms a regional outage on their side"
    );
  }
  expect(incident.roles.some((role) => role.role === "incident_commander")).toBe(true);

  for (const allowed of [
    incident.id,
    incident.summary,
    "S1",
    "S2",
    "investigating",
    incident.created_at,
    incident.updated_at
  ]) {
    expect(markdown).toContain(allowed);
  }
  for (const forbidden of [
    incident.evidence[0].excerpt,
    "Payment provider confirms a regional outage on their side",
    "incident_commander",
    incident.roles[1].principal_id
  ]) {
    expect(markdown).not.toContain(forbidden);
  }
});

it("passes the allowlisted markdown to the copy action", async () => {
  const user = userEvent.setup();
  const onCopy = vi.fn();
  render(
    <I18nProvider>
      <IncidentSummaryCard incident={incident} onCopy={onCopy} />
    </I18nProvider>
  );

  await user.click(screen.getByRole("button", { name: /copy/i }));

  expect(onCopy).toHaveBeenCalledWith(buildSummaryMarkdown(incident));
});

it("is named Summary Card, distinct from the Incident Card", () => {
  render(
    <I18nProvider>
      <IncidentSummaryCard incident={incident} onCopy={() => {}} />
    </I18nProvider>
  );

  expect(screen.getByRole("heading")).toHaveTextContent(/summary card/i);
  expect(screen.getByRole("heading")).not.toHaveTextContent(/^incident card$/i);
});
