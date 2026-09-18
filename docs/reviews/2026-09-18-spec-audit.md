# Spec audit, 2026-09-18

**Purpose:** find contradictions between the project's documents, then judge
whether Sprint 18 (Context optimization and redaction) is ready to design.
**Output:** findings only; the audit itself changed no document.
**Follow-up:** F1–F16 were applied on 2026-09-18, in the commit after this report.
**Precedence used:** requirements and policy > ADR > UX spec and governance
reconciliation > sprint-plan and v0.1 > old sprint designs. When code
contradicts a requirement, the finding asks the user rather than assuming the
requirement wins.
**Scope:** requirements, policy, ADRs, planning, the UX layer and `CONTEXT.md`.
Sprint designs 8–17 were treated as history and are reported only where a
current document still relies on them.

**Labels:** **FIX-NOW** means a mechanical correction that needs no
judgement. **DECISION** means you have to choose which side is right.

## How this was checked

Four read-only agents covered four lanes: requirements against planning, the UX
layer against the documents above it, Sprint 18 readiness, and glossary drift.
Every agent had to quote both sides at `path:line`. The findings the agents
disagreed on, and every finding that bears on Sprint 18, were then re-read at
source. Three agent claims turned out to be wrong and are corrected here:

- **Token count.** An agent reported 34 tokens in `:root`, making the
  playbook's "33 tokens" stale. The 34th line it counted is a comment line in
  `--control-edge`'s explanation that begins with `--surface`. There are 33
  real tokens, so the playbook is correct.
- **Dispositions.** An agent said the code ships four dispositions. It ships
  five, including `Informational` (`crates/thalassa-domain/src/lib.rs:427-438`),
  which matches the policy baseline. Only `requirements-summary.md:145` lists
  four, so this is FIX-NOW, not a decision.
- **Class-asserting tests.** The playbook's claim of four test files is wrong,
  as an agent reported. Only `shell.test.tsx` and
  `design-system/components.test.tsx` assert class names.
  `IncidentNarrative.test.tsx` queries the `time` element, and
  `operations-console.acceptance.test.tsx` uses `data-widget-id` and
  `data-testid`.

---

## 1. Decisions Sprint 18 cannot start without

These are contradictions between documents, or between a document and the
code, and they sit directly on Sprint 18's path. Settle them in the design
interview.

### D1. Gemini has no owning sprint *(verified)*
- `docs/planning/sprint-plan.md:322` (Sprint 17): "OpenAI, Anthropic, Gemini and OpenAI-compatible provider adapters."
- `docs/design/sprint-17-ai-provider-gateway.md:111`: "**Gemini.** Deferred to Sprint 18 by binding decision 9."
- Sprint 18 in `sprint-plan.md:334-341` does not mention Gemini, and neither
  does `v0.1-definition.md`. Gemini is required by
  `requirements-summary.md:171`.
- **Choose:** put Gemini in Sprint 18, move it to a named later sprint, or
  record a deferral. Then update the plan to match.

### D2. The `ALLOW` redaction action *(verified)*
- `docs/policies/operational-policy-baseline.md:186`: "`ALLOW`: permit only for a named provider, model, workspace and purpose."
- `sprint-plan.md:338` and `v0.1-definition.md:63` list only five actions:
  drop, mask, hash, truncate and aggregate.
- Today, `hosted_ai_data_classes` is one global list. There is no per-provider
  or per-model permission.
- **Choose:** add `ALLOW` to Sprint 18, or defer it to Sprint 20. Without it,
  Confidential data can never reach an approved provider.

### D3. Can Restricted or secret data reach a local model? *(verified)*
- `docs/adr/0005-provider-neutral-ai.md:7`: "immutable Restricted data remains protected regardless of model location."
- `operational-policy-baseline.md:242`: "immutable `Restricted` data remains blocked by default."
- Code, `crates/thalassa-policy/src/lib.rs:213-219`: the immutable deny
  applies only to `HostedAi | ExternalIntegration | AuditLog`. For
  `LocalModel`, the only check is the editable `local_model_data_classes`
  list. So a payload flagged `contains_immutable_secret: true` at a class the
  list allows is permitted to a local model.
- **Choose:** decide whether the rule for local models is immutable (the ADR)
  or a default (the policy), then amend the losing document. A related
  question: does a remote vLLM or Ollama endpoint count as local?
  `gateway.rs` decides by `kind.is_local()`.

### D4. v0.1 relies on a "policy file" that does not exist *(verified)*
- `docs/planning/v0.1-definition.md:101`: "A single operator needs a policy file, not an administration surface."
- Policy is stored only in the SQLite `policy_store`, and there is no file
  loader. The only import mechanism is Policy Center (Sprint 20), which v0.1
  excludes.
- As things stand, no v0.1 user can choose, preview or activate the redaction
  actions Sprint 18 builds.
- **Choose:** baseline-only redaction in v0.1, a minimal file import with
  validate, preview and version, or a thin slice of Sprint 20.

### D5. The redaction preview and the fail-closed state have no screen *(verified)*
- Required by `sprint-plan.md:341` ("Redaction preview and policy failure
  behavior"), by `v0.1-definition.md:126` ("shows what was sent to the model
  and what was withheld"), and by `operational-policy-baseline.md:210`
  ("The user should see the reason and a safe recovery path").
- The UX spec and the reconciliation design specify only the one-line
  `sensitive fields redacted: N`. They define no preview view and no failure
  message.
- **Choose:** where the preview lives, and whether it is ephemeral or
  persisted. Sprint 17 stores no prompt content, so a persisted preview means
  Sprint 18 owns a content store. Also decide whether the preview ever shows a
  withheld raw value, and where it ends and Sprint 20's "Redaction test
  payloads" begin. The last point implies the preview has to be a Rust
  contract, not just a UI.

### D6. Nothing defines or produces the count N
- The reconciliation design has two counts on one card: `:128` "Data
  omitted/redacted | Count + reason text", and `:164` `sensitive fields
  redacted: N`.
- Every signal in the code is a boolean: `EvidenceRedaction.masked`,
  `LogEntry.masked` and the manifest's `masked`.
- The labels also differ. The baseline has typed labels (`<REDACTED:EMAIL>`),
  while the code uses a single `"<REDACTED>"`.
- **Choose:** what N counts (fields, matches, or dropped and aggregated
  records), whether "omitted" and "redacted" are one count or two, and the
  label format.

### D7. Only two of the five policies are separate in the code
- The requirement and the policy separate send, store, display, export and
  audit (`operational-policy-baseline.md:216-220`, `sprint-plan.md:339`).
- The code allows `LocalStorage | Ui => true` unconditionally
  (`thalassa-policy/src/lib.rs:235`, *verified*), and `PolicyDocument` has no
  store or display lists.
- **Choose:** the policy schema change and its migration path for the stored
  `document_json`.

### D8. Who classifies data, and does redaction change the class?
- About 30 call sites assert `EgressRequest::verified(DataClass::Internal, …)`
  instead of classifying the data. The baseline puts "private logs" and
  "topology detail" in Confidential, so today's callers under-classify.
- `gateway.rs:218` always sends `contains_immutable_secret: false`
  (*verified*). This is Sprint 17's recorded debt.
- **Choose:** classification by source, by field or by content, with a default
  for each connector. Decide whether a masked Confidential field becomes
  Internal, and where the classifier's output type lives, given that the domain
  crate cannot depend on the policy crate.

### D9. Secret detection: three keyword lists and no detector
- `observability/masking.rs:3-8` has 5 key-name words, matched on key names
  only. `correlation/source_records.rs:929-951` has 18 markers.
  `thalassa-domain/src/lib.rs:5026-5045` has 16 markers, and the three lists
  disagree.
- A log line that is not JSON is returned raw with `masked: false`
  (`observability/loki.rs:160-171`, *verified*). So the v0.1 demo claim that
  "a secret in the log output appears masked" (`v0.1-definition.md:126`)
  fails on free text.
- `incident_unsafe_content` rejects a whole incident text on any keyword,
  while the baseline asks for `DROP` or `MASK`. The digit-run false positive
  fixed on 2026-09-15 came from this same function.
- **Choose:**
  - what the immutable detector is: value patterns, entropy, Luhn or PII
  - how its patterns are versioned without becoming "configurable"
  - whether the three lists fold into the detector
  - whether incident text moves from rejection to masking
  - whether free-text logs are in scope for Sprint 18

### D10. Failure vocabulary
- `v0.1-definition.md:28` "stops the request rather than degrading it" sits
  beside `operational-policy-baseline.md:210` "a safe recovery path". The docs
  never say whether falling back to a local model counts as degrading.
- The only deny reason is `UnverifiedClassificationOrRedaction`.
- A corrupt policy document stops the app from starting, and shows no reason.
- **Choose:** the deny reasons, the recovery path, and what storage and
  display do when classification fails.

### D11. `HASH` and the pipeline order
- The baseline says "deterministic non-reversible identifier". An unkeyed hash
  of an email or IP address can be reversed with a dictionary. A keyed hash
  needs a key store.
- The baseline also puts "summarize" before "validate egress"
  (`operational-policy-baseline.md:201-203`). If summarizing calls a model,
  that is egress before validation.

**Beyond D1–D11:** the readiness lane also produced interview questions on the
context budget, the test corpus and "correct redaction", and the list of
hosted-egress paths the secret-leak tests must cover. Those tests were pulled
into v0.1 by `v0.1-definition.md:149`. See the knock-on section below.

### Knock-on from Sprints 19–25 into the Sprint 18 design
- **Sprint 19** is the first automated caller. It needs a verified
  classification, a redacted Assistant Log with policy version, redaction rule
  version and context fingerprint, and the count and budget fields.
- **Sprint 20** has redaction test payloads and an effective-policy preview, so
  the rule schema and the preview must be data-driven and callable without a
  live request.
- **Sprint 22:** terminal masking needs the detector to work on free text.
- **Sprint 23:** integrations derive their egress allowlist from Sprint 18's
  classification.
- **Sprint 24:** roles decide who may see withheld content in the preview.

---

## 2. Other decisions (not blocking Sprint 18)

| # | Conflict | Sides | Note |
|---|---|---|---|
| O1 | Sprint order against the priority order | `requirements-summary.md:73` says the sprint plan must follow the priority ordering: compliance is 7th, remediation 8th. But Sprint 21 (approval and actions) comes before Sprint 25 (security and compliance). | Reorder the sprints, or relax §6. |
| O2 | v0.1 against actions in the UX spec | `v0.1-definition.md:100` "Any mutating action … read-only by definition" vs `aiops-command-center.md:172-173` "Acknowledge, Run rollback runbook" | Add a v0.1 exception, or mark those actions as Sprint 21/22. |
| O3 | Where the risk-class and execution-mode pills apply | The requirements, UX spec and playbook invariant say every action and command surface. The reconciliation design (`:140-141`) says the Runbooks view only. The code renders no pill anywhere, and `risk_class` in `ipc.ts:214` is never shown. | Decide whether local incident actions count as actions. |
| O4 | ADR 0006 against AI traffic | `docs/adr/0006-integration-transport-policy.md:19` "GET-only reads", while the AI adapters `.post(…)` (`anthropic.rs:164`, `local.rs:125`, `openai_compatible.rs:115`) *(verified)* | Amend ADR 0006 to scope AI requests, or write a new ADR. |
| O5 | "Show evidence" has no missing-source field | `v0.1-definition.md:124-125` and `requirements-summary.md:186` ("State when evidence is missing or contradictory") vs the reconciliation table at `:127-128` | Add the field. The requirement wins. Affects Sprint 19. |
| O6 | The `CriticalNumberLink` open question rests on the reversed model | `aiops-command-center.md:405-407` vs the reconciliation design `:126`, which already reuses the mechanism | Close it. |
| O7 | Incident priority | `requirements-summary.md:125` keeps severity and priority separate, and the UI renders priority (`OperationsConsole.tsx:429`). But `Incident` has no priority field, and the reconciliation design's backlog item (`:289-293`) has no sprint. | Assign it a sprint. |
| O8 | Incident widget can be hidden | `requirements-summary.md:284` vs `aiops-command-center.md:118` "hide/show" *(verified)*. The code already pins the widget (`OperationsConsole.tsx:95`). | Fix the spec so a rebuild doesn't regress the code. |
| O9 | Where the governance rules came from | The reconciliation design `:34-35` says all five rules come from the requirements. "Tide Line" and the literal `sensitive fields redacted: N` appear in no requirement. | They are UX-owned rules, so changing them is Class C, not a requirements change. Record that. |

---

## 3. FIX-NOW: mechanical corrections

| # | Where | Fix |
|---|---|---|
| F1 | `aiops-command-center.md:412-414` | It still calls the redaction-disclosure rule "dropped", which was reversed on 2026-09-11 *(verified)*. Also `:376` "as of 2026-09-10". |
| F2 | Reconciliation design `:263-264` | "Sections 2, 3, 4 — … Sprint 18 … Sprint 19 … Sprint 22 … respectively" pairs them wrongly *(verified)*. It should read: Section 2 → Sprint 19, Section 3 → Sprints 21/22, Section 4 → Sprint 18. |
| F3 | `aiops-command-center.md:159-160` | The status `Identified` is not in the lifecycle *(verified)*. Use `Mitigating`, per the policy and `IncidentStatus`. |
| F4 | `aiops-command-center.md:271-273` | It points to the 2026-09-10 rebrand plan and its `--color-*` tokens *(verified)*. Repoint to ADR 0007 and the handoff. |
| F5 | Reconciliation design `:272-273` | "use that table, not the handoff README's names" is now inverted by the rename and ADR 0007 *(verified)*. `--link` (`:117`) is not a defined token. |
| F6 | Reconciliation design `:3` | "Approved design (pending user's file review)", while every other document treats it as authoritative *(verified)*. |
| F7 | `ux-ui-change-playbook.md:117-119` | "4 test files assert class names": only 2 do *(verified)*. |
| F8 | `requirements-summary.md:145` | It lists four dispositions, but the policy and the code have five, including `Informational` *(verified)*. |
| F9 | `requirements-summary.md:139-143` | The lifecycle diagram draws Reopened from Mitigating. Policy `:109` and the code allow it only from Monitoring, Resolved or Closed *(verified)*. |
| F10 | `2026-09-10-direction-review.md:69-71` | Sprints 22–24 are marked "Cut". v0.1 `:96` says "deferred with a reason, not forgotten" *(verified)*. Cutting them would drop requirements. |
| F11 | `sprint-plan.md:456` | Sprint 25 lists "OS keychain integration", which shipped in Sprint 5. |
| F12 | `sprint-plan.md:93` | "A signed development build", while v0.1 `:107` excludes code signing. |
| F13 | `system-requirements.md:3` | "Requirements discovery in progress" contradicts its own `:231`. |
| F14 | `direction-review.md:101` | Links `docs/design/2026-09-07-visual-system-pass.md`, which no longer exists. |
| F15 | `sprint-plan.md` | No pointer to the v0.1 narrowing, which the direction review itself notes. |
| F16 | `sprint-17-ai-provider-gateway.md` and `v0.1:55` | v0.1 says "budgets" are built. The Sprint 17 design `:652` says "**Nothing configures a window budget.**" |

---

## 4. `CONTEXT.md` glossary drift

`CONTEXT.md` has not been touched since 2026-08-30. Its glossary entries
(lines 11–121) contain no implementation detail. Lines 125–143, "Confirmed
product boundaries", are requirements rather than terms.

**Changed meaning (verified where marked):**
- **Signal** *(verified)*: `:29` includes metrics, logs, traces and
  deployment changes. The code's `SignalKind` is only alert, anomaly,
  security finding and health check. Metrics, logs and traces are evidence
  sources, and changes are `ChangeEvent`. DECISION.
- **Workspace** *(verified)*: `:17` says it "may represent a personal setup, a
  team or an enterprise boundary". In the code a `Workspace` is a fixed child
  of a `Team`. FIX-NOW.
- **Evidence** lacks its redaction state. There are also two evidence types,
  `Evidence` and `EvidenceRef` (DECISION).
- **AI Assistant Log** says "selected context". The policy says redacted
  context fingerprint, with no unredacted prompts by default. FIX-NOW.
- **Severity, urgency and priority**: the glossary says they are "separate
  fields", but the code has no urgency field and `Incident` has no priority
  field. DECISION.

**Missing, and needed by Sprint 18.** FIX-NOW unless marked:
- Data Class
- Immutable Restricted Data
- Content Declaration (classification and redaction verified)
- Redaction Preview
- Context Fingerprint
- Hosted vs Local model
- Egress Destination: DECISION. Five policies against six code destinations.
- Redaction Action: DECISION. Also collides with the glossary's "Action".
- Redaction count: DECISION, same as D6.
- Request Budget: DECISION
- Provider Gateway / Fallback Order: DECISION. The documents use three names
  (fallback, provider order and failover) for two concepts.

**Missing, other.** All FIX-NOW:
- Priority
- Derived severity and severity override
- Business Impact
- Hypothesis and Confidence
- Correlation Candidate (used at `:41`, never defined)
- Change Event
- Topology
- Organization, Team, Principal and Resource Scope
- Suppression Rule, Maintenance Window, Anomaly and Scheduled Health Check

**Overloaded terms.** Each is a DECISION:
- **Command Center:** the product, the Home view and the Operations Console.
- **Workspace:** the scope, the primary workspace and the Incident Workspace.
- **Autonomy vs Execution Mode:** they duplicate each other.
- **Integration vs Connector.**
- **Severity:** S1–S5, `FindingSeverity`, and SEV1–3 in the mockup.
- **Finding:** an AI finding and a vulnerability finding.
- **Suppressed:** a disposition, `SuppressionState` and a candidate status.
- **Blast radius:** the reach of an incident and a limit on an automated
  action.
- **Action:** the glossary's Action, the redaction actions and UI buttons.
- **AI Assistant Log vs the Sprint 17 audit store.**
- **Skill vs Runbook:** a Skill "does not grant execution privilege", but
  Runbooks are executable.

---

## 5. Categories with no findings

- No current document links to a Sprint 8–17 design as the authority for a
  decision that was later reversed. The one reverse case is D1, where a sprint
  design decided something the plan never recorded.
- No sprint in `sprint-plan.md` depends on work scheduled after it.
- These counts in the direction review and the playbook are still true:
  - 966 locale keys in parity
  - 24 test files querying by role or text
  - 33 `:root` tokens
- The reversal of the 2026-09-10 governance drop is complete everywhere except
  F1.

## Suggested order

1. Apply the FIX-NOW items F1–F16. None needs a decision, and F1–F6 are in
   documents the Sprint 18 interview will read.
2. Bring D1–D11 into the Sprint 18 `/grill-with-docs` session as the opening
   frontier. D3, D4 and D5 decide the sprint's shape and should come first.
3. Add the Sprint 18 glossary terms to `CONTEXT.md` as the interview settles
   them, not before.
4. Take up O1–O9 when their sprint comes up. O4 and O5 touch Sprint 19.
