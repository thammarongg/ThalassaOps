import { describe, expect, it } from "vitest";
import type { ModelRequest, ModelResponse, ProviderErrorReason } from "../../contracts/ipc";
import { isModelResponse, providerErrorReasonWireValues } from "../../contracts/guards";

const request: ModelRequest = {
  request_id: "11111111-1111-4111-8111-111111111111",
  instruction: null,
  messages: [{ role: "user", content: "public test question" }],
  data_class: "public",
  declaration: "operator_declared",
  budget: {
    max_input_tokens: null,
    max_output_tokens: 128,
    max_cost_micros: null
  },
  timeout_ms: 1_000,
  model: {
    explicit: { provider_id: "openai", model_id: "model" }
  },
  failover: "permitted"
};

const response: ModelResponse = {
  request_id: request.request_id,
  provider_id: "fallback",
  model_id: "model",
  content: "fixture completion",
  usage: { input_tokens: 8, output_tokens: 4, cost_micros: 12 },
  finish: "complete",
  attempts: [
    {
      provider_id: "openai",
      model_id: "model",
      outcome: { failed: "rate_limited" }
    },
    {
      provider_id: "fallback",
      model_id: "model",
      outcome: "answered"
    }
  ]
};

describe("AI IPC contracts", () => {
  it("accepts a well-formed model response with non-empty attempts", () => {
    expect(isModelResponse(response, request, ["openai", "fallback"])).toBe(true);
  });

  it("rejects a model response with no attempts", () => {
    const malformed = structuredClone(response);
    malformed.attempts = [];
    expect(isModelResponse(malformed, request, ["openai", "fallback"])).toBe(false);
  });

  it("rejects a response from a provider the request could not reach", () => {
    const malformed = structuredClone(response);
    malformed.provider_id = "unconfigured";
    expect(isModelResponse(malformed, request, ["openai", "fallback"])).toBe(false);
  });

  it("enumerates exactly the Rust ProviderErrorReason wire values", () => {
    const expected: ProviderErrorReason[] = [
      "unreachable",
      "unauthorized",
      "model_unavailable",
      "rate_limited",
      "budget_exhausted",
      "malformed_response",
      "invalid_request",
      "deadline_exceeded",
      "cancelled"
    ];
    expect(providerErrorReasonWireValues).toEqual(expected);
  });
});
