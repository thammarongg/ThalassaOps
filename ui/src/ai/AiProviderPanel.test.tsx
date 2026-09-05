import "@testing-library/jest-dom/vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup } from "@testing-library/react";
import type { IpcResult, Invoke, ProviderSummary } from "../../contracts/ipc";
import { I18nProvider } from "../i18n";
import { AiProviderPanel } from "./AiProviderPanel";
import { localProvider, unauthorizedHostedProvider } from "./ai-fixtures";

const renderPanel = (invoke: Invoke, providerOrder: string[] = []) =>
  render(
    <I18nProvider>
      <AiProviderPanel invoke={invoke} providerOrder={providerOrder} />
    </I18nProvider>
  );

afterEach(cleanup);

describe("AI provider panel", () => {
  it("renders credential presence, unauthorized health, and the real endpoint", async () => {
    const invoke = vi.fn().mockResolvedValue({
      ok: true,
      value: [unauthorizedHostedProvider]
    } satisfies IpcResult<ProviderSummary[]>);

    renderPanel(invoke as unknown as Invoke);

    expect(await screen.findByRole("article")).toHaveTextContent(unauthorizedHostedProvider.id);
    expect(screen.getByText("Unauthorized")).toBeInTheDocument();
    expect(screen.getByText(/credential configured/i)).toBeInTheDocument();
    expect(screen.getByText(unauthorizedHostedProvider.endpoint)).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith(
      "ai_providers",
      expect.objectContaining({
        envelope: expect.objectContaining({
          command: "ai.providers",
          capability: "ConnectorRead",
          payload: {}
        })
      })
    );
  });

  it("sends the reordered provider ids through the real order command", async () => {
    const invoke = vi.fn().mockImplementation((tauriCommand: string) =>
      tauriCommand === "ai_providers"
        ? Promise.resolve({
            ok: true,
            value: [unauthorizedHostedProvider, localProvider]
          } satisfies IpcResult<ProviderSummary[]>)
        : Promise.resolve({
            ok: true,
            value: ["ollama", "openai"]
          } satisfies IpcResult<string[]>)
    );

    renderPanel(invoke as unknown as Invoke, ["openai", "ollama"]);
    const providerRows = await screen.findAllByRole("article");
    expect(providerRows).toHaveLength(2);
    expect(providerRows[1]).toHaveTextContent(localProvider.endpoint);
    expect(providerRows[1]).toHaveTextContent("Unknown");
    screen.getByRole("button", { name: "Move ollama up" }).click();

    await waitFor(() => {
      const orderCall = invoke.mock.calls.find(([command]) => command === "ai_set_provider_order");
      expect(orderCall).toBeDefined();
      expect(orderCall?.[1]).toEqual({
        envelope: expect.objectContaining({
          command: "ai.set_provider_order",
          capability: "ConnectorAct",
          payload: { provider_order: ["ollama", "openai"] }
        })
      });
    });
  });
});
