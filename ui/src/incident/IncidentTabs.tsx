// SPDX-License-Identifier: Apache-2.0

import type { Incident } from "../../contracts/ipc";
import { useTranslation } from "../i18n";
import { IncidentEvidencePanel } from "./IncidentEvidencePanel";
import type { EvidenceState } from "./incidentEvidence";
import {
  INCIDENT_TABS,
  type IncidentEvidenceByTab,
  type IncidentTabId,
  type IncidentTabStates
} from "./incidentTabConfig";

const stateFor = (
  state: EvidenceState,
  evidence: IncidentEvidenceByTab,
  tab: (typeof INCIDENT_TABS)[number]
): EvidenceState => {
  if (state.status !== "ready") return state;
  const selected = tab.select(evidence);
  return tab.isEmpty(selected) ? { status: "empty" } : { status: "ready", evidence: selected };
};

/**
 * Tab chrome and the active evidence panel. The shell supplies every state;
 * this component never resolves evidence or owns the active tab.
 */
export function IncidentTabs({
  incident,
  states,
  activeId,
  onSelect
}: {
  incident: Incident;
  states: Partial<IncidentTabStates>;
  activeId: IncidentTabId;
  onSelect: (id: IncidentTabId) => void;
}) {
  const { t } = useTranslation();
  /*
   * Read the current association set on every render. This deliberately is not
   * memoised: later sprints can add evidence to an open incident while this
   * component remains mounted.
   */
  const associatedIds = new Set(incident.evidence_ids);
  const grouped: IncidentEvidenceByTab = {};
  for (const tab of INCIDENT_TABS) {
    const state = states[tab.id] ?? { status: "empty" };
    grouped[tab.id] =
      state.status === "ready" ? state.evidence.filter((item) => associatedIds.has(item.id)) : [];
  }

  const renderedTabs = INCIDENT_TABS.map((tab) => ({
    tab,
    state: stateFor(states[tab.id] ?? { status: "empty" }, grouped, tab)
  }));
  const active = renderedTabs.find(({ tab }) => tab.id === activeId) ?? renderedTabs[0];

  if (!active) return null;

  const panelId = `incident-tabpanel-${active.tab.id}`;
  const activeTabId = `incident-tab-${active.tab.id}`;

  return (
    <section className="incident-tabs" aria-label={t("incident.tabs.title")}>
      <div className="tabs" role="tablist">
        {renderedTabs.map(({ tab, state }) => {
          const empty = state.status === "empty";
          return (
            <button
              key={tab.id}
              id={`incident-tab-${tab.id}`}
              type="button"
              role="tab"
              className="tab"
              aria-controls={`incident-tabpanel-${tab.id}`}
              aria-selected={active.tab.id === tab.id}
              aria-disabled={empty ? "true" : "false"}
              disabled={empty}
              data-testid={`tab-${tab.id}`}
              onClick={() => {
                if (!empty) onSelect(tab.id);
              }}
            >
              {t(tab.labelKey)}
              {empty && (
                <span data-testid={`tab-${tab.id}-empty`} aria-hidden="true">
                  ○
                </span>
              )}
              {state.status === "unavailable" && (
                <span data-testid={`tab-${tab.id}-unavailable`} aria-hidden="true">
                  !
                </span>
              )}
            </button>
          );
        })}
      </div>
      <div
        id={panelId}
        role="tabpanel"
        aria-labelledby={activeTabId}
        data-testid={`tabpanel-${active.tab.id}`}
      >
        <IncidentEvidencePanel state={active.state} />
      </div>
    </section>
  );
}
