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

> Bookkeeping: Tasks 1-12 shipped as `080fc95`, `72c3860`, `8247c13`, `ccd36ee`,
> `06402cb`, `336b4bf`, `2af2c4e`, `345bd59`, `16c20f6`, `8abea4b` and `a6c6c4d`,
> but their step boxes were never ticked as the work landed. They are left
> unticked rather than back-filled from the commit log; the commits are the
> record.


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
                                      |
                                      +-- Task 14 mount the surfaces
                                      +-- Task 15 wire the audit store
```

Tasks 6, 7 and 8 are independent of each other and of Tasks 3-5. Everything
else is sequential.

Tasks 14 and 15 were added on 2026-09-06 after Task 13, when the user settled
the two open decisions design section 15 records as 9 and 10. Task 16 was added
the same day, when reviewing Task 14's mount against the backend showed the
fallback order has no read counterpart (design section 15 item 11). All three
are required before the sprint merges.

Task 14 is done (`0b109b0`, `e380b7f`). Run Task 16 next, then Task 15: Task 16
edits `src-tauri/src/app/ai.rs` and `ui/contracts/ipc.ts`, which are inside Task
15's boundary, and it is far the smaller of the two.

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

> **Amended 2026-09-05, after Task 1 was committed as `0cce241`.** `ModelRequest`
> gains one more field, `declaration: ContentDeclaration`, an enum whose only
> Sprint 17 variant is `OperatorDeclared`. Task 1 shipped without it and the
> omission is what stopped Task 4: the policy runtime denies every request whose
> verification flags are false, and the caller had nowhere to set them. Design
> sections 13.6 and 14.6 record the decision. Add the field and a validation
> test as the first step of Task 4; do not rewrite `0cce241`.

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
`PolicyRuntime::evaluate_egress` (`crates/thalassa-policy/src/lib.rs`, line 207)
denies when `classification_verified` or `redaction_verified` is false, denies
`Restricted` and immutable-secret content to `HostedAi`, and otherwise checks
the destination's permitted data classes from the policy document.
`EgressDestination` already has `HostedAi` and `LocalModel`. The gateway picks
the destination from the selected provider's kind — a local provider is
`LocalModel`.

The gateway does **not** invent the verification flags, and it does not call
`EgressRequest::verified` on the caller's behalf. It maps
`ModelRequest::declaration` onto them: `ContentDeclaration::OperatorDeclared`
sets both flags, because a person asserted the content was safe to send, and
that assertion arrived inside the request. There is no other variant in Sprint
17, so there is no other path to a set flag. Design 13.6.

`ModelRequest::data_class` is `ModelDataClass`, which Task 1 made a `String`
alias so `thalassa-domain` need not depend on `thalassa-policy`. The gateway is
the crate that resolves that string to a `DataClass`; a string that names no
data class is a typed refusal, never a silent `Public`.

- [ ] **Step 0: Land the amended contract**

Add `declaration: ContentDeclaration` to `ModelRequest` and the
`ContentDeclaration` enum to `crates/thalassa-domain/src/lib.rs`, following the
serde conventions already in the file. Extend the Task 1 validation tests with
one case that a request carrying the declaration validates. Commit separately:

```bash
git commit -m "feat(ai): carry the caller's content declaration in the request"
```

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
- a `data_class` string that names no `DataClass` is refused, and the fake was
  never called;
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

- [x] **Step 1: Write the acceptance test**

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

- [x] **Step 2: Run every gate**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
npm run format:check && npm run lint && npm run typecheck && npm test
```

Report the exact counts against the 577 / 216 baseline.

- [x] **Step 3: Commit**

```bash
git commit -m "test(ai): verify one request contract across hosted and local providers"
```

Done: `0cd2549`. `src-tauri/tests/ai_acceptance.rs` (3 tests) runs the real
gateway, registry, budget ledger and policy runtime over two fixture adapters
and asserts the exit criterion directly: the hosted and local responses are
equal once `provider_id`, `model_id`, `usage` and `attempts` are removed, and
the payload sent to each adapter is the same apart from `model_id`. The
restricted split asserts `ImmutableRestrictedData` for the hosted destination
with the adapter never called, and an answer from the local one under a policy
document that permits `Restricted` there. A third test records the one
destination-sensitive part of the contract: a `max_cost_micros` bound is
honoured by the priced hosted model and refused with `UnpricedCost` by the
unpriced local one, per design section 7.

`ui/src/ai/ai.acceptance.test.tsx` (3 tests) drives the panel through health
and credential facts, an empty order that says failover is off, two additions
and a reorder that round-trip through `ai_set_provider_order`, and a re-save
that sends no `credential` key; every call is asserted against both the tauri
command name and the envelope command and capability.

Neither acceptance test drives `AppState::ai_complete` to a success: the
handler builds its registry privately from real HTTP adapters, so there is no
seam a fixture provider can enter through, and `ai_ipc.rs` covers only its four
refusal paths. The handler's success path — `complete_model`, `finish_ai`, the
serialized `ModelResponse` — has no test on this branch. The missing seam is the
same one open decision 10's follow-up has to build.

**The third Rust bullet is not asserted.** Nothing writes to `AiRequestStore`,
so there is no request row to count; see open decision 10 in the design. A test
that called `record_request` by hand would be a green check on a path the
application never takes.

Gates on the branch: `cargo fmt --check` clean, `cargo clippy --all-targets
--all-features -D warnings` clean, `cargo test` 642 passed (639 before),
`npm run format:check`, `lint`, `typecheck` clean, `npm test` 230 passed
(227 before).

---

### Task 14: Mount the Provider Surface and the Incident Workspace

Closes open decision 9. The user's call, 2026-09-06: mount both. The provider
surface goes into the existing `integrations` area, not a new nav member —
design section 12 places it in "the existing connector/model status area", and
`ux-ui-concept.md`'s nav tree has no separate AI-admin area to add one to.

**Files:**
- Modify: `ui/src/shell.tsx`, `ui/src/shell.test.tsx`
- Modify if a key is missing: `ui/src/locales/en.ts`, `ui/src/locales/th.ts`

**Grounding.** `Integrations` (`shell.tsx:478`) owns the connector list inside
the `integrations` area. `AiProviderPanel` already composes `AiProviderForm` and
`AiFallbackOrder` itself, so mounting is one child plus the `providerOrder`
state it reads and writes: its props are `{ invoke, providerOrder,
onProviderOrderChange? }`. `IncidentWorkspace` takes `{ invoke }` and nothing
else. `"incidents"` is already in the `Area` union and the `areas` list, so it
needs a branch in the `shell-main` conditional, not a new nav entry — today it
falls through to `EmptyState titleKey="shell.routeUnavailable"`.

Do not add an `ai` member to `Area`. Do not restyle either component; this task
routes to what Tasks 12 and Sprint 16 already built and tested.

- [ ] **Step 1: Write the failing test**

In `ui/src/shell.test.tsx`, against the shell — not against either component in
isolation, which is what already passes:

- selecting the `incidents` nav entry renders the incident queue, and
  `shell.routeUnavailable` is not in the document;
- selecting `integrations` renders both the connector list and the AI provider
  panel;
- the fallback order the panel reports through `onProviderOrderChange` is the
  order the panel is re-rendered with — assert the round trip, not just that the
  handler fired;
- both mounted surfaces receive the same `invoke` the shell was given, so the
  capability envelopes stay the real ones.

- [ ] **Step 2: Run test to verify it fails**
- [ ] **Step 3: Implement**
- [ ] **Step 4: Run the gate and commit**

Two commits, the incident mount second so it can be dropped on its own:

```bash
npm run format:check && npm run lint && npm run typecheck && npm test
git commit -m "feat(ai): mount the provider surface in the integrations area"
git commit -m "feat(incident): route the incidents area to the workspace"
```

Done: `0b109b0` and `e380b7f`, 232 frontend tests (230 before). `AiProviderPanel`
renders inside `Integrations` beside the connector list — the early returns became
a `connectorContent` expression so the panel is not lost behind a loading or empty
connector state — and the `incidents` branch renders `IncidentWorkspace` with the
shell's own `invoke`, asserted by a test that checks `shell.routeUnavailable` is
gone rather than only that a heading appeared.

Reviewing this mount against the backend is what found open decision 11; the
provider-order state this task seeded with `[]` had nothing truthful to read
from. Task 16 removed the seed.

---

### Task 15: Wire the Audit Store

Closes open decision 10. The user's call, 2026-09-06: fix the contracts, do not
record successes only. Every sub-decision below is settled; implement them, do
not re-open them.

**Files:**
- Modify: `crates/thalassa-domain/src/lib.rs` (`ModelAttempt`),
  `crates/thalassa-ai/src/gateway.rs` (`GatewayError`),
  `crates/thalassa-ai/src/budget.rs` if seeding needs a constructor
- Modify: `src-tauri/src/ai/store.rs`, `src-tauri/src/app/ai.rs`,
  `src-tauri/src/app/mod.rs` (`AppState`)
- Modify: `src-tauri/tests/ai_store.rs`, `src-tauri/tests/ai_ipc.rs`,
  `src-tauri/tests/ai_acceptance.rs`, `crates/thalassa-ai/tests/gateway.rs`
- Modify: `ui/contracts/ipc.ts` and its guards only if a serialized shape moves

**The four settled decisions:**

1. **A refusal records with no attempt.** `record_request` rejects an empty
   attempt list today, but the policy and budget checks run before the first
   attempt is pushed. Relax the check to: a *successful* outcome records at
   least one attempt; a refusal may record none. Design section 3 promises a
   record per request, and a denial is the row an auditor most wants.
2. **`GatewayError` carries the attempts.** Every failing path drops the vector
   the gateway built, so a failover that burned tokens before giving up is
   unrecoverable from the return shape. Either add the field to each variant
   that can follow an attempt or wrap the whole error — prefer the wrapper
   (`GatewayFailure { error, attempts }`) so the variants stay readable and no
   caller can forget the vector.
3. **Per-attempt usage is `Option<ModelUsage>`.** `AiAttemptRecord.usage` is
   `ModelUsage` today and `ModelUsage` derives `Default`. Make the recorded
   usage optional and let `None` mean *not observed*. Never write
   `ModelUsage::default()` for an attempt that reported nothing: a zero row
   passes every validator and states something no one observed. That is the
   Sprint 16 Task 12 defect, and it is the reason this task exists.
4. **The ledger is seeded from the store.** `complete_model` builds
   `BudgetLedger::new()` per request, so `WindowBudget` never accumulates.
   Read the principal's window usage back out of `ai_requests` and seed the
   ledger with it (`BudgetLedger::with_window` exists; add a seeded constructor
   if the accumulated usage cannot be set through it). Section 7's window
   accounting reads usage out of the store, so this is the same wiring, not a
   second feature.

**The seam that makes the test real.** `build_registry` is private and
constructs real HTTP adapters, so no fixture provider can enter through the IPC
layer and `ai_ipc.rs` covers only refusals. Add a registry injection seam to
`AppState` as part of this task. **The acceptance for this task is a test that
drives `AppState::ai_complete` to a success through that seam and then asserts a
real row in `ai_requests` with its attempt rows.** A test that calls
`record_request` by hand is a green check on a path the application never takes;
Sprint 16 shipped six defects that were green exactly that way.

- [ ] **Step 1: Write the failing tests**

- a success through `ai_complete` writes one request row and one attempt row
  with the usage the provider reported;
- a policy denial through `ai_complete` writes a request row with the deny
  reason and no attempt row;
- a failover — first provider fails, second answers — writes both attempts in
  ordinal order, the failed one with `usage: None`, not a zero;
- a second request from the same principal is refused by the window budget that
  the first request's recorded usage exhausted.

- [ ] **Step 2: Run tests to verify they fail**
- [ ] **Step 3: Implement**
- [ ] **Step 4: Run every gate and commit**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
npm run format:check && npm run lint && npm run typecheck && npm test
git commit -m "feat(ai): record every model request in the audit store"
```

Done: `a488cc2`, 646 Rust (643 before) and 234 frontend (unchanged). All seven
gates green. All four decisions landed as settled:

- `record_request` refuses an empty attempt list only when `outcome.error` is
  `None`, so a refusal records a request row with no attempt;
- `GatewayFailure { error, attempts }` wraps every failing path, and the
  deadline and cancellation early returns were moved *after* the attempt is
  pushed, so an attempt that burned tokens before the deadline is no longer
  dropped;
- `ModelAttempt.usage` and `AiAttemptRecord.usage` are `Option<ModelUsage>`; the
  gateway writes `Some(usage)` on an answer and `None` on a failure, and no zero
  is invented anywhere;
- `complete_model` seeds `BudgetLedger::with_window_usage` from
  `AiRequestStore::window_usage`.

The seam is `AppState::ai_registry_override` with a `with_ai_registry` builder,
and the four tests drive the real handler rather than the store: a success
records one request row with the reported usage, a failover records both
attempts with the failed one's usage `None`, a second request is refused with
`window_input_tokens` from the first request's recorded usage, and — in
`ai_ipc.rs` — a policy denial records a request row with `ImmutableRestrictedData`
and zero attempt rows.

Two limitations surfaced in review and are recorded as design debts 12 and 13:
`WindowBudget` has no period, so `window_usage` sums the principal's whole
history and the window never rolls; and nothing in production sets a window
budget, so the bound the accounting now supports is not yet configurable.

---

### Task 16: Read the Fallback Order Back

Closes open decision 11, found on 2026-09-06 while reviewing Task 14's mount
against the backend. Run this **before** Task 15: it touches
`src-tauri/src/app/ai.rs` and `ui/contracts/ipc.ts`, both inside Task 15's
boundary, and it is small enough that Task 15's heavier edits should land on
top of it rather than the other way round.

**Files:**
- Modify: `src-tauri/src/ai/config.rs` if a getter is missing,
  `src-tauri/src/app/ai.rs`, and the IPC descriptor module that
  `ai_providers_descriptor` lives in
- Modify: `ui/contracts/ipc.ts` and its guards, `ui/src/ai/AiProviderPanel.tsx`
- Modify: `src-tauri/tests/ai_ipc.rs`, `ui/src/ai/AiProviderPanel.test.tsx`,
  `ui/src/ai/ai.acceptance.test.tsx`, `ui/src/shell.test.tsx`

**Grounding.** `ProviderConfiguration` is `{ id, kind, endpoint, models }` and
`ProviderSummary` adds only `health` and `credential_configured` — neither
carries a position, because `ProviderConfigStore` keeps the order in a separate
`provider_order: Vec<String>`. Do not add a position field to either; the order
is a property of the set, not of a provider.

Add `ai.provider_order` as a `ConnectorRead` command returning `Vec<String>`,
mirroring `ai_providers`: same authorization, same empty-payload parse, same
`finish_ai`. Additive only — do not change `ai.providers`' return shape, which
would break Task 11's `isProviderSummary` array guard and Task 13's acceptance
mock for no gain.

In `AiProviderPanel`, fetch the order inside `loadProviders` alongside the
providers so a mounted panel shows what is configured. The `providerOrder` prop
becomes optional: a caller may seed it, but the panel no longer depends on one
to be truthful.

- [ ] **Step 1: Write the failing tests**

- Rust: `ai_provider_order` returns the configured order, and refuses the same
  four ways `ai_providers` does (wrong command, wrong capability, bounded scope,
  non-empty payload);
- **the one that would have caught this**: mock `ai_provider_order` returning
  `["openai", "ollama"]`, assert both render in the fallback region in that
  order on mount, then add a third provider and assert the payload sent to
  `ai_set_provider_order` is `["openai", "ollama", <third>]` — not `[<third>]`;
- add the same case to `ai.acceptance.test.tsx`, which is the sprint's exit
  check and today proves only the empty-to-populated direction.

- [ ] **Step 2: Run tests to verify they fail**
- [ ] **Step 3: Implement**
- [ ] **Step 4: Run every gate and commit**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
npm run format:check && npm run lint && npm run typecheck && npm test
git commit -m "feat(ai): read the configured fallback order back through IPC"
```

Done: `2a07dfe`, 643 Rust (642 before) and 234 frontend (232 before). All seven
gates green. `ai_provider_order` mirrors `ai_providers` exactly — same
authorization, same empty-payload parse, same `finish_ai` — and is registered in
`main.rs`, which turned out to be the only `invoke_handler` in the tree and so
had to be added to the file boundary mid-task. The panel now reads the order in
`loadProviders` and the `providerOrder` prop is optional, so `Integrations`
passes only `invoke` and no longer asserts an empty order on the panel's behalf.

The regression test is the one that would have caught the defect: with
`["openai", "ollama"]` configured, adding a third provider sends
`["openai", "ollama", "vllm"]`, not `["vllm"]`. The same case is in
`ai.acceptance.test.tsx`.

---

## What this plan deliberately does not do

- No streaming, no tool calling, no Gemini, no prompt templates. Design 3.2.
- No content persistence. Sprint 19 adds it with Sprint 18's redaction.
- No retry against a provider that just failed; failover moves on.
- No chat UI. Sprint 19 owns the assistant.
