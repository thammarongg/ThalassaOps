# Sprint 17 — AI provider gateway

> **Status: draft, not approved.** Sections 2 and 14 carry decisions that are
> the product owner's, not the implementer's. No task plan should be written
> against this document until those are settled.

## 1. Outcome

One provider-neutral path from a model request to a model response, with hosted
and local providers behind adapters that the rest of the application never sees.
A caller states what it wants and which data class the request carries; the
gateway chooses a provider, enforces the budget, applies the timeout, and
returns either a completion or a typed refusal. Selecting a different provider
changes nothing above the gateway.

The sprint's exit criterion is that the same request contract runs against a
hosted provider and a local one without a UI change. Sprint 19 is the first
consumer; it must be able to drive this gateway without any provider-specific
branch.

## 2. Binding decisions

1. **The gateway sends nothing that a caller has not classified.**
   `PolicyRuntime::evaluate_egress` already denies any request whose
   `classification_verified` or `redaction_verified` is false
   (`crates/thalassa-policy/src/lib.rs`), and it denies `Restricted` data and
   anything carrying an immutable secret to `HostedAi` outright. Sprint 17 does
   not weaken, bypass or pre-set those flags. It calls the policy runtime the
   same way `correlation_evidence` does and reports the denial.
2. **Sprint 18 owns classification and redaction, so Sprint 17 cannot send
   incident content to a hosted provider.** This is the sprint's central
   constraint and section 14.1 states what that leaves. Building the gateway now
   and the redaction that feeds it later is the sequence the sprint plan chose;
   the honest consequence is that Sprint 17's hosted path is exercised by
   fixtures and by a caller-supplied `Public` payload, not by real telemetry.
3. **Provider selection grants no authority.** ADR 0005 is binding: connector
   capabilities, resource scopes and policy remain the authorization boundary,
   and a local model is not more privileged than a hosted one. The gateway
   carries no tool-calling surface in this sprint — Sprint 19 owns the tool
   registry.
4. **Credentials reuse the existing store.** `CredentialStore`
   (`src-tauri/src/connectors.rs`) already wraps the OS keyring with an
   in-memory test double and addresses secrets by reference. Provider secrets
   are stored under `provider/<provider_id>` through that same trait. No new
   secret path, no secret in the database, and no secret returned to the UI —
   the read model exposes `credential_configured: bool`, exactly as the
   connector summary does.
5. **The gateway is Rust-only.** Sprint 17 adds a `thalassa-ai` crate and the
   `src-tauri` wiring. It adds no workspace UI beyond what section 12 lists,
   because the surface that would consume it is Sprint 19's.
6. **A local provider is a provider, not an exception.** Ollama and vLLM go
   through the same registry, the same budget accounting and the same health
   model as OpenAI. They differ in `EgressDestination::LocalModel` and in
   having no cost.
7. **Determinism is a test property, not a runtime one.** Every adapter is
   testable against a recorded fixture; no test performs a network call. The
   fixture corpus keeps the `2026-08-28` fixture day the rest of the repository
   uses.

## 3. Scope

### 3.1 Included

- a provider registry with declared model capability metadata;
- adapters for OpenAI, Anthropic, Gemini and OpenAI-compatible endpoints;
- a local path for Ollama and vLLM;
- a provider-neutral request and response contract in `thalassa-domain`;
- per-request and per-window token and cost budgets, with a typed refusal when
  a budget is exhausted;
- request timeout and cancellation;
- provider health with a typed unavailability reason;
- cost metadata for hosted providers, recorded per request;
- the policy egress call on every request, with the denial surfaced;
- an audit record per model request;
- English and Thai strings for the surfaces section 12 adds.

### 3.2 Excluded

- **Tool calling, function calling and MCP.** Sprint 19 owns the tool registry
  and its capability scopes. A gateway that could call tools before that
  registry exists would be the authorization boundary, which ADR 0005 forbids.
- **Streaming responses.** The investigation surface that would render tokens
  as they arrive is Sprint 19's. Adding a streaming contract now would be
  designed against no consumer.
- **Context optimization, summarization and redaction.** Sprint 18.
- **Prompt templates and system-behaviour policies, Skills and plugins.** Later.
- **Embeddings and vector storage.** No sprint in the plan requires them yet.
- **A model-selection UI.** Sprint 19 renders the assistant; this sprint
  exposes the registry through IPC and nothing more.
- **Retry on provider error.** See section 14.3.

## 4. Canonical language

### 4.1 Provider

One configured endpoint that can answer a model request: a hosted API account
or a local runtime. A provider has an id, a kind, a credential reference when
the kind needs one, an endpoint when the kind allows one, a health state and a
set of models.

### 4.2 Model

One addressable model on a provider, with capability metadata: context window,
maximum output tokens, whether it accepts a system instruction, and its cost per
million input and output tokens where the provider publishes one.

### 4.3 Model request

A provider-neutral request: an instruction, a list of messages, a data class, a
budget, a deadline and an optional model preference. It carries no provider
name. The caller states the data class; the gateway never infers it.

### 4.4 Budget

A bound on what one request, or one caller's window, may consume — in tokens
for every provider and in cost for hosted ones. A budget is enforced before the
call using the request's estimated size and after the call using the provider's
reported usage.

### 4.5 Provider health

Whether a provider answered its last probe, with a typed reason when it did
not: unreachable, unauthorized, model unavailable, rate limited, or budget
exhausted. Health is observed, never assumed from configuration.

## 5. Architecture

### 5.1 Crate boundary

`crates/thalassa-ai` holds the provider-neutral contracts, the registry, the
budget accounting and the adapter trait. It has no `reqwest` dependency and no
knowledge of any provider's wire format: adapters live in
`src-tauri/src/ai/providers/`, next to the other outbound clients, and are
constructed the way `ObservabilityClient::new` is — configuration from stored
metadata, secret from `CredentialStore`, a `reqwest::Client` with an explicit
timeout and `redirect::Policy::none()`.

This split is what makes the contract testable without a network: the crate's
tests drive a fake adapter, and the adapter tests drive recorded fixtures.

### 5.2 The adapter trait

```rust
pub trait ModelProvider: Send + Sync {
    fn manifest(&self) -> &ProviderManifest;
    fn complete(&self, request: &ProviderRequest, deadline: Instant)
        -> Result<ProviderResponse, ProviderError>;
    fn probe(&self, deadline: Instant) -> Result<ProviderHealth, ProviderError>;
}
```

`ProviderRequest` is what survives policy: the gateway builds it from a
`ModelRequest` only after the egress decision allows it. An adapter therefore
cannot send data the policy runtime refused, because it never receives it.

### 5.3 Registry

`ProviderRegistry` owns the configured providers and their manifests, mirroring
`ConnectorManifest` in `crates/thalassa-connectors`: a manifest declares what a
provider can do, and the runtime decides whether it may. Selection is explicit —
the caller names a model, or names a capability requirement and the registry
returns the first configured provider that satisfies it in a stable order. There
is no implicit fallback to another provider on failure; see section 14.3.

## 6. Domain contracts

Added to `thalassa-domain`, so the UI contract generator and the IPC layer see
them the way they see every other domain type:

```rust
pub struct ModelRequest {
    pub request_id: Uuid,
    pub instruction: Option<String>,
    pub messages: Vec<ModelMessage>,
    pub data_class: DataClass,
    pub budget: ModelBudget,
    pub timeout_ms: u64,
    pub model: ModelSelector,
}

pub enum ModelSelector { Explicit { provider_id: String, model_id: String }, Capability(ModelCapabilityRequirement) }

pub struct ModelResponse {
    pub request_id: Uuid,
    pub provider_id: String,
    pub model_id: String,
    pub content: String,
    pub usage: ModelUsage,
    pub finish: ModelFinishReason,
}

pub struct ModelUsage { pub input_tokens: u64, pub output_tokens: u64, pub cost_micros: Option<u64> }

pub enum ModelFinishReason { Complete, MaxOutputTokens, Cancelled, ProviderStop }
```

Cost is `Option<u64>` in micros because a local provider has none and a hosted
one may not report one. It is never a float and never a rendered currency
string: the UI formats it.

## 7. Budgets

A budget is checked twice. Before the call, against an estimate; after the call,
against the provider's reported usage. Both matter: an estimate that is too low
must not let a request through unbounded, and a provider that reports more usage
than it was asked for must still be recorded honestly.

- `max_input_tokens` and `max_output_tokens` bound one request. The output bound
  is also passed to the provider, so the provider stops rather than the gateway
  truncating.
- `max_cost_micros` bounds one request for hosted providers. A provider with no
  published price cannot be checked against a cost budget, and a request that
  sets one against such a provider is refused rather than allowed unpriced.
- A window budget bounds a caller across a rolling period. Sprint 17 records
  usage per request in the audit store; the window accounting reads it back.

A budget refusal is a typed error, not an exception and not a truncated
response. The caller learns which bound was hit and what the request would have
cost.

## 8. Timeout and cancellation

Every request carries a deadline. The gateway passes it to the adapter, which
sets it on the HTTP client, and the gateway also enforces it independently so a
misbehaving adapter cannot outlive it. A cancelled request returns
`ModelFinishReason::Cancelled` with the usage the provider reported, if any, so
a cancelled hosted call still costs what it cost and the audit record says so.

Cancellation is cooperative and explicit: the caller holds a token. Sprint 17
exposes it through the IPC layer as a `ai.cancel` command keyed by `request_id`.

## 9. Persistence

One new table, `ai_requests`, recording per request: id, provider id, model id,
data class, input and output tokens, cost micros, finish reason, policy version,
started and finished timestamps, and the typed error when there was one. It
records **no prompt and no completion content**. That is deliberate: the content
is the thing the policy runtime governs, and Sprint 18 has not yet built the
redaction that would make storing it safe. Sprint 19's AI Assistant Log will
need content; it must add it with redaction in place, not by widening this table
quietly.

## 10. IPC contract

| Command | Capability | Permission |
| --- | --- | --- |
| `ai.providers` | `WorkspaceRead` | `ViewWorkspace` |
| `ai.configure_provider` | `WorkspaceWrite` | `ManageConnectors` |
| `ai.probe` | `WorkspaceRead` | `ViewWorkspace` |
| `ai.complete` | `AiInvoke` | `InvestigateIncident` |
| `ai.cancel` | `AiInvoke` | `InvestigateIncident` |

`AiInvoke` is a new capability. It exists so that a principal who may read the
workspace is not thereby permitted to spend money against a hosted provider,
which is the same separation the connector capabilities already make between
reading and acting. Section 14.4 records what this leaves unresolved.

Every payload uses `deny_unknown_fields` and exact keys, like every other
command in `src-tauri/src/app/`. Errors use `IpcErrorCode` with a typed
`details.reason`, following the incident and correlation precedent: a caller
reads the reason, never the message.

## 11. Provider health

`ai.probe` asks one provider for its cheapest possible answer — a model list
where the provider offers one, a zero-output completion where it does not — and
records the outcome with a typed reason. Health is never inferred from having a
credential configured: `credential_configured: true` and
`health: Unauthorized` is a normal, informative state, and the read model must
be able to express it.

## 12. UI surface

Deliberately minimal, because Sprint 19 owns the assistant:

- the existing connector/model status area gains a provider row per configured
  provider, showing kind, health, and whether a credential is configured;
- a provider configuration form that writes through `ai.configure_provider` and
  never reads a secret back;
- English and Thai strings for both, with the key-parity test the repository
  already enforces.

No chat surface, no model picker, no cost dashboard. Sprint 25 owns cost
reporting.

## 13. Safety and policy

### 13.1 Every request is an egress decision

The gateway calls `evaluate_egress` with the caller's data class and
`EgressDestination::HostedAi` or `LocalModel` according to the selected
provider, and refuses on denial. The decision's `policy_version` is recorded on
the request row, so a later audit can tell which policy admitted it.

### 13.2 A local model is still egress

`LocalModel` is a separate destination with its own permitted data classes, not
an exemption. A local runtime may be on another host, may log prompts, and is
not automatically trusted. The policy document decides; the gateway does not.

### 13.3 The endpoint of an OpenAI-compatible provider is attacker-controlled
input

A custom endpoint is a URL a user typed. It is validated as an absolute URL with
an `https` scheme — or `http` only for a loopback host, which is what a local
Ollama actually needs — and the client is built with redirects disabled, so a
compliant-looking endpoint cannot bounce a credential to a third party. This
mirrors `ObservabilityClient`.

### 13.4 No secret reaches the UI or the audit record

The credential is read from the store at call time and dropped. The read model
carries a boolean. The audit record carries provider id, never the key.

## 14. Open decisions — these need the product owner

### 14.1 What may Sprint 17 actually send?

Sprint 18 delivers classification and redaction; until then no caller can
honestly set `classification_verified`. Three options:

- **(a) Fixture-only hosted path.** The gateway is complete and exercised
  against recorded fixtures and a live probe; no real content leaves. Sprint 18
  connects it to real data. Honest, and leaves the exit criterion demonstrable
  only against fixtures.
- **(b) Public-only live path.** A caller may send content it declares
  `Public` — a hand-typed question with no telemetry in it. Demonstrates the
  real path end to end; relies on a human declaration that nothing in the string
  is sensitive.
- **(c) Defer the hosted adapters to Sprint 18** and ship only the local path
  now, where the egress destination is `LocalModel` and the data never leaves
  the machine.

Recommendation: **(b) plus (a)** — fixtures for every adapter's wire format, and
a live path restricted to `Public`, refused by the policy runtime for anything
else. It proves the contract without pretending redaction exists.

### 14.2 Which providers ship in this sprint?

Four adapters is a lot of wire format for one sprint, and each needs a recorded
fixture corpus and an error-mapping table. Shipping OpenAI-compatible plus
Anthropic plus a local path would cover OpenAI, vLLM, Ollama and most custom
endpoints, leaving Gemini — whose request shape differs most — for Sprint 18.
This is a scope decision, not a technical one.

### 14.3 What happens when a provider fails?

No automatic failover is proposed: silently answering from a different model
changes the answer's provenance, and an investigation that cites evidence must
say which model produced it. But a rate-limited provider with a configured
sibling is exactly when an operator wants a fallback. The decision is whether
the gateway may fail over when the caller explicitly permits it per request.

### 14.4 Does `AiInvoke` belong to the incident permission?

The table above attaches `ai.complete` to `InvestigateIncident`, which makes
model spend a property of incident work. That is the narrowest existing fit, but
it also means a principal who may investigate may spend. Sprint 20's Policy
Center is where a spend permission would properly live; the question is whether
to introduce it now or accept the coupling and record it as a debt.

### 14.5 Where do window budgets reset?

A rolling window needs a clock and an owner: per principal, per workspace, or
per provider account. Per principal matches the audit model; per provider
account matches how the bill actually arrives.

## 15. Known limitations and debts

1. **No content is persisted**, so a failed investigation cannot be replayed
   from the request row alone. Sprint 19 must add content with Sprint 18's
   redaction, not by widening `ai_requests`.
2. **Token estimation before the call is approximate.** Providers count tokens
   with their own tokenizers. The pre-call budget check is therefore a guard,
   not a guarantee; the post-call check against reported usage is the accurate
   one, and it can only refuse the *next* request.
3. **No streaming** means a long completion is invisible until it finishes.
4. **Health is a point observation**, not a rolling window. A provider that
   fails one request in ten reads as healthy.

## 16. Testing

- Contract tests in `thalassa-ai` drive a fake adapter: budget refusals,
  deadline expiry, cancellation, and the mapping from provider error to typed
  reason.
- Adapter tests replay a recorded response per provider and assert the request
  body the adapter would have sent, so a wire-format regression is visible
  without a network call.
- A policy test asserts that a `Restricted` request to a hosted provider is
  denied before any adapter is constructed, and that the same request to a
  local provider follows the local data classes.
- An IPC test asserts the exact payload keys and the typed error reasons.
- No test performs a network call, and no fixture contains a real key.
