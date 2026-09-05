# Sprint 17 — AI provider gateway

> **Status: approved 2026-09-05.** The three open questions that gated this
> design were answered by the product owner and are now binding decisions 8, 9
> and 10. Section 14 records what is left as debt rather than what is
> undecided.

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
   constraint; binding decision 8 and section 14.1 state what it leaves. Building the gateway now
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
8. **The live path accepts `Public` and nothing else.** Every adapter ships with
   a recorded fixture corpus proving its wire format, and a real request may
   leave the machine only when the caller declares the content `Public`. The
   policy runtime refuses every other class on its own; the gateway adds no
   second gate and no override. Sprint 18 connects real telemetry once
   classification and redaction exist. Section 13.5 states what a `Public`
   declaration does and does not mean.
9. **Three adapters ship: OpenAI-compatible, Anthropic, and the local path.**
   Together they cover OpenAI, vLLM, Ollama and custom endpoints. Gemini's
   request shape differs most from the rest and is deferred to Sprint 18, where
   it can be added behind the contract this sprint freezes.
10. **Failover is opt-in, named in the response, and configurable.** See section
    8.1. The gateway never silently answers from a different model: a request
    that permits failover carries the permission explicitly, the response says
    which provider actually answered and which one it fell back from, and the
    order it may fall back through is a setting the operator controls rather
    than an order the registry invents.

## 3. Scope

### 3.1 Included

- a provider registry with declared model capability metadata;
- adapters for OpenAI-compatible endpoints and Anthropic;
- a local path for Ollama and vLLM;
- opt-in failover through an operator-configured provider order, reported in
  the response;
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
  exposes the registry through IPC and the provider-order setting, and nothing
  more.
- **Gemini.** Deferred to Sprint 18 by binding decision 9.
- **Retry against the same provider.** Failover moves to the next provider in
  the configured order; it does not re-send to the one that just failed. A
  retry policy is a separate concern and no sprint requires one yet.

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
    pub failover: FailoverPermission,
}

pub enum ModelSelector { Explicit { provider_id: String, model_id: String }, Capability(ModelCapabilityRequirement) }

pub enum FailoverPermission { Forbidden, Permitted }

pub struct ModelResponse {
    pub request_id: Uuid,
    pub provider_id: String,
    pub model_id: String,
    pub content: String,
    pub usage: ModelUsage,
    pub finish: ModelFinishReason,
    pub attempts: Vec<ModelAttempt>,
}

pub struct ModelAttempt {
    pub provider_id: String,
    pub model_id: String,
    pub outcome: ModelAttemptOutcome,
}

pub enum ModelAttemptOutcome { Answered, Failed(ProviderErrorReason) }

pub struct ModelUsage { pub input_tokens: u64, pub output_tokens: u64, pub cost_micros: Option<u64> }

pub enum ModelFinishReason { Complete, MaxOutputTokens, Cancelled, ProviderStop }
```

Cost is `Option<u64>` in micros because a local provider has none and a hosted
one may not report one. It is never a float and never a rendered currency
string: the UI formats it.

`attempts` is always populated, with one entry when nothing failed. A caller
therefore reads the answering provider from the same field whether or not a
failover happened, and cannot accidentally treat a fallback answer as having
come from the requested model. `usage` is the answering attempt's usage; a
failed attempt that still consumed tokens records its own usage on the request
row (section 9), because a provider that charged for a failed call charged for
it.

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

The deadline covers the whole request, failover included. A request that permits
failover does not get a fresh timeout per attempt: it gets the deadline it
asked for, and the gateway stops trying when that deadline passes, whichever
attempt it is on.

### 8.1 Failover

Failover happens only when three things hold: the request carries
`FailoverPermission::Permitted`, the failure is one the next provider could
plausibly answer — unreachable, rate limited, model unavailable — and the
operator's configured order names a next provider whose manifest satisfies the
same capability requirement. An `Unauthorized` failure does not fail over: a
missing credential is a configuration problem the operator must see, not a
reason to spend against a different account. A budget refusal never fails over,
because the budget is the caller's, not the provider's.

The order is a stored setting, `provider_order`, that the operator edits — a
list of provider ids. The registry never invents a fallback order, and a
provider absent from the list is never selected as a fallback even when it is
configured and healthy. This is deliberate: a fallback is a decision about which
vendor may receive the request, and that is an operator's decision.

Every attempt is recorded, in order, in `ModelResponse::attempts` and on the
request row. A failover is visible to the caller, to the audit trail, and in the
UI; a completion whose provenance is not the requested model can never be
mistaken for one that is.

## 9. Persistence

One new table, `ai_requests`, recording per request: id, data class, budget,
finish reason, policy version, started and finished timestamps, and the typed
error when there was one. A second table, `ai_request_attempts`, records one row
per attempt — request id, ordinal, provider id, model id, outcome, input and
output tokens, cost micros — so a failover's full cost is recoverable and a
failed attempt that still consumed tokens is not lost. Neither table records
**any prompt or completion content**. That is deliberate: the content
is the thing the policy runtime governs, and Sprint 18 has not yet built the
redaction that would make storing it safe. Sprint 19's AI Assistant Log will
need content; it must add it with redaction in place, not by widening this table
quietly.

## 10. IPC contract

| Command | Capability | Permission |
| --- | --- | --- |
| `ai.providers` | `WorkspaceRead` | `ViewWorkspace` |
| `ai.configure_provider` | `WorkspaceWrite` | `ManageConnectors` |
| `ai.set_provider_order` | `WorkspaceWrite` | `ManageConnectors` |
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
- a fallback-order control that writes through `ai.set_provider_order`: an
  ordered list of the configured providers the operator permits as fallbacks,
  empty by default, so failover does nothing until someone chooses it;
- English and Thai strings for all three, with the key-parity test the
  repository already enforces.

No chat surface, no model picker, no cost dashboard. Sprint 26 owns capacity,
reliability and provider cost reporting; this sprint only records the per-attempt
numbers that sprint will aggregate.

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

### 13.5 A `Public` declaration is a human claim, not a machine fact

Binding decision 8 lets a caller send content it declares `Public`. Nothing in
Sprint 17 can verify that claim — the classifier that would is Sprint 18's. So
the declaration is exactly as trustworthy as the person making it, and the
design says so rather than implying the gateway checked. Two things keep the
blast radius small: the only caller in this sprint is a hand-typed question, so
no telemetry, evidence excerpt or incident field can reach a provider by
accident; and the policy runtime independently refuses `Internal`,
`Confidential` and `Restricted` to `HostedAi`, so a mistaken declaration is the
only path through, not a missing check.

Sprint 18 must replace the declaration with a verified classification before any
automated caller is connected. Until then, a `Public` request is the one place
in the application where a human assertion substitutes for a policy control.

## 14. Decisions taken on 2026-09-05

The product owner settled the three questions this design was blocked on. They
are binding decisions 8, 9 and 10; this section records what was chosen over
what, so a later reader does not reopen a closed question.

### 14.1 What Sprint 17 may send — fixtures plus a `Public`-only live path

Chosen over a fixture-only path and over deferring the hosted adapters. Every
adapter carries a recorded fixture corpus, and a live request may leave only
when its caller declares the content `Public`. Section 13.5 states plainly that
the declaration is a human claim the application cannot verify until Sprint 18,
and what keeps its blast radius small.

### 14.2 Three adapters — OpenAI-compatible, Anthropic, local

Chosen over all four. The three cover OpenAI, vLLM, Ollama and custom
endpoints; Gemini's request shape differs most and is deferred to Sprint 18,
behind the contract this sprint freezes. Each adapter costs a fixture corpus and
an error-mapping table, which is what made four too many for one sprint.

### 14.3 Failover — opt-in, named, and operator-configured

Chosen over both no failover and automatic failover. The product owner added two
requirements beyond the question asked: the response must say which provider
answered, and the operator must be able to configure which providers may be
used as fallbacks rather than accepting an order the system picks. Section 8.1
is the resulting contract, `ModelResponse::attempts` carries the provenance, and
`ai.set_provider_order` carries the setting, empty by default.

### 14.4 `AiInvoke` stays attached to `InvestigateIncident`, as a recorded debt

Not put to the product owner: it is an implementation-level coupling with no
better home today. A principal permitted to investigate an incident is thereby
permitted to spend against a hosted provider. A spend permission belongs in
Sprint 20's Policy Center, alongside the rest of the permission model, and
introducing a half-modelled one here would have to be migrated then. Recorded as
debt 5 in section 15.

### 14.5 Window budgets are per principal

Not put to the product owner: per principal matches the audit model, where every
request already carries an actor, and it is the only owner the application can
attribute a request to today. Per provider account matches how the bill arrives
and is the better fit once Sprint 20 models accounts; recorded as debt 6.

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
5. **`AiInvoke` is attached to `InvestigateIncident`**, so a principal who may
   investigate may spend against a hosted provider. A spend permission belongs
   in Sprint 20's Policy Center; see section 14.4.
6. **Window budgets are owned per principal**, which does not match how a
   provider bill arrives. Sprint 20 models the permissions and Sprint 24 the
   membership that a per-account budget would need; revisit it there. See
   section 14.5.
7. **A `Public` declaration is unverified** until Sprint 18 delivers
   classification. It is the one control in the application that rests on a
   human assertion; see section 13.5.


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

## 17. Reconciliation with Sprints 18-26

Checked against `docs/planning/sprint-plan.md` so this sprint neither duplicates
a later one nor blocks it.

- **Sprint 18** adds classification, redaction and Gemini. It is what replaces
  the unverified `Public` declaration of section 13.5, and it inherits a frozen
  request contract to add Gemini behind. Nothing here hard-codes a redaction
  behaviour, which that sprint's plan explicitly forbids.
- **Sprint 19** adds the tool registry, structured findings and the AI Assistant
  Log. This sprint deliberately ships no tool surface and persists no content,
  so Sprint 19 adds both with redaction in place rather than inheriting an
  unredacted store. Its assistant is the first real caller of `ai.complete`.
- **Sprint 20** owns the Policy Center, where a spend permission belongs. Debt 5
  is its inheritance, not a gap to close here.
- **Sprint 21** owns actions and approvals. The gateway grants no mutation
  authority, which is what ADR 0005 requires and what keeps that sprint's
  approval framework the only path to a change.
- **Sprint 23** generates management summaries from evidence and will drive this
  gateway. It needs no new contract, but its content leaves for a third party,
  so it must derive its own egress allowlist from Sprint 18's classification —
  the same warning the Sprint 16 design records for the summary card.
- **Sprint 24** models organization, team and workspace membership. A per-
  provider-account budget needs that model; debt 6 waits for it.
- **Sprint 25** hardens the keychain, secret storage and IPC capabilities.
  Binding decision 4 keeps provider secrets inside the existing
  `CredentialStore`, so that hardening covers them automatically instead of
  finding a second, parallel secret path.
- **Sprint 26** benchmarks investigation latency and token cost and reports
  provider cost metadata. Section 9's per-attempt rows are the data it will
  read; this sprint builds no aggregation of its own.
