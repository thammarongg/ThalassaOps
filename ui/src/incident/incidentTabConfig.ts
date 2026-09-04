// SPDX-License-Identifier: Apache-2.0

import type { EvidenceRef, EvidenceSourceKind } from "../../contracts/ipc";
import type { EvidenceState } from "./incidentEvidence";

export type IncidentEvidenceByTab = Record<string, EvidenceRef[]>;

type IncidentTabDefinition = {
  id: string;
  labelKey: string;
  sourceKinds: readonly EvidenceSourceKind[];
  select: (evidence: IncidentEvidenceByTab) => EvidenceRef[];
  isEmpty: (evidence: EvidenceRef[]) => boolean;
};

/*
 * Fixture evidence stands in for an operational alert in the deterministic
 * incident corpus, so it stays visible in Alerts. There is no fixture tab, and
 * silently dropping it would make the evidence disappear from the workspace.
 */
const alertSourceKinds = [
  "alertmanager",
  "prometheus",
  "health_check",
  "fixture"
] as const satisfies readonly EvidenceSourceKind[];
const topologySourceKinds = [
  "kubernetes",
  "cloud"
] as const satisfies readonly EvidenceSourceKind[];
const changeSourceKinds = [
  "github",
  "gitlab",
  "argo_cd"
] as const satisfies readonly EvidenceSourceKind[];
const vulnerabilitySourceKinds = [
  "trivy",
  "falco",
  "kyverno",
  "opa_gatekeeper"
] as const satisfies readonly EvidenceSourceKind[];

const isEmpty = (evidence: EvidenceRef[]) => evidence.length === 0;

/**
 * The registry owns both the source partition and the tab chrome. A fifth tab
 * only adds another entry; the partitioner and renderer iterate this array.
 */
export const INCIDENT_TABS = [
  {
    id: "alerts",
    labelKey: "incident.tabs.alerts",
    sourceKinds: alertSourceKinds,
    select: (evidence) => evidence.alerts,
    isEmpty
  },
  {
    id: "topology",
    labelKey: "incident.tabs.topology",
    sourceKinds: topologySourceKinds,
    select: (evidence) => evidence.topology,
    isEmpty
  },
  {
    id: "changes",
    labelKey: "incident.tabs.changes",
    sourceKinds: changeSourceKinds,
    select: (evidence) => evidence.changes,
    isEmpty
  },
  {
    id: "vulnerabilities",
    labelKey: "incident.tabs.vulnerabilities",
    sourceKinds: vulnerabilitySourceKinds,
    select: (evidence) => evidence.vulnerabilities,
    isEmpty
  }
] satisfies readonly IncidentTabDefinition[];

export type IncidentTab = (typeof INCIDENT_TABS)[number];
export type IncidentTabId = IncidentTab["id"];
export type IncidentTabStates = Record<IncidentTabId, EvidenceState>;

/** Partition one resolved response without issuing another evidence request. */
export const partitionIncidentEvidence = (evidence: EvidenceRef[]): IncidentEvidenceByTab => {
  const grouped: IncidentEvidenceByTab = {};
  for (const tab of INCIDENT_TABS) {
    grouped[tab.id] = evidence.filter((item) =>
      tab.sourceKinds.some((sourceKind) => sourceKind === item.source_kind)
    );
  }
  return grouped;
};

/** Return one independent state per registered tab for the shell. */
export const statesForEvidence = (state: EvidenceState): IncidentTabStates => {
  if (state.status !== "ready") {
    const states = {} as IncidentTabStates;
    for (const tab of INCIDENT_TABS) states[tab.id] = state;
    return states;
  }

  const grouped = partitionIncidentEvidence(state.evidence);
  const states = {} as IncidentTabStates;
  for (const tab of INCIDENT_TABS) {
    const selected = tab.select(grouped);
    states[tab.id] =
      selected.length > 0 ? { status: "ready", evidence: selected } : { status: "empty" };
  }
  return states;
};
