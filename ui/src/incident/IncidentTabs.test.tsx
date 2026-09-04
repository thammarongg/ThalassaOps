// SPDX-License-Identifier: Apache-2.0

import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { EvidenceRef } from "../../contracts/ipc";
import { I18nProvider, i18n } from "../i18n";
import { incidentFixtureEvidence, incidentFixturePage } from "./incident-fixtures";
import type { IncidentTabId, IncidentTabStates } from "./incidentTabConfig";
import { IncidentTabs } from "./IncidentTabs";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  void i18n.changeLanguage("en");
});

const vulnerabilityEvidence: EvidenceRef = {
  ...incidentFixtureEvidence[0],
  id: "evidence-checkout-vulnerability",
  source_kind: "trivy",
  endpoint: "fixture://trivy/checkout"
};

const states: IncidentTabStates = {
  alerts: { status: "ready", evidence: incidentFixtureEvidence },
  topology: { status: "empty" },
  changes: { status: "empty" },
  vulnerabilities: { status: "ready", evidence: [vulnerabilityEvidence] }
};

const renderTabs = (
  incident = incidentFixturePage.items[0],
  overrides: Partial<{
    states: IncidentTabStates;
    activeId: IncidentTabId;
    onSelect: (id: IncidentTabId) => void;
  }> = {}
) =>
  render(
    <I18nProvider>
      <IncidentTabs
        incident={incident}
        states={overrides.states ?? states}
        activeId={overrides.activeId ?? "alerts"}
        onSelect={overrides.onSelect ?? vi.fn()}
      />
    </I18nProvider>
  );

it("reads the association set on every render rather than memoising it", () => {
  const incidentWithNoVulnerability = incidentFixturePage.items[0];
  const incidentWithVulnerability = {
    ...incidentWithNoVulnerability,
    evidence_ids: [...incidentWithNoVulnerability.evidence_ids, vulnerabilityEvidence.id]
  };

  const { rerender } = renderTabs(incidentWithNoVulnerability);
  expect(screen.getByRole("tab", { name: /vulnerabilit/i })).toHaveAttribute(
    "aria-disabled",
    "true"
  );

  rerender(
    <I18nProvider>
      <IncidentTabs
        incident={incidentWithVulnerability}
        states={states}
        activeId="alerts"
        onSelect={vi.fn()}
      />
    </I18nProvider>
  );
  expect(screen.getByRole("tab", { name: /vulnerabilit/i })).toHaveAttribute(
    "aria-disabled",
    "false"
  );
});

it("distinguishes an empty tab from an unavailable one", () => {
  const tabStates: IncidentTabStates = {
    alerts: { status: "empty" },
    topology: { status: "unavailable", cause: "missing" },
    changes: { status: "empty" },
    vulnerabilities: { status: "empty" }
  };

  renderTabs(incidentFixturePage.items[0], { states: tabStates, activeId: "topology" });

  expect(screen.getByTestId("tab-alerts-empty")).toBeInTheDocument();
  expect(screen.getByTestId("tab-topology-unavailable")).toBeInTheDocument();
  expect(screen.getByRole("tab", { name: /topology/i })).toHaveAttribute("aria-disabled", "false");

  fireEvent.click(screen.getByRole("tab", { name: /topology/i }));
  expect(screen.getByTestId("incident-evidence-unavailable")).toBeInTheDocument();
});

it("keeps fixture evidence in alerts and separates vulnerability evidence", () => {
  const incident = {
    ...incidentFixturePage.items[0],
    evidence_ids: [...incidentFixturePage.items[0].evidence_ids, vulnerabilityEvidence.id]
  };
  const { rerender } = renderTabs(incident);

  expect(screen.getByText(incidentFixtureEvidence[1].id)).toBeInTheDocument();
  expect(screen.getByRole("tab", { name: /topology/i })).toHaveAttribute("aria-disabled", "true");
  expect(screen.getByRole("tab", { name: /changes/i })).toHaveAttribute("aria-disabled", "true");

  rerender(
    <I18nProvider>
      <IncidentTabs
        incident={incident}
        states={states}
        activeId="vulnerabilities"
        onSelect={vi.fn()}
      />
    </I18nProvider>
  );
  expect(screen.getByText(vulnerabilityEvidence.id)).toBeInTheDocument();
});
