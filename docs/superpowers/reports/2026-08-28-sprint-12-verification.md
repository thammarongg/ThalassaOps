# Sprint 12 Resource Topology Verification

Date: 2026-08-28  
Branch: `sprint-12-resource-topology`  
Comparison base: `main` (`745af3040039c078c95c6888e9fe028953832d59`)  
Reviewer: independent verification pass

## Executive result

The complete Sprint 12 branch was reviewed against `main`, including the seams between the Rust topology engine and the React contract/UI. The deliverables are implemented, traceable to source and tests, and the exit criterion is credibly met: an incident can show affected resources and probable dependency paths, with every displayed relationship linked to admitted evidence.

All required gates pass at the end of this pass. No code defect is deliberately left open; the remaining open items are the fixture-backed scope of Sprint 12 and pre-existing dependency audit advisories.

## Deliverable traceability

| Deliverable | Implementation | Verification |
| --- | --- | --- |
| Service/resource graph | `src-tauri/src/topology/derive.rs`, `fixtures.rs`, `traversal.rs`; provider-neutral `TopologyNode`, `TopologyEdge`, and `TopologyPath` contracts | `src-tauri/tests/topology_engine.rs`, `crates/thalassa-domain/tests/topology_contracts.rs`, UI contract and fixture-parity tests |
| Ownership and team mapping | `src-tauri/src/topology/ownership.rs`; explicit label, resource scope, environment default, fixture, and unassigned outcomes | `src-tauri/tests/topology_ownership.rs`, team-filter tests, unassigned-owner assertions |
| Upstream/downstream impact | `src-tauri/src/topology/traversal.rs`; bounded direction/depth traversal, cycle closure, depth-limit reporting, edge confidence | traversal and engine tests for both directions, depth zero/limit, cycles, orientation, and confidence |
| Environment/Team/Incident filtering | `src-tauri/src/topology/filter.rs` and `topology/mod.rs`; intersection semantics and incident root precedence | `src-tauri/tests/topology_filters.rs`, engine tests, React workspace/acceptance tests |
| Graph-to-evidence navigation | evidence-bearing nodes, edges, paths and metrics; `TopologyEvidenceStore`; `TopologyEvidencePanel.tsx` and selectable graph/path UI | IPC evidence tests, contract guards, cross-navigation acceptance tests, trusted native-link test |
| Capability and scope enforcement | `src-tauri/src/app/topology.rs`; read-only snapshot/evidence descriptors and real command-path authorization | `src-tauri/tests/topology_ipc.rs`, app topology unit tests, descriptor contract tests |
| Rust/TypeScript contract seam | `crates/thalassa-domain/src/lib.rs`, `ui/contracts/ipc.ts`, `ui/contracts/guards.ts`, and mirrored fixtures | Rust/TypeScript shape round-trip, runtime guard, count, evidence, path, and fixture-parity tests |

## Defects found and fixed

The following are the coherent defect groups found during independent review. The commit subjects are included to keep each correction traceable without relying on an unreviewed implementation claim.

### Contract, graph, and evidence integrity

- `92da733 fix: include node evidence in topology paths` and `3a6b034 fix: route topology relations to evidence` make every listed path record the evidence of each node and edge and route relationships to the evidence destination.
- `587526a fix: harden topology contract validation`, `4a42ad8 fix: validate topology summary counts`, `91e9c92 fix: validate topology path confidence`, `b083ca2 fix: require complete topology path evidence`, and `3f54423 fix: reject empty topology contract text` close Rust/UI drift around summary counts, path orientation, cycle closure, confidence, evidence unions, and empty display fields.
- `a9f2f2f fix: require topology node focus identities` aligns node topology drill-down keys with the backend-issued node ID. `ee4b2a2 fix: align topology UI contract fixtures` removes fabricated UI-only nodes, edges, evidence, paths, counts, and incident IDs and verifies parity with the Rust fixture.
- `81f990a fix: require evidence identity matches`, `062b4ea fix: prevent ambiguous topology evidence matches`, `8a728a2 fix: prefer typed topology evidence identity`, `9decba3 fix: require typed evidence for topology resources`, `68026be fix: scope kubernetes evidence by namespace`, and `30ba8f3 fix: preserve native topology evidence matching` prevent similar names, kinds, namespaces, or environments from borrowing unrelated evidence.
- `975071a fix: reject conflicting topology evidence`, `2119b0a fix: reject conflicting topology edge identities`, `eb526a2 fix: reject ambiguous topology resource identities`, `c3b060d fix: reject conflicting topology node identities`, `9c422fb fix: reject conflicting topology observability records`, `36d5bcf fix: reject duplicate topology provenance`, and `bbb9bae fix: reject partial topology evidence references` remove ambiguous records or mark their source unverified rather than choosing by input order or silently dropping a missing reference.

### Incident, filtering, traversal, and provider seams

- `2e5c39e fix: align alert incident queue ids` removes duplicate alert prefixes so Operations incident IDs match topology incident IDs.
- `955a06 fix: honor incident root resolution precedence` and `042ca2f fix: preserve incident source precedence` enforce explicit affected resources first, then a valid source binding, with fixture bindings used only when no source binding was attempted. Unresolved source references no longer fall back to invented roots.
- `22f59eb fix: honor focused topology traversal` makes focus override incident roots for traversal; `20a1fa9 fix: expose topology traversal controls` and `892c11e fix: expose zero-depth topology traversal` expose direction and bounded depth through the UI/IPC seam.
- `604d6df fix: keep kubernetes selectors namespace scoped` and `b83a063 fix: qualify kubernetes owner namespaces` prevent cross-namespace service selection and owner binding. `4233807 fix: reject ambiguous namespace containment` prevents an arbitrary containment parent.
- `af72f5b fix: require exact topology environment matches` removes partial environment-hint matches. `c896435 fix: preserve embedded topology namespaces` preserves namespace identity when embedded source records are used.
- `239212d fix: expose cycle edge provenance`, `ecfe446 fix: include cycle edges in path confidence`, and `431ee7e fix: render bidirectional topology paths` keep the closing relationship visible, evidenced, and confidence-bounded in the probable-path UI.

### Redaction, malformed input, and status handling

- `a4aa730 fix: honor topology redaction boundaries`, `7e9b1ec fix: reject topology credential evidence`, `dde7cd8 fix: reject sensitive topology evidence fields`, and `91e9c92 fix: reject sensitive native evidence links` ensure unsafe excerpts, URLs, credentials, tokens, ARNs, account/subscription identifiers, and pagination cursors are omitted rather than pattern-masked or rendered.
- `2379f40 fix: reject contradictory topology redaction state` rejects evidence marked both unparsed and masked. `da7f0ed fix: enforce topology display redaction` applies the same boundary to inbound UI display data.
- `d1f8f35 fix: omit unsafe topology metric identities`, `1fd90f0 fix: omit malformed topology display keys`, and `b3219b3 fix: downgrade unsafe topology statuses` fail closed for unsafe keys and source fields. `f237f84 fix: preserve unknown topology health state` prevents unknown health from being fabricated as healthy.
- `13120f3 fix: reject negative topology counts`, `12f3025 fix: omit invalid environment counts`, and `7a1cb2c fix: mark invalid replica health unknown` reject negative or malformed counts and expose unknown health instead of inventing a value.
- `2bf1e7c fix: keep topology filter status unique` and `aea44c1 fix: reject duplicate topology source statuses` make source-status output deterministic and reject duplicate identities. `a2cb6c0 fix: align topology egress classification` aligns source retention/UI egress with the Sprint 12 policy classification.

### UI navigation and resilience

- `def088e fix: link topology evidence to trusted sources` restricts native evidence actions to backend-issued HTTPS URLs. `b6140b4 fix: expose topology relationship provenance` and `a746a69 fix: make topology relationships selectable` expose typed provenance, edge sequences, and keyboard-reachable relationship selection.
- `f81b22b fix: clear topology selections outside filtered view` removes stale node/edge focus after a filter response. `d48b533 fix: clear stale topology view on invalid snapshot` clears the prior graph when a later IPC response is invalid instead of showing stale data beside an error.
- `7b0ac45 style: format topology modules` and `2923d49 fix: satisfy topology redaction guard lint` keep the added Rust/UI paths within the repository formatting and lint gates.

### Gate-only corrections

The first final gate run exposed two stale test fixtures caused by the strengthened contracts. The domain snapshot helper now assigns each generated node its own required focus key, and the app command-path test now expects fail-closed omission (`EvidenceMissing`) for contradictory redaction metadata; these are test alignment corrections, not weakened production checks.

## Security, privacy, capability, and causal language

Both new IPC commands are read-only and capability-scoped. The real command path checks the command name, capability, unbounded envelope scope, active principal and membership, workspace grant, role permission, source policy, input scope, and UI egress policy; foreign-scope IDs are denied through the command path. Evidence requests are resolved all-or-nothing from IDs emitted by the current snapshot.

The admitted fixture and final snapshot contain no credentials, tokens, ARNs, account IDs, subscription IDs, or pagination cursors. Unsafe source records and unsafe UI display values are omitted or downgraded to unverified, and native links are revalidated before opening. Relationship paths are labeled probable structural paths and preserve provenance; they do not claim proven causation.

## Accessibility and localization

The new topology strings are present in both `en.ts` and `th.ts`, with locale key parity covered by the existing i18n test. Direction/depth controls, node and edge selections, path evidence actions, and native evidence actions are keyboard reachable; graph relationships are buttons with accessible labels and pressed state, and focus-visible styling remains available.

## Acceptance evidence

The cross-navigation acceptance tests cover opening topology from an incident, retaining the incident filter, showing the affected checkout resource and probable dependency paths, and excluding unrelated catalog resources. Additional tests cover direction/depth changes through IPC, zero-depth behavior, cycles and depth limits, graph-to-edge/path evidence navigation, trusted native URLs, malformed snapshots, redaction boundaries, and foreign-scope authorization.

## Final gates

| Gate | Result |
| --- | --- |
| `npm run format:check` | PASS |
| `npm run lint` | PASS |
| `npm run typecheck` | PASS |
| `npm test` | PASS — 10 files, 89 tests |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --workspace` | PASS — 290 tests plus zero-test doc-test targets |

## Deliberately left open

- Sprint 12 remains fixture-backed by design. Live provider-to-topology input adaptation and network integration are outside this sprint’s deliverable and are not represented as fabricated live data.
- `npm ci` reports the pre-existing dependency audit advisories and deprecation/allow-scripts warnings. No dependency change was needed for this verification pass; remediation is a separate dependency-maintenance task.
- No known code, contract, policy, redaction, accessibility, localization, or acceptance defect remains open after the final gates.
