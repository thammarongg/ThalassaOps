import "@testing-library/jest-dom/vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup } from "@testing-library/react";
import type { IpcResult, Invoke, ProviderSummary } from "../../contracts/ipc";
import { I18nProvider } from "../i18n";
import { AiProviderPanel } from "./AiProviderPanel";
import { localProvider, unauthorizedHostedProvider } from "./ai-fixtures";

const renderPanel = (invoke: Invoke, providerOrder?: string[]) =>
  render(
    <I18nProvider>
      <AiProviderPanel
        invoke={invoke}
        {...(providerOrder === undefined ? {} : { providerOrder })}
      />
    </I18nProvider>
  );

afterEach(cleanup);

describe("AI provider panel", () => {
  it("renders credential presence, unauthorized health, and the real endpoint", async () => {
    const invoke = vi.fn().mockImplementation((tauriCommand: string) =>
      tauriCommand === "ai_providers"
        ? Promise.resolve({
            ok: true,
            value: [unauthorizedHostedProvider]
          } satisfies IpcResult<ProviderSummary[]>)
        : Promise.resolve({ ok: true, value: [] } satisfies IpcResult<string[]>)
    );

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
    expect(invoke).toHaveBeenCalledWith(
      "ai_provider_order",
      expect.objectContaining({
        envelope: expect.objectContaining({
          command: "ai.provider_order",
          capability: "ConnectorRead",
          payload: {}
        })
      })
    );
  });

  it("sends the reordered provider ids through the real order command", async () => {
    const invoke = vi.fn().mockImplementation((tauriCommand: string) => {
      if (tauriCommand === "ai_providers") {
        return Promise.resolve({
          ok: true,
          value: [unauthorizedHostedProvider, localProvider]
        } satisfies IpcResult<ProviderSummary[]>);
      }
      if (tauriCommand === "ai_provider_order") {
        return Promise.resolve({
          ok: true,
          value: ["openai", "ollama"]
        } satisfies IpcResult<string[]>);
      }
      return Promise.resolve({
        ok: true,
        value: ["ollama", "openai"]
      } satisfies IpcResult<string[]>);
    });

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

  it("reads the configured order before adding another provider", async () => {
    const user = userEvent.setup();
    const thirdProvider: ProviderSummary = {
      ...localProvider,
      id: "vllm",
      kind: "vllm",
      endpoint: "http://model-host.example.test:8000/v1/chat/completions"
    };
    const invoke = vi
      .fn()
      .mockImplementation((tauriCommand: string, args?: { envelope?: { payload?: unknown } }) => {
        if (tauriCommand === "ai_providers") {
          return Promise.resolve({
            ok: true,
            value: [unauthorizedHostedProvider, localProvider, thirdProvider]
          } satisfies IpcResult<ProviderSummary[]>);
        }
        if (tauriCommand === "ai_provider_order") {
          return Promise.resolve({ ok: true, value: ["openai", "ollama"] } satisfies IpcResult<
            string[]
          >);
        }
        const payload = args?.envelope?.payload as { provider_order: string[] };
        return Promise.resolve({ ok: true, value: payload.provider_order } satisfies IpcResult<
          string[]
        >);
      });

    renderPanel(invoke as unknown as Invoke);

    const fallbackRegion = await screen.findByRole("region", { name: "Fallback order" });
    await waitFor(() => {
      const ordered = within(fallbackRegion).getAllByRole("list")[0];
      expect(ordered.children[0]).toHaveTextContent("openai");
      expect(ordered.children[1]).toHaveTextContent("ollama");
    });

    await user.click(
      within(fallbackRegion).getByRole("button", { name: "Add vllm to fallback order" })
    );
    await waitFor(() => {
      const orderCall = invoke.mock.calls.find(([command]) => command === "ai_set_provider_order");
      expect(orderCall?.[1]).toEqual({
        envelope: expect.objectContaining({
          payload: { provider_order: ["openai", "ollama", "vllm"] }
        })
      });
    });
  });
});
