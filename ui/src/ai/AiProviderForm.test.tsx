import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup } from "@testing-library/react";
import type { IpcResult, Invoke } from "../../contracts/ipc";
import { I18nProvider } from "../i18n";
import { AiProviderForm } from "./AiProviderForm";
import { unauthorizedHostedProvider } from "./ai-fixtures";

afterEach(cleanup);

describe("AI provider form", () => {
  it("never renders the write-only credential and omits it when unchanged", async () => {
    const invoke = vi.fn().mockResolvedValue({
      ok: true,
      value: unauthorizedHostedProvider
    } satisfies IpcResult<typeof unauthorizedHostedProvider>);

    render(
      <I18nProvider>
        <AiProviderForm
          invoke={invoke as unknown as Invoke}
          provider={unauthorizedHostedProvider}
        />
      </I18nProvider>
    );

    expect(screen.queryByDisplayValue("test-key-not-real")).not.toBeInTheDocument();
    expect(screen.queryByText("test-key-not-real")).not.toBeInTheDocument();
    fireEvent.submit(screen.getByRole("button", { name: /save/i }).closest("form")!);

    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
    const [tauriCommand, args] = invoke.mock.calls[0] as [
      string,
      { envelope: { command: string; capability: string; payload: Record<string, unknown> } }
    ];
    expect(tauriCommand).toBe("ai_configure_provider");
    expect(args.envelope.command).toBe("ai.configure_provider");
    expect(args.envelope.capability).toBe("ConnectorAct");
    expect(args.envelope.payload).toEqual({
      id: unauthorizedHostedProvider.id,
      kind: unauthorizedHostedProvider.kind,
      endpoint: unauthorizedHostedProvider.endpoint,
      models: unauthorizedHostedProvider.models
    });
    expect(args.envelope.payload).not.toHaveProperty("credential");
  });
});
