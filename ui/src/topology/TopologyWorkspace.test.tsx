import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { open } from "@tauri-apps/plugin-shell";
import type {
  CommandEnvelope,
  ConsoleEvidenceId,
  EvidenceRef,
  IpcError,
  IpcResult,
  IncidentQueueItem,
  Invoke,
  OperationsSnapshot,
  TopologyRequest,
  TopologySnapshot
} from "../../contracts/ipc";
import { I18nProvider, i18n } from "../i18n";
import en from "../locales/en";
import th from "../locales/th";
import { TopologyWorkspace } from "./TopologyWorkspace";
import {
  topologyDegradedSnapshotFixture,
  topologyEmptySnapshotFixture,
  topologyIncidentSnapshotFixture,
  topologyIncidentsFixture,
  topologySnapshotFixture
} from "./topology-fixtures";
import { operationsSnapshotFor } from "./operations-queue-fixtures";

vi.mock("@tauri-apps/plugin-shell", () => ({ open: vi.fn().mockResolvedValue(undefined) }));

afterEach(() => {
  cleanup();
  localStorage.clear();
  void i18n.changeLanguage("en");
});

const scope = { resource_ids: [] };
const observedAt = "2026-08-28T09:00:00Z";
const checkoutIncidentId = topologyIncidentsFixture[0].id;

/**
 * The mocked `Invoke` used across these tests: the contract callable plus
 * vitest's call log for asserting command envelopes and payloads.
 */
type TopologyInvokeMock = Invoke & {
  mock: { calls: [string, { envelope: CommandEnvelope<unknown> }][] };
};

const ok = <T,>(value: T): IpcResult<T> => ({ ok: true, value });

const ipcError = (message: string): IpcError => ({
  code: "INTERNAL_ERROR",
  message,
  details: {}
});

const evidenceFor = (id: ConsoleEvidenceId): EvidenceRef => ({
  id,
  source_kind: "fixture",
  connector_id: null,
  scope,
  endpoint: "fixture://topology",
  query: "topology:snapshot",
  observed_at: observedAt,
  excerpt: `${id} admitted excerpt`,
  native_url: null,
  redaction: {
    classification_verified: true,
    redaction_verified: true,
    masked: false,
    unparsed: false
  }
});

const withGraphCounts = (
  snapshot: TopologySnapshot,
  nodes: TopologySnapshot["nodes"],
  edges: TopologySnapshot["edges"],
  paths: TopologySnapshot["paths"]
): TopologySnapshot => ({
  ...snapshot,
  nodes,
  edges,
  paths,
  summary: {
    ...snapshot.summary,
    visible_nodes: { ...snapshot.summary.visible_nodes, value: nodes.length },
    visible_edges: { ...snapshot.summary.visible_edges, value: edges.length },
    affected_nodes: {
      ...snapshot.summary.affected_nodes,
      value: nodes.filter((node) => node.affected_by_incident).length
    },
    probable_paths: { ...snapshot.summary.probable_paths, value: paths.length }
  }
});

/** The unfiltered response: a full graph with no focus and therefore no paths. */
const unfocusedSnapshot = (): TopologySnapshot =>
  withGraphCounts(
    { ...topologySnapshotFixture, focus_node_id: null },
    topologySnapshotFixture.nodes,
    topologySnapshotFixture.edges,
    []
  );

/** Incident-filtered response: the blast radius and its probable paths. */
const incidentBlastRadiusSnapshot = (): TopologySnapshot => {
  const base = topologyIncidentSnapshotFixture;
  const pathNodeIds = new Set(base.paths.flatMap((path) => path.node_ids));
  const nodes = base.nodes.filter((node) => node.affected_by_incident || pathNodeIds.has(node.id));
  const nodeIds = new Set(nodes.map((node) => node.id));
  const edges = base.edges.filter(
    (edge) => nodeIds.has(edge.upstream_node_id) && nodeIds.has(edge.downstream_node_id)
  );
  return withGraphCounts({ ...base, focus_node_id: null }, nodes, edges, base.paths);
};

const restrictSnapshotTo = (
  predicate: (node: TopologySnapshot["nodes"][number]) => boolean
): TopologySnapshot => {
  const base = topologySnapshotFixture;
  const nodes = base.nodes.filter(predicate);
  const nodeIds = new Set(nodes.map((node) => node.id));
  const edges = base.edges.filter(
    (edge) => nodeIds.has(edge.upstream_node_id) && nodeIds.has(edge.downstream_node_id)
  );
  return withGraphCounts({ ...base, focus_node_id: null }, nodes, edges, []);
};

const defaultSnapshotFor = (request: TopologyRequest): TopologySnapshot => {
  if (request.filter.incident_id) return incidentBlastRadiusSnapshot();
  if (request.focus_node_id) return topologySnapshotFixture;
  return unfocusedSnapshot();
};

const topologyInvoke = ({
  snapshotFor = defaultSnapshotFor,
  snapshotResult,
  evidenceFor: evidenceHandler,
  incidents = topologyIncidentsFixture,
  incidentsResult
}: {
  snapshotFor?: (request: TopologyRequest) => TopologySnapshot;
  snapshotResult?: () => IpcResult<TopologySnapshot>;
  evidenceFor?: (ids: ConsoleEvidenceId[]) => IpcResult<EvidenceRef[]>;
  incidents?: IncidentQueueItem[];
  incidentsResult?: () => IpcResult<OperationsSnapshot>;
} = {}) =>
  vi.fn().mockImplementation((name: string, args: { envelope: CommandEnvelope<unknown> }) => {
    if (name === "operations_snapshot") {
      return Promise.resolve(incidentsResult?.() ?? ok(operationsSnapshotFor(incidents)));
    }
    if (name === "topology_snapshot") {
      if (snapshotResult) return Promise.resolve(snapshotResult());
      return Promise.resolve(ok(snapshotFor(args.envelope.payload as TopologyRequest)));
    }
    if (name === "topology_evidence") {
      const { evidence_ids } = args.envelope.payload as { evidence_ids: ConsoleEvidenceId[] };
      return Promise.resolve(evidenceHandler?.(evidence_ids) ?? ok([]));
    }
    return Promise.resolve(ok({}));
  }) as unknown as TopologyInvokeMock;

const renderWorkspace = (invoke: TopologyInvokeMock, initialIncidentId?: string) =>
  render(
    <I18nProvider>
      <TopologyWorkspace invoke={invoke} initialIncidentId={initialIncidentId} />
    </I18nProvider>
  );

const topologySnapshotCalls = (invoke: TopologyInvokeMock) =>
  invoke.mock.calls
    .filter(([name]) => name === "topology_snapshot")
    .map(([, args]) => args as { envelope: CommandEnvelope<TopologyRequest> });

const checkoutButton = () =>
  screen.getByRole("button", { name: "checkout, Service, Unknown, Platform" });

it("renders the workspace from the topology snapshot IPC command", async () => {
  const invoke = topologyInvoke();
  renderWorkspace(invoke);

  expect(await screen.findByRole("heading", { name: "Resource topology" })).toBeInTheDocument();
  expect(checkoutButton()).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /^GCP staging,/ })).toBeInTheDocument();
  expect(screen.getByRole("region", { name: "Relationships" })).toBeInTheDocument();
  expect(screen.getAllByText("depends on").length).toBeGreaterThan(0);
  expect(screen.getAllByText("contains").length).toBeGreaterThan(0);
  expect(screen.getAllByText("80%").length).toBeGreaterThan(0);
  expect(screen.getByText("unassigned owner")).toBeInTheDocument();
  expect(screen.getByText("Last sync: 2026-08-28T09:00:00Z")).toBeInTheDocument();

  const calls = topologySnapshotCalls(invoke);
  expect(calls.length).toBeGreaterThan(0);
  expect(calls[0].envelope.command).toBe("topology.snapshot");
  expect(calls[0].envelope.capability).toBe("WorkspaceRead");
  expect(calls[0].envelope.payload).toEqual({
    filter: { environment_ids: [], team_ids: [], incident_id: null },
    focus_node_id: null,
    traversal: { direction: "both", max_depth: 3 }
  });
  expect(invoke).toHaveBeenCalledWith(
    "operations_snapshot",
    expect.objectContaining({
      envelope: expect.objectContaining({
        command: "operations.snapshot",
        capability: "WorkspaceRead"
      })
    })
  );
});

it("shows a loading state until the first snapshot resolves", () => {
  const pendingSnapshot = Promise.race<IpcResult<TopologySnapshot>>([]);
  const invoke = vi.fn().mockImplementation((name: string) => {
    if (name === "operations_snapshot")
      return Promise.resolve(ok(operationsSnapshotFor(topologyIncidentsFixture)));
    if (name === "topology_snapshot") return pendingSnapshot;
    return Promise.resolve(ok({}));
  });
  renderWorkspace(invoke as unknown as TopologyInvokeMock);

  expect(screen.getAllByText("Loading topology…").length).toBeGreaterThan(0);
  expect(screen.queryByRole("button", { name: /^checkout,/ })).not.toBeInTheDocument();
});

it("surfaces a localized error state instead of a blank view when the snapshot IPC fails", async () => {
  const invoke = topologyInvoke({
    snapshotResult: () => ({ ok: false, error: ipcError("denied") })
  });
  renderWorkspace(invoke);

  const alerts = await screen.findAllByRole("alert");
  expect(alerts.length).toBeGreaterThan(0);
  for (const alert of alerts) {
    expect(alert).toHaveTextContent("The topology snapshot is unavailable.");
  }
  expect(screen.queryByRole("button", { name: /^checkout,/ })).not.toBeInTheDocument();

  await i18n.changeLanguage("th");
  await waitFor(() =>
    expect(screen.getAllByRole("alert")[0]).toHaveTextContent("ไม่สามารถโหลดภาพรวมโทโพโลยีได้")
  );
});
it("rejects a contract-invalid snapshot with the error state", async () => {
  const invoke = topologyInvoke({
    snapshotResult: () => ok({ ...topologySnapshotFixture, evidence: [] })
  });
  renderWorkspace(invoke);

  const alerts = await screen.findAllByRole("alert");
  expect(alerts.length).toBeGreaterThan(0);
  expect(alerts[0]).toHaveTextContent("The topology snapshot is unavailable.");
});

it("clears the previous graph when a later snapshot response is invalid", async () => {
  const user = userEvent.setup();
  let snapshotCalls = 0;
  const invoke = topologyInvoke({
    snapshotResult: () => {
      snapshotCalls += 1;
      return snapshotCalls <= 2
        ? ok(unfocusedSnapshot())
        : ok({ ...topologySnapshotFixture, evidence: [] });
    }
  });
  renderWorkspace(invoke);

  expect(await screen.findByRole("button", { name: /^checkout,/ })).toBeInTheDocument();
  await user.selectOptions(screen.getByRole("combobox", { name: "Direction" }), "upstream");

  await waitFor(() => {
    expect(screen.queryByRole("button", { name: /^checkout,/ })).not.toBeInTheDocument();
  });
  expect(screen.getAllByRole("alert")[0]).toHaveTextContent(
    "The topology snapshot is unavailable."
  );
});

it("shows node detail for a selected resource and re-reads traversal through IPC", async () => {
  const user = userEvent.setup();
  const invoke = topologyInvoke();
  renderWorkspace(invoke);

  await user.click(
    await screen.findByRole("button", { name: "checkout, Service, Unknown, Platform" })
  );

  const detail = await screen.findByRole("complementary", { name: "Resource detail" });
  expect(within(detail).getByRole("heading", { name: "checkout" })).toBeInTheDocument();
  expect(within(detail).getByText("AWS production")).toBeInTheDocument();
  expect(within(detail).getByText("Platform (explicit label)")).toBeInTheDocument();

  const lastCall = topologySnapshotCalls(invoke).at(-1);
  expect(lastCall?.envelope.payload).toMatchObject({
    focus_node_id: "node:kubernetes:env-aws-prod:service:uid-service-checkout"
  });
  expect(await screen.findByRole("heading", { name: "Impact from checkout" })).toBeInTheDocument();
});

it("renders upstream and downstream probable paths from the focused resource", async () => {
  const user = userEvent.setup();
  const invoke = topologyInvoke();
  renderWorkspace(invoke);

  await user.click(
    await screen.findByRole("button", { name: "checkout, Service, Unknown, Platform" })
  );

  expect(await screen.findByRole("heading", { name: "Upstream impact" })).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "Downstream impact" })).toBeInTheDocument();
  expect(screen.getAllByText("probable structural path").length).toBeGreaterThan(0);
  expect(
    screen.getByText("checkout → checkout-api → checkout-rds → checkout-rds-replica")
  ).toBeInTheDocument();
});

it("renders a keyboard-reachable relationship map and edge detail", async () => {
  const user = userEvent.setup();
  const invoke = topologyInvoke({
    evidenceFor: (ids) => ok(ids.map(evidenceFor))
  });
  renderWorkspace(invoke);

  const visual = await screen.findByRole("region", { name: "Visual relationships" });
  const edgeButton = within(visual).getByRole("button", {
    name: "Select relationship: checkout depends on checkout-api"
  });
  expect(edgeButton).toHaveAttribute("aria-pressed", "false");

  await user.click(edgeButton);

  expect(edgeButton).toHaveAttribute("aria-pressed", "true");
  const detail = await screen.findByRole("complementary", { name: "Relationship detail" });
  expect(within(detail).getByText("depends on")).toBeInTheDocument();
  expect(within(detail).getByText("checkout → checkout-api")).toBeInTheDocument();

  await user.click(
    within(detail).getByRole("button", {
      name: "View evidence for the relationship between checkout and checkout-api"
    })
  );
  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith(
      "topology_evidence",
      expect.objectContaining({
        envelope: expect.objectContaining({
          payload: { evidence_ids: ["evidence-topology-edge-checkout-api"] }
        })
      })
    )
  );
});

it("shows typed edge provenance and edge sequences for probable paths", async () => {
  const user = userEvent.setup();
  renderWorkspace(topologyInvoke());

  await user.click(
    await screen.findByRole("button", { name: "checkout, Service, Unknown, Platform" })
  );

  const relationships = await screen.findByRole("region", { name: "Relationships" });
  expect(within(relationships).getAllByText("fixture:topology").length).toBeGreaterThan(0);

  const paths = screen.getByRole("region", { name: "Probable dependency paths" });
  expect(within(paths).getAllByText("Edge sequence").length).toBeGreaterThan(0);
  expect(within(paths).getAllByText("Provenance").length).toBeGreaterThan(0);
  expect(within(paths).getAllByText("fixture:topology").length).toBeGreaterThan(0);
  expect(within(paths).getByText(/checkout-rds-replica depends on checkout-rds/)).toBeInTheDocument();
});

it("renders bidirectional probable paths in their own group", async () => {
  const user = userEvent.setup();
  const bothDirectionSnapshot = withGraphCounts(
    { ...topologySnapshotFixture, focus_node_id: null },
    topologySnapshotFixture.nodes,
    topologySnapshotFixture.edges,
    [{ ...topologySnapshotFixture.paths[0], direction: "both" }]
  );
  const invoke = topologyInvoke({
    snapshotFor: (request) => (request.focus_node_id ? bothDirectionSnapshot : unfocusedSnapshot())
  });
  renderWorkspace(invoke);

  await user.click(
    await screen.findByRole("button", { name: "checkout, Service, Unknown, Platform" })
  );

  expect(
    await screen.findByRole("heading", { name: "Upstream and downstream" })
  ).toBeInTheDocument();
  expect(screen.getByText("checkout → prod → AWS production")).toBeInTheDocument();
});

it("labels cycle-stopped paths explicitly", async () => {
  const user = userEvent.setup();
  renderWorkspace(topologyInvoke());

  await user.click(
    await screen.findByRole("button", { name: "checkout, Service, Unknown, Platform" })
  );

  expect(await screen.findByText("stopped by a cycle")).toBeInTheDocument();
  expect(screen.getAllByText("ends here").length).toBeGreaterThan(0);
});

it("re-reads the graph through IPC when the environment filter changes", async () => {
  const user = userEvent.setup();
  const invoke = topologyInvoke({
    snapshotFor: (request) =>
      request.filter.environment_ids.length
        ? restrictSnapshotTo((node) => node.environment_id === "env-gcp-staging")
        : defaultSnapshotFor(request)
  });
  renderWorkspace(invoke);

  await user.selectOptions(
    await screen.findByRole("combobox", { name: "Environment" }),
    "env-gcp-staging"
  );

  await waitFor(() =>
    expect(topologySnapshotCalls(invoke).at(-1)?.envelope.payload).toMatchObject({
      filter: { environment_ids: ["env-gcp-staging"] }
    })
  );
  expect(await screen.findByRole("button", { name: /^catalog,/ })).toBeInTheDocument();
  expect(screen.queryByText("checkout")).not.toBeInTheDocument();
  expect(screen.getByText("No probable paths start from this selection.")).toBeInTheDocument();
});

it("clears a focus node when an environment filter removes it", async () => {
  const user = userEvent.setup();
  const invoke = topologyInvoke({
    snapshotFor: (request) =>
      request.filter.environment_ids.length
        ? restrictSnapshotTo((node) => node.environment_id === "env-gcp-staging")
        : defaultSnapshotFor(request)
  });
  renderWorkspace(invoke);

  await user.click(
    await screen.findByRole("button", { name: "checkout, Service, Unknown, Platform" })
  );
  expect(await screen.findByRole("heading", { name: "Impact from checkout" })).toBeInTheDocument();

  await user.selectOptions(
    await screen.findByRole("combobox", { name: "Environment" }),
    "env-gcp-staging"
  );

  await waitFor(() =>
    expect(topologySnapshotCalls(invoke).at(-1)?.envelope.payload).toMatchObject({
      filter: { environment_ids: ["env-gcp-staging"] },
      focus_node_id: null
    })
  );
  expect(screen.queryByRole("complementary", { name: "Resource detail" })).not.toBeInTheDocument();
});

it("re-reads the graph through IPC when the team filter changes", async () => {
  const user = userEvent.setup();
  const platformTeam = "00000000-0000-0000-0000-000000000013";
  const invoke = topologyInvoke({
    snapshotFor: (request) =>
      request.filter.team_ids.length
        ? restrictSnapshotTo((node) => node.ownership.team_id === platformTeam)
        : defaultSnapshotFor(request)
  });
  renderWorkspace(invoke);

  await user.selectOptions(await screen.findByRole("combobox", { name: "Team" }), platformTeam);

  await waitFor(() =>
    expect(topologySnapshotCalls(invoke).at(-1)?.envelope.payload).toMatchObject({
      filter: { team_ids: [platformTeam] }
    })
  );
  expect(await screen.findByRole("button", { name: /^checkout,/ })).toBeInTheDocument();
  expect(screen.queryByText("unassigned-worker")).not.toBeInTheDocument();
  expect(screen.getAllByText("checkout-rds").length).toBeGreaterThan(0);
  expect(screen.getAllByText("catalog").length).toBeGreaterThan(0);
});

it("re-reads the graph through IPC when traversal direction and depth change", async () => {
  const user = userEvent.setup();
  const invoke = topologyInvoke();
  renderWorkspace(invoke);

  await user.selectOptions(
    await screen.findByRole("combobox", { name: "Direction" }),
    "downstream"
  );
  await user.selectOptions(await screen.findByRole("combobox", { name: "Maximum depth" }), "5");

  await waitFor(() =>
    expect(topologySnapshotCalls(invoke).at(-1)?.envelope.payload).toMatchObject({
      traversal: { direction: "downstream", max_depth: 5 }
    })
  );
});

it("allows a zero traversal depth to show the selected graph without paths", async () => {
  const user = userEvent.setup();
  const invoke = topologyInvoke();
  renderWorkspace(invoke);

  await user.selectOptions(await screen.findByRole("combobox", { name: "Maximum depth" }), "0");

  await waitFor(() =>
    expect(topologySnapshotCalls(invoke).at(-1)?.envelope.payload).toMatchObject({
      traversal: { max_depth: 0 }
    })
  );
});

it("narrows to the incident blast radius when an incident is selected", async () => {
  const user = userEvent.setup();
  const invoke = topologyInvoke();
  renderWorkspace(invoke);

  await user.selectOptions(
    await screen.findByRole("combobox", { name: "Incident" }),
    checkoutIncidentId
  );

  await waitFor(() =>
    expect(topologySnapshotCalls(invoke).at(-1)?.envelope.payload).toMatchObject({
      filter: { incident_id: checkoutIncidentId }
    })
  );
  expect(
    await screen.findByRole("heading", { name: "Impact from the selected incident" })
  ).toBeInTheDocument();
  expect(screen.getByText("affected by incident")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /^checkout,/ })).toBeInTheDocument();
  expect(screen.queryByText("catalog")).not.toBeInTheDocument();
  expect(screen.getAllByText("probable structural path").length).toBeGreaterThan(0);
});

it("expands the blast radius again when the incident filter is cleared", async () => {
  const user = userEvent.setup();
  const invoke = topologyInvoke();
  renderWorkspace(invoke, checkoutIncidentId);

  expect(
    await screen.findByRole("heading", { name: "Impact from the selected incident" })
  ).toBeInTheDocument();
  expect(
    topologySnapshotCalls(invoke).some(
      (call) => call.envelope.payload.filter.incident_id === checkoutIncidentId
    )
  ).toBe(true);

  await user.selectOptions(screen.getByRole("combobox", { name: "Incident" }), "");

  await waitFor(() =>
    expect(topologySnapshotCalls(invoke).at(-1)?.envelope.payload).toMatchObject({
      filter: { incident_id: null }
    })
  );
  expect(await screen.findByRole("button", { name: /^catalog,/ })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /^checkout-rds,/ })).toBeInTheDocument();
});

it("mounts directly into the incident view when opened from the Operations Console", async () => {
  renderWorkspace(topologyInvoke(), checkoutIncidentId);

  expect(
    await screen.findByRole("heading", { name: "Impact from the selected incident" })
  ).toBeInTheDocument();
  expect(screen.getByText("affected by incident")).toBeInTheDocument();
  expect(screen.getByRole("combobox", { name: "Incident" })).toHaveValue(checkoutIncidentId);
});

it("opens node evidence through the topology evidence command with backend-issued ids", async () => {
  const user = userEvent.setup();
  const invoke = topologyInvoke({
    evidenceFor: (ids) => ok(ids.map(evidenceFor))
  });
  renderWorkspace(invoke);

  await user.click(await screen.findByRole("button", { name: "View evidence for checkout" }));

  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith(
      "topology_evidence",
      expect.objectContaining({
        envelope: expect.objectContaining({
          command: "topology.evidence",
          capability: "ResourceRead",
          payload: {
            evidence_ids: [
              "evidence-topology-alert-checkout",
              "evidence-topology-k8s-service-checkout",
              "evidence-topology-metric-checkout"
            ]
          }
        })
      })
    )
  );
  const drawer = await screen.findByRole("dialog", { name: "Evidence" });
  expect(within(drawer).getByText("Evidence for checkout")).toBeInTheDocument();
  expect(
    within(drawer).getByText("evidence-topology-alert-checkout admitted excerpt")
  ).toBeInTheDocument();
  expect(within(drawer).getAllByText("fixture://topology").length).toBeGreaterThan(0);
  expect(within(drawer).getAllByText("topology:snapshot").length).toBeGreaterThan(0);
  expect(within(drawer).getAllByText("Fixture").length).toBeGreaterThan(0);
  expect(within(drawer).getAllByRole("status").length).toBeGreaterThan(0);
  expect(within(drawer).getAllByRole("status")[0]).toHaveTextContent("No fields masked");
  expect(within(drawer).getAllByRole("status")[0]).toHaveTextContent("parsed");
});

it("opens only the backend-issued trusted native evidence URL", async () => {
  const user = userEvent.setup();
  const invoke = topologyInvoke({
    evidenceFor: (ids) =>
      ok(
        ids.map((id) => ({ ...evidenceFor(id), native_url: "https://evidence.example.test/item" }))
      )
  });
  renderWorkspace(invoke);

  await user.click(await screen.findByRole("button", { name: "View evidence for checkout" }));
  const drawer = await screen.findByRole("dialog", { name: "Evidence" });
  await user.click(within(drawer).getAllByRole("button", { name: "Open trusted source" })[0]);

  expect(open).toHaveBeenCalledWith("https://evidence.example.test/item");
});

it("opens impact path evidence with only the ids the snapshot issued", async () => {
  const user = userEvent.setup();
  const invoke = topologyInvoke({
    evidenceFor: (ids) => ok(ids.map(evidenceFor))
  });
  renderWorkspace(invoke);

  await user.click(
    await screen.findByRole("button", { name: "checkout, Service, Unknown, Platform" })
  );
  const expectedIds = topologySnapshotFixture.paths[0].evidence_ids;
  await user.click(
    await screen.findByRole("button", {
      name: `View evidence for the path checkout → prod → AWS production`
    })
  );

  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith(
      "topology_evidence",
      expect.objectContaining({
        envelope: expect.objectContaining({
          command: "topology.evidence",
          payload: { evidence_ids: expectedIds }
        })
      })
    )
  );
  const drawer = await screen.findByRole("dialog", { name: "Evidence" });
  for (const id of expectedIds) {
    expect(within(drawer).getByText(`${id} admitted excerpt`)).toBeInTheDocument();
  }
});

it("surfaces a localized error when the evidence command fails", async () => {
  const user = userEvent.setup();
  const invoke = topologyInvoke({
    evidenceFor: () => ({ ok: false, error: ipcError("evidence denied") })
  });
  renderWorkspace(invoke);

  await user.click(await screen.findByRole("button", { name: "View evidence for checkout" }));

  const drawer = await screen.findByRole("dialog", { name: "Evidence" });
  expect(
    await within(drawer).findByText("Evidence could not be loaded for this selection.")
  ).toBeInTheDocument();
});

it("exposes an evidence affordance on every rendered node and impact path", async () => {
  const user = userEvent.setup();
  renderWorkspace(topologyInvoke());

  await user.click(
    await screen.findByRole("button", { name: "checkout, Service, Unknown, Platform" })
  );
  await screen.findByRole("heading", { name: "Upstream impact" });

  const nodesRegion = await screen.findByRole("region", { name: "Resources" });
  for (const item of within(nodesRegion).getAllByRole("listitem")) {
    expect(within(item).getByRole("button", { name: /View evidence for/ })).toBeInTheDocument();
  }

  const pathsRegion = screen.getByRole("region", { name: "Probable dependency paths" });
  for (const item of within(pathsRegion).getAllByRole("listitem")) {
    expect(within(item).getByRole("button", { name: /View evidence for/ })).toBeInTheDocument();
  }
});

it("keeps rendering when one source is unavailable", async () => {
  renderWorkspace(topologyInvoke({ snapshotResult: () => ok(topologyDegradedSnapshotFixture) }));

  expect(
    await screen.findByText("kubernetes:env-gcp-staging is unavailable (unreachable).")
  ).toBeInTheDocument();
  expect(await screen.findByRole("button", { name: /^checkout-rds,/ })).toBeInTheDocument();
  expect(screen.queryByText("catalog")).not.toBeInTheDocument();
});

it("shows an empty state when no topology data is available", async () => {
  renderWorkspace(topologyInvoke({ snapshotResult: () => ok(topologyEmptySnapshotFixture) }));

  expect(
    await screen.findByText("No topology data is available for this workspace.")
  ).toBeInTheDocument();
  expect(screen.getByText("No probable paths start from this selection.")).toBeInTheDocument();
});

it("notes when the incident filter list is unavailable without blocking the graph", async () => {
  renderWorkspace(
    topologyInvoke({ incidentsResult: () => ({ ok: false, error: ipcError("queue denied") }) })
  );

  expect(await screen.findByText("The incident filter list is unavailable.")).toBeInTheDocument();
  expect(await screen.findByRole("button", { name: /^checkout,/ })).toBeInTheDocument();
});

it("keeps the topology catalog structurally identical in English and Thai", () => {
  const keyPaths = (value: unknown, prefix = ""): string[] =>
    Object.entries(value as Record<string, unknown>).flatMap(([key, inner]) =>
      typeof inner === "object" && inner !== null
        ? keyPaths(inner, `${prefix}${key}.`)
        : [`${prefix}${key}`]
    );
  expect(keyPaths(th.topology).sort()).toEqual(keyPaths(en.topology).sort());
});
