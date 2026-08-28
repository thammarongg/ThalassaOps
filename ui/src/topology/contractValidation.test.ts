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
