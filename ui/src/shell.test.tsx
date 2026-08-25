import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { I18nProvider } from "./i18n";
import { Shell } from "./shell";

const context = {
  organization_name: "Local Organization",
  team_name: "Local Team",
  workspace_name: "Local Workspace",
  policy_version: 1
};
afterEach(() => {
  cleanup();
  localStorage.clear();
});

it("navigates product areas from the command palette with keyboard and closes it with Escape", async () => {
  const user = userEvent.setup();
  render(
    <I18nProvider>
      <Shell invoke={vi.fn().mockResolvedValue({ ok: true, value: context })} />
    </I18nProvider>
  );
  await user.keyboard("{Control>}k{/Control}");
  const input = await screen.findByRole("textbox", { name: "Command palette" });
  await user.type(input, "inc");
  await user.keyboard("{Enter}");
  expect(screen.getByRole("heading", { name: "Incidents" })).toBeInTheDocument();
  await user.keyboard("{Meta>}k{/Meta}{Escape}");
  expect(screen.queryByRole("dialog", { name: "Command palette" })).not.toBeInTheDocument();
});

it("pins a navigation item and opens the honest terminal placeholder", async () => {
  const user = userEvent.setup();
  render(
    <I18nProvider>
      <Shell invoke={vi.fn().mockResolvedValue({ ok: true, value: context })} />
    </I18nProvider>
  );
  await user.click(screen.getByRole("button", { name: "Pin Incidents" }));
  expect(screen.getByRole("navigation", { name: "Favorites" })).toHaveTextContent("Incidents");
  await user.click(screen.getByRole("button", { name: "Open terminal" }));
  expect(screen.getByRole("dialog", { name: "Terminal" })).toHaveTextContent("not yet available");
  await user.click(screen.getByRole("button", { name: "Open external terminal" }));
  expect(screen.getByRole("status")).toHaveTextContent("not yet available");
});

it("shows an unavailable policy indicator and context error when the context request is denied", async () => {
  render(
    <I18nProvider>
      <Shell
        invoke={vi.fn().mockResolvedValue({
          ok: false,
          error: {
            code: "POLICY_DENIED",
            message: "Policy denied the workspace context request.",
            details: {}
          }
        })}
      />
    </I18nProvider>
  );

  await screen.findByRole("button", {
    name: "Organization: Workspace context is unavailable."
  });
  const policyStatus = screen.getByText("Policy version …").parentElement;
  expect(policyStatus?.querySelector(".indicator")).toHaveClass("indicator--unavailable");
});

it("adds and tests a fixture connector through the integrations IPC commands", async () => {
  const user = userEvent.setup();
  const connector = { id: "fixture-1", kind: "fixture", display_name: "Fixture connector", enabled: true, config_metadata: {}, credential_configured: false, health_state: "healthy" };
  const invoke = vi.fn().mockImplementation((name: string) => {
    if (name === "system_context") return Promise.resolve({ ok: true, value: context });
    if (name === "connector_list") return Promise.resolve({ ok: true, value: invoke.mock.calls.some(([command]) => command === "connector_add") ? [connector] : [] });
    if (name === "connector_add" || name === "connector_test") return Promise.resolve({ ok: true, value: connector });
    return Promise.resolve({ ok: true, value: { connector, manifest: { capabilities: [] }, logs: [] } });
  });
  render(<I18nProvider><Shell invoke={invoke} /></I18nProvider>);
  await user.click(screen.getByRole("button", { name: "Integrations" }));
  await user.click(await screen.findByRole("button", { name: "Add fixture connector" }));
  expect(invoke).toHaveBeenCalledWith("connector_add", expect.objectContaining({ envelope: expect.objectContaining({ command: "connector.add", capability: "ConnectorAct" }) }));
  await user.click(await screen.findByRole("button", { name: "Test connection" }));
  expect(invoke).toHaveBeenCalledWith("connector_test", expect.objectContaining({ envelope: expect.objectContaining({ command: "connector.test", capability: "ConnectorAct" }) }));
});

it("filters Kubernetes resources by health and shows a masked manifest banner", async () => {
  const user = userEvent.setup();
  const connector = { id: "k8s-1", kind: "kubernetes", display_name: "Cluster", enabled: true, config_metadata: { context_name: "test" }, credential_configured: false, health_state: "healthy" };
  const inventory = { availability: [], topology: [], resources: [
    { resource: { kind: "Pod", name: "prod/crashing", labels: {} }, conditions: [], containers: [], health: "crash_loop_back_off" },
    { resource: { kind: "Service", name: "stage/web", labels: {} }, conditions: [], containers: [], health: "healthy" }
  ] };
  const invoke = vi.fn().mockImplementation((name: string) => {
    if (name === "system_context") return Promise.resolve({ ok: true, value: context });
    if (name === "connector_list") return Promise.resolve({ ok: true, value: [connector] });
    if (name === "kubernetes_inventory") return Promise.resolve({ ok: true, value: inventory });
    if (name === "kubernetes_pod_logs") return Promise.resolve({ ok: true, value: "logs" });
    if (name === "kubernetes_pod_events") return Promise.resolve({ ok: true, value: [] });
    if (name === "kubernetes_resource_manifest") return Promise.resolve({ ok: true, value: { yaml: "token: <REDACTED>", masked: true, risk_class: "READ-ONLY" } });
    return Promise.resolve({ ok: true, value: {} });
  });
  render(<I18nProvider><Shell invoke={invoke} /></I18nProvider>);
  await user.click(screen.getByRole("button", { name: "Integrations" }));
  await user.click(await screen.findByRole("button", { name: "Inspect cluster" }));
  expect(await screen.findByText("crash_loop_back_off")).toBeInTheDocument();
  await user.selectOptions(screen.getByLabelText("Health"), "crash_loop_back_off");
  expect(screen.getByText("Pod/prod/crashing")).toBeInTheDocument();
  expect(screen.queryByText("Service/stage/web")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Pod/prod/crashing" }));
  await user.click(screen.getByRole("button", { name: "View manifest" }));
  expect(await screen.findByRole("status")).toHaveTextContent("Sensitive fields redacted");
});
