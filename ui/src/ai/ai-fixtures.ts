import type { ProviderSummary } from "../../contracts/ipc";

const model = {
  id: "ops-model",
  context_window_tokens: 8_192,
  max_output_tokens: 1_024,
  supports_system_instruction: true,
  input_cost_micros_per_million_tokens: 2_000_000,
  output_cost_micros_per_million_tokens: 6_000_000
} as const;

export const unauthorizedHostedProvider: ProviderSummary = {
  id: "openai",
  kind: "open_ai_compatible",
  endpoint: "https://api.example.test/v1/chat/completions",
  models: [model],
  health: "unauthorized",
  credential_configured: true
};

export const localProvider: ProviderSummary = {
  id: "ollama",
  kind: "ollama",
  endpoint: "http://model-host.example.test:11434/api/chat",
  models: [
    {
      ...model,
      input_cost_micros_per_million_tokens: null,
      output_cost_micros_per_million_tokens: null
    }
  ],
  health: null,
  credential_configured: false
};
