import type {
  ChangeKind,
  ChangeSnapshot,
  EvidenceSourceKind,
  ResourceScope
} from "../../contracts/ipc";

export const changeFixtureClock = "2026-08-29T09:00:00Z" as const;

export const changeSourceKinds: EvidenceSourceKind[] = ["github", "gitlab", "argo_cd"];

export const changeKindWireValues: ChangeKind[] = [
  "deployment",
  "configuration",
  "maintenance",
  "connector",
  "code_commit",
  "code_merge",
  "sync",
  "rollback"
];

export const precedingChangeReason = "preceding_change" as const;

const scope: ResourceScope = {
  organization_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
  team_id: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
  workspace_id: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
  environment_id: null,
  resource_ids: []
};

const evidenceId = "evidence-change-event-fixture";
const drillDown = {
  destination: "evidence" as const,
  evidence_ids: [evidenceId],
  filter_key: "change-event-fixture"
};
const drillDownReference = {
  source_query: "github change event fixture",
  scope,
  time_window: null,
  evidence_ids: [evidenceId]
};

/** Typed copy of the canonical change contract used by frontend tests. */
export const changeSnapshotFixture: ChangeSnapshot = {
  generated_at: changeFixtureClock,
  scope,
  request_window: {
    start: "2026-08-29T08:00:00Z",
    end: changeFixtureClock
  },
  lookback_seconds: 3600,
  events: [
    {
      id: "11111111-1111-4111-8111-111111111111",
      source: "github",
      kind: "code_commit",
      outcome: "succeeded",
      occurred_at: "2026-08-29T08:45:00Z",
      ingested_at: "2026-08-29T08:45:05Z",
      scope,
      targets: [{ kind: "deployment", id: "deployment/checkout-api" }],
      revision: null,
      actor: { kind: "human", handle: "release-engineer" },
      repository: null,
      environment: null,
      diff_stat: null,
      changed_paths: [],
      source_link: null,
      source_record: {
        source_kind: "github",
        native_id: "push-2026-08-29-0845",
        revision: null,
        content_digest: "sha256:fixture-change-event",
        evidence_ids: [evidenceId]
      },
      evidence_ids: [evidenceId],
      drill_down: drillDown,
      drill_down_reference: drillDownReference
    }
  ],
  timeline: {
    window: {
      start: "2026-08-29T08:00:00Z",
      end: changeFixtureClock
    },
    entry_ids: ["11111111-1111-4111-8111-111111111111"],
    truncated: false
  },
  associations: [],
  metrics: [
    {
      key: "changes_in_window",
      source: null,
      value: 1,
      unit: "count",
      evidence_ids: [evidenceId],
      drill_down: drillDown,
      drill_down_reference: drillDownReference
    },
    {
      key: "associated_changes",
      source: null,
      value: 0,
      unit: "count",
      evidence_ids: [evidenceId],
      drill_down: drillDown,
      drill_down_reference: drillDownReference
    },
    {
      key: "changes_by_source",
      source: "github",
      value: 1,
      unit: "count",
      evidence_ids: [evidenceId],
      drill_down: drillDown,
      drill_down_reference: drillDownReference
    }
  ],
  source_statuses: [
    {
      source_key: "github",
      state: "fresh",
      reason: null,
      detail: null,
      observed_at: changeFixtureClock,
      evidence_ids: [evidenceId]
    }
  ]
};
