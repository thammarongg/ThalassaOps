import type {
  ChangeKind,
  ChangeSnapshot,
  EvidenceSourceKind,
  ResourceScope
} from "../../contracts/ipc";

export const changeFixtureClock = "2026-08-28T09:00:00Z" as const;

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

const mergeEvidenceId = "evidence-change-merge-fixture";
const mergeDrillDown = {
  destination: "evidence" as const,
  evidence_ids: [mergeEvidenceId],
  filter_key: "change-merge-fixture"
};
const mergeDrillDownReference = {
  source_query: "gitlab change event fixture",
  scope,
  time_window: null,
  evidence_ids: [mergeEvidenceId]
};

/** Typed copy of the canonical change contract used by frontend tests. */
export const changeSnapshotFixture: ChangeSnapshot = {
  generated_at: changeFixtureClock,
  scope,
  request_window: {
    start: "2026-08-28T08:00:00Z",
    end: changeFixtureClock
  },
  lookback_seconds: 3600,
  events: [
    {
      id: "11111111-1111-4111-8111-111111111111",
      source: "github",
      kind: "code_commit",
      outcome: "succeeded",
      occurred_at: "2026-08-28T08:45:00Z",
      ingested_at: "2026-08-28T08:45:05Z",
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
    },
    {
      id: "22222222-2222-4222-8222-222222222222",
      source: "gitlab",
      kind: "code_merge",
      outcome: "succeeded",
      occurred_at: "2026-08-28T08:50:00Z",
      ingested_at: null,
      scope,
      targets: [{ kind: "deployment", id: "deployment/checkout" }],
      revision: {
        id: "9f2c41ab6d7e5c8039ba4712fd6e1c53a807b6d4",
        short_id: "9f2c41a",
        parent_ids: ["4b71ce90d2a83f16bb5d0e47ca9328f1de60b7a5"]
      },
      actor: { kind: "automation", handle: "release-bot" },
      repository: {
        host: "gitlab.example",
        namespace: "storefront",
        name: "checkout",
        reference: "refs/heads/main"
      },
      environment: "prod",
      diff_stat: { files_changed: 3, insertions: 41, deletions: 12, unit: "count" },
      changed_paths: ["services/checkout/handler.rs", "services/checkout/config.toml"],
      source_link: {
        kind: "pull_request",
        url: "https://gitlab.example/storefront/checkout/-/merge_requests/128"
      },
      source_record: {
        source_kind: "gitlab",
        native_id: "merge-request-128",
        revision: "9f2c41ab6d7e5c8039ba4712fd6e1c53a807b6d4",
        content_digest: "sha256:fixture-change-merge",
        evidence_ids: [mergeEvidenceId]
      },
      evidence_ids: [mergeEvidenceId],
      drill_down: mergeDrillDown,
      drill_down_reference: mergeDrillDownReference
    }
  ],
  timeline: {
    window: {
      start: "2026-08-28T08:00:00Z",
      end: changeFixtureClock
    },
    entry_ids: ["11111111-1111-4111-8111-111111111111", "22222222-2222-4222-8222-222222222222"],
    truncated: false
  },
  associations: [
    {
      change_id: "22222222-2222-4222-8222-222222222222",
      candidate_id: "candidate-checkout",
      qualification: "probable_structural",
      lead_time_seconds: 360,
      target: { kind: "deployment", id: "deployment/checkout" },
      topology_path_ids: [],
      evidence_ids: [mergeEvidenceId]
    }
  ],
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
      value: 1,
      unit: "count",
      evidence_ids: [mergeEvidenceId],
      drill_down: mergeDrillDown,
      drill_down_reference: mergeDrillDownReference
    },
    {
      key: "changes_by_source",
      source: "github",
      value: 1,
      unit: "count",
      evidence_ids: [evidenceId],
      drill_down: drillDown,
      drill_down_reference: drillDownReference
    },
    {
      key: "changes_by_source",
      source: "gitlab",
      value: 1,
      unit: "count",
      evidence_ids: [mergeEvidenceId],
      drill_down: mergeDrillDown,
      drill_down_reference: mergeDrillDownReference
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
    },
    {
      source_key: "gitlab",
      state: "fresh",
      reason: null,
      detail: null,
      observed_at: changeFixtureClock,
      evidence_ids: [mergeEvidenceId]
    }
  ]
};
