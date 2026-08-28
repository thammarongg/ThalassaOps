import { expect, it } from "vitest";
import { isScope } from "../../contracts/guards";
import { isTopologySnapshot } from "./contractValidation";
import { topologySnapshotFixture } from "./topology-fixtures";

it("accepts the null optional scope fields emitted by Rust serde", () => {
  expect(
    isScope({
      organization_id: null,
      team_id: null,
      workspace_id: null,
      environment_id: null,
      resource_ids: []
    })
  ).toBe(true);
});

it("rejects relation drill-downs that do not open evidence", () => {
  const snapshot = structuredClone(topologySnapshotFixture);
  snapshot.edges[0].drill_down = {
    ...snapshot.edges[0].drill_down,
    destination: "topology"
  };

  expect(isTopologySnapshot(snapshot)).toBe(false);
});

it("rejects paths whose node and edge sequences do not match", () => {
  const snapshot = structuredClone(topologySnapshotFixture);
  snapshot.paths[0].node_ids = [snapshot.paths[0].root_node_id];

  expect(isTopologySnapshot(snapshot)).toBe(false);
});

it("rejects paths that omit evidence for a listed node or edge", () => {
  const snapshot = structuredClone(topologySnapshotFixture);
  snapshot.paths[0].evidence_ids = snapshot.paths[0].evidence_ids.slice(1);
  snapshot.paths[0].drill_down.evidence_ids = snapshot.paths[0].evidence_ids;

  expect(isTopologySnapshot(snapshot)).toBe(false);
});

it("rejects empty optional topology text fields", () => {
  const nodeSnapshot = structuredClone(topologySnapshotFixture);
  nodeSnapshot.nodes[0].provider = "";
  expect(isTopologySnapshot(nodeSnapshot)).toBe(false);

  const evidenceSnapshot = structuredClone(topologySnapshotFixture);
  evidenceSnapshot.evidence[0].query = "";
  expect(isTopologySnapshot(evidenceSnapshot)).toBe(false);
});

it("rejects credential-bearing native evidence links", () => {
  const snapshot = structuredClone(topologySnapshotFixture);
  snapshot.evidence[0].native_url = "https://evidence.example.test/item?token=opaque";

  expect(isTopologySnapshot(snapshot)).toBe(false);
});

it("rejects rendered nodes without evidence or a paired owner", () => {
  const missingEvidence = structuredClone(topologySnapshotFixture);
  missingEvidence.nodes[0].evidence_ids = [];
  expect(isTopologySnapshot(missingEvidence)).toBe(false);

  const mismatchedOwner = structuredClone(topologySnapshotFixture);
  mismatchedOwner.nodes[0].ownership.team_id = null;
  expect(isTopologySnapshot(mismatchedOwner)).toBe(false);
});

it("rejects graph references that are outside the emitted snapshot", () => {
  expect(isTopologySnapshot(topologySnapshotFixture)).toBe(true);
  const snapshot = structuredClone(topologySnapshotFixture);
  snapshot.edges[0].upstream_node_id = "node:missing";
  expect(isTopologySnapshot(snapshot)).toBe(false);

  const invalidFocus = structuredClone(topologySnapshotFixture);
  invalidFocus.focus_node_id = "node:missing";
  expect(isTopologySnapshot(invalidFocus)).toBe(false);
});

it("rejects summary counts that disagree with the graph", () => {
  const snapshot = structuredClone(topologySnapshotFixture);
  snapshot.summary.visible_nodes.value += 1;

  expect(isTopologySnapshot(snapshot)).toBe(false);
});

it("keeps the UI fixture identity catalog aligned with the Rust topology fixture", () => {
  const expectedNodeIds = [
    "node:cloud:env-aws-prod:cloud_resource:checkout-rds",
    "node:cloud:env-aws-prod:cloud_resource:checkout-rds-replica",
    "node:cloud:env-gcp-staging:cluster:catalog-cluster",
    "node:fixture:env-aws-prod:environment:env-aws-prod",
    "node:fixture:env-gcp-staging:environment:env-gcp-staging",
    "node:kubernetes:env-aws-prod:namespace:uid-namespace-prod",
    "node:kubernetes:env-aws-prod:node:uid-node-worker-a",
    "node:kubernetes:env-aws-prod:pod:uid-pod-checkout-api-0",
    "node:kubernetes:env-aws-prod:service:uid-service-checkout",
    "node:kubernetes:env-aws-prod:workload:uid-workload-checkout-api",
    "node:kubernetes:env-aws-prod:workload:uid-workload-unassigned-worker",
    "node:kubernetes:env-gcp-staging:namespace:uid-namespace-staging",
    "node:kubernetes:env-gcp-staging:pod:uid-pod-catalog-api-0",
    "node:kubernetes:env-gcp-staging:service:uid-service-catalog",
    "node:kubernetes:env-gcp-staging:workload:uid-workload-catalog-api"
  ];

  expect(topologySnapshotFixture.nodes.map((node) => node.id).sort()).toEqual(expectedNodeIds);
  expect(topologySnapshotFixture.edges).toHaveLength(20);
  expect(topologySnapshotFixture.evidence).toHaveLength(26);
  expect(topologySnapshotFixture.paths).toHaveLength(4);
  expect(topologySnapshotFixture.nodes.map((node) => node.name)).not.toContain("orders-topic");
});
