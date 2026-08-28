import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it } from "vitest";
import { I18nProvider } from "../i18n";
import en from "../locales/en";
import th from "../locales/th";
import { TopologyWorkspace } from "./TopologyWorkspace";
import {
  topologyBrokenPathSnapshotFixture,
  topologyDegradedSnapshotFixture,
  topologyEmptySnapshotFixture,
  topologyIncidentSnapshotFixture,
  topologyIncidentsFixture,
  topologySnapshotFixture
} from "./topology-fixtures";

afterEach(() => {
  cleanup();
});

const renderWorkspace = (
  snapshot: Parameters<typeof TopologyWorkspace>[0]["snapshot"],
  incidents = topologyIncidentsFixture
) =>
  render(
    <I18nProvider>
      <TopologyWorkspace snapshot={snapshot} incidents={incidents} />
    </I18nProvider>
  );

const checkoutButton = () =>
  screen.getByRole("button", { name: "checkout, Service, Degraded, Platform" });

it("renders the healthy topology fixture with nodes, typed edges and confidence", () => {
  renderWorkspace(topologySnapshotFixture);

  expect(checkoutButton()).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /^GCP staging,/ })).toBeInTheDocument();
  expect(screen.getByRole("region", { name: "Relationships" })).toBeInTheDocument();
  expect(screen.getAllByText("depends on").length).toBeGreaterThan(0);
  expect(screen.getAllByText("contains").length).toBeGreaterThan(0);
  expect(screen.getAllByText("80%").length).toBeGreaterThan(0);
  expect(screen.getByText("unassigned owner")).toBeInTheDocument();
});

it("shows node detail for a keyboard-selected resource", async () => {
  const user = userEvent.setup();
  renderWorkspace(topologySnapshotFixture);

  checkoutButton().focus();
  await user.keyboard("{Enter}");

  const detail = screen.getByRole("complementary", { name: "Resource detail" });
  expect(within(detail).getByText("Service")).toBeInTheDocument();
  expect(within(detail).getByText("AWS production")).toBeInTheDocument();
  expect(within(detail).getByText("none")).toBeInTheDocument();
  expect(within(detail).getByText("Platform (explicit label)")).toBeInTheDocument();
  expect(within(detail).getByText("request_count: 1250")).toBeInTheDocument();
  expect(within(detail).getByRole("heading", { name: "checkout" })).toBeInTheDocument();
});

it("renders upstream and downstream probable paths from the focused resource", () => {
  renderWorkspace(topologySnapshotFixture);

  expect(screen.getByRole("heading", { name: "Impact from checkout" })).toBeInTheDocument();
  expect(screen.getAllByText("probable structural path").length).toBeGreaterThan(0);
  expect(screen.getByRole("heading", { name: "Upstream impact" })).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "Downstream impact" })).toBeInTheDocument();
  expect(
    screen.getByText("checkout → checkout-api → checkout-rds → orders-topic")
  ).toBeInTheDocument();
});

it("labels cycle-stopped and depth-truncated paths explicitly", () => {
  renderWorkspace(topologySnapshotFixture);
  expect(screen.getAllByText("ends here").length).toBeGreaterThan(0);
  expect(screen.getByText("stopped by a cycle")).toBeInTheDocument();
  expect(screen.getByText("truncated by depth limit")).toBeInTheDocument();
  expect(
    screen.getByText("More dependencies may exist beyond the requested depth.")
  ).toBeInTheDocument();
});

it("changes the rendered set when the environment filter changes", async () => {
  const user = userEvent.setup();
  renderWorkspace(topologySnapshotFixture);

  await user.selectOptions(
    screen.getByRole("combobox", { name: "Environment" }),
    "env-gcp-staging"
  );

  expect(screen.queryByText("checkout")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: /^staging-orders,/ })).toBeInTheDocument();
  expect(screen.getByText("No probable paths start from this selection.")).toBeInTheDocument();
});

it("changes the rendered set when the team filter changes and excludes unassigned owners", async () => {
  const user = userEvent.setup();
  renderWorkspace(topologySnapshotFixture);

  await user.selectOptions(
    screen.getByRole("combobox", { name: "Team" }),
    "11111111-1111-4111-8111-111111111111"
  );

  expect(checkoutButton()).toBeInTheDocument();
  expect(screen.queryByText("unassigned-worker")).not.toBeInTheDocument();
  expect(screen.queryByText("checkout-rds")).not.toBeInTheDocument();
  expect(screen.queryByText("staging-orders")).not.toBeInTheDocument();
});

it("narrows to affected resources and probable paths when an incident is selected", () => {
  renderWorkspace(topologyIncidentSnapshotFixture);

  expect(screen.getByText("affected by incident")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /^checkout,/ })).toBeInTheDocument();
  expect(screen.queryByText("staging-orders")).not.toBeInTheDocument();
  expect(screen.queryByText("payments-svc")).not.toBeInTheDocument();
  expect(
    screen.getByRole("heading", { name: "Impact from the selected incident" })
  ).toBeInTheDocument();
  expect(screen.getAllByText("probable structural path").length).toBeGreaterThan(0);
});

it("expands the blast radius again when the incident filter is cleared", async () => {
  const user = userEvent.setup();
  renderWorkspace(topologyIncidentSnapshotFixture);

  await user.selectOptions(screen.getByRole("combobox", { name: "Incident" }), "");

  expect(screen.getByRole("button", { name: /^staging-orders,/ })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /^payments-svc,/ })).toBeInTheDocument();
});

it("exposes an evidence drill-down affordance on every rendered node and path", () => {
  renderWorkspace(topologySnapshotFixture);

  const nodesRegion = screen.getByRole("region", { name: "Resources" });
  for (const item of within(nodesRegion).getAllByRole("listitem")) {
    expect(within(item).getByRole("button", { name: /View evidence for/ })).toBeInTheDocument();
  }

  const pathsRegion = screen.getByRole("region", { name: "Probable dependency paths" });
  for (const item of within(pathsRegion).getAllByRole("listitem")) {
    expect(within(item).getByRole("button", { name: /View evidence for/ })).toBeInTheDocument();
  }
});

it("resolves requested evidence in the evidence drawer", async () => {
  const user = userEvent.setup();
  renderWorkspace(topologySnapshotFixture);

  await user.click(screen.getByRole("button", { name: "View evidence for checkout" }));

  const drawer = screen.getByRole("dialog", { name: "Evidence" });
  expect(within(drawer).getByText("Evidence for checkout")).toBeInTheDocument();
  expect(within(drawer).getByText("Fixture")).toBeInTheDocument();
  expect(within(drawer).getByText("fixture://topology")).toBeInTheDocument();
  expect(within(drawer).getByText("fixture service checkout")).toBeInTheDocument();
});

it("keeps rendering when one source is unavailable", () => {
  renderWorkspace(topologyDegradedSnapshotFixture);

  expect(
    screen.getByText("kubernetes:env-gcp-staging is unavailable (unreachable).")
  ).toBeInTheDocument();
  expect(checkoutButton()).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /^checkout-rds,/ })).toBeInTheDocument();
  expect(screen.queryByText("staging-orders")).not.toBeInTheDocument();
});

it("shows an empty state when no topology data is available", () => {
  renderWorkspace(topologyEmptySnapshotFixture, []);

  expect(screen.getByText("No topology data is available for this workspace.")).toBeInTheDocument();
  expect(screen.getByText("No probable paths start from this selection.")).toBeInTheDocument();
});

it("shows a loading state while the snapshot is not available", () => {
  renderWorkspace(null, []);

  expect(screen.getAllByText("Loading topology…").length).toBeGreaterThan(0);
});

it("surfaces a per-path error and unavailable evidence for a broken snapshot", async () => {
  const user = userEvent.setup();
  renderWorkspace(topologyBrokenPathSnapshotFixture);

  expect(
    screen.getByText(
      "This path references resources that are not in the snapshot and cannot be shown."
    )
  ).toBeInTheDocument();
  expect(screen.getByText("stopped by a cycle")).toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "View evidence for this path" }));

  expect(screen.getByText("Evidence is unavailable for this selection.")).toBeInTheDocument();
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
