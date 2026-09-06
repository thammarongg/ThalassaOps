// SPDX-License-Identifier: Apache-2.0

import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type { CommandEnvelope, Invoke, ProviderSummary } from "../../contracts/ipc";
import { I18nProvider, i18n } from "../i18n";
import en from "../locales/en";
import { AiProviderPanel } from "./AiProviderPanel";
import { localProvider, unauthorizedHostedProvider } from "./ai-fixtures";

afterEach(() => {
  cleanup();
  void i18n.changeLanguage("en");
});

type InvokeMock = ReturnType<typeof vi.fn> & {
  mock: { calls: [string, { envelope: CommandEnvelope<unknown> }][] };
};

const providers: ProviderSummary[] = [unauthorizedHostedProvider, localProvider];

/// The order command answers with the order it was given, the way the Rust
/// handler does, so the round trip through the surface is real.
const aiInvokeMock = (
  providerOrder: string[] = [],
  providerList: ProviderSummary[] = providers
): InvokeMock =>
  vi.fn((tauriCommand: string, args: { envelope: CommandEnvelope<unknown> }) => {
    if (tauriCommand === "ai_providers") {
      return Promise.resolve({ ok: true, value: providerList });
    }
    if (tauriCommand === "ai_provider_order") {
      return Promise.resolve({ ok: true, value: providerOrder });
    }
    if (tauriCommand === "ai_set_provider_order") {
      const { provider_order: providerOrder } = args.envelope.payload as {
        provider_order: string[];
      };
      return Promise.resolve({ ok: true, value: providerOrder });
    }
    if (tauriCommand === "ai_configure_provider") {
      return Promise.resolve({ ok: true, value: unauthorizedHostedProvider });
    }
    return Promise.reject(new Error(`Unexpected command: ${tauriCommand}`));
  }) as unknown as InvokeMock;

const renderPanel = (invoke: InvokeMock) =>
  render(
    <I18nProvider>
      <AiProviderPanel invoke={invoke as unknown as Invoke} />
    </I18nProvider>
  );

const rowFor = async (providerId: string) => {
  const rows = await screen.findAllByRole("article");
  const row = rows.find((candidate) => within(candidate).queryByText(providerId) !== null);
  if (row === undefined) throw new Error(`No provider row for ${providerId}`);
  return row;
};

it("lets an operator read provider health and build a fallback order across both destinations", async () => {
  const user = userEvent.setup();
  const invoke = aiInvokeMock();
  renderPanel(invoke);

  // A configured credential and an unauthorized health are both facts, and the
  // surface states both rather than hiding the contradiction.
  const hostedRow = await rowFor(unauthorizedHostedProvider.id);
  expect(within(hostedRow).getByText(en.ai.healthStates.unauthorized)).toBeInTheDocument();
  expect(within(hostedRow).getByText(en.ai.credentialConfigured)).toBeInTheDocument();
  expect(within(hostedRow).getByText(unauthorizedHostedProvider.endpoint)).toBeInTheDocument();

  const localRow = await rowFor(localProvider.id);
  expect(within(localRow).getByText(en.ai.healthStates.unknown)).toBeInTheDocument();
  expect(within(localRow).getByText(en.ai.credentialNotConfigured)).toBeInTheDocument();

  // Failover is off until an operator chooses an order, and the copy says so.
  expect(screen.getByText(en.ai.fallback.failoverOff)).toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Add openai to fallback order" }));
  await user.click(await screen.findByRole("button", { name: "Add ollama to fallback order" }));

  const order = await screen.findByRole("list");
  await waitFor(() => expect(within(order).getAllByRole("listitem")).toHaveLength(2));
  expect(within(order).getAllByRole("listitem")[0]).toHaveTextContent(
    unauthorizedHostedProvider.id
  );
  expect(within(order).getAllByRole("listitem")[1]).toHaveTextContent(localProvider.id);

  await user.click(screen.getByRole("button", { name: "Move ollama up" }));

  const orderCalls = invoke.mock.calls.filter(([name]) => name === "ai_set_provider_order");
  expect(orderCalls.map(([, args]) => args.envelope.payload)).toEqual([
    { provider_order: ["openai"] },
    { provider_order: ["openai", "ollama"] },
    { provider_order: ["ollama", "openai"] }
  ]);
  await waitFor(() =>
    expect(within(screen.getByRole("list")).getAllByRole("listitem")[0]).toHaveTextContent(
      localProvider.id
    )
  );
});

it("preserves the configured fallback order when adding another provider", async () => {
  const user = userEvent.setup();
  const thirdProvider: ProviderSummary = {
    ...localProvider,
    id: "vllm",
    kind: "vllm",
    endpoint: "http://model-host.example.test:8000/v1/chat/completions"
  };
  const invoke = aiInvokeMock(["openai", "ollama"], [...providers, thirdProvider]);
  renderPanel(invoke);

  const order = (await screen.findAllByRole("list"))[0];
  await waitFor(() => {
    expect(within(order).getAllByRole("listitem")[0]).toHaveTextContent("openai");
    expect(within(order).getAllByRole("listitem")[1]).toHaveTextContent("ollama");
  });
  await user.click(screen.getByRole("button", { name: "Add vllm to fallback order" }));

  await waitFor(() => {
    const orderCall = invoke.mock.calls.find(([name]) => name === "ai_set_provider_order");
    expect(orderCall?.[1].envelope.payload).toEqual({
      provider_order: ["openai", "ollama", "vllm"]
    });
  });
});

it("re-saves a provider without sending the stored secret", async () => {
  const user = userEvent.setup();
  const invoke = aiInvokeMock();
  renderPanel(invoke);

  const hostedRow = await rowFor(unauthorizedHostedProvider.id);
  await user.click(within(hostedRow).getByRole("button", { name: en.ai.editProvider }));

  // The stored secret is never rendered back: the field opens empty and the
  // hint says the stored credential is kept if it stays that way.
  expect(await screen.findByLabelText(/credential/i)).toHaveValue("");
  expect(screen.getByText(en.ai.form.credentialKeep)).toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: en.ai.form.save }));

  const configureCall = await waitFor(() => {
    const call = invoke.mock.calls.find(([name]) => name === "ai_configure_provider");
    expect(call).toBeDefined();
    return call;
  });
  expect(configureCall?.[1].envelope.payload).toEqual({
    id: unauthorizedHostedProvider.id,
    kind: unauthorizedHostedProvider.kind,
    endpoint: unauthorizedHostedProvider.endpoint,
    models: unauthorizedHostedProvider.models
  });
  expect(configureCall?.[1].envelope.payload).not.toHaveProperty("credential");
});

it("addresses every call to the tauri command and envelope command the handler requires", async () => {
  const user = userEvent.setup();
  const invoke = aiInvokeMock();
  renderPanel(invoke);

  await user.click(await screen.findByRole("button", { name: "Add openai to fallback order" }));
  const hostedRow = await rowFor(unauthorizedHostedProvider.id);
  await user.click(within(hostedRow).getByRole("button", { name: en.ai.editProvider }));
  await screen.findByLabelText(/credential/i);
  await user.click(screen.getByRole("button", { name: en.ai.form.save }));

  await waitFor(() =>
    expect(invoke.mock.calls.some(([name]) => name === "ai_configure_provider")).toBe(true)
  );

  const addressed = invoke.mock.calls.map(([tauriCommand, args]) => ({
    tauriCommand,
    envelopeCommand: args.envelope.command,
    capability: args.envelope.capability
  }));
  const unique = addressed.filter(
    (call, index) =>
      addressed.findIndex((candidate) => candidate.tauriCommand === call.tauriCommand) === index
  );
  expect(unique).toEqual([
    { tauriCommand: "ai_providers", envelopeCommand: "ai.providers", capability: "ConnectorRead" },
    {
      tauriCommand: "ai_provider_order",
      envelopeCommand: "ai.provider_order",
      capability: "ConnectorRead"
    },
    {
      tauriCommand: "ai_set_provider_order",
      envelopeCommand: "ai.set_provider_order",
      capability: "ConnectorAct"
    },
    {
      tauriCommand: "ai_configure_provider",
      envelopeCommand: "ai.configure_provider",
      capability: "ConnectorAct"
    }
  ]);
});
