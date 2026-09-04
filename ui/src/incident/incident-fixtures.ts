// SPDX-License-Identifier: Apache-2.0

import type {
  BusinessImpact,
  EvidenceRef,
  ImpactDimensions,
  Incident,
  IncidentPage,
  IncidentSeverityOverride,
  IncidentTimelineEvent,
  IncidentTimelinePage,
  ResourceScope
} from "../../contracts/ipc";

/*
 * Every incident fixture shares the 2026-08-28 fixture day the correlation,
 * change and topology fixtures use, so a workspace assembled from all four
 * describes one afternoon rather than four unrelated ones.
 */
export const incidentFixtureClock = "2026-08-28T09:00:00Z" as const;

const ORGANIZATION = "11111111-1111-4111-8111-111111111111";
const TEAM = "22222222-2222-4222-8222-222222222222";
const WORKSPACE = "33333333-3333-4333-8333-333333333333";
const ACTOR = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const REQUEST = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";

export const incidentFixtureCheckoutId = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
export const incidentFixtureSearchId = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
export const incidentFixtureBillingId = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";

const CHECKOUT_TRIGGER = "ffffffff-ffff-4fff-8fff-ffffffffffff";
const SEARCH_TRIGGER = "0a000000-0000-4000-8000-00000000000a";
const BILLING_TRIGGER = "0b000000-0000-4000-8000-00000000000b";
const CHECKOUT_VULNERABILITY_EVIDENCE = "evidence-checkout-vulnerability";

const eventId = (ordinal: number) => `1a000000-0000-4000-8000-00000000000${ordinal}`;

const scope: ResourceScope = {
  organization_id: ORGANIZATION,
  team_id: TEAM,
  workspace_id: WORKSPACE,
  environment_id: null,
  resource_ids: []
};

const dimensions = (overrides: Partial<ImpactDimensions>): ImpactDimensions => ({
  availability: "none",
  customer_reach: "none",
  business_criticality: "none",
  data_integrity: "none",
  security_privacy: "none",
  financial_contractual: "none",
  trajectory: "stable",
  production: true,
  ...overrides
});

const checkoutImpact: BusinessImpact = {
  level: "critical",
  summary: "Checkout rejects every card payment",
  customer_scope: "all production customers",
  service_criticality: "tier-0",
  trajectory: "expanding",
  dimensions: dimensions({
    availability: "critical",
    customer_reach: "high",
    business_criticality: "high",
    financial_contractual: "medium",
    trajectory: "expanding"
  }),
  evidence_ids: ["evidence-checkout-error-rate"]
};

const checkoutInitialImpact: BusinessImpact = {
  level: "high",
  summary: "Checkout rejects some card payments",
  customer_scope: "customers paying by card",
  service_criticality: "tier-0",
  trajectory: "expanding",
  dimensions: dimensions({
    availability: "high",
    customer_reach: "medium",
    business_criticality: "high",
    trajectory: "expanding"
  }),
  evidence_ids: ["evidence-checkout-error-rate"]
};

const searchImpact: BusinessImpact = {
  level: "high",
  summary: "Search latency doubled after the index rebuild",
  customer_scope: "customers on the catalogue page",
  service_criticality: "tier-1",
  trajectory: "stable",
  dimensions: dimensions({
    availability: "high",
    customer_reach: "medium",
    business_criticality: "medium"
  }),
  evidence_ids: ["evidence-search-latency"]
};

const billingImpact: BusinessImpact = {
  level: "medium",
  summary: "Nightly invoice export retried twice",
  customer_scope: "internal finance operators",
  service_criticality: "tier-2",
  trajectory: "improving",
  dimensions: dimensions({
    availability: "medium",
    business_criticality: "low",
    trajectory: "improving"
  }),
  evidence_ids: ["evidence-billing-export"]
};

/*
 * The search incident carries an override so the fixtures exercise the
 * derived-versus-selected split: `derived` must equal the incident's own
 * derived severity, and `selected` must differ from it.
 */
const searchOverride: IncidentSeverityOverride = {
  derived: "S2",
  selected: "S1",
  actor_id: ACTOR,
  reason: "The catalogue page is the only entry point during the campaign",
  evidence_ids: ["evidence-search-override"]
};

const owner = (assignedAt: string) => [
  {
    role: "owner" as const,
    principal_id: ACTOR,
    assigned_by: ACTOR,
    assigned_at: assignedAt
  }
];

const checkoutIncident: Incident = {
  id: incidentFixtureCheckoutId,
  summary: "Checkout unavailable for customers",
  scope,
  owning_team_id: TEAM,
  business_impact: checkoutImpact,
  derived_severity: "S1",
  severity_override: null,
  status: "investigating",
  disposition: null,
  duplicate_of_incident_id: null,
  trigger_ids: [CHECKOUT_TRIGGER],
  signal_ids: [],
  evidence_ids: [
    "evidence-checkout-error-rate",
    "evidence-checkout-trace",
    CHECKOUT_VULNERABILITY_EVIDENCE
  ],
  hypothesis_ids: [],
  action_ids: [],
  roles: owner("2026-08-28T08:41:00Z"),
  version: 4,
  created_at: "2026-08-28T08:40:00Z",
  updated_at: "2026-08-28T09:00:00Z"
};

const searchIncident: Incident = {
  id: incidentFixtureSearchId,
  summary: "Search latency regression after the index rebuild",
  scope,
  owning_team_id: TEAM,
  business_impact: searchImpact,
  derived_severity: "S2",
  severity_override: searchOverride,
  status: "triage",
  disposition: null,
  duplicate_of_incident_id: null,
  trigger_ids: [SEARCH_TRIGGER],
  signal_ids: [],
  evidence_ids: ["evidence-search-latency", "evidence-search-override"],
  hypothesis_ids: [],
  action_ids: [],
  roles: owner("2026-08-28T08:31:00Z"),
  version: 2,
  created_at: "2026-08-28T08:30:00Z",
  updated_at: "2026-08-28T08:55:00Z"
};

const billingIncident: Incident = {
  id: incidentFixtureBillingId,
  summary: "Invoice export retried twice overnight",
  scope,
  owning_team_id: TEAM,
  business_impact: billingImpact,
  derived_severity: "S3",
  severity_override: null,
  status: "monitoring",
  disposition: null,
  duplicate_of_incident_id: null,
  trigger_ids: [BILLING_TRIGGER],
  signal_ids: [],
  evidence_ids: ["evidence-billing-export"],
  hypothesis_ids: [],
  action_ids: [],
  roles: owner("2026-08-28T08:21:00Z"),
  version: 3,
  created_at: "2026-08-28T08:20:00Z",
  updated_at: "2026-08-28T08:50:00Z"
};

/*
 * `format_cursor` renders the last item's `updated_at` with `to_rfc3339`, which
 * writes the `+00:00` offset rather than `Z`. The fixture keeps that form so a
 * hook that mangles the cursor cannot pass against a friendlier one.
 */
export const incidentFixtureCursor = `2026-08-28T08:50:00+00:00|${incidentFixtureBillingId}`;

export const incidentFixturePage: IncidentPage = {
  items: [checkoutIncident, searchIncident, billingIncident],
  next_cursor: incidentFixtureCursor
};

/**
 * The evidence the checkout incident's three identifiers resolve to. It carries
 * exactly those ids, in ascending order, because `isEvidenceResponse` rejects
 * any response that is not an exact cover of the request, and because the
 * domain validator rejects an unsorted request in the first place.
 */
export const incidentFixtureEvidence: EvidenceRef[] = [
  {
    id: "evidence-checkout-error-rate",
    source_kind: "prometheus",
    connector_id: "connector-prometheus",
    scope,
    endpoint: "fixture://prometheus/checkout",
    query: "sum(rate(checkout_errors_total[5m]))",
    observed_at: "2026-08-28T08:39:00Z",
    excerpt: "checkout error rate 41% over five minutes",
    native_url: "https://prometheus.fixture.internal/graph",
    redaction: {
      classification_verified: true,
      redaction_verified: true,
      masked: false,
      unparsed: false
    }
  },
  {
    id: "evidence-checkout-trace",
    source_kind: "fixture",
    connector_id: null,
    scope,
    endpoint: "fixture://trace/checkout",
    query: null,
    observed_at: "2026-08-28T08:39:30Z",
    excerpt: "checkout span fails at the payment provider call",
    native_url: null,
    redaction: {
      classification_verified: true,
      redaction_verified: true,
      masked: true,
      unparsed: false
    }
  },
  {
    id: CHECKOUT_VULNERABILITY_EVIDENCE,
    source_kind: "trivy",
    connector_id: null,
    scope,
    endpoint: "fixture://trivy/checkout",
    query: null,
    observed_at: "2026-08-28T08:38:30Z",
    excerpt: "checkout dependency contains a high-severity vulnerable package",
    native_url: null,
    redaction: {
      classification_verified: true,
      redaction_verified: true,
      masked: false,
      unparsed: false
    }
  }
];

const event = (
  ordinal: number,
  sequence: number,
  kind: IncidentTimelineEvent["kind"],
  payload: IncidentTimelineEvent["payload"],
  occurredAt: string
): IncidentTimelineEvent => ({
  id: eventId(ordinal),
  incident_id: incidentFixtureCheckoutId,
  sequence,
  kind,
  actor_id: ACTOR,
  reason: null,
  occurred_at: occurredAt,
  request_id: REQUEST,
  policy_version: 7,
  payload
});

/*
 * `next_sequence` is the sequence of the last event on the page, not the one
 * after it: the repository loads `WHERE sequence > ?`, so a resuming reader
 * sends this number back unchanged.
 */
export const incidentFixtureTimeline: IncidentTimelinePage = {
  incident_id: incidentFixtureCheckoutId,
  events: [
    event(
      1,
      1,
      "incident_created",
      {
        kind: "created",
        data: {
          summary: "Checkout unavailable for customers",
          scope,
          owning_team_id: TEAM,
          derived_severity: "S1",
          trigger_ids: [CHECKOUT_TRIGGER],
          initial_roles: owner("2026-08-28T08:41:00Z")
        }
      },
      "2026-08-28T08:40:00Z"
    ),
    event(
      2,
      2,
      "triggers_attached",
      { kind: "triggers_attached", data: { trigger_ids: [CHECKOUT_TRIGGER] } },
      "2026-08-28T08:41:00Z"
    ),
    event(
      3,
      3,
      "status_transitioned",
      {
        kind: "status_transitioned",
        data: {
          from: "detected",
          to: "triage",
          transition: {
            target: "triage",
            context: {
              business_impact: checkoutImpact,
              owner: ACTOR,
              duplicate_checked: true
            }
          }
        }
      },
      "2026-08-28T08:45:00Z"
    ),
    event(
      4,
      4,
      "status_transitioned",
      {
        kind: "status_transitioned",
        data: {
          from: "triage",
          to: "investigating",
          transition: {
            target: "investigating",
            context: {
              note: "Card authorisation calls time out at the gateway",
              evidence_ids: ["evidence-checkout-trace"]
            }
          }
        }
      },
      "2026-08-28T08:50:00Z"
    ),
    event(
      5,
      5,
      "severity_changed",
      {
        kind: "severity_changed",
        data: {
          previous_impact: checkoutInitialImpact,
          current_impact: checkoutImpact,
          previous_severity: "S2",
          current_severity: "S1",
          previous_override: null,
          current_override: null
        }
      },
      "2026-08-28T08:55:00Z"
    ),
    event(
      6,
      6,
      "commented",
      {
        kind: "commented",
        data: { body: "Payment provider confirms a regional outage on their side" }
      },
      incidentFixtureClock
    )
  ],
  next_sequence: 6
};
