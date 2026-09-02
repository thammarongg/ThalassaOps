// SPDX-License-Identifier: Apache-2.0

import { describe, expect, test } from "vitest";
import {
  isIncident,
  isIncidentBusinessImpact,
  isIncidentTimelinePage,
  isIncidentTriggerInput
} from "../../contracts/guards";
import type {
  BusinessImpact,
  ImpactDimensions,
  Incident,
  IncidentSeverityOverride,
  IncidentTimelineEvent,
  IncidentTimelinePage,
  IncidentTriggerInput,
  ResourceScope
} from "../../contracts/ipc";

const ORGANIZATION = "11111111-1111-4111-8111-111111111111";
const TEAM = "22222222-2222-4222-8222-222222222222";
const WORKSPACE = "33333333-3333-4333-8333-333333333333";
const ACTOR = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const REQUEST = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const INCIDENT = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
const TRIGGER = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
const EVENT_ONE = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
const EVENT_TWO = "ffffffff-ffff-4fff-8fff-ffffffffffff";
const AT = "2026-08-30T09:00:00Z";

const scope: ResourceScope = {
  organization_id: ORGANIZATION,
  team_id: TEAM,
  workspace_id: WORKSPACE,
  environment_id: null,
  resource_ids: []
};

const dimensions: ImpactDimensions = {
  availability: "high",
  customer_reach: "medium",
  business_criticality: "medium",
  data_integrity: "none",
  security_privacy: "none",
  financial_contractual: "low",
  trajectory: "stable",
  production: true
};

const businessImpact: BusinessImpact = {
  level: "high",
  summary: "Checkout unavailable",
  customer_scope: "production customers",
  service_criticality: "tier-0",
  trajectory: "stable",
  dimensions,
  evidence_ids: ["evidence-checkout"]
};

const override: IncidentSeverityOverride = {
  derived: "S2",
  selected: "S1",
  actor_id: ACTOR,
  reason: "worse than assessed",
  evidence_ids: ["evidence-override"]
};

const incidentFixture: Incident = {
  id: INCIDENT,
  summary: "Checkout unavailable for customers",
  scope,
  owning_team_id: TEAM,
  business_impact: businessImpact,
  derived_severity: "S2",
  severity_override: override,
  status: "detected",
  disposition: null,
  duplicate_of_incident_id: null,
  trigger_ids: [TRIGGER],
  signal_ids: [],
  evidence_ids: ["evidence-checkout", "evidence-manual-report", "evidence-override"],
  hypothesis_ids: [],
  action_ids: [],
  roles: [{ role: "owner", principal_id: ACTOR, assigned_by: ACTOR, assigned_at: AT }],
  version: 1,
  created_at: AT,
  updated_at: AT
};

const timelineEvent = (
  id: string,
  sequence: number,
  kind: IncidentTimelineEvent["kind"],
  payload: IncidentTimelineEvent["payload"]
): IncidentTimelineEvent => ({
  id,
  incident_id: INCIDENT,
  sequence,
  kind,
  actor_id: ACTOR,
  reason: null,
  occurred_at: AT,
  request_id: REQUEST,
  policy_version: 7,
  payload
});

const timelineFixture: IncidentTimelinePage = {
  incident_id: INCIDENT,
  events: [
    timelineEvent(EVENT_ONE, 1, "incident_created", {
      kind: "created",
      data: {
        summary: "Checkout unavailable for customers",
        scope,
        owning_team_id: TEAM,
        derived_severity: "S2",
        trigger_ids: [TRIGGER],
        initial_roles: []
      }
    }),
    timelineEvent(EVENT_TWO, 2, "triggers_attached", {
      kind: "triggers_attached",
      data: { trigger_ids: [TRIGGER] }
    })
  ],
  next_sequence: 3
};

describe("incident wire guards", () => {
  test("accepts the canonical incident and timeline fixtures", () => {
    expect(isIncident(incidentFixture)).toBe(true);
    expect(isIncidentTimelinePage(timelineFixture)).toBe(true);
  });

  test("incident guards reject unknown status", () => {
    expect(isIncident({ ...incidentFixture, status: "acknowledged" })).toBe(false);
  });

  test("incident timeline guards reject unordered events", () => {
    expect(
      isIncidentTimelinePage({
        incident_id: incidentFixture.id,
        events: [timelineFixture.events[1], timelineFixture.events[0]],
        next_sequence: null
      })
    ).toBe(false);
  });

  test("incident guards reject unknown keys and malformed identifiers", () => {
    expect(isIncident({ ...incidentFixture, unexpected: true })).toBe(false);
    expect(
      isIncident({
        ...incidentFixture,
        id: "00000000-0000-0000-0000-000000000000"
      })
    ).toBe(false);
    expect(isIncident({ ...incidentFixture, version: 1.5 })).toBe(false);
    expect(isIncidentTimelinePage({ ...timelineFixture, unexpected: true })).toBe(false);
  });

  test("severity payloads carry explicit override state before and after", () => {
    const severityEvent = timelineEvent(EVENT_ONE, 3, "severity_changed", {
      kind: "severity_changed",
      data: {
        previous_impact: businessImpact,
        current_impact: {
          ...businessImpact,
          level: "none",
          dimensions: {
            ...dimensions,
            availability: "none",
            customer_reach: "none",
            business_criticality: "none",
            financial_contractual: "none"
          },
          evidence_ids: ["evidence-recheck"]
        },
        previous_severity: "S2",
        current_severity: "S5",
        previous_override: override,
        current_override: null
      }
    });
    expect(
      isIncidentTimelinePage({
        incident_id: INCIDENT,
        events: [severityEvent],
        next_sequence: null
      })
    ).toBe(true);

    const ambiguousOverride = severityEvent.payload as Record<string, unknown>;
    const data = ambiguousOverride.data as Record<string, unknown>;
    delete data.previous_override;
    delete data.current_override;
    data.override_detail = override;
    expect(
      isIncidentTimelinePage({
        incident_id: INCIDENT,
        events: [severityEvent],
        next_sequence: null
      })
    ).toBe(false);
  });

  test("trigger inputs accept exactly the six supported kinds", () => {
    const sourceBacked: IncidentTriggerInput[] = [
      { kind: "alert", source_id: "alert-checkout" },
      { kind: "anomaly", source_id: "anomaly-checkout" },
      { kind: "scheduled_health_check", source_id: "check-checkout" },
      { kind: "vulnerability_finding", source_id: "finding-checkout" }
    ];
    for (const input of sourceBacked) {
      expect(isIncidentTriggerInput(input)).toBe(true);
    }
    const userReport: IncidentTriggerInput = {
      kind: "user_report",
      reporter_id: ACTOR,
      observed_at: AT,
      summary: "customers report checkout failures",
      scope
    };
    expect(isIncidentTriggerInput(userReport)).toBe(true);
    const manualReport: IncidentTriggerInput = {
      kind: "manual_report",
      observed_at: AT,
      summary: "checkout is returning errors",
      scope
    };
    expect(isIncidentTriggerInput(manualReport)).toBe(true);
    expect(isIncidentTriggerInput({ kind: "correlation_candidate", source_id: "x" })).toBe(false);
    expect(isIncidentTriggerInput({ kind: "alert" })).toBe(false);
  });
  test("timeline events must match the page incident and payload kinds", () => {
    const foreignIncident = timelineEvent(EVENT_TWO, 2, "triggers_attached", {
      kind: "triggers_attached",
      data: { trigger_ids: [TRIGGER] }
    });
    expect(
      isIncidentTimelinePage({
        incident_id: INCIDENT,
        events: [{ ...foreignIncident, incident_id: ACTOR }],
        next_sequence: null
      })
    ).toBe(false);

    const kindMismatch = timelineEvent(EVENT_TWO, 2, "triggers_attached", {
      kind: "created",
      data: {
        summary: "mismatched",
        scope,
        owning_team_id: TEAM,
        derived_severity: "S2",
        trigger_ids: [TRIGGER],
        initial_roles: []
      }
    });
    expect(
      isIncidentTimelinePage({
        incident_id: INCIDENT,
        events: [kindMismatch],
        next_sequence: null
      })
    ).toBe(false);
  });

  test("status payloads must target the transition target", () => {
    const statusEvent = timelineEvent(EVENT_ONE, 1, "status_transitioned", {
      kind: "status_transitioned",
      data: {
        from: "detected",
        to: "resolved",
        transition: {
          target: "triage",
          context: {
            business_impact: businessImpact,
            owner: ACTOR,
            duplicate_checked: true
          }
        }
      }
    });
    expect(
      isIncidentTimelinePage({
        incident_id: INCIDENT,
        events: [statusEvent],
        next_sequence: null
      })
    ).toBe(false);
  });

  test("incidents enforce cross-field ownership severity and closure", () => {
    expect(isIncident({ ...incidentFixture, owning_team_id: ACTOR })).toBe(false);
    expect(isIncident({ ...incidentFixture, derived_severity: "S3" })).toBe(false);
    expect(
      isIncident({
        ...incidentFixture,
        severity_override: { ...override, derived: "S3" }
      })
    ).toBe(false);
    expect(
      isIncident({
        ...incidentFixture,
        evidence_ids: ["evidence-manual-report", "evidence-override"]
      })
    ).toBe(false);
    expect(
      isIncident({
        ...incidentFixture,
        disposition: "duplicate",
        duplicate_of_incident_id: null
      })
    ).toBe(false);
    expect(
      isIncident({
        ...incidentFixture,
        disposition: null,
        duplicate_of_incident_id: ACTOR
      })
    ).toBe(false);
  });

  test("evidence ids accept order independent uniqueness with scalar bounds", () => {
    const unsorted = {
      ...businessImpact,
      evidence_ids: ["evidence-b", "evidence-a"]
    };
    expect(isIncidentBusinessImpact(unsorted)).toBe(true);

    const scalarBound = {
      ...businessImpact,
      evidence_ids: ["é".repeat(101)]
    };
    expect(isIncidentBusinessImpact(scalarBound)).toBe(true);

    const oversize = {
      ...businessImpact,
      evidence_ids: ["x".repeat(201)]
    };
    expect(isIncidentBusinessImpact(oversize)).toBe(false);

    const whitespace = {
      ...businessImpact,
      evidence_ids: ["evi dence"]
    };
    expect(isIncidentBusinessImpact(whitespace)).toBe(false);

    const duplicated = {
      ...businessImpact,
      evidence_ids: ["evidence-checkout", "evidence-checkout"]
    };
    expect(isIncidentBusinessImpact(duplicated)).toBe(false);
  });
});
