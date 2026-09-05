# Sprint 17 AI Provider Gateway Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Every claim in this plan was checked against the code on 2026-09-05.** Where
> a task names a file, a symbol or an enum member, that name exists. The Sprint
> 16 retrospective is the reason: six defects there came from plan snippets that
> referenced contracts the repository did not have, and each one passed its own
> mocked test. If something here disagrees with the code, the code wins — say so
> in the plan before building on it.

Design: `docs/design/sprint-17-ai-provider-gateway.md` (approved 2026-09-05).

## Global Constraints

- **No test performs a network call.** Every adapter is driven by a recorded
  fixture. This is binding decision 7 and it is also what makes the suite
  runnable in CI.
- **No fixture, test or document contains a real API key.** A credential in a
  test is a literal like `test-key-not-real`.
- **Fixtures keep the `2026-08-28` fixture day** the rest of the repository
  uses. Sprint 14 lost two days to fixtures dated one day off, silently.
- **The policy runtime is called, never emulated.** A task that needs an egress
  decision calls `PolicyRuntime::evaluate_egress`; none pre-sets
  `classification_verified` or `redaction_verified`.
- **No prompt or completion content is persisted.** Design section 9.
- Gates before every commit: `cargo fmt --all -- --check`,
  `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`,
  and for any task touching `ui/`: `npm run format:check && npm run lint &&
  npm run typecheck && npm test`. Baseline at the start of this sprint is 577
  Rust tests and 216 frontend tests.
- Conventional commit subjects, scope `ai`: `feat(ai):`, `fix(ai):`,
  `test(ai):`.

## Task DAG

```
Task 1 domain contracts
  |
  +-- Task 2 thalassa-ai crate: manifest, trait, registry
  |     |
  |     +-- Task 3 budget accounting
  |     |     |
  |     +-----+-- Task 4 gateway: policy, deadline, cancellation, failover
  |     |                 |
  |     +-- Task 6 OpenAI-compatible adapter
  |     +-- Task 7 Anthropic adapter
  |     +-- Task 8 local adapter (Ollama, vLLM)
  |
  +-- Task 5 migration 0007 and the request/attempt repository
        |
        +-- Task 9 provider configuration and provider_order
              |
              +-- Task 10 IPC commands and the AiInvoke capability
                    |     (also needs Task 4)
                    +-- Task 11 TypeScript contracts and guards
                          |
                          +-- Task 12 UI: provider rows, form, fallback order
                                |
                                +-- Task 13 acceptance
```

Tasks 6, 7 and 8 are independent of each other and of Tasks 3-5. Everything
else is sequential.

## File Map

**New:**

- `crates/thalassa-ai/Cargo.toml`, `crates/thalassa-ai/src/lib.rs`
- `crates/thalassa-ai/src/registry.rs`, `budget.rs`, `gateway.rs`
- `crates/thalassa-ai/tests/gateway.rs`
- `src-tauri/migrations/0007_ai_requests.sql`
- `src-tauri/src/ai/mod.rs`, `store.rs`, `config.rs`
- `src-tauri/src/ai/providers/mod.rs`, `openai_compatible.rs`, `anthropic.rs`, `local.rs`
- `src-tauri/src/ai/fixtures/<provider>/<case>.json`
- `src-tauri/src/app/ai.rs`
- `src-tauri/tests/ai_ipc.rs`
- `ui/src/ai/AiProviderPanel.tsx`, `AiProviderForm.tsx`, `AiFallbackOrder.tsx`,
  their tests, `ai-fixtures.ts`, `ai.css`
- `ui/src/ai/ai.acceptance.test.tsx`

**Modified:**

- `Cargo.toml` (workspace members)
- `crates/thalassa-domain/src/lib.rs` (contracts)
- `crates/thalassa-ipc/src/lib.rs` (`Capability::AiInvoke`, descriptors)
- `crates/thalassa-ipc/tests/contracts.rs`
- `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs` or `main.rs` (module + commands)
- `src-tauri/src/app/mod.rs` (migration constant, `apply_migrations`)
- `ui/contracts/ipc.ts`, `ui/contracts/guards.ts`
- `ui/src/locales/en.ts`, `ui/src/locales/th.ts`

---

### Task 1: Domain Contracts

**Files:**
- Modify: `crates/thalassa-domain/src/lib.rs`
- Test: the crate's existing test module

**Interfaces:**
- Produces: `ModelRequest`, `ModelMessage`, `ModelRole`, `ModelSelector`,
  `ModelCapabilityRequirement`, `FailoverPermission`, `ModelBudget`,
  `ModelResponse`, `ModelAttempt`, `ModelAttemptOutcome`, `ModelUsage`,
  `ModelFinishReason`, `ProviderErrorReason`, `ProviderKind`, `ProviderHealth`,
  `ModelDescriptor`, and `validate_model_request`.

**Grounding.** `DataClass` already exists in `thalassa-policy`, not in
`thalassa-domain`; check which crate the request should hold before writing the
field, and do not duplicate the enum. `Permission` lives at
`crates/thalassa-domain/src/lib.rs` line 2153 and already has `Investigate` —
there is no `InvestigateIncident`. Follow the serde conventions the file already
uses: `#[serde(rename = "snake_case")]` on enum variants, as
`IncidentEventKind` and `EvidenceSourceKind` do.

- [ ] **Step 1: Write the failing test**

Test `validate_model_request`, not the struct definitions. The rules worth a
test are the ones a caller can get wrong:

- an empty `messages` list is rejected;
- a message body that is empty or whitespace is rejected, reusing
  `validate_incident_text`'s bound style — note it counts `chars()`, not bytes,
  and any UI-side bound must match (this is the Sprint 16 comment-length trap);
- `timeout_ms` of zero, or above a stated ceiling, is rejected;
- a `ModelBudget` whose `max_output_tokens` is zero is rejected;
- a budget that sets `max_cost_micros` is *accepted* here — the "provider
  publishes no price" refusal belongs to the gateway (Task 4), which knows the
  provider, and must not be duplicated in the domain.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p thalassa-domain model_request`
Expected: FAIL — `validate_model_request` does not exist.

- [ ] **Step 3: Implement**

Contracts as design section 6 states them. `cost_micros` is `Option<u64>`;
nothing is an `f64`. `ModelAttempt` carries `provider_id`, `model_id` and
`outcome`, and `ModelResponse::attempts` is never empty.

- [ ] **Step 4: Run the gate and commit**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git commit -m "feat(ai): add provider-neutral model request and response contracts"
```

---

### Task 2: The `thalassa-ai` Crate — Manifest, Trait, Registry

**Files:**
- Create: `crates/thalassa-ai/Cargo.toml`, `src/lib.rs`, `src/registry.rs`
- Modify: `Cargo.toml` (workspace `members`)
- Test: `crates/thalassa-ai/tests/registry.rs`

**Interfaces:**
- Produces: `ProviderManifest`, `ModelProvider` trait, `ProviderRegistry`,
  `ProviderError`.

**Grounding.** Mirror `crates/thalassa-connectors`: its `Cargo.toml` uses
`version.workspace = true` and workspace dependencies only, and
`ConnectorManifest`/`ConnectorCapability` are the shape to follow — a manifest
declares what a provider can do and the runtime decides whether it may. **This
crate must not depend on `reqwest`**; adapters live in `src-tauri` (design 5.1).

- [ ] **Step 1: Write the failing test**

- a registry selects an explicitly named provider and model, and returns a typed
  error when the model is not on that provider's manifest;
- capability selection returns providers in a **stable, declared order**, not
  hash order — assert the order with three registered providers;
- a provider whose manifest lacks the required capability is never selected;
- registration rejects two providers with the same id.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p thalassa-ai`
Expected: FAIL — the crate does not exist; `cargo` reports an unknown package.

- [ ] **Step 3: Implement**

The trait as design 5.2 states it. `ProviderError` carries
`ProviderErrorReason` from Task 1 so the mapping table is stated once.

- [ ] **Step 4: Run the gate and commit**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git commit -m "feat(ai): add the provider registry and adapter contract"
```

---

### Task 3: Budget Accounting

**Files:**
- Create: `crates/thalassa-ai/src/budget.rs`
- Test: `crates/thalassa-ai/tests/budget.rs`

**Interfaces:**
- Produces: `BudgetLedger`, `BudgetRefusal`, `estimate_input_tokens`.

**Grounding.** Design section 7. The pre-call check uses an estimate and the
post-call check uses reported usage; both exist, and the post-call one can only
refuse the *next* request (debt 2).

- [ ] **Step 1: Write the failing test**

- a request whose estimated input exceeds `max_input_tokens` is refused before
  any provider is asked, and the refusal names which bound was hit;
- `max_output_tokens` is passed to the provider rather than truncating the
  response afterwards — assert it reaches the `ProviderRequest`;
- a cost budget against a model with no published price is **refused**, not
  silently allowed: this is the case that would otherwise spend unbounded;
- reported usage larger than the estimate is recorded as reported, and the
  window ledger reflects the larger number;
- a window ledger refuses the request that would cross the window bound and
  admits the one that would not.

Estimation is approximate by construction (debt 2). Assert the *policy* — that
an over-estimate refuses — not a specific token count, or the test becomes a
tokenizer regression test.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p thalassa-ai budget`
Expected: FAIL — `BudgetLedger` does not exist.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run the gate and commit**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git commit -m "feat(ai): enforce token and cost budgets before and after a call"
```

---

### Task 4: The Gateway — Policy, Deadline, Cancellation, Failover

**Files:**
- Create: `crates/thalassa-ai/src/gateway.rs`
- Test: `crates/thalassa-ai/tests/gateway.rs`

**Interfaces:**
- Consumes: `ProviderRegistry`, `BudgetLedger`, `PolicyRuntime`.
- Produces: `Gateway::complete(request, deadline, cancel) -> Result<ModelResponse, GatewayError>`.

**Grounding — read before writing the test.**
`PolicyRuntime::evaluate_egress` (`crates/thalassa-policy/src/lib.rs`, around
line 205) denies when `classification_verified` or `redaction_verified` is
false, denies `Restricted` and immutable-secret content to `HostedAi`, and
otherwise checks the destination's permitted data classes from the policy
document. `EgressDestination` already has `HostedAi` and `LocalModel`. The
gateway picks the destination from the selected provider's kind — a local
provider is `LocalModel` — and it does **not** set the verification flags on the
caller's behalf.

Failover rules are design 8.1 and they are not symmetric: unreachable, rate
limited and model unavailable may fail over; `Unauthorized` may not, because a
missing credential is a configuration fault the operator must see; a budget
refusal may not, because the budget is the caller's.

- [ ] **Step 1: Write the failing test**

Drive a fake `ModelProvider`. The tests that earn their place:

- a `Restricted` request to a hosted provider is denied **before any provider is
  constructed** — assert the fake was never called, not merely that an error
  came back;
- the same request to a local provider follows the local data classes from the
  policy document;
- the deadline covers the whole request including failover: a request permitting
  failover, with two providers each slower than the remaining time, returns a
  deadline error rather than trying the second after the deadline passed;
- cancellation returns `ModelFinishReason::Cancelled` **and** the usage the
  provider reported, so a cancelled hosted call still records what it cost;
- `FailoverPermission::Forbidden` with a rate-limited provider returns the
  error and never touches the next provider;
- `Permitted` with a rate-limited first provider and a configured order returns
  the second provider's answer, and `attempts` has two entries in order: the
  first `Failed(RateLimited)`, the second `Answered`;
- `Permitted` with an `Unauthorized` first provider does **not** fail over;
- a provider absent from the configured order is never chosen as a fallback,
  even when it is registered and healthy;
- `attempts` has exactly one entry when nothing failed.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p thalassa-ai gateway`
Expected: FAIL — `Gateway` does not exist.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run the gate and commit**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git commit -m "feat(ai): gate every model request on policy, deadline and budget"
```

---

### Task 5: Migration 0007 and the Request Store

**Files:**
- Create: `src-tauri/migrations/0007_ai_requests.sql`, `src-tauri/src/ai/store.rs`
- Modify: `src-tauri/src/app/mod.rs`, `src-tauri/src/ai/mod.rs`
- Test: `src-tauri/tests/ai_store.rs`

**Grounding.** Migrations are `include_str!`-ed constants in
`src-tauri/src/app/mod.rs` (lines 26-32) and applied by `apply_migrations`;
`0006_incidents.sql` is the newest. Follow its header style: a comment stating
what the migration is for and any deviation from the design's schema block.

**Two tables, not one** (design 9): `ai_requests` for the request and
`ai_request_attempts` for one row per attempt, so a failover's full cost and a
failed attempt that still consumed tokens are both recoverable. Neither stores
prompt or completion content — assert that in a test by inserting a request
whose message body is a recognisable literal and asserting it appears in no
column of either table.

- [ ] **Step 1: Write the failing test**

- a completed request writes one request row and one attempt row;
- a failed-over request writes one request row and two attempt rows, ordinal 0
  and 1, in order;
- no column of either table contains the message body;
- the policy version from the egress decision is stored on the request row;
- reading back a window of requests for a principal returns them newest first.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p thalassaops ai_store`
Expected: FAIL — the module does not exist.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run the gate and commit**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
git commit -m "feat(ai): record model requests and attempts without content"
```

---

### Task 6: The OpenAI-Compatible Adapter

**Files:**
- Create: `src-tauri/src/ai/providers/openai_compatible.rs`,
  `src-tauri/src/ai/fixtures/openai/*.json`
- Test: in-module tests plus fixtures

**Grounding.** Copy the construction shape of `ObservabilityClient::new`
(`src-tauri/src/observability/client.rs`): configuration parsed from stored
metadata, secret fetched from `CredentialStore` by reference, and a
`reqwest::Client` built with an explicit `timeout` and
`redirect::Policy::none()`. `reqwest` is already a dependency of `src-tauri`
with `rustls-tls` and no default features.

Endpoint validation is design 13.3: absolute URL, `https` only, except `http`
for a loopback host, which is what a local runtime needs. Redirects disabled so
a compliant-looking endpoint cannot bounce the credential to a third party.

- [ ] **Step 1: Write the failing test**

- the request body the adapter *would* send for a given `ProviderRequest`,
  asserted against a recorded fixture — this is what catches a wire-format
  regression without a network call;
- `max_output_tokens` from the budget appears in the request body;
- a recorded 200 response maps to `ModelResponse` with usage and finish reason;
- each of 401, 404, 429 and 500 maps to the right `ProviderErrorReason`;
- a response missing `usage` yields a typed malformed-response error rather than
  zero usage, because zero usage would silently under-bill the ledger;
- a non-loopback `http://` endpoint is rejected at construction.

- [ ] **Step 2: Run test to verify it fails**
- [ ] **Step 3: Implement**
- [ ] **Step 4: Run the gate and commit**

```bash
git commit -m "feat(ai): add the OpenAI-compatible provider adapter"
```

---

### Task 7: The Anthropic Adapter

**Files:**
- Create: `src-tauri/src/ai/providers/anthropic.rs`,
  `src-tauri/src/ai/fixtures/anthropic/*.json`

Same test shape as Task 6, against Anthropic's own request and response format —
notably that the system instruction is a top-level field rather than a message,
and that usage is reported as `input_tokens` and `output_tokens`. Assert the
mapping from its stop reason to `ModelFinishReason`, including the
max-tokens case, which is the one a budget makes reachable.

- [ ] **Step 1: Write the failing test**
- [ ] **Step 2: Run test to verify it fails**
- [ ] **Step 3: Implement**
- [ ] **Step 4: Run the gate and commit**

```bash
git commit -m "feat(ai): add the Anthropic provider adapter"
```

---

### Task 8: The Local Adapter

**Files:**
- Create: `src-tauri/src/ai/providers/local.rs`,
  `src-tauri/src/ai/fixtures/local/*.json`

vLLM serves an OpenAI-compatible API and should reuse Task 6's request builder
rather than growing a second copy; Ollama's native `/api/chat` does not, and
needs its own mapping. Decide which of the two this module covers and say so in
a comment — one adapter with a kind discriminator is fine, two is fine, silently
assuming vLLM and Ollama are the same wire format is not.

The local path uses `EgressDestination::LocalModel` and reports no cost. A test
must assert that a local provider's `ModelUsage::cost_micros` is `None` rather
than `Some(0)`: zero would claim a price of zero, `None` says there is no price.

- [ ] **Step 1: Write the failing test**
- [ ] **Step 2: Run test to verify it fails**
- [ ] **Step 3: Implement**
- [ ] **Step 4: Run the gate and commit**

```bash
git commit -m "feat(ai): add the local model provider adapter"
```

---

### Task 9: Provider Configuration and the Fallback Order

**Files:**
- Create: `src-tauri/src/ai/config.rs`
- Test: `src-tauri/tests/ai_config.rs`

**Grounding.** Secrets go through the existing `CredentialStore` trait
(`src-tauri/src/connectors.rs`), which has a keyring implementation and an
`InMemoryCredentialStore` for tests, addressed by reference. Connector
references are `connector/<id>`; provider references are `provider/<id>`
(binding decision 4). The read model exposes `credential_configured: bool` the
way `ConnectorSummary` does — **no test may assert a secret value coming back
out of the read model**, and the implementation must make that impossible
rather than merely avoided.

- [ ] **Step 1: Write the failing test**

- configuring a provider stores the secret under `provider/<id>` and the read
  model reports `credential_configured: true` without the value;
- reconfiguring without a new secret keeps the stored one;
- removing a provider deletes its credential;
- `provider_order` defaults to empty, so failover does nothing until an operator
  chooses it;
- `provider_order` rejects an id that is not a configured provider, and rejects
  a duplicate id.

- [ ] **Step 2: Run test to verify it fails**
- [ ] **Step 3: Implement**
- [ ] **Step 4: Run the gate and commit**

```bash
git commit -m "feat(ai): store provider configuration and the fallback order"
```

---

### Task 10: IPC Commands and the `AiInvoke` Capability

**Files:**
- Create: `src-tauri/src/app/ai.rs`, `src-tauri/tests/ai_ipc.rs`
- Modify: `crates/thalassa-ipc/src/lib.rs`,
  `crates/thalassa-ipc/tests/contracts.rs`, `src-tauri/src/main.rs`

**Grounding — the exact shapes.** `Capability` (`crates/thalassa-ipc/src/lib.rs`
line 70) has nine members and none is `WorkspaceWrite`; `Permission`
(`crates/thalassa-domain/src/lib.rs` line 2153) has eight and none is
`InvestigateIncident`. This task adds `Capability::AiInvoke` and nothing else to
either enum, and extends the contract test in the same commit.

Descriptors follow `correlation_evidence_descriptor()` (line 164). The read and
configuration commands reuse the connector capabilities exactly as
`connector_test` does — `CommandDescriptor::new("connector", verb,
Capability::ConnectorAct, Permission::Read)`, see
`src-tauri/src/app/connectors.rs` line 50 — and every handler checks
`envelope.command`, `envelope.capability`, the scope and the membership status
before doing anything, as that function does.

Payload structs are `#[serde(deny_unknown_fields)]` with exact keys, and
rejections go through one `invalid_ai_request(reason)` helper that builds
`IpcError::new(IpcErrorCode::InvalidRequest, "...", json!({ "reason": reason }))`
— the shape `invalid_incident_request` uses (`src-tauri/src/app/incident.rs`
line 518). **The reason is in `details`, never in `code`.** Sprint 16 lost a
task to a plan that put it in `code`.

- [ ] **Step 1: Write the failing test**

- each command rejects an unknown payload key;
- `ai.complete` with the wrong capability is `PERMISSION_DENIED` and the denial
  names only the required command, echoing no payload;
- a policy denial surfaces with its typed reason;
- a budget refusal surfaces with its own reason, distinct from a policy denial —
  a caller must be able to tell "not permitted" from "too expensive";
- `ai.cancel` for an unknown `request_id` is a typed error, not a silent success;
- the tauri command names are snake_case (`ai_complete`) while the envelope
  command is dotted (`ai.complete`), and the test asserts **both**.

- [ ] **Step 2: Run test to verify it fails**
- [ ] **Step 3: Implement**

Register every command in the `invoke_handler` list in `src-tauri/src/main.rs`
(the existing list around line 364). A command that exists but is not registered
is invisible at runtime and green in every unit test.

- [ ] **Step 4: Run the gate and commit**

```bash
git commit -m "feat(ai): expose the gateway through capability-scoped IPC"
```

---

### Task 11: TypeScript Contracts and Guards

**Files:**
- Modify: `ui/contracts/ipc.ts`, `ui/contracts/guards.ts`
- Test: `ui/src/ai/ai-contracts.test.ts`

**Grounding.** Follow how the incident contracts were added in Sprint 16:
types mirror the Rust serde representation exactly, and a guard exists for
anything that arrives from the wire and is rendered. `isEvidenceResponse`
(`ui/contracts/guards.ts` line 361) is the shape to follow — it validates the
response *against the request*, which is what catches a backend that answers
something else.

- [ ] **Step 1: Write the failing test**

- a guard accepts a well-formed `ModelResponse` and rejects one whose
  `attempts` is empty, since the contract says it never is;
- a guard rejects a response whose `provider_id` is not among the providers the
  request could have reached;
- the `ProviderErrorReason` union in TypeScript has exactly the members the Rust
  enum has — enumerate them in the test the way `guards.ts` enumerates
  `signalKinds`, so adding a Rust variant without the TypeScript one fails.

- [ ] **Step 2: Run test to verify it fails**
- [ ] **Step 3: Implement**
- [ ] **Step 4: Run the gate and commit**

```bash
npm run format:check && npm run lint && npm run typecheck && npm test
git commit -m "feat(ai): add the model gateway contracts and guards"
```

---

### Task 12: The Provider Surface

**Files:**
- Create: `ui/src/ai/AiProviderPanel.tsx`, `AiProviderForm.tsx`,
  `AiFallbackOrder.tsx`, their tests, `ai-fixtures.ts`, `ai.css`
- Modify: `ui/src/locales/en.ts`, `ui/src/locales/th.ts`

**Grounding.** Design section 12 is deliberately small: provider rows with
health, a configuration form, and the fallback-order control. No chat surface.
The locale parity test (`ui/src/locales/locales.test.ts`) compares `en` and `th`
key sets; Sprint 16 added a second test binding a union to its locale keys, and
the provider-kind and health-reason labels need the same treatment or a new
enum member renders as a raw key with every test green.

- [ ] **Step 1: Write the failing test**

- a configured provider with `credential_configured: true` and
  `health: "unauthorized"` renders both facts — this combination is normal and
  informative (design 11), not a contradiction to hide;
- the form never renders a secret value, and submitting without changing the
  secret does not send one;
- the fallback order is empty by default and the copy says what that means:
  failover is off;
- reordering emits the new order through `onReorder` in the order shown;
- a provider not in the order is visibly not a fallback.

- [ ] **Step 2: Run test to verify it fails**
- [ ] **Step 3: Implement**
- [ ] **Step 4: Run the gate and commit**

```bash
npm run format:check && npm run lint && npm run typecheck && npm test
git commit -m "feat(ai): add the provider status, configuration and fallback surface"
```

---

### Task 13: Acceptance

**Files:**
- Create: `ui/src/ai/ai.acceptance.test.tsx`
- Also: a Rust end-to-end test in `src-tauri/tests/ai_ipc.rs`

The sprint's exit criterion is that the same request contract runs against a
hosted provider and a local one without a UI change. Prove exactly that:

- [ ] **Step 1: Write the acceptance test**

Rust, with fixture-backed adapters and the real gateway, registry, budget
ledger, policy runtime and store:

- one `ModelRequest` is answered by the OpenAI-compatible adapter and then, with
  only the selector changed, by the local adapter. **Assert the two
  `ModelResponse` values are identical apart from `provider_id`, `model_id`,
  `usage` and `attempts`** — that is the exit criterion, stated as an assertion
  rather than as a claim;
- a `Restricted` request is refused for the hosted provider and permitted for
  the local one when the policy document allows it there, showing the two
  destinations are really distinct;
- the store holds one request row and the right attempt rows for both.

Frontend: the provider surface renders from fixtures and the fallback order
round-trips. Assert the tauri command name and the envelope command for every
call, as the Sprint 16 acceptance test does — reading the command off the wrong
argument of `invoke` is a mistake this repository has already made once.

- [ ] **Step 2: Run every gate**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
npm run format:check && npm run lint && npm run typecheck && npm test
```

Report the exact counts against the 577 / 216 baseline.

- [ ] **Step 3: Commit**

```bash
git commit -m "test(ai): verify one request contract across hosted and local providers"
```

---

## What this plan deliberately does not do

- No streaming, no tool calling, no Gemini, no prompt templates. Design 3.2.
- No content persistence. Sprint 19 adds it with Sprint 18's redaction.
- No retry against a provider that just failed; failover moves on.
- No chat UI. Sprint 19 owns the assistant.
