# Sprint 12 Resource and Service Topology Design

**Status:** Design specification
**Date:** 2026-08-28
**Sprint:** 12 — Resource and service topology

## Goal

Build a deterministic, read-only topology projection that lets an operator see
services and resources, understand their ownership, select a workspace,
environment, team or Sprint 11 queue item, and inspect bounded upstream or
downstream structural paths with the evidence that supports every displayed
node, edge and metric.

The Sprint 12 exit criterion is:

> "An incident can show affected resources and probable dependency paths."

The word **probable** is part of the product contract. A topology path is a
structural, evidence-backed relationship assembled from source data. It is not
a normalized signal, a correlated incident explanation, a root-cause finding or
a proven causal chain.

## Scope and boundaries

Sprint 12 adds a topology read model and the UI needed to inspect it:

- a provider-neutral service/resource graph;
- typed nodes and typed, directed edges with source provenance and structural
  confidence;
- ownership and team mapping with an explicit unassigned state;
- bounded upstream/downstream traversal with deterministic cycle and depth
  handling;
- Environment, Team and Incident filters;
- graph-to-evidence navigation; and
- deterministic topology fixtures built from the source contracts already
  delivered by Kubernetes, observability and cloud inventory work.

The topology projection is local-first and read-only. It consumes records that
the existing Kubernetes, Prometheus/Alertmanager, Loki/OpenTelemetry and
AWS/Azure/GCP modules already produce, together with deterministic fixture
records. It may normalize those records into graph nodes and edges, but it
does not create a new external integration or replace the source modules.

The following are explicitly outside Sprint 12:

- provisioning infrastructure, running Terraform or OpenTofu, applying
  Kubernetes changes, capturing new live cloud fixtures, or adding a network
  integration;
- a canonical incident entity, an incident lifecycle, incident writes,
  responder roles, assignment, comments, notifications or incident actions;
  the Incident filter reads the Sprint 11 `IncidentQueueItem` projection and
  its fixtures only;
- signal normalization, signal deduplication, correlation windows,
  maintenance windows, suppression or explainable correlation reasons; those
  belong to Sprint 13;
- AI investigation, model calls, hypotheses, mutation proposals, terminal
  execution or remediation; and
- any claim that a structural dependency path proves causality.

Sprint 12 does not change the meaning of the Sprint 11 incident queue. An
`incident_id` in this document means the stable `IncidentQueueItem.id` emitted
by `operations.snapshot`, never `thalassa_domain::IncidentId` and never a new
incident model.

## Contract rules carried from earlier sprints

These rules are binding for every Rust and TypeScript type in this design:

1. There is one type per concept. Reuse existing `ResourceScope`,
   `EvidenceRef`, `DrillDownTarget`, `DrillDownReference`, `NumberUnit`,
   `ConsoleHealthState`, `SourceStatus` and `TeamId`; do not create topology
   aliases or companion representations for them.
2. Numeric values are stored as `f64` in the topology model and as `number` in
   TypeScript. Rust rejects `NaN`, positive infinity and negative infinity with
   a typed topology error before serialization. Rust never sends a formatted
   numeric string for topology confidence or metrics.
3. User-visible reasons, statuses, path qualifications and ownership states are
   typed enums. React maps their wire values to English/Thai i18n keys. Rust
   does not manufacture user-visible English sentences for those states.
4. Absent source data is represented by `Option`/`null`, an unavailable source
   state or an omitted record. An empty string is never used as an absent
   value. Empty arrays/maps mean that a source was verified and has no members;
   they do not mean that a source failed.
5. Every displayed critical number or node metric has evidence IDs and a typed
   drill-down reference. A renderable node, edge and path also has at least one
   verified evidence ID. Unverified source records are not rendered as trusted
   graph facts.
6. All IPC JSON fields use the existing snake_case convention. Enum wire
   values are explicit and stable. The TypeScript types in this document are
   the exact mirror of the Rust serialized shape, not a second UI model.

## Architecture

```text
Existing source projections and Sprint 11 queue projection
  ├── KubernetesInventory.resources + topology
  ├── CloudResource + EnvironmentStatus
  ├── NormalizedAlert + Prometheus MetricFixture + evidence
  ├── IncidentQueueItem from operations.snapshot
  └── deterministic topology fixture edges/ownership rules
                         │
                         ▼
                 Topology source adapters
       scope → identity → labels/metadata masking → evidence admission
                         │
                         ▼
                 Topology graph builder
       nodes → ownership → typed edges → source status → validation
                         │
                         ▼
               Filter + bounded traversal
       Environment ∩ Team ∩ Incident → upstream/downstream paths
                         │
                         ▼
                    TopologySnapshot
                         │
       topology.snapshot (WorkspaceRead, Read)
       topology.evidence (ResourceRead, Read)
                         │
                         ▼
           React Topology Workspace and evidence drawer
```

The topology module is a projection boundary, not a provider boundary. The
source adapters call existing provider-neutral functions or consume their
already-captured results. They do not call another Tauri command recursively,
construct a provider URL, resolve credentials, invoke a provider CLI or run a
network request as part of Sprint 12. A future live refresh may supply the same
`TopologyInput` from existing connector paths, but that is not part of this
sprint.

The graph builder has four phases:

1. **Identity and scope.** Convert each source resource to one stable node
   identity, reject records outside the current workspace, and maintain an
   index by source identity and normalized kind/name.
2. **Safe metadata and evidence.** Apply the existing masking/classification
   path to labels and edge metadata before they enter the graph. Admit only
   verified evidence references. A malformed or unverified record changes its
   source status and is omitted from the trusted graph.
3. **Structural graph.** Add source-backed nodes and typed edges. Exact
   duplicate edges are collapsed only as graph construction hygiene; Sprint 12
   does not deduplicate or correlate signals. An edge that cannot resolve both
   endpoints is skipped and reported through source status.
4. **Selection and traversal.** Apply all requested filters, identify incident
   roots from the existing queue projection, and traverse at most the
   requested bounded depth. The resulting snapshot is validated before the
   UI egress policy is checked and the value is serialized.

### Module layout

```text
crates/thalassa-domain/
  src/lib.rs                         # canonical topology wire contracts
  tests/topology_contracts.rs        # Rust JSON shape and numeric invariants

crates/thalassa-ipc/
  src/lib.rs                         # topology.snapshot/evidence descriptors
  tests/contracts.rs                 # descriptor/capability assertions

src-tauri/src/topology/
  mod.rs                             # public topology exports and orchestration
  fixtures.rs                        # deterministic source/graph fixtures
  derive.rs                          # Kubernetes/cloud/observability adapters
  ownership.rs                       # deterministic team mapping
  traversal.rs                       # bounded upstream/downstream walk
  filter.rs                          # Environment/Team/Incident selection
  evidence.rs                        # workspace-scoped evidence lookup

src-tauri/src/app/topology.rs       # Tauri authorization and IPC handlers
src-tauri/src/lib.rs                # topology module export
src-tauri/src/app/mod.rs            # AppState topology entry points
src-tauri/src/main.rs               # topology Tauri command registration

src-tauri/tests/
  topology_sources.rs                # source derivation and provenance
  topology_ownership.rs              # team mapping and unassigned behavior
  topology_traversal.rs              # paths, cycles and depth limits
  topology_filters.rs                # filter composition and incident roots
  topology_ipc.rs                    # command/capability/policy boundary

ui/contracts/ipc.ts                  # exact TypeScript mirror below
ui/src/topology/
  TopologyWorkspace.tsx              # composition and request lifecycle
  TopologyFilters.tsx                # typed filter controls
  TopologyGraph.tsx                  # accessible graph/list rendering
  TopologyPathList.tsx               # probable path and termination states
  TopologyEvidencePanel.tsx          # evidence and trusted native links
  topology-fixtures.ts               # copied Rust fixture JSON
  topology-contracts.test.ts         # wire-shape and fixture tests
  TopologyWorkspace.test.tsx         # UI journey and accessibility
ui/src/shell.tsx                     # Topology navigation entry point
ui/src/locales/en.ts                 # English topology keys
ui/src/locales/th.ts                 # Thai topology keys
ui/src/styles.css                    # graph, path and focus styles
```

The domain crate owns the wire contracts. `src-tauri/src/topology/model.rs`
must not create a second model; if a producer module needs a convenient import
path it re-exports the domain types exactly as Sprint 11 does.

The backend workers share this exact non-IPC adapter input shape. It is an
internal Rust value assembled from existing source results; it is not accepted
from React and therefore has no TypeScript counterpart:

```rust
// Existing imports: chrono::DateTime/Utc, BTreeMap, ResourceScope,
// KubernetesInventory, CloudResource, EnvironmentStatus, NormalizedAlert,
// IncidentQueueItem, MetricFixture, SourceStatus and EvidenceRef.
pub struct TopologyInput {
    pub generated_at: DateTime<Utc>,
    pub scope: ResourceScope,
    pub kubernetes: BTreeMap<String, KubernetesInventory>,
    pub cloud_resources: Vec<CloudResource>,
    pub environments: Vec<EnvironmentStatus>,
    pub alerts: Vec<NormalizedAlert>,
    pub metrics: Vec<MetricFixture>,
    pub incident_queue: Vec<IncidentQueueItem>,
    pub ownership_rules: Vec<TopologyOwnershipRule>,
    pub fixture_edges: Vec<TopologyEdge>,
    pub incident_root_nodes: BTreeMap<String, Vec<String>>,
    pub source_status: Vec<SourceStatus>,
    pub evidence: Vec<EvidenceRef>,
}
```

`kubernetes` is keyed by the source environment ID because the existing
`KubernetesInventory` contract contains resources and topology edges but does
not add a second environment wrapper. `fixture_edges` uses the final
`TopologyEdge` shape, so fixtures do not create a companion edge type.
`incident_root_nodes` is a fixture/adapter index keyed by the existing queue
item ID; it is not persisted incident state and is never exposed as an
incident entity or write API.

## Graph data model

### Node identity and kinds

Node IDs are stable source-qualified strings, not random UUIDs and not display
labels. The builder uses the first available identity in this order:

1. the source's stable native identifier (`Resource.native_id` or
   `CloudResource.id`);
2. the source-qualified environment, kind and canonical name; and
3. a deterministic fixture key.

The serialized ID format is:

```text
node:<source>:<environment-key>:<kind>:<native-id-or-canonical-name>
```

The value is treated as opaque by React. It is never used as a query, URL,
credential reference or user-visible reason. If an identity contains a
sensitive key/value or cannot be made into a safe identifier, the source record
is omitted and its `SourceStatus` becomes `unverified`; the builder never
substitutes an empty label or a random ID.

The graph uses a closed, provider-neutral node-kind enum. Kubernetes
Deployment, StatefulSet and DaemonSet resources map to `workload`; a cloud
EKS/AKS/GKE inventory item maps to `cloud_resource` and remains visibly
provider-qualified in metadata. Unsupported source kinds do not become a
generic node with an untyped status; they are reported as unavailable source
data until a later contract adds the kind.

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum TopologyNodeKind {
    #[serde(rename = "environment")]
    Environment,
    #[serde(rename = "cluster")]
    Cluster,
    #[serde(rename = "namespace")]
    Namespace,
    #[serde(rename = "workload")]
    Workload,
    #[serde(rename = "service")]
    Service,
    #[serde(rename = "pod")]
    Pod,
    #[serde(rename = "node")]
    Node,
    #[serde(rename = "cloud_resource")]
    CloudResource,
    #[serde(rename = "observability_target")]
    ObservabilityTarget,
}
```

`TopologyNode.status` reuses the existing `ConsoleHealthState` enum. The
status is a source health observation, not a causal conclusion. An
observability record can attach evidence to a node but cannot silently upgrade
or downgrade its health.

### Ownership and node contract

Ownership is a resolved mapping, not a responder role and not an incident
assignment. `team_id` and `team_name` are both absent when the mapping is
unassigned. A team name is copied from the canonical workspace/team context;
React never invents a display name from a UUID.

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum TopologyOwnershipSource {
    #[serde(rename = "explicit_label")]
    ExplicitLabel,
    #[serde(rename = "resource_scope")]
    ResourceScope,
    #[serde(rename = "environment_default")]
    EnvironmentDefault,
    #[serde(rename = "fixture")]
    Fixture,
    #[serde(rename = "unassigned")]
    Unassigned,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TopologyOwnership {
    pub team_id: Option<TeamId>,
    pub team_name: Option<String>,
    pub source: TopologyOwnershipSource,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TopologyMetric {
    pub key: String,
    pub value: f64,
    pub unit: NumberUnit,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub drill_down: DrillDownTarget,
    pub drill_down_reference: DrillDownReference,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TopologyNode {
    pub id: String,
    pub kind: TopologyNodeKind,
    pub name: String,
    pub native_kind: Option<String>,
    pub native_id: Option<String>,
    pub environment_id: Option<String>,
    pub provider: Option<String>,
    pub scope: ResourceScope,
    pub status: ConsoleHealthState,
    pub labels: BTreeMap<String, String>,
    pub ownership: TopologyOwnership,
    pub metric: Option<TopologyMetric>,
    pub affected_by_incident: bool,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub drill_down: DrillDownTarget,
}
```

The `TopologyMetric` value is `f64` even when its unit is `count`. The builder
uses it for a node metric and for summary metrics; there is no string-valued
topology number and no second `CriticalNumber` variant. `metric` is `null`
when the source has no numeric node metric. A metric with no verified evidence
is omitted rather than rendered as zero.

`affected_by_incident` is true only for an explicit root resource selected by
the `incident_id` filter. Context nodes reached through traversal are false;
this lets the UI distinguish affected resources from structural context. With
no Incident filter all nodes have `false`.

### Edge kind, direction, provenance and confidence

Every edge is directed from its **upstream node** to its **downstream node**.
This is an invariant independent of provider terminology:

- an Environment contains a Namespace or CloudResource;
- a Workload owns a Pod;
- a Service selects a Pod;
- an upstream component routes to or supports a downstream component; and
- a fixture `depends_on` relationship is serialized in upstream-to-downstream
  order so traversal never has to guess what `from` means.

`TopologyEdgeKind` is the relationship vocabulary. `confidence` is the
confidence that the structural edge was assembled correctly from its stated
source, not the probability that it caused an incident. It must be finite and
between `0.0` and `1.0`, inclusive.

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum TopologyEdgeKind {
    #[serde(rename = "contains")]
    Contains,
    #[serde(rename = "owns")]
    Owns,
    #[serde(rename = "selects")]
    Selects,
    #[serde(rename = "routes_to")]
    RoutesTo,
    #[serde(rename = "runs_on")]
    RunsOn,
    #[serde(rename = "depends_on")]
    DependsOn,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TopologySourceKind {
    #[serde(rename = "kubernetes")]
    Kubernetes,
    #[serde(rename = "cloud")]
    Cloud,
    #[serde(rename = "observability")]
    Observability,
    #[serde(rename = "fixture")]
    Fixture,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TopologyEdgeProvenance {
    pub source: TopologySourceKind,
    pub source_key: String,
    pub observed_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TopologyEdge {
    pub id: String,
    pub upstream_node_id: String,
    pub downstream_node_id: String,
    pub kind: TopologyEdgeKind,
    pub provenance: Vec<TopologyEdgeProvenance>,
    pub confidence: f64,
    pub metadata: BTreeMap<String, String>,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub drill_down: DrillDownTarget,
}
```

An edge ID is stable for the edge kind, endpoint IDs and source key. Exact
duplicate records with the same identity are represented by one edge with
sorted provenance entries. Edges with the same endpoints but different kinds
remain distinct. This graph-level identity normalization must not be described
as Sprint 13 signal deduplication.

### Probable paths and termination

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TopologyDirection {
    #[serde(rename = "upstream")]
    Upstream,
    #[serde(rename = "downstream")]
    Downstream,
    #[serde(rename = "both")]
    Both,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TopologyPathKind {
    #[serde(rename = "probable_structural")]
    ProbableStructural,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TopologyPathTermination {
    #[serde(rename = "leaf")]
    Leaf,
    #[serde(rename = "cycle_detected")]
    CycleDetected,
    #[serde(rename = "depth_limit")]
    DepthLimit,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TopologyPath {
    pub id: String,
    pub root_node_id: String,
    pub terminal_node_id: String,
    pub node_ids: Vec<String>,
    pub edge_ids: Vec<String>,
    pub direction: TopologyDirection,
    pub depth: u16,
    pub confidence: f64,
    pub kind: TopologyPathKind,
    pub termination: TopologyPathTermination,
    pub cycle_edge_id: Option<String>,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub drill_down: DrillDownTarget,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TopologyTraversal {
    pub direction: TopologyDirection,
    pub max_depth: u16,
}
```

Paths contain simple `node_ids`: a node is never repeated in that list. When
the next edge would return to a node already in the current path, traversal
stops with `cycle_detected`, leaves the closing edge in `cycle_edge_id`, and
does not append the repeated node. The path's `edge_ids` contain only edges
between the listed simple nodes; `cycle_edge_id` is included in the path's
evidence set and can be opened in the evidence panel.

`max_depth` counts edges from the root. `0` returns the selected graph nodes
and no dependency paths. When the walk reaches `max_depth` and has an eligible
non-cycle neighbor, the current path terminates with `depth_limit`; a leaf at
that depth terminates with `leaf`. Sprint 12 rejects a request above the
inclusive maximum of `8` rather than silently running an unbounded traversal.

The path confidence is the minimum edge confidence in the path. Every path
has `kind = probable_structural`; there is deliberately no `proven_causal`
variant. The UI must use the localized “probable structural path” label for
this enum and must not call it a root cause.

Traversal is deterministic: roots are sorted by node ID, neighbors by edge ID
then downstream node ID, and final paths by root, direction, depth, terminal
node and edge IDs. The adjacency walk uses a per-path visited set, so one
cycle cannot hide unrelated branches.

### Filter, summary and snapshot contract

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TopologyFilter {
    pub environment_ids: Vec<String>,
    pub team_ids: Vec<TeamId>,
    /// Sprint 11 IncidentQueueItem.id; this is not IncidentId.
    pub incident_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TopologyRequest {
    pub filter: TopologyFilter,
    pub focus_node_id: Option<String>,
    pub traversal: TopologyTraversal,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TopologySummary {
    pub visible_nodes: TopologyMetric,
    pub visible_edges: TopologyMetric,
    pub affected_nodes: TopologyMetric,
    pub probable_paths: TopologyMetric,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TopologySnapshot {
    pub generated_at: String,
    pub scope: ResourceScope,
    pub filter: TopologyFilter,
    pub focus_node_id: Option<String>,
    pub traversal: TopologyTraversal,
    pub summary: TopologySummary,
    pub nodes: Vec<TopologyNode>,
    pub edges: Vec<TopologyEdge>,
    pub paths: Vec<TopologyPath>,
    pub source_status: Vec<SourceStatus>,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TopologyEvidenceRequest {
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}
```

Filter semantics are explicit:

- an empty `environment_ids` array means all environments in the current
  workspace; a non-empty array is an OR within Environment;
- an empty `team_ids` array means all resolved teams; a non-empty array is an
  OR within Team; unassigned nodes are excluded when a team filter is active;
- `incident_id: null` means no Incident selection; a value must exactly match
  one Sprint 11 `IncidentQueueItem.id` in the current workspace; and
- the three dimensions compose as an AND. A selected Incident determines the
  affected roots, then Environment and Team restrict the final visible graph.

`focus_node_id` is optional and must be a node ID previously emitted by the
backend for the current workspace. When it is present, paths start at that
node. When it is absent and `incident_id` is present, paths start at the
incident's affected roots. When neither is present, the snapshot still shows
the filtered graph but returns no paths.

`TopologySummary` values are `TopologyMetric` instances, not bare counts. The
evidence set for `visible_nodes` and `visible_edges` is the sorted union of
the records counted. `affected_nodes` uses the selected incident root
evidence, and `probable_paths` uses the sorted union of path evidence. If a
source has no verified evidence for a metric, that metric is omitted from the
summary and the affected source is marked `unverified`; it is never rendered
as a guessed zero.

## TypeScript mirror

The following declarations are the exact `ui/contracts/ipc.ts` counterparts of
the Rust types above. The UI worker must copy them before implementing the
fixture view; it must not invent a UI-only graph shape. `UUID`,
`ResourceScope`, `ConsoleHealthState`, `ConsoleEvidenceId`, `EvidenceRef`,
`DrillDownTarget`, `DrillDownReference`, `NumberUnit` and `SourceStatus` are
the existing declarations in that file and are reused unchanged.

```ts
export type TopologyNodeKind =
  | "environment"
  | "cluster"
  | "namespace"
  | "workload"
  | "service"
  | "pod"
  | "node"
  | "cloud_resource"
  | "observability_target";

export type TopologyOwnershipSource =
  | "explicit_label"
  | "resource_scope"
  | "environment_default"
  | "fixture"
  | "unassigned";

export type TopologyOwnership = {
  team_id: UUID | null;
  team_name: string | null;
  source: TopologyOwnershipSource;
  evidence_ids: ConsoleEvidenceId[];
};

export type TopologyMetric = {
  key: string;
  value: number;
  unit: NumberUnit;
  evidence_ids: ConsoleEvidenceId[];
  drill_down: DrillDownTarget;
  drill_down_reference: DrillDownReference;
};

export type TopologyNode = {
  id: string;
  kind: TopologyNodeKind;
  name: string;
  native_kind: string | null;
  native_id: string | null;
  environment_id: string | null;
  provider: string | null;
  scope: ResourceScope;
  status: ConsoleHealthState;
  labels: Record<string, string>;
  ownership: TopologyOwnership;
  metric: TopologyMetric | null;
  affected_by_incident: boolean;
  evidence_ids: ConsoleEvidenceId[];
  drill_down: DrillDownTarget;
};

export type TopologyEdgeKind =
  | "contains"
  | "owns"
  | "selects"
  | "routes_to"
  | "runs_on"
  | "depends_on";

export type TopologySourceKind = "kubernetes" | "cloud" | "observability" | "fixture";

export type TopologyEdgeProvenance = {
  source: TopologySourceKind;
  source_key: string;
  observed_at: string | null;
};

export type TopologyEdge = {
  id: string;
  upstream_node_id: string;
  downstream_node_id: string;
  kind: TopologyEdgeKind;
  provenance: TopologyEdgeProvenance[];
  confidence: number;
  metadata: Record<string, string>;
  evidence_ids: ConsoleEvidenceId[];
  drill_down: DrillDownTarget;
};

export type TopologyDirection = "upstream" | "downstream" | "both";
export type TopologyPathKind = "probable_structural";
export type TopologyPathTermination = "leaf" | "cycle_detected" | "depth_limit";

export type TopologyPath = {
  id: string;
  root_node_id: string;
  terminal_node_id: string;
  node_ids: string[];
  edge_ids: string[];
  direction: TopologyDirection;
  depth: number;
  confidence: number;
  kind: TopologyPathKind;
  termination: TopologyPathTermination;
  cycle_edge_id: string | null;
  evidence_ids: ConsoleEvidenceId[];
  drill_down: DrillDownTarget;
};

export type TopologyTraversal = {
  direction: TopologyDirection;
  max_depth: number;
};

export type TopologyFilter = {
  environment_ids: string[];
  team_ids: UUID[];
  incident_id: string | null;
};

export type TopologyRequest = {
  filter: TopologyFilter;
  focus_node_id: string | null;
  traversal: TopologyTraversal;
};

export type TopologySummary = {
  visible_nodes: TopologyMetric;
  visible_edges: TopologyMetric;
  affected_nodes: TopologyMetric;
  probable_paths: TopologyMetric;
};

export type TopologySnapshot = {
  generated_at: string;
  scope: ResourceScope;
  filter: TopologyFilter;
  focus_node_id: string | null;
  traversal: TopologyTraversal;
  summary: TopologySummary;
  nodes: TopologyNode[];
  edges: TopologyEdge[];
  paths: TopologyPath[];
  source_status: SourceStatus[];
  evidence: EvidenceRef[];
};

export type TopologyEvidenceRequest = { evidence_ids: ConsoleEvidenceId[] };
```

The existing shared drill-down destination gains one explicit wire value:

```rust
#[serde(rename = "topology")]
Topology,
```

and the TypeScript union gains `| "topology"`. Existing Sprint 11 values are
unchanged. No `TopologySnapshotResponse` or other response alias is added;
`topology.snapshot` returns `TopologySnapshot`, and `topology.evidence`
returns `EvidenceRef[]` through the existing `IpcResult` envelope.

The exact JSON representation of the request with all filters absent is:

```json
{
  "filter": {
    "environment_ids": [],
    "team_ids": [],
    "incident_id": null
  },
  "focus_node_id": null,
  "traversal": {
    "direction": "both",
    "max_depth": 3
  }
}
```

The React fixture must preserve `number` values for `confidence`, `depth` and
all topology metrics. It may localize/format them at render time, but it must
not change the contract to strings.

## Source derivation

### Kubernetes

The Kubernetes adapter consumes one or more existing `KubernetesInventory`
values and the environment scope supplied by the caller:

- `KubernetesResource.resource` becomes a node. `Pod`, `Service`, `Node`, and
  `Namespace` map directly; `Deployment`, `StatefulSet` and `DaemonSet` map to
  `Workload`. `Resource.labels` are copied only after the shared masking path.
- Namespaced resources get a Namespace `contains` edge when the namespace is
  present in the canonical resource name. An Environment `contains` edge is
  created for each verified namespace. A missing namespace stays absent and
  does not become `""`.
- Existing `KubernetesTopologyEdge` records map `relationship = "owns"` to
  `Owns` and `relationship = "selects"` to `Selects`. Endpoints resolve within
  the same environment and inventory. An unresolved endpoint is skipped and
  marks the Kubernetes source `unverified`.
- An exact owner UID match receives confidence `1.0`; a same-environment
  kind/name fallback receives `0.9`; an exact Service selector match receives
  `0.9`. These numbers describe identity matching, not causal likelihood.
- Kubernetes health maps to the existing console health state: `healthy` to
  `healthy`, `degraded`, `crash_loop_back_off`, `oom_killed` and `pending` to
  `degraded`, and `unknown` to `unknown`.

The adapter does not issue a second Kubernetes request to discover an edge,
does not execute `kubectl`, and does not infer a dependency merely because two
resources share a label.

### Cloud inventory

The cloud adapter consumes existing `CloudResource` values and the Sprint 11
`EnvironmentStatus` projection:

- each EnvironmentStatus is an Environment node with its existing provider,
  health and evidence;
- a `CloudResourceType::KubernetesCluster` becomes a `Cluster` node, while
  `CloudResourceType::ComputeInstance` and other admitted cloud types become a
  `CloudResource` node; each carries provider, resource type, stable resource
  ID, location and health in safe metadata; and
- an exact environment/resource association becomes an Environment →
  Cluster or CloudResource `contains` edge with confidence `1.0`.

Cloud account labels, resource IDs, portal links and status details pass
through the existing cloud masking/sanitization path. Credentials, auth
headers, CLI commands and raw provider response bodies never enter a node,
edge, metadata map or evidence excerpt.

An EKS/AKS/GKE item is not silently merged with a Kubernetes inventory item.
The provider cloud resource remains a separate node unless the existing
source contracts provide an exact stable identity that the adapter can prove
belongs to the same environment. A name similarity is not enough.

### Observability

Observability is evidence and resource-association input, not an excuse to
invent dependency edges:

- an Alertmanager `NormalizedAlert` with an unambiguous `resource_reference`
  attaches its admitted alert evidence to the matching node;
- a Prometheus `MetricFixture` whose labels identify exactly one existing
  service, workload or pod attaches its metric evidence to that node;
- Loki/OpenTelemetry evidence already admitted by an existing source adapter
  may attach by its verified `ResourceScope`; Sprint 12 does not issue a new
  Loki, OTLP or trace query; and
- an unresolved or ambiguous observability reference remains visible through
  its typed source status/evidence state but does not create a graph edge.

Observability does not create `depends_on` or `routes_to` edges from co-occurring
labels, identical timestamps, shared connectors or incident membership. Those
would be correlation claims and belong to later contracts.

### Incident queue projection

The topology builder consumes the `incident_queue` array from the existing
`OperationsSnapshot`. It resolves affected roots in this order:

1. explicit resource IDs in `IncidentQueueItem.affected_scope.resource_ids`;
2. an exact source reference from the existing alert/metric/health-check
   record already used to build that queue item; and
3. a deterministic fixture binding keyed by the queue item's stable `id`.

The binding is an adapter index, not an incident entity, status store or
lifecycle. If an incident has only a broad scope and no exact resource
identity, the snapshot may show that scoped graph but returns no affected root
or probable path; it reports the limitation as typed source status instead of
claiming every resource in the scope is affected. The Sprint 12 fixture
contains an exact queue-item-to-service binding so the exit criterion is
demonstrable without inventing an incident model.

### Deterministic fixture catalog

The fixture builder uses the source contracts above and adds only static,
committed graph facts. Its fixed evaluation timestamp is
`2026-08-28T09:00:00Z`, matching Sprint 11. The graph fixture contains:

- Environment `env-aws-prod` with Service `checkout`, Workload
  `checkout-api`, Pod `checkout-api-0`, and CloudResource `checkout-rds`;
- a second healthy GCP staging environment with a Service and Workload;
- one explicit fixture `depends_on` chain from `checkout` through
  `checkout-api` to `checkout-rds`, plus a two-edge cycle used only by the
  traversal tests;
- one platform-team mapping from a verified `team` label, one
  `ResourceScope.team_id` fallback, and one unassigned node;
- the Sprint 11 `alert-checkout-s1` queue item with the exact `checkout` node
  as its affected root; and
- verified fixture evidence for every node, edge, ownership mapping,
  summary metric and queue-item binding.

All fixture IDs, source keys, timestamps, labels, edge kinds, confidence
values and evidence IDs are sorted and stable. The fixtures do not capture a
new cloud response, call a provider, run a network request or add an
integration.

The fixture module exposes these exact helper signatures so backend tests and
the React fixture copy use the same evaluation context:

```rust
pub fn fixture_time() -> DateTime<Utc>;
pub fn fixture_scope() -> ResourceScope;
pub fn default_topology_request() -> TopologyRequest;
pub fn topology_fixture_input(scope: ResourceScope) -> TopologyInput;
```

`default_topology_request()` returns the unfiltered request with
`incident_id: None`, no focused node, `direction: Both` and `max_depth: 3`.
The helpers are deterministic constructors, not production data accessors.

## Ownership and team mapping

Ownership resolution is deterministic and conservative. The implementation
uses an internal, non-IPC rule input whose only purpose is to resolve the
output `TopologyOwnership`:

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TopologyOwnershipSelector {
    #[serde(rename = "node_id")]
    NodeId { node_id: String },
    #[serde(rename = "label")]
    Label { key: String, value: String },
    #[serde(rename = "environment")]
    Environment { environment_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TopologyOwnershipRule {
    pub selector: TopologyOwnershipSelector,
    pub team_id: TeamId,
    pub team_name: String,
    pub source: TopologyOwnershipSource,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}
```

This rule is fixture/adapter input and does not cross a Tauri command in
Sprint 12. There is no UI command to create, edit or persist a rule. Duplicate
selectors, blank team names and rules outside the current workspace are
rejected before graph construction.

Resolution precedence is:

1. exact `NodeId` fixture mapping;
2. the highest-specificity matching explicit `Label` mapping;
3. a `ResourceScope.team_id` already attached to the source resource;
4. an exact Environment mapping; and
5. `Unassigned` with `team_id = null` and `team_name = null`.

For equal-specificity label rules, the builder sorts by selector key, value,
team ID and evidence IDs and rejects a conflict rather than choosing based on
input order. A resolved mapping carries the rule/scope evidence IDs. An
unassigned mapping may have an empty ownership evidence list, but the node
itself still requires verified graph evidence. Team IDs are used for filtering;
team names are display data and are never parsed as identifiers.

Ownership does not imply incident ownership, responder assignment, approval
authority or mutation permission. The graph has no principal, role or action
field.

## Filtering and impact traversal

### Selection order

The backend applies the request in a fixed order:

1. build the full current-workspace graph and validate source evidence;
2. resolve `focus_node_id` and/or the IncidentQueueItem roots;
3. traverse from those roots using the requested direction and depth over
   workspace-valid edges;
4. apply Environment and Team filters to nodes, then retain only edges whose
   endpoints remain visible; and
5. calculate summary metrics and validate all references.

When a filter removes a focus/root node, no path is produced and the graph
remains honest about the reduced selection. The backend never expands a team
or environment filter to recover a missing root.

### Upstream and downstream meaning

- **Upstream** walks from a node to edges whose `downstream_node_id` is the
  current node, returning providers, owners and containing resources.
- **Downstream** walks from a node to edges whose `upstream_node_id` is the
  current node, returning consumers, selected pods and contained resources.
- **Both** executes the two walks independently and labels each path with its
  direction. A path is not duplicated merely because both directions visit
  the same node through different edge sequences.

For an Incident filter, affected roots are marked on their nodes and the
traversal shows contextual resources reachable in the requested direction.
The UI labels this as **affected resources** and **probable structural paths**;
it does not label a path “root cause”, “caused by” or “confirmed dependency”.

### Cycles and depth

Cycle detection is per path, not global. A cycle produces one path ending in
`cycle_detected` with its closing edge ID and evidence, while another branch
from the same node continues independently. A depth-limited path is marked
`depth_limit` only when another eligible edge exists beyond the bound. The UI
must expose these typed termination labels and must not imply that a depth
limit means the path ends there in the real system.

`max_depth` is checked before traversal. The allowed range is `0..=8`; invalid
values return a typed invalid-request error and do not partially execute.

### Numeric validation

The graph builder validates every `TopologyMetric.value`, edge `confidence`,
path `confidence` and derived count before constructing the snapshot. Counts
are represented as finite `f64` values with `NumberUnit::Count`. The internal
error is typed so tests can distinguish it from malformed source data:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologyNumberField {
    MetricValue,
    EdgeConfidence,
    PathConfidence,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TopologyError {
    #[error("invalid topology request")]
    InvalidRequest,
    #[error("topology node was not found")]
    NodeNotFound,
    #[error("incident queue item was not found")]
    IncidentNotFound,
    #[error("topology scope is not allowed")]
    ScopeDenied,
    #[error("topology evidence is not verified")]
    EvidenceUnverified,
    #[error("topology evidence is missing")]
    EvidenceMissing,
    #[error("topology number is not finite")]
    NonFiniteNumber(TopologyNumberField),
    #[error("topology confidence is outside the allowed range")]
    ConfidenceOutOfRange,
    #[error("topology source is malformed")]
    MalformedSource,
}
```

The error text above is an internal diagnostic vocabulary. AppState maps its
variants to the existing serializable `IpcError` code and React renders a
localized message key; no source payload or dynamic number is interpolated
into an error message.

## Graph-to-evidence navigation contract

The graph is evidence-first at every level:

- a node's `evidence_ids` identify the admitted inventory, alert, metric or
  fixture evidence behind the node; its `drill_down.destination` is `topology`
  and its `filter_key` is the backend-issued node ID;
- an edge's evidence IDs identify the exact Kubernetes selector/owner,
  cloud-association record or fixture relationship; its drill-down opens the
  evidence destination;
- a path's evidence IDs are the sorted union of its node/edge evidence plus a
  closing cycle edge when present; its drill-down opens the evidence panel;
- a node or summary `TopologyMetric` carries both `drill_down` and
  `drill_down_reference`, including source query, scope, optional time window
  and evidence IDs; and
- the snapshot itself includes the admitted `EvidenceRef` set so the UI can
  render a first response without a second call. The evidence command exists
  for refresh/selection and follows the same scope checks.

React may use `filter_key` to focus a local graph or construct a new
`TopologyRequest.focus_node_id`, but it must never concatenate it into a
provider query or URL. `topology.evidence` accepts only an array of evidence
IDs:

```json
{
  "evidence_ids": ["evidence-topology-checkout-service"]
}
```

The backend rejects an empty list, duplicates, IDs not emitted by the current
snapshot, cross-workspace IDs and unverified evidence. It returns evidence in
request order only after resolving all IDs; a partial success is not allowed.

An evidence panel displays source kind, connector, endpoint, query, observed
time, excerpt and redaction state. It never displays credentials,
authorization headers, provider error bodies or an unmasked Restricted value.
An existing `native_url` is opened only through the existing shell permission
after the existing HTTPS/trusted-source guard; the UI cannot submit a URL.

## Trust, capability and policy boundary

### New IPC commands

The command envelope remains unbounded at the transport boundary. Rust resolves
that envelope to the current workspace before reading source data. No topology
request can widen membership scope by submitting an environment, team or node
ID in the envelope.

| Tauri function | Envelope command | Capability | Permission | Scope | Purpose |
| --- | --- | --- | --- | --- | --- |
| `topology_snapshot` | `topology.snapshot` | `WorkspaceRead` | `Read` | Unbounded envelope resolved to the current workspace | Return the filtered, redacted topology projection and probable structural paths. |
| `topology_evidence` | `topology.evidence` | `ResourceRead` | `Read` | Unbounded envelope resolved to the current workspace; evidence IDs checked server-side | Return only evidence IDs already emitted by a valid topology snapshot. |

There is no `topology.write`, `incident.write`, `topology.act`, provider CLI
command, query command or manual traversal command. Traversal is an internal
pure function of `TopologyRequest` and the current graph.

Both handlers use the established authorization sequence:

1. construct the exact `CommandDescriptor` (`topology.snapshot` with
   `WorkspaceRead`/`Read`, or `topology.evidence` with
   `ResourceRead`/`Read`);
2. reject a wrong command/capability, a bounded or unexpected envelope scope,
   an inactive membership, a principal mismatch, a membership grant outside
   the current workspace or a role without `Read`;
3. parse the typed payload and reject invalid depth, unknown focus nodes,
   duplicate filter IDs and unknown/cross-scope IncidentQueueItem IDs before
   graph work;
4. admit only source records and evidence whose source policy/classification
   is verified;
5. build/filter/traverse and run the complete topology validation; and
6. evaluate the existing `EgressDestination::Ui` policy with verified
   `Internal` data before serializing `IpcResult`.

The snapshot command does not request live data in this sprint, so it performs
no external egress. If a later adapter supplies a live source, it must reuse
the existing Kubernetes/cloud/observability capability and transport checks;
the topology command does not grant a connector capability. Any retained local
topology/evidence audit metadata must pass the existing `AuditLog` policy with
verified data before storage. A policy denial fails closed.

The command never requires `IncidentWrite`, `ConnectorAct`, `PolicyManage` or
`ExecuteAction`. Incident filtering uses `WorkspaceRead` because it reads the
Sprint 11 workspace projection, not the canonical incident domain. Evidence
lookup uses `ResourceRead` because evidence is resource-scoped and IDs are
validated against the workspace snapshot.

### Masking and redaction

The existing source masking and policy runtime remain authoritative:

- Kubernetes resource labels and observability label maps pass through the
  existing sensitive-key path before becoming `TopologyNode.labels`;
- edge `metadata` passes through the same recursive masking behavior used for
  JSON source objects. Keys matching the existing deny list (including
  `password`, `secret`, `token`, `key` and `credential`) have their values
  replaced by the existing redacted marker. Fields that cannot be parsed are
  omitted or mark the source `unverified`; an unparsed value is never marked
  `masked`;
- cloud credential resolution, signed/bearer authorization headers, kubeconfig
  credentials, provider CLI commands and raw provider errors are transient and
  never copied into node labels, edge metadata, evidence, fixtures or audit
  metadata;
- node names, native IDs and connector IDs are admitted through the existing
  safe-identifier/safe-text path. A sensitive or unparseable identity causes
  the record to be omitted, not displayed with a blank value;
- `EvidenceRef` is admitted only when both
  `classification_verified` and `redaction_verified` are true. An evidence
  item with an immutable Restricted value is omitted and its source becomes
  `unverified`;
- the UI may display a verified local masked excerpt and the typed `masked`
  and `unparsed` flags, but it cannot turn an unparsed excerpt into a masked
  claim; and
- the separate UI, local-storage, external-integration and AuditLog policies
  continue to apply. Sprint 12 adds no value-pattern redaction and does not
  weaken immutable Restricted-data blocking or fail-closed egress.

`TopologyNode.labels` and `TopologyEdge.metadata` are display metadata, not
authorization input. They cannot alter scope, ownership permission, edge
direction, confidence or traversal limits.

## React interaction contract

The topology workspace is a focused, accessible graph view rather than a new
provider-specific dashboard:

1. The filter bar exposes Environment, Team and Incident controls. It sends a
   complete `TopologyRequest` with explicit empty arrays/nulls, direction and
   a bounded depth.
2. The graph renders an accessible list/table representation alongside the
   visual relationship view so keyboard and screen-reader users can select a
   node or edge without relying on spatial layout or color.
3. Affected incident roots use text and a non-color status indicator. Context
   nodes are visibly distinct through labels, not color alone.
4. Node selection shows owner/team, provider, status, safe labels, metric (if
   present), source provenance and an evidence control. The node control can
   recenter the graph by sending its backend-issued ID as `focus_node_id`.
5. Edge and path selection shows relation kind, upstream/downstream direction,
   structural confidence, provenance, termination state and evidence. The UI
   always labels `probable_structural` as probable.
6. The evidence drawer reuses the existing source/query/time-window/masking
   presentation and opens only trusted HTTPS native links.
7. Empty graph, no incident roots, depth-limited, cycle-terminated, stale,
   unavailable and unverified states are all typed/localized. No state is
   represented by an empty string or color alone.

The UI has no graph editing, drag-to-create-edge, ownership mutation,
incident write, remediation button or provider query field. It may persist only
the existing local presentation preferences (selected layout/depth if the
product shell already supports them); filters remain request data and are not
authorization or policy data.

## Verification and acceptance

Rust contract and unit tests must cover:

- exact JSON serialization for every new enum and the additive `topology`
  drill-down destination;
- Rust/TypeScript field-name, nullability and enum-value parity;
- stable node/edge IDs, supported source-kind mappings and deterministic
  provenance ordering;
- Kubernetes owner/selector edges, cloud containment, observability evidence
  attachment, unsupported/ambiguous source records and partial-source status;
- ownership precedence, team-name mapping, unassigned nodes, duplicate/conflict
  rules and team-filter behavior;
- upstream, downstream and both-direction traversal, depth `0`, depth `8`,
  depth rejection, branch continuation, cycle termination, path ordering,
  minimum-edge confidence and the absence of any causal/proven-root field;
- Environment ∩ Team ∩ Incident filter composition, unknown incident IDs,
  broad incident scopes without exact roots, backend-issued focus-node checks
  and affected-root marking;
- finite f64 metrics/confidence, rejection of non-finite values with the typed
  `TopologyError::NonFiniteNumber` variant and rejection of out-of-range
  confidence;
- critical summary/node metric evidence and drill-down references;
- `topology.snapshot` command/capability/scope/membership/role/payload/policy
  failures; and
- `topology.evidence` empty/duplicate/unknown/cross-scope/unverified IDs,
  UI-policy denial and serialized-output secret scans.

React tests must cover:

- rendering the copied topology fixture before the IPC backend exists;
- an Incident filter showing affected resources and a probable structural path;
- Environment and Team filter composition and explicit empty/null request
  shape;
- upstream/downstream/both controls, depth-limit and cycle labels;
- ownership display including unassigned and localized source labels;
- node/edge/path evidence controls sending only backend-issued IDs;
- f64 confidence/metric rendering without changing the fixture contract;
- keyboard navigation, focus state, screen-reader labels and non-color status;
- stale, unavailable, unverified, empty and malformed source states; and
- identical English/Thai locale object structure.

The sprint is accepted only when the deterministic fixture demonstrates the
quoted exit criterion, no new network or provisioning path exists, no incident
lifecycle/write/action path exists, no Sprint 13 signal normalization or
correlation is introduced, every rendered graph fact is evidence-backed, and
`npm run format:check` passes.
