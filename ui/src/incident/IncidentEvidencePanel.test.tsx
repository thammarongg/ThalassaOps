// SPDX-License-Identifier: Apache-2.0

import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { I18nProvider, i18n } from "../i18n";
import type { EvidenceState, EvidenceUnavailableCause } from "./incidentEvidence";
import { IncidentEvidencePanel } from "./IncidentEvidencePanel";
import { incidentFixtureEvidence } from "./incident-fixtures";

afterEach(() => {
  cleanup();
  void i18n.changeLanguage("en");
});

const renderPanel = (state: EvidenceState) =>
  render(
    <I18nProvider>
      <IncidentEvidencePanel state={state} />
    </I18nProvider>
  );

const causes: EvidenceUnavailableCause[] = ["missing", "scope", "unverified", "unknown"];

it("renders the resolved evidence", () => {
  renderPanel({ status: "ready", evidence: incidentFixtureEvidence });

  for (const item of incidentFixtureEvidence) {
    expect(screen.getByText(item.id)).toBeInTheDocument();
    expect(screen.getByText(item.excerpt)).toBeInTheDocument();
  }
  expect(screen.getByText("Prometheus")).toBeInTheDocument();
});

/*
 * The empty state says the incident has no associations of this kind; the
 * unavailable states say resolution failed. A retrospective reader who cannot
 * tell them apart cannot tell an uneventful incident from an unreadable one.
 */
it("distinguishes empty from unavailable and loading", () => {
  const { unmount } = renderPanel({ status: "empty" });
  const empty = screen.getByTestId("incident-evidence-empty").textContent;
  unmount();

  renderPanel({ status: "loading" });
  const loading = screen.getByTestId("incident-evidence-loading").textContent;
  expect(loading).not.toBe(empty);
});

it("states a distinct reason for every unavailable cause", () => {
  const messages = causes.map((cause) => {
    const { unmount } = renderPanel({ status: "unavailable", cause });
    const text = screen.getByTestId("incident-evidence-unavailable").textContent ?? "";
    unmount();
    return text;
  });

  expect(messages.every((message) => message.trim() !== "")).toBe(true);
  expect(new Set(messages).size).toBe(causes.length);
});

it("announces an unavailable panel to assistive technology", () => {
  renderPanel({ status: "unavailable", cause: "missing" });

  expect(screen.getByRole("alert")).toBeInTheDocument();
});

/*
 * `native_url` reaches the UI from a source record. Only an https URL the
 * guard admits may become a clickable link; the fixture's second reference has
 * none, so exactly one link is offered.
 */
it("offers a native link only for evidence that carries a trusted one", () => {
  renderPanel({ status: "ready", evidence: incidentFixtureEvidence });

  expect(screen.getAllByRole("button", { name: /open in source/i })).toHaveLength(1);
});

it("states the redaction of every reference", () => {
  renderPanel({ status: "ready", evidence: incidentFixtureEvidence });

  expect(screen.getAllByTestId("incident-evidence-redaction")).toHaveLength(
    incidentFixtureEvidence.length
  );
});

it("translates its states", async () => {
  await i18n.changeLanguage("th");
  renderPanel({ status: "unavailable", cause: "scope" });

  expect(screen.getByTestId("incident-evidence-unavailable").textContent).not.toMatch(
    /incident\.evidence/
  );
  expect(screen.getByTestId("incident-evidence-unavailable").textContent?.trim()).not.toBe("");
});
