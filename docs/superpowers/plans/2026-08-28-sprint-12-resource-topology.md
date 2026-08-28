# Sprint 12 Resource and Service Topology Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a deterministic, read-only service/resource topology that can show an existing Sprint 11 queue item’s affected resources and bounded probable dependency paths with evidence.

**Architecture:** Add the canonical Rust topology contracts to thalassa-domain, mirror them exactly in ui/contracts/ipc.ts, and feed a provider-neutral builder from existing Kubernetes, cloud, observability and Sprint 11 queue projections. The builder emits a filtered TopologySnapshot through topology.snapshot (WorkspaceRead/Read) and topology.evidence (ResourceRead/Read); React renders the same contract from copied fixtures while backend work proceeds.

**Tech Stack:** Rust 2021, Tauri 2, Tokio-compatible existing connector modules, Serde, Chrono, SQLite/local-first state already in the repository, React 18, TypeScript, Vite, Vitest, Testing Library and the existing ThalassaOps design system.

**Spec:** docs/design/sprint-12-resource-topology.md

## Global Constraints

- There is one type per concept. Reuse existing ResourceScope, EvidenceRef, DrillDownTarget, DrillDownReference, NumberUnit, ConsoleHealthState, SourceStatus and TeamId; do not create topology aliases or companion representations for them.
- Numeric values are stored as f64 in the topology model and as number in TypeScript. Rust rejects NaN, positive infinity and negative infinity with a typed topology error before serialization. Rust never sends a formatted numeric string for topology confidence or metrics.
- User-visible reasons, statuses, path qualifications and ownership states are typed enums. React maps their wire values to English/Thai i18n keys. Rust does not manufacture user-visible English sentences for those states.
- Absent source data is represented by Option/null, an unavailable source state or an omitted record. An empty string is never used as an absent value. Empty arrays/maps mean that a source was verified and has no members; they do not mean that a source failed.
- Every displayed critical number or node metric has evidence IDs and a typed drill-down reference. A renderable node, edge and path also has at least one verified evidence ID. Unverified source records are not rendered as trusted graph facts.
- All IPC JSON fields use the existing snake_case convention. Enum wire values are explicit and stable. The TypeScript types in the design are the exact mirror of the Rust serialized shape, not a second UI model.
- Do not provision infrastructure, run Terraform or OpenTofu, apply Kubernetes changes, capture new live cloud fixtures, or add a network integration.
- The Incident filter consumes only the Sprint 11 IncidentQueueItem projection and its fixtures. Do not add an incident entity, incident lifecycle, incident write, responder role, assignment, comment, notification or action.
- Signal normalization, signal deduplication, correlation windows, maintenance windows, suppression and explainable correlation reasons remain Sprint 13 work.
- TopologyPath.kind is always probable_structural; no implementation may present a structural path as a proven causal chain or root cause.
- No AI investigation, model call, mutation proposal, terminal execution or remediation is part of this sprint.
- New IPC commands are exactly topology.snapshot (WorkspaceRead, Read) and topology.evidence (ResourceRead, Read). No command accepts a provider URL, provider query, credential, rule definition or mutation.
- Command authorization checks command name, capability, unbounded envelope scope, active membership, principal identity, workspace grant and role permission before topology work; UI egress is checked before return.
- Existing Kubernetes, observability and cloud transport, credential, masking, sanitized-error and policy paths remain authoritative. The topology adapter consumes their results and never reimplements provider HTTP or credential resolution.
- Existing UI masking semantics remain authoritative: an unparsed excerpt is not marked masked, and immutable Restricted data is blocked fail-closed.
- The exact exit criterion is: "An incident can show affected resources and probable dependency paths."
- Run npm ci before any frontend gate. A gate that cannot run is blocked and must be reported; it is not a passing gate.

---

## File map and parallel handoff

Task 2 is the synchronization point for the two developers. It defines the Rust and TypeScript shapes and commits copied fixtures. After Task 2:

- the Rust backend worker owns Tasks 3–6: src-tauri/src/topology, src-tauri/src/app/topology.rs, crates/thalassa-domain and crates/thalassa-ipc; and
- the React UI worker owns Task 7: ui/src/topology, ui/src/shell.tsx, locale files and styles, building against ui/src/topology/topology-fixtures.ts without importing Rust code.

Task 8 starts only after both workers have completed their task-level tests. No worker changes a contract field name, enum wire value, nullability rule or fixture ID without updating the design and the copied fixture in the same change.

### Task 2: Define topology contracts and deterministic fixtures

**Files:**

- Modify: crates/thalassa-domain/src/lib.rs — add TopologyNodeKind, TopologyOwnershipSource, TopologyOwnership, TopologyMetric, TopologyNode, TopologyEdgeKind, TopologySourceKind, TopologyEdgeProvenance, TopologyEdge, TopologyDirection, TopologyPathKind, TopologyPathTermination, TopologyPath, TopologyTraversal, TopologyFilter, TopologyRequest, TopologySummary, TopologySnapshot and TopologyEvidenceRequest; extend DrillDownDestination with topology.
- Create: crates/thalassa-domain/tests/topology_contracts.rs — Rust JSON shape, round-trip, validation and finite-number tests.
- Modify: crates/thalassa-ipc/src/lib.rs — add topology_snapshot_descriptor() and topology_evidence_descriptor().
- Modify: crates/thalassa-ipc/tests/contracts.rs — assert both descriptors and their command names.
- Create: src-tauri/src/topology/mod.rs — declare the topology module and re-export domain contracts.
- Create: src-tauri/src/topology/fixtures.rs — define TopologyInput, TopologyOwnershipSelector, TopologyOwnershipRule and the deterministic input fixture.
- Modify: src-tauri/src/lib.rs — export pub mod topology.
- Modify: ui/contracts/ipc.ts — add the exact TypeScript mirror from the design and topology to DrillDownDestination.
- Create: ui/src/topology/topology-fixtures.ts — copy the asserted Rust JSON into a typed TopologySnapshot fixture.
- Create: ui/src/topology/topology-contracts.test.ts — validate copied fixture keys, enum values and finite numeric fields.

**Interfaces:**

- Consumes: existing ResourceScope, EvidenceRef, DrillDownTarget, DrillDownReference, NumberUnit, ConsoleHealthState, SourceStatus, TeamId, KubernetesInventory, CloudResource, EnvironmentStatus, NormalizedAlert, MetricFixture and IncidentQueueItem.
- Produces: the exact Rust and TypeScript wire contracts in docs/design/sprint-12-resource-topology.md, TopologyInput, topology_fixture_input(scope), fixture_time() and the two IPC descriptors.

**Tests to add:**

- literal JSON assertions for every topology enum and the additive topology drill-down value;
- round-trip serialization of a full TopologySnapshot containing a node, edge, cycle-terminated path, metric and all filter fields;
- field-name/nullability parity for the copied TypeScript fixture;
- rejection of NaN, positive infinity, negative infinity and confidence outside [0.0, 1.0] before serialization; and
- fixture stability, required evidence, explicit incident_id: null, and stable IDs for the checkout service, workload, pod, environment, cloud resource and dependency edges.

- [ ] **Step 1: Write the failing Rust contract tests**

Create crates/thalassa-domain/tests/topology_contracts.rs with these representative assertions and one assertion for every enum member in the design:

~~~rust
#[test]
fn topology_wire_values_and_request_shape_are_stable() {
    assert_eq!(serde_json::to_value(TopologyNodeKind::Service).unwrap(), json!("service"));
    assert_eq!(serde_json::to_value(TopologyEdgeKind::DependsOn).unwrap(), json!("depends_on"));
    assert_eq!(serde_json::to_value(TopologySourceKind::Kubernetes).unwrap(), json!("kubernetes"));
    assert_eq!(serde_json::to_value(TopologyDirection::Both).unwrap(), json!("both"));
    assert_eq!(serde_json::to_value(TopologyPathKind::ProbableStructural).unwrap(), json!("probable_structural"));
    assert_eq!(serde_json::to_value(TopologyPathTermination::CycleDetected).unwrap(), json!("cycle_detected"));
    assert_eq!(serde_json::to_value(DrillDownDestination::Topology).unwrap(), json!("topology"));

    let request = TopologyRequest {
        filter: TopologyFilter {
            environment_ids: vec![],
            team_ids: vec![],
            incident_id: None,
        },
        focus_node_id: None,
        traversal: TopologyTraversal {
            direction: TopologyDirection::Both,
            max_depth: 3,
        },
    };
    assert_eq!(serde_json::to_value(request).unwrap(), json!({
        "filter": { "environment_ids": [], "team_ids": [], "incident_id": null },
        "focus_node_id": null,
        "traversal": { "direction": "both", "max_depth": 3 }
    }));
}
~~~

- [ ] **Step 2: Run the focused contract test and record the expected failure**

Run: cargo test -p thalassa-domain --test topology_contracts

Expected: FAIL because the topology types and descriptors have not been defined.

- [ ] **Step 3: Add the domain contract types exactly once**

Copy the Rust shapes from the design without adding TopologyNodeId, TopologyEdgeId, TopologyNodeMetric, response aliases or UI-only wrappers. Derive Deserialize and Serialize on every wire type; use PartialEq rather than Eq for values containing f64. Keep TopologyRequest, TopologyFilter, TopologyTraversal and TopologyEvidenceRequest deserializable.

Implement TopologySnapshot::validate() with this signature:

~~~rust
pub fn validate(&self) -> Result<(), TopologyError>;
~~~

It rejects empty IDs, missing evidence, unknown references, duplicate node/edge/path IDs, duplicate provenance identity, non-finite metric/confidence values, confidence outside [0.0, 1.0], path node repetition, path depth above 8, and a path whose kind is not ProbableStructural. It does not return source payload text.

- [ ] **Step 4: Add descriptors and the fixture-only input**

Implement the IPC descriptors as the only command metadata source:

~~~rust
pub fn topology_snapshot_descriptor() -> CommandDescriptor {
    CommandDescriptor::new("topology", "snapshot", Capability::WorkspaceRead, Permission::Read)
}

pub fn topology_evidence_descriptor() -> CommandDescriptor {
    CommandDescriptor::new("topology", "evidence", Capability::ResourceRead, Permission::Read)
}
~~~

In src-tauri/src/topology/fixtures.rs, define the non-IPC TopologyInput shape from the design with kubernetes: BTreeMap<String, KubernetesInventory>, cloud_resources, environments, alerts, metrics, incident_queue, ownership_rules, fixture_edges: Vec<TopologyEdge>, incident_root_nodes: BTreeMap<String, Vec<String>>, source_status and evidence. Add the fixed timestamp 2026-08-28T09:00:00Z.

The fixture must contain:

- env-aws-prod, env-gcp-staging, service/checkout, workload/checkout-api, pod/checkout-api-0, cloud_resource/checkout-rds and a second staging service/workload;
- a depends_on chain with at least three edges, a two-edge cycle for traversal tests, and one Kubernetes owns/selects pair;
- one explicit-label ownership mapping, one ResourceScope.team_id fallback, one unassigned node and the Sprint 11 queue item alert-checkout-s1 mapped to the checkout service; and
- verified fixture evidence for every node, edge, ownership mapping, summary metric and incident root.

Use readable stable fixture keys in IDs (node:fixture:..., edge:fixture:..., evidence-topology-...); no random UUID and no provider credential appears in the fixture.

- [ ] **Step 5: Mirror the contract in TypeScript and test copied JSON**

Add the exact unions and object fields from the design to ui/contracts/ipc.ts. Keep TopologyMetric.value, TopologyEdge.confidence and TopologyPath.confidence as number; keep TopologyNode.metric, TopologyNode.native_id, TopologyNode.environment_id, TopologyNode.provider, TopologyOwnership.team_id, TopologyOwnership.team_name, TopologyRequest.focus_node_id and TopologyFilter.incident_id nullable.

In topology-contracts.test.ts, assert:

~~~ts
expect(typeof snapshot.summary.visible_nodes.value).toBe("number");
expect(Number.isFinite(snapshot.summary.visible_nodes.value)).toBe(true);
expect(snapshot.filter).toEqual({ environment_ids: [], team_ids: [], incident_id: null });
expect(snapshot.nodes.every((node) => node.evidence_ids.length > 0)).toBe(true);
expect(snapshot.edges.every((edge) => edge.evidence_ids.length > 0)).toBe(true);
expect(snapshot.paths.every((path) => path.kind === "probable_structural")).toBe(true);
~~~

- [ ] **Step 6: Run the contract suites**

Run:

~~~bash
cargo test -p thalassa-domain --test topology_contracts
cargo test -p thalassa-ipc --test contracts
npm ci
npm test -- ui/src/topology/topology-contracts.test.ts
npm run typecheck
~~~

Expected: PASS, with Rust JSON and TypeScript fixture field names, enum values, nullability and numeric types identical.

- [ ] **Step 7: Commit the synchronization point**

~~~bash
git add crates/thalassa-domain/src/lib.rs crates/thalassa-domain/tests/topology_contracts.rs crates/thalassa-ipc/src/lib.rs crates/thalassa-ipc/tests/contracts.rs src-tauri/src/topology src-tauri/src/lib.rs ui/contracts/ipc.ts ui/src/topology/topology-fixtures.ts ui/src/topology/topology-contracts.test.ts
git commit -m "feat: define resource topology contracts and fixtures"
~~~

**Acceptance criteria:**

- All Rust and TypeScript types exactly match the design, including null/Option, snake_case fields, f64/number values and explicit enum wire strings.
- The fixture is deterministic, evidence-backed and includes a queue-item incident root, ownership variants, Kubernetes/cloud records, a dependency chain and a cycle.
- Domain validation rejects non-finite values and invalid graph references with typed errors before IPC serialization.
- The two IPC descriptors are the only new command metadata and use WorkspaceRead/Read and ResourceRead/Read exactly.

### Task 3: Derive nodes, edges, provenance and safe evidence from existing sources

**Files:**

- Modify: src-tauri/src/topology/mod.rs — expose TopologyBuilder::from_input and TopologyBuilder::snapshot_at signatures while keeping provider adapters private.
- Create: src-tauri/src/topology/derive.rs — map Kubernetes inventories, cloud resources, environments, alerts and metrics into canonical node/edge types.
- Modify: src-tauri/src/topology/fixtures.rs — construct source records and fixture edges using existing contracts.
- Create: src-tauri/tests/topology_sources.rs — source mapping, stable identity, provenance, confidence and redaction tests.
- Modify: src-tauri/src/lib.rs — ensure topology is exported for integration tests.

**Interfaces:**

- Consumes: TopologyInput, KubernetesInventory, KubernetesResource, KubernetesTopologyEdge, CloudResource, EnvironmentStatus, NormalizedAlert, MetricFixture, ResourceScope and verified EvidenceRef.
- Produces: TopologyBuilder::from_input(input: TopologyInput) -> TopologyBuilder and TopologyBuilder::snapshot_at(&self, request: &TopologyRequest) -> Result<TopologySnapshot, TopologyError> with source-derived nodes, edges, evidence and source statuses. Task 3 may return no paths for an unfocused request; Task 5 adds traversal.

**Tests to add:**

- Kubernetes resource-kind mapping, namespace containment, owner and selector edge mapping;
- cloud EnvironmentStatus/CloudResource mapping and exact environment containment;
- observability alert/metric evidence attachment only for unambiguous resource references;
- stable IDs independent of input ordering and duplicate-edge provenance merging;
- unresolved endpoints, unsupported kinds, out-of-scope records and unverified evidence becoming source status rather than trusted graph facts;
- exact confidence values (1.0 for exact identity, 0.9 for safe owner/selector fallback) and finite-value validation; and
- serialized output scans for password, token, authorization, credential_reference, sk-live- and raw provider error bodies.

- [ ] **Step 1: Write the failing source-derivation test**

~~~rust
#[test]
fn fixture_sources_produce_stable_nodes_edges_and_provenance() {
    let input = topology_fixture_input(fixture_scope());
    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("fixture source records should build");

    assert!(snapshot.nodes.iter().any(|node| {
        node.kind == TopologyNodeKind::Service && node.name == "checkout"
    }));
    assert!(snapshot.edges.iter().any(|edge| {
        edge.kind == TopologyEdgeKind::Owns
            && edge.provenance.iter().any(|item| {
                item.source == TopologySourceKind::Kubernetes
            })
    }));
    assert!(snapshot.edges.iter().any(|edge| {
        edge.kind == TopologyEdgeKind::Contains
            && edge.provenance.iter().any(|item| {
                item.source == TopologySourceKind::Cloud
            })
    }));
    assert!(snapshot.nodes.iter().all(|node| !node.evidence_ids.is_empty()));
    assert!(snapshot.edges.iter().all(|edge| edge.confidence.is_finite()));
}
~~~

- [ ] **Step 2: Run the focused source test and record the expected failure**

Run: cargo test -p thalassaops --test topology_sources

Expected: FAIL because the builder and source adapters do not exist.

- [ ] **Step 3: Implement stable source identity and node mapping**

Use the exact node ID rule from the design: node:<source>:<environment-key>:<kind>:<native-id-or-canonical-name>. Prefer Resource.native_id or CloudResource.id, then source-qualified canonical name, then a fixture key. Treat IDs as opaque UI values and reject an identity that fails safe-identifier checks.

Map Kubernetes Pod, Service, Node and Namespace directly; map Deployment, StatefulSet and DaemonSet to Workload. Map EnvironmentStatus to Environment, CloudResourceType::KubernetesCluster to Cluster and ComputeInstance/other admitted cloud types to CloudResource. Copy native_kind, native_id, provider, scope, status, sanitized labels, unresolved ownership, affected_by_incident: false, evidence IDs and a DrillDownTarget with destination Topology.

Do not add a TopologyNodeId alias, use random IDs, or create a node from an unsupported source kind.

- [ ] **Step 4: Implement typed edge mapping and evidence admission**

Map existing Kubernetes relationships exactly:

~~~rust
"owns"    -> TopologyEdgeKind::Owns
"selects" -> TopologyEdgeKind::Selects
~~~

Add Namespace/Environment and Environment/CloudResource Contains edges only when both endpoints resolve in the same scope. Add fixture edges using the final TopologyEdge shape. Set upstream_node_id and downstream_node_id consistently for every source. Sort provenance, metadata and evidence_ids before emission.

Use confidence 1.0 for exact stable identity, 0.9 for a same-environment owner-name or selector match and the fixture confidence value for deterministic dependency edges. Reject all non-finite/out-of-range values through TopologyError.

Admit only EvidenceRef records whose classification and redaction are verified and whose scope is inside the current workspace. A missing/unverified evidence ID omits the affected node or edge and marks its source unverified; it never creates a fake evidence record with a blank excerpt.

- [ ] **Step 5: Attach observability evidence without creating correlation edges**

For an alert, match its existing ResourceReference::Resolved to one node by environment, kind and canonical name. For a metric, require exactly one match from its service/workload/pod labels. Attach evidence to that node and preserve source provenance; skip unresolved or ambiguous matches and update observability source status. Do not add DependsOn or RoutesTo edges based on shared labels, timestamps, connectors or queue membership.

- [ ] **Step 6: Add source failure and serialized leak tests**

Add these assertions:

~~~rust
#[test]
fn unresolved_and_unverified_source_data_is_not_rendered_as_a_trusted_edge() {
    let mut input = topology_fixture_input(fixture_scope());
    input.kubernetes.get_mut("env-aws-prod").unwrap().topology[0].to_name = "missing".into();
    input.evidence[0].redaction.classification_verified = false;

    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .expect("source failures should be partial");
    assert!(snapshot.source_status.iter().any(|source| {
        source.state == SourceState::Unverified
    }));
    assert!(snapshot
        .edges
        .iter()
        .all(|edge| !edge.downstream_node_id.ends_with(":missing")));
}

#[test]
fn topology_serialization_contains_no_credential_or_provider_error_payload() {
    let snapshot = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&default_topology_request())
        .unwrap();
    let text = serde_json::to_string(&snapshot).unwrap().to_ascii_lowercase();
    for forbidden in [
        "password", "token", "authorization", "credential_reference",
        "sk-live-", "raw provider error"
    ] {
        assert!(!text.contains(forbidden), "found forbidden value {forbidden}");
    }
}
~~~

- [ ] **Step 7: Run the source suite and commit**

Run:

~~~bash
cargo test -p thalassaops --test topology_sources
cargo test --workspace
cargo fmt --all -- --check
~~~

Expected: PASS with no network call, provider CLI invocation or added source integration.

~~~bash
git add src-tauri/src/topology src-tauri/tests/topology_sources.rs src-tauri/src/lib.rs
git commit -m "feat: derive evidence-backed topology records"
~~~

**Acceptance criteria:**

- Existing Kubernetes, cloud and observability contracts produce provider-neutral nodes/edges with stable IDs and typed provenance.
- Every emitted node and edge has verified evidence, safe labels/metadata and a typed drill-down target.
- Source failures remain visible through SourceStatus; no missing source is converted into a healthy or trusted graph fact.
- No signal correlation, provisioning, live fixture capture, provider CLI or network integration is added.

### Task 4: Resolve ownership and team mappings

**Files:**

- Create: src-tauri/src/topology/ownership.rs — implement TopologyOwnershipSelector, TopologyOwnershipRule validation and deterministic resolution into TopologyOwnership.
- Modify: src-tauri/src/topology/mod.rs — call ownership resolution before node emission.
- Modify: src-tauri/src/topology/fixtures.rs — provide explicit-label, scope-fallback, environment-default and unassigned fixture cases.
- Create: src-tauri/tests/topology_ownership.rs — precedence, conflict, scope and filter-facing ownership assertions.

**Interfaces:**

- Consumes: source-derived nodes, TopologyOwnershipRule, ResourceScope.team_id, canonical TeamId/team name and verified ownership evidence.
- Produces: one TopologyOwnership per node; team_id/team_name are None for unassigned resources, and source is one of the five documented enum values.

**Tests to add:**

- exact precedence: NodeId fixture rule, explicit Label rule, ResourceScope team, Environment default, Unassigned;
- deterministic equal-specificity ordering and conflict rejection;
- team name copied from the canonical mapping, never derived from a UUID;
- team IDs outside workspace rejected before graph emission; and
- ownership evidence preservation and no evidence for an honest unassigned mapping.

- [ ] **Step 1: Write the failing ownership test**

~~~rust
#[test]
fn ownership_resolution_uses_specific_mapping_then_scope_then_unassigned() {
    let input = topology_fixture_input(fixture_scope());
    let snapshot = TopologyBuilder::from_input(input)
        .snapshot_at(&default_topology_request())
        .unwrap();

    let checkout = snapshot.nodes.iter().find(|node| node.name == "checkout").unwrap();
    assert_eq!(checkout.ownership.source, TopologyOwnershipSource::ExplicitLabel);
    assert!(checkout.ownership.team_id.is_some());

    let fallback = snapshot.nodes.iter().find(|node| node.name == "checkout-api-0").unwrap();
    assert_eq!(fallback.ownership.source, TopologyOwnershipSource::ResourceScope);

    let unassigned = snapshot.nodes.iter().find(|node| node.name == "unassigned-worker").unwrap();
    assert_eq!(unassigned.ownership.source, TopologyOwnershipSource::Unassigned);
    assert_eq!(unassigned.ownership.team_id, None);
    assert_eq!(unassigned.ownership.team_name, None);
}
~~~

- [ ] **Step 2: Run the focused ownership test and record the expected failure**

Run: cargo test -p thalassaops --test topology_ownership

Expected: FAIL because ownership resolution is not implemented.

- [ ] **Step 3: Validate rules without dynamic user-facing text**

Reject blank selector keys/values, blank team names, duplicate selectors, conflicting equal-specificity rules, missing rule evidence and rules whose team scope is outside the current workspace. Return typed internal variants such as TopologyError::MalformedSource and TopologyError::ScopeDenied; AppState maps them to existing IPC codes and React i18n keys.

- [ ] **Step 4: Implement deterministic precedence**

Apply this exact order:

1. NodeId fixture rule;
2. highest-specificity Label rule;
3. source ResourceScope.team_id;
4. exact Environment rule; and
5. Unassigned.

For equal-specificity labels, sort by label key, label value, team ID and evidence IDs. Reject conflicting mappings instead of relying on input order. Copy the canonical team name from the rule/context. Never create a principal, incident owner, responder role or action permission.

- [ ] **Step 5: Run and commit**

Run:

~~~bash
cargo test -p thalassaops --test topology_ownership
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
~~~

Expected: PASS with no incident write or policy mutation path.

~~~bash
git add src-tauri/src/topology src-tauri/tests/topology_ownership.rs
git commit -m "feat: resolve topology ownership and teams"
~~~

**Acceptance criteria:**

- Every node has a deterministic resolved ownership state or explicit unassigned state.
- Label/scope/environment mapping precedence and conflict behavior are tested and stable.
- Team filtering uses canonical team IDs and cannot grant access, alter scope or imply responder authority.

### Task 5: Implement bounded upstream/downstream traversal and probable paths

**Files:**

- Create: src-tauri/src/topology/traversal.rs — implement direction-aware adjacency traversal, cycle detection, depth limits and path confidence.
- Modify: src-tauri/src/topology/mod.rs — invoke traversal for focus_node_id or Incident roots and populate TopologySummary.
- Modify: src-tauri/src/topology/fixtures.rs — keep dependency chain and cycle fixture stable.
- Create: src-tauri/tests/topology_traversal.rs — direction, depth, cycle, branch, ordering and numeric tests.

**Interfaces:**

- Consumes: validated TopologyNode/TopologyEdge, TopologyTraversal, focus/root node IDs and verified evidence.
- Produces: TopologyPath values with kind = ProbableStructural, confidence = min(edge.confidence), TopologyPathTermination, optional cycle_edge_id, sorted evidence and deterministic TopologySummary.probable_paths.

**Tests to add:**

- upstream reverse adjacency and downstream forward adjacency;
- both returning independently labelled directions;
- max_depth = 0, max_depth = 2, max_depth = 8 and rejection of 9;
- cycle termination without repeated node IDs and independent branch continuation;
- depth-limit termination only when an eligible next edge exists;
- path confidence as the minimum finite edge confidence;
- stable path IDs/order when nodes, edges and roots are reversed; and
- serialized path scan proving it contains probable_structural and no proven_causal, root_cause or equivalent field.

- [ ] **Step 1: Write the failing traversal tests**

~~~rust
#[test]
fn incident_root_shows_both_probable_directions_and_stops_at_a_cycle() {
    let request = TopologyRequest {
        filter: TopologyFilter {
            environment_ids: vec![],
            team_ids: vec![],
            incident_id: Some("alert-checkout-s1".into()),
        },
        focus_node_id: None,
        traversal: TopologyTraversal {
            direction: TopologyDirection::Both,
            max_depth: 8,
        },
    };
    let snapshot = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&request)
        .unwrap();

    assert!(snapshot.nodes.iter().any(|node| {
        node.affected_by_incident && node.name == "checkout"
    }));
    assert!(snapshot.paths.iter().any(|path| {
        path.kind == TopologyPathKind::ProbableStructural
    }));
    assert!(snapshot.paths.iter().any(|path| {
        path.termination == TopologyPathTermination::CycleDetected
    }));
    assert!(snapshot.paths.iter().all(|path| {
        let unique = path.node_ids.iter().collect::<std::collections::BTreeSet<_>>();
        unique.len() == path.node_ids.len()
    }));
}
~~~

- [ ] **Step 2: Run the focused traversal test and record the expected failure**

Run: cargo test -p thalassaops --test topology_traversal

Expected: FAIL because traversal is not implemented.

- [ ] **Step 3: Build sorted adjacency lists**

Index edges by upstream_node_id for downstream walks and by downstream_node_id for upstream walks. Sort neighbors by edge ID, then node ID. Sort Incident roots by node ID. Reject a focus/root node not present in the current workspace graph before walking.

- [ ] **Step 4: Implement cycle-safe bounded walks**

Use a per-path visited set. On a closing edge to a visited node, emit the current simple path with termination = CycleDetected, set cycle_edge_id, do not repeat the node, and continue sibling branches. At max_depth, emit DepthLimit only if a non-cycle eligible neighbor remains; otherwise emit Leaf. Return no dependency path when max_depth == 0.

- [ ] **Step 5: Implement probable path confidence and evidence**

Set each path confidence to the minimum edge confidence in its edge sequence. Set kind to the only enum value ProbableStructural. Union node/edge evidence in sorted order, include the closing cycle edge’s evidence when cycle_edge_id is present, and attach a DrillDownTarget with destination Evidence.

- [ ] **Step 6: Populate summary metrics with f64 values**

Create four TopologyMetric values: visible_nodes, visible_edges, affected_nodes and probable_paths. Store counts as finite f64 with NumberUnit::Count; attach evidence IDs from exactly the records counted and a DrillDownReference with source query, scope, optional time window and evidence IDs. Reject a metric without evidence.

- [ ] **Step 7: Run and commit**

Run:

~~~bash
cargo test -p thalassaops --test topology_traversal
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
~~~

Expected: PASS with deterministic byte-identical paths for equal inputs and no causal field.

~~~bash
git add src-tauri/src/topology src-tauri/tests/topology_traversal.rs
git commit -m "feat: traverse bounded probable topology paths"
~~~

**Acceptance criteria:**

- Incident roots and focused nodes produce deterministic upstream/downstream/both paths.
- Cycles terminate explicitly, branches continue, depth is bounded to 0..=8, and path confidence is finite and evidence-backed.
- Paths are structurally probable only; no root-cause or causal conclusion is represented.
- All four summary metrics use f64 values and verified drill-down evidence.

### Task 6: Apply filters and expose capability-scoped topology IPC

**Files:**

- Create: src-tauri/src/topology/filter.rs — validate and compose Environment/Team/Incident filters and resolve queue-item roots.
- Create: src-tauri/src/topology/evidence.rs — workspace-scoped lookup for topology evidence IDs only.
- Create: src-tauri/src/app/topology.rs — AppState::topology_snapshot and AppState::topology_evidence authorization/egress boundary.
- Modify: src-tauri/src/topology/mod.rs — integrate filter validation and evidence store.
- Modify: src-tauri/src/app/mod.rs — expose the app module entry point if required by existing module visibility.
- Modify: src-tauri/src/main.rs — register exactly topology_snapshot and topology_evidence Tauri handlers.
- Modify: src-tauri/src/lib.rs — export the app/topology path used by integration tests.
- Modify: crates/thalassa-ipc/src/lib.rs — use Task 2 descriptors as the command source of truth.
- Create: src-tauri/tests/topology_filters.rs — filter semantics and incident root tests.
- Create: src-tauri/tests/topology_ipc.rs — command, capability, membership, payload and policy matrix.

**Interfaces:**

- Consumes: TopologyRequest, TopologyEvidenceRequest, TopologyInput, OperationsSnapshot.incident_queue, TopologyBuilder, existing AppState, CommandEnvelope<Value>, IpcResult, IpcError, PolicyRuntime and ResourceScope.
- Produces: AppState::topology_snapshot(envelope) -> IpcResult<TopologySnapshot> and AppState::topology_evidence(envelope) -> IpcResult<Vec<EvidenceRef>>; registered Tauri commands with no mutation path.

**Tests to add:**

- empty/all, Environment OR, Team OR, Incident exact-match and three-way AND semantics;
- unknown incident, cross-workspace incident, broad incident scope with no exact root and backend-issued focus-node validation;
- affected root marking and contextual path inclusion;
- evidence empty/duplicate/unknown/cross-scope/unverified behavior;
- command descriptor mismatch, wrong capability, bounded envelope, inactive membership, principal mismatch, workspace grant denial, role denial and malformed payload;
- Ui egress denial and local AuditLog denial when retention is attempted; and
- successful result leak scans for credentials, authorization headers, credential references, unmasked Restricted data and raw provider errors.

- [ ] **Step 1: Write filter tests before implementation**

~~~rust
#[test]
fn environment_team_and_incident_filters_compose_as_and() {
    let team_id = fixture_scope().team_id.unwrap();
    let request = TopologyRequest {
        filter: TopologyFilter {
            environment_ids: vec!["env-aws-prod".into()],
            team_ids: vec![team_id],
            incident_id: Some("alert-checkout-s1".into()),
        },
        focus_node_id: None,
        traversal: TopologyTraversal {
            direction: TopologyDirection::Both,
            max_depth: 3,
        },
    };
    let snapshot = TopologyBuilder::from_input(topology_fixture_input(fixture_scope()))
        .snapshot_at(&request)
        .unwrap();
    assert!(snapshot.nodes.iter().all(|node| {
        node.environment_id.as_deref() == Some("env-aws-prod")
    }));
    assert!(snapshot.nodes.iter().any(|node| node.affected_by_incident));
    assert_eq!(snapshot.filter, request.filter);
}

#[test]
fn unknown_incident_and_unbounded_depth_are_rejected() {
    let unknown = TopologyRequest {
        filter: TopologyFilter {
            environment_ids: vec![],
            team_ids: vec![],
            incident_id: Some("missing".into()),
        },
        focus_node_id: None,
        traversal: TopologyTraversal {
            direction: TopologyDirection::Both,
            max_depth: 3,
        },
    };
    assert!(matches!(
        validate_topology_request(&unknown, &topology_fixture_input(fixture_scope())),
        Err(TopologyError::IncidentNotFound)
    ));

    let too_deep = TopologyRequest {
        traversal: TopologyTraversal {
            max_depth: 9,
            ..unknown.traversal
        },
        ..unknown
    };
    assert!(matches!(
        validate_topology_request(&too_deep, &topology_fixture_input(fixture_scope())),
        Err(TopologyError::InvalidRequest)
    ));
}
~~~

- [ ] **Step 2: Run the focused filter test and record the expected failure**

Run: cargo test -p thalassaops --test topology_filters

Expected: FAIL because request validation and filter composition are not implemented.

- [ ] **Step 3: Implement filter validation and Incident roots**

Validate non-blank, unique Environment IDs and focus IDs; validate unique Team IDs; accept incident_id: None; resolve a non-null ID only against the existing Sprint 11 queue projection in the current workspace. Use affected_scope.resource_ids, exact source references and the fixture incident_root_nodes index in that order. A broad scope without an exact root returns a valid graph with no affected root/path and a typed source-status limitation; it never marks every scoped node affected.

Apply Environment and Team as OR within each dimension and AND between dimensions. Keep an edge only when both endpoints survive. Preserve the complete request in TopologySnapshot.filter, focus_node_id and traversal.

- [ ] **Step 4: Implement workspace-scoped evidence lookup**

Define a private TopologyEvidenceStore over the snapshot’s admitted EvidenceRef values. Accept only { "evidence_ids": string[] }; reject empty, duplicate, unknown, cross-scope and unverified IDs. Resolve all IDs before returning any value so a mixed request cannot partially succeed. Do not accept a query, URL, connector selector, node label or filter key in the evidence request.

- [ ] **Step 5: Implement AppState authorization in the established order**

Use the Task 2 descriptors and existing authorize_operations conventions:

1. compare envelope command and capability to the descriptor;
2. reject bounded envelope scope, inactive membership, principal mismatch, missing current-workspace grant and roles without Permission::Read;
3. parse and validate the typed request before graph/evidence work;
4. evaluate verified Internal source/local retention policy and reject policy denial;
5. build the snapshot or resolve evidence; and
6. evaluate verified Internal to Ui before returning IpcResult::Ok.

Map TopologyError to existing IpcErrorCode values without interpolating source payloads: invalid request to INVALID_REQUEST, missing node/incident to NOT_FOUND, scope to PERMISSION_DENIED, unverified/policy to POLICY_DENIED, malformed projection to INTERNAL_ERROR.

topology.snapshot requires WorkspaceRead/Read; topology.evidence requires ResourceRead/Read. Neither command requires IncidentRead, IncidentWrite, ConnectorAct, PolicyManage or ExecuteAction.

- [ ] **Step 6: Register the two Tauri handlers and add security matrix tests**

Add synchronous handlers because the Sprint 12 fixture builder is in-memory and has no I/O:

~~~rust
#[tauri::command]
fn topology_snapshot(
    envelope: CommandEnvelope<serde_json::Value>,
    state: tauri::State<'_, thalassaops::app::AppState>,
) -> thalassaops::app::IpcResult<thalassa_domain::TopologySnapshot> {
    state.topology_snapshot(envelope)
}
~~~

Register it and topology_evidence exactly once in tauri::generate_handler!. Add tests for wrong command/capability, a bounded envelope containing a foreign UUID, suspended membership, mismatched principal, role without Read, malformed filter/payload, UI policy denial, unknown evidence, cross-scope evidence and unverified evidence.

- [ ] **Step 7: Run backend gates and commit**

Run:

~~~bash
cargo test -p thalassaops --test topology_filters
cargo test -p thalassaops --test topology_ipc
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
~~~

Expected: PASS with only the two documented topology commands and no external request or mutation.

~~~bash
git add crates/thalassa-ipc/src/lib.rs src-tauri/src/topology src-tauri/src/app/topology.rs src-tauri/src/app/mod.rs src-tauri/src/main.rs src-tauri/src/lib.rs src-tauri/tests/topology_filters.rs src-tauri/tests/topology_ipc.rs
git commit -m "feat: expose filtered topology through secure read IPC"
~~~

**Acceptance criteria:**

- Environment, Team and Incident filters compose exactly as documented and preserve honest no-root behavior.
- Both IPC commands enforce capability, scope, membership, role, payload, evidence and policy boundaries before returning data.
- No incident lifecycle/write/action, provider query, network integration or credential path exists.

### Task 7: Build the React topology workspace and graph-to-evidence journey

**Files:**

- Create: ui/src/topology/TopologyWorkspace.tsx — request lifecycle, snapshot state and composition.
- Create: ui/src/topology/TopologyFilters.tsx — Environment, Team, Incident, direction and bounded-depth controls.
- Create: ui/src/topology/TopologyGraph.tsx — accessible graph/list rendering of nodes and typed edges.
- Create: ui/src/topology/TopologyPathList.tsx — probable path, confidence, direction, cycle and depth-limit rendering.
- Create: ui/src/topology/TopologyEvidencePanel.tsx — source/query/time/redaction details and trusted HTTPS native-link opening.
- Create: ui/src/topology/TopologyWorkspace.test.tsx — fixture-first UI journey and accessibility tests.
- Create: ui/src/topology/topology.acceptance.test.tsx — deterministic topology journey (Task 8 may extend it with final backend assertions).
- Modify: ui/src/shell.tsx — add a topology route/nav entry and render TopologyWorkspace.
- Modify: ui/src/locales/en.ts — add all topology copy and typed-state keys.
- Modify: ui/src/locales/th.ts — add identical topology key structure in Thai.
- Modify: ui/src/styles.css — graph/list layout, affected-node, direction, confidence, focus and responsive styles.

**Interfaces:**

- Consumes: TopologyRequest, TopologySnapshot, TopologyNode, TopologyEdge, TopologyPath, TopologyMetric, TopologyEvidenceRequest, EvidenceRef, Invoke, command("topology", "snapshot") and command("topology", "evidence").
- Produces: TopologyWorkspace({ invoke, initialRequest? }) and accessible graph/evidence controls that send only backend-issued IDs.

**Tests to add:**

- render the copied fixture before the backend exists;
- issue the complete explicit request with empty arrays/nulls, direction and max_depth;
- select the Sprint 11 queue item and show the affected checkout resource plus a probable structural path;
- Environment and Team filter composition, ownership/unassigned state and source labels;
- upstream/downstream/both controls, depth-limit and cycle-terminated path labels;
- node/edge/path controls send only issued evidence IDs to topology.evidence;
- source/query/time-window/masked/unparsed evidence presentation and trusted HTTPS native-link guard;
- f64 confidence/metric rendering without changing values to strings;
- keyboard focus, screen-reader labels and status text independent of color; and
- English/Thai locale object parity.

- [ ] **Step 1: Write the failing fixture-first journey test**

~~~tsx
test("incident filter shows affected resources and probable dependency paths", async () => {
  const invoke = vi.fn().mockResolvedValue({
    ok: true,
    value: topologySnapshotFixture
  });
  render(<I18nProvider><TopologyWorkspace invoke={invoke} /></I18nProvider>);

  await userEvent.setup().selectOptions(
    screen.getByRole("combobox", { name: /incident/i }),
    "alert-checkout-s1"
  );

  expect(await screen.findByText("checkout")).toBeInTheDocument();
  expect(screen.getByText(/affected/i)).toBeInTheDocument();
  expect(screen.getByText(/probable structural/i)).toBeInTheDocument();
  expect(screen.getByText(/depends on/i)).toBeInTheDocument();
  expect(invoke).toHaveBeenCalledWith("topology_snapshot", expect.objectContaining({
    envelope: expect.objectContaining({
      command: "topology.snapshot",
      capability: "WorkspaceRead",
      scope: { resource_ids: [] },
      payload: expect.objectContaining({
        filter: expect.objectContaining({
          environment_ids: [],
          team_ids: [],
          incident_id: "alert-checkout-s1"
        }),
        traversal: { direction: "both", max_depth: 3 }
      })
    })
  }));
});
~~~

- [ ] **Step 2: Run the focused UI test and record the expected failure**

Run: npm ci && npm test -- ui/src/topology/TopologyWorkspace.test.tsx

Expected: FAIL because the topology route and components do not exist.

- [ ] **Step 3: Add typed request controls and snapshot loading**

Initialize this complete request:

~~~ts
const defaultTopologyRequest: TopologyRequest = {
  filter: { environment_ids: [], team_ids: [], incident_id: null },
  focus_node_id: null,
  traversal: { direction: "both", max_depth: 3 }
};
~~~

Call topology_snapshot with a fresh request ID, lowercase topology.snapshot, WorkspaceRead, unbounded { resource_ids: [] } scope and the request object. Render loading/error/unavailable states using locale keys. Never pass a provider URL/query or an unvalidated node ID from arbitrary DOM text.

- [ ] **Step 4: Render graph facts accessibly**

Render a visual relationship view and an accessible list/table. For each node show kind, name, provider/environment, health text, ownership source/team, affected text when true, safe labels, optional metric and evidence button. For each edge show upstream/downstream names, typed relation, provenance source, confidence and evidence button. Do not communicate health, affected state or confidence through color alone.

- [ ] **Step 5: Render probable paths honestly**

Display path.kind as the localized probable structural label, direction, finite confidence, node sequence, edge sequence and termination. Map cycle_detected and depth_limit to localized labels. Do not render root cause, caused by, confirmed dependency or an equivalent phrase. A depth-limited path must state that more context may exist beyond the requested bound.

- [ ] **Step 6: Implement filters and ownership controls**

Use typed select controls for Environment, Team and Incident. Empty Environment/Team arrays mean all; Incident uses null for no selection. Direction is upstream, downstream or both; depth options are 0, 1, 2, 3, 5 and 8. Submit a complete request after a control changes. Display unassigned ownership as a localized state and do not create a team label from a missing ID.

- [ ] **Step 7: Implement graph-to-evidence navigation**

Collect evidence IDs only from TopologyNode.evidence_ids, TopologyEdge.evidence_ids, TopologyPath.evidence_ids or TopologyMetric.evidence_ids, deduplicate them locally, and invoke:

~~~ts
invoke("topology_evidence", {
  envelope: {
    request_id: crypto.randomUUID(),
    command: "topology.evidence",
    capability: "ResourceRead",
    scope: { resource_ids: [] },
    payload: { evidence_ids: issuedEvidenceIds }
  }
});
~~~

Reject an empty local set without IPC. Display source, connector, endpoint, query, observed time, excerpt, masked and unparsed. Reuse isTrustedNativeUrl; open only the exact HTTPS URL that passed the existing guard. Never construct a URL, query or command from filter_key.

- [ ] **Step 8: Add route, locale parity and styles**

Add topology to the shell’s Area union, areas array, navigation and route branch. Add every topology title, relation, node kind, ownership source, direction, path termination, source state, evidence state and control label to both locale objects with identical nested keys. Add focus-visible, affected-root, path termination and non-color status styles without introducing a graph library.

- [ ] **Step 9: Run UI gates and commit**

Run:

~~~bash
npm ci
npm test -- ui/src/topology/TopologyWorkspace.test.tsx
npm test -- ui/src/topology/topology-contracts.test.ts
npm run typecheck
npm run lint
npm run format:check
~~~

Expected: PASS with fixture-only rendering, no external request and no mutation control.

~~~bash
git add ui/src/topology ui/src/shell.tsx ui/src/locales/en.ts ui/src/locales/th.ts ui/src/styles.css
git commit -m "feat: add the topology workspace and evidence navigation"
~~~

**Acceptance criteria:**

- The React worker renders the copied Rust fixture before backend IPC exists and uses the exact contract types.
- Selecting alert-checkout-s1 visibly marks affected resources and shows probable structural dependency paths with direction, confidence and termination state.
- Every graph fact and metric navigates to backend-issued evidence IDs only; native links are HTTPS/trusted-source guarded.
- Filters, ownership states, path qualifications, source failures and evidence states are localized, keyboard accessible and not color-only.

### Task 8: Run complete regression, fixture acceptance and release verification

**Files:**

- Modify: ui/src/topology/topology.acceptance.test.tsx — finalize the full fixture journey and no-network/no-mutation assertions.
- Create: docs/superpowers/reports/2026-08-28-sprint-12-verification.md — actual command results, scope audit and exit criterion.
- Modify only for a defect proven by a failing test: files listed in Tasks 2–7.

**Interfaces:**

- Consumes: completed topology contracts, source adapters, ownership resolver, traversal/filter/IPC handlers, React workspace and copied fixtures.
- Produces: a committed, unpushed, unmerged sprint branch and verification report with actual command outcomes.

**Tests to add:**

- full incident-filter journey showing checkout as affected and at least one probable path;
- source independence with one unavailable cloud source and one healthy environment still visible;
- cycle and depth-limit labels, team/unassigned state and all critical/node metrics with evidence;
- no operations mutation, incident.write, connector_act, provider/network invocation or arbitrary evidence query; and
- exact English/Thai locale shape parity.

- [ ] **Step 1: Finalize the deterministic acceptance test**

The test loads topologySnapshotFixture, selects alert-checkout-s1, verifies the affected root, verifies one upstream and one downstream path, opens a node evidence control, and asserts that the only IPC commands are topology_snapshot and topology_evidence with the documented capabilities and payloads.

- [ ] **Step 2: Run Rust formatting, lint and tests**

Run:

~~~bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
~~~

Expected: PASS with no warnings, no test count regression and no new network/provisioning path.

- [ ] **Step 3: Install frontend dependencies**

Run: npm ci

Expected: exit code 0. If dependency installation is unavailable, record the command/output as blocked and do not report frontend gates as passing.

- [ ] **Step 4: Run all frontend gates, including the required sprint gate**

Run:

~~~bash
npm run format:check
npm run lint
npm run typecheck
npm run build
npm test
~~~

Expected: PASS for all five commands. The sprint gate is specifically npm run format:check.

- [ ] **Step 5: Audit scope, IPC and serialized output**

Run:

~~~bash
git diff --check
git diff --stat main...HEAD
git diff main...HEAD -- ':!docs/design/sprint-12-resource-topology.md' ':!docs/superpowers/plans/2026-08-28-sprint-12-resource-topology.md'
~~~

Verify that the diff contains no Terraform/OpenTofu execution, live cloud capture, network integration, incident lifecycle/write, responder role, AI/model call, mutation/remediation path or Sprint 13 signal normalization/correlation. Verify that only the two topology IPC commands are new and that all serialized node/edge/path/evidence values pass the forbidden-secret scan.

- [ ] **Step 6: Write the verification report with actual results**

Record branch name, commit IDs, commands, exit codes, test counts where available, fixture journey observations, known dependency advisories and scope audit. Include this exact section:

~~~markdown
## Exit criterion

> "An incident can show affected resources and probable dependency paths."
~~~

Do not report a command as PASS if it was not run. Do not turn a pre-existing warning or advisory into a silent success.

- [ ] **Step 7: Commit the acceptance artifact without pushing or merging**

~~~bash
git add ui/src/topology/topology.acceptance.test.tsx docs/superpowers/reports/2026-08-28-sprint-12-verification.md
git commit -m "test: record sprint 12 topology acceptance"
~~~

**Acceptance criteria:**

- Rust and frontend tests/gates pass, including npm run format:check after npm ci.
- The deterministic fixture demonstrates affected resources and probable dependency paths with evidence, cycle/depth honesty and no mutation.
- The final diff stays inside Sprint 12 boundaries and preserves capability, policy, masking, redaction and localization conventions.
- The verification report records actual outcomes and the exact exit criterion.
- The branch is committed, unpushed and unmerged.
