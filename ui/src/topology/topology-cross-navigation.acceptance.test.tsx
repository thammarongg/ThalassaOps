import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type {
  CommandEnvelope,
  TopologyRequest,
  TopologySnapshot,
  Invoke
} from "../../contracts/ipc";
import { I18nProvider } from "../i18n";
import { Shell } from "../shell";
import { operationsSnapshotFor } from "./operations-queue-fixtures";
import {
  topologyIncidentSnapshotFixture,
  topologyIncidentsFixture,
  topologySnapshotFixture
} from "./topology-fixtures";

vi.mock("@tauri-apps/plugin-shell", () => ({ open: vi.fn().mockResolvedValue(undefined) }));

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const context = {
  organization_name: "Local Organization",
  team_name: "Local Team",
  workspace_name: "Local Workspace",
  policy_version: 1
};

const checkoutIncidentId = topologyIncidentsFixture[0].id;

/** The unfiltered response: a full graph with no focus and therefore no paths. */
const unfocusedSnapshot = (): TopologySnapshot => ({
  ...topologySnapshotFixture,
  focus_node_id: null,
  paths: []
});

/** Incident-filtered response: the blast radius and its probable paths. */
const incidentBlastRadiusSnapshot = (): TopologySnapshot => {
  const base = topologyIncidentSnapshotFixture;
  const pathNodeIds = new Set(base.paths.flatMap((path) => path.node_ids));
  const nodes = base.nodes.filter((node) => node.affected_by_incident || pathNodeIds.has(node.id));
  const nodeIds = new Set(nodes.map((node) => node.id));
  const edges = base.edges.filter(
    (edge) => nodeIds.has(edge.upstream_node_id) && nodeIds.has(edge.downstream_node_id)
  );
  return { ...base, nodes, edges, focus_node_id: null };
};

type ShellInvokeMock = Invoke & {
  mock: { calls: [string, { envelope: CommandEnvelope<unknown> }][] };
};

const shellInvoke = (): ShellInvokeMock =>
  vi.fn().mockImplementation((name: string, args: { envelope: CommandEnvelope<unknown> }) => {
    if (name === "system_context") return Promise.resolve({ ok: true, value: context });
    if (name === "operations_snapshot")
      return Promise.resolve({
        ok: true,
        value: operationsSnapshotFor(topologyIncidentsFixture)
      });
    if (name === "topology_snapshot") {
      const request = args.envelope.payload as TopologyRequest;
      if (request.filter.incident_id) {
        return Promise.resolve({ ok: true, value: incidentBlastRadiusSnapshot() });
      }
      if (request.focus_node_id) {
        return Promise.resolve({ ok: true, value: topologySnapshotFixture });
      }
      return Promise.resolve({ ok: true, value: unfocusedSnapshot() });
    }
    return Promise.resolve({ ok: true, value: {} });
  }) as unknown as ShellInvokeMock;

const topologySnapshotCalls = (invoke: ShellInvokeMock) =>
  invoke.mock.calls
    .filter(([name]) => name === "topology_snapshot")
    .map(([, args]) => args as { envelope: CommandEnvelope<TopologyRequest> });

it("opens an incident from the Operations Console into the filtered topology view", async () => {
  const user = userEvent.setup();
  const invoke = shellInvoke();
  render(
    <I18nProvider>
      <Shell invoke={invoke} />
    </I18nProvider>
  );

  expect(await screen.findByText("Checkout latency breach")).toBeInTheDocument();
  await user.click(
    screen.getByRole("button", {
      name: /dependency paths for Checkout latency breach in topology/
    })
  );

  expect(await screen.findByRole("button", { name: /^checkout,/ })).toBeInTheDocument();

  await waitFor(() =>
    expect(
      topologySnapshotCalls(invoke).some(
        (call) =>
          call.envelope.command === "topology.snapshot" &&
          call.envelope.payload.filter.incident_id === checkoutIncidentId
      )
    ).toBe(true)
  );
  expect(
    await screen.findByRole("heading", { name: "Impact from the selected incident" })
  ).toBeInTheDocument();
  expect(screen.getByText("affected by incident")).toBeInTheDocument();
  expect(screen.queryByText("staging-orders")).not.toBeInTheDocument();
  expect(screen.getByRole("combobox", { name: "Incident" })).toHaveValue(checkoutIncidentId);
  expect(screen.getAllByText("probable structural path").length).toBeGreaterThan(0);
});

it("navigates to the topology area from the shell without an incident filter", async () => {
  const user = userEvent.setup();
  const invoke = shellInvoke();
  render(
    <I18nProvider>
      <Shell invoke={invoke} />
    </I18nProvider>
  );

  await user.click(await screen.findByRole("button", { name: "Resource topology" }));

  expect(await screen.findByRole("heading", { name: "Resource topology" })).toBeInTheDocument();
  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith(
      "topology_snapshot",
      expect.objectContaining({
        envelope: expect.objectContaining({
          command: "topology.snapshot",
          capability: "WorkspaceRead",
          payload: {
            filter: { environment_ids: [], team_ids: [], incident_id: null },
            focus_node_id: null,
            traversal: { direction: "both", max_depth: 3 }
          }
        })
      })
    )
  );
  expect(screen.getByRole("button", { name: /^GCP staging,/ })).toBeInTheDocument();
  expect(screen.getByRole("combobox", { name: "Incident" })).toHaveValue("");
  expect(screen.queryByText("affected by incident")).not.toBeInTheDocument();
});
