//! Deterministic source records used by topology tests and local previews.
//!
//! These constructors deliberately build the same provider-neutral contracts
//! that live adapters return.  They never resolve credentials or access a
//! provider.

use crate::cloud::{CloudHealthState, CloudProvider, CloudResource, CloudResourceType};
use crate::kubernetes::{
    KubernetesHealth, KubernetesInventory, KubernetesOwner, KubernetesReplicaSummary,
    KubernetesResource, KubernetesTopologyEdge,
};
use crate::observability::alertmanager::{
    AlertSourceReference, NormalizedAlert, ResourceReference,
};
use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, BTreeSet};
use thalassa_domain::{
    BusinessImpact, ConsoleHealthState, ConsolePriority, ConsoleSeverity, CriticalNumber,
    DrillDownDestination, DrillDownReference, DrillDownTarget, EvidenceRedaction, EvidenceRef,
    EvidenceSourceKind, IncidentQueueItem, MetricFixture, MetricFixtureSample, MetricFixtureSource,
    NumberUnit, QueueItemSourceKind, QueueStatus, Resource, ResourceScope, SourceState,
    SourceStatus, TopologyDirection, TopologyEdge, TopologyEdgeKind, TopologyEdgeProvenance,
    TopologyFilter, TopologyOwnershipRule, TopologyOwnershipSelector, TopologyRequest,
    TopologySourceKind, TopologyTraversal,
};
use uuid::Uuid;

/// Provider-neutral input consumed by [`super::TopologyBuilder`].
#[derive(Clone, Debug)]
pub struct TopologyInput {
    pub generated_at: DateTime<Utc>,
    pub scope: ResourceScope,
    pub kubernetes: BTreeMap<String, KubernetesInventory>,
    pub cloud_resources: Vec<CloudResource>,
    pub environments: Vec<thalassa_domain::EnvironmentStatus>,
    pub alerts: Vec<NormalizedAlert>,
    pub metrics: Vec<MetricFixture>,
    pub incident_queue: Vec<IncidentQueueItem>,
    pub ownership_rules: Vec<TopologyOwnershipRule>,
    pub fixture_edges: Vec<TopologyEdge>,
    pub incident_root_nodes: BTreeMap<String, Vec<String>>,
    pub source_status: Vec<SourceStatus>,
    pub evidence: Vec<EvidenceRef>,
}

/// Fixed topology fixture evaluation time.
pub fn fixture_time() -> DateTime<Utc> {
    match DateTime::<Utc>::from_timestamp(1_787_907_600, 0) {
        Some(timestamp) => timestamp,
        None => DateTime::<Utc>::UNIX_EPOCH,
    }
}

/// Workspace scope shared by all deterministic topology records.
pub fn fixture_scope() -> ResourceScope {
    ResourceScope::workspace(
        Uuid::from_u128(0x00000000000000000000000000000012),
        Uuid::from_u128(0x00000000000000000000000000000013),
        Uuid::from_u128(0x00000000000000000000000000000014),
    )
}

/// The default unfiltered topology request used by source tests.
pub fn default_topology_request() -> TopologyRequest {
    TopologyRequest {
        filter: TopologyFilter {
            environment_ids: Vec::new(),
            team_ids: Vec::new(),
            incident_id: None,
        },
        focus_node_id: None,
        traversal: TopologyTraversal {
            direction: TopologyDirection::Both,
            max_depth: 3,
        },
    }
}

/// Build a complete deterministic topology input from source contracts.
pub fn topology_fixture_input(scope: ResourceScope) -> TopologyInput {
    let generated_at = fixture_time();
    let team_id = match scope.team_id {
        Some(team_id) => team_id,
        None => Uuid::nil(),
    };
    let aws_inventory = aws_inventory(&scope, generated_at);
    let gcp_inventory = gcp_inventory(&scope, generated_at);
    let service_resource_id = Uuid::from_u128(0x00000000000000000000000000000101);

    let service_id = kubernetes_node_id("env-aws-prod", "Service", "uid-service-checkout");
    let workload_id = kubernetes_node_id("env-aws-prod", "Deployment", "uid-workload-checkout-api");
    let database_id = cloud_node_id("env-aws-prod", "checkout-rds");
    let replica_id = cloud_node_id("env-aws-prod", "checkout-rds-replica");
    let unassigned_id = kubernetes_node_id(
        "env-aws-prod",
        "Deployment",
        "uid-workload-unassigned-worker",
    );

    let environments = vec![
        fixture_environment(
            "env-aws-prod",
            "AWS production",
            Some("aws"),
            ConsoleHealthState::Degraded,
            "one service is degraded",
            "evidence-topology-environment-aws",
            &scope,
        ),
        fixture_environment(
            "env-gcp-staging",
            "GCP staging",
            Some("gcp"),
            ConsoleHealthState::Healthy,
            "all connected resources are healthy",
            "evidence-topology-environment-gcp",
            &scope,
        ),
    ];

    let cloud_resources = vec![
        CloudResource {
            provider: CloudProvider::Aws,
            environment_id: "env-aws-prod".into(),
            resource_type: CloudResourceType::ComputeInstance,
            id: "checkout-rds".into(),
            name: "checkout-rds".into(),
            location: "us-east-1".into(),
            health: CloudHealthState::Degraded,
            status_detail: "database is serving traffic".into(),
            console_url: "https://console.example/cloud/checkout-rds".into(),
            cli_command: "cloudctl describe checkout-rds".into(),
        },
        CloudResource {
            provider: CloudProvider::Aws,
            environment_id: "env-aws-prod".into(),
            resource_type: CloudResourceType::ComputeInstance,
            id: "checkout-rds-replica".into(),
            name: "checkout-rds-replica".into(),
            location: "us-east-1".into(),
            health: CloudHealthState::Healthy,
            status_detail: "replica is available".into(),
            console_url: "https://console.example/cloud/checkout-rds-replica".into(),
            cli_command: "cloudctl describe checkout-rds-replica".into(),
        },
        CloudResource {
            provider: CloudProvider::Gcp,
            environment_id: "env-gcp-staging".into(),
            resource_type: CloudResourceType::KubernetesCluster,
            id: "catalog-cluster".into(),
            name: "catalog-cluster".into(),
            location: "us-central1".into(),
            health: CloudHealthState::Healthy,
            status_detail: "cluster is ready".into(),
            console_url: "https://console.example/cloud/catalog-cluster".into(),
            cli_command: "cloudctl describe catalog-cluster".into(),
        },
    ];

    let evidence = fixture_evidence_set(&scope);
    let fixture_edges = vec![
        fixture_edge(
            "edge:fixture:checkout-depends-on-api",
            &service_id,
            &workload_id,
            "evidence-topology-edge-checkout-api",
            0.8,
        ),
        fixture_edge(
            "edge:fixture:api-depends-on-rds",
            &workload_id,
            &database_id,
            "evidence-topology-edge-api-rds",
            0.85,
        ),
        fixture_edge(
            "edge:fixture:rds-depends-on-replica",
            &database_id,
            &replica_id,
            "evidence-topology-edge-rds-replica",
            0.7,
        ),
        fixture_edge(
            "edge:fixture:replica-depends-on-rds",
            &replica_id,
            &database_id,
            "evidence-topology-edge-replica-rds",
            0.7,
        ),
    ];

    let incident = fixture_incident(&scope, service_resource_id);
    let mut incident_root_nodes = BTreeMap::new();
    incident_root_nodes.insert(incident.id.clone(), vec![service_id]);

    TopologyInput {
        generated_at,
        scope: scope.clone(),
        kubernetes: BTreeMap::from([
            ("env-aws-prod".into(), aws_inventory),
            ("env-gcp-staging".into(), gcp_inventory),
        ]),
        cloud_resources,
        environments,
        alerts: vec![fixture_alert()],
        metrics: vec![MetricFixture {
            key: "metric-checkout-requests".into(),
            scope: scope.clone(),
            labels: BTreeMap::from([
                ("__name__".into(), "checkout_requests_total".into()),
                ("environment".into(), "env-aws-prod".into()),
                ("namespace".into(), "prod".into()),
                ("service".into(), "checkout".into()),
            ]),
            samples: vec![MetricFixtureSample {
                timestamp_seconds: generated_at.timestamp(),
                value: "42".into(),
            }],
            source: MetricFixtureSource {
                connector_id: "prometheus-prod".into(),
                query: "checkout_requests_total".into(),
                endpoint: "/api/v1/query_range".into(),
            },
        }],
        incident_queue: vec![incident],
        ownership_rules: vec![
            TopologyOwnershipRule {
                selector: TopologyOwnershipSelector::Label {
                    key: "team".into(),
                    value: "platform".into(),
                },
                team_id,
                team_name: "Platform".into(),
                source: thalassa_domain::TopologyOwnershipSource::ExplicitLabel,
                evidence_ids: vec!["evidence-topology-ownership-platform".into()],
            },
            TopologyOwnershipRule {
                selector: TopologyOwnershipSelector::Environment {
                    environment_id: "env-gcp-staging".into(),
                },
                team_id,
                team_name: "Platform".into(),
                source: thalassa_domain::TopologyOwnershipSource::EnvironmentDefault,
                evidence_ids: vec!["evidence-topology-ownership-environment".into()],
            },
            TopologyOwnershipRule {
                selector: TopologyOwnershipSelector::NodeId {
                    node_id: unassigned_id,
                },
                team_id,
                team_name: "Platform".into(),
                source: thalassa_domain::TopologyOwnershipSource::Unassigned,
                evidence_ids: vec!["evidence-topology-ownership-unassigned".into()],
            },
        ],
        fixture_edges,
        incident_root_nodes,
        source_status: fixture_source_status(),
        evidence,
    }
}

pub(crate) fn kubernetes_node_id(environment_id: &str, kind: &str, native_id: &str) -> String {
    format!(
        "node:kubernetes:{environment_id}:{}:{native_id}",
        topology_kind_name(kind)
    )
}

pub(crate) fn cloud_node_id(environment_id: &str, native_id: &str) -> String {
    format!("node:cloud:{environment_id}:cloud_resource:{native_id}")
}

fn topology_kind_name(kind: &str) -> &str {
    match kind {
        "Deployment" | "StatefulSet" | "DaemonSet" => "workload",
        "Service" => "service",
        "Pod" => "pod",
        "Node" => "node",
        "Namespace" => "namespace",
        _ => "resource",
    }
}

fn aws_inventory(scope: &ResourceScope, _generated_at: DateTime<Utc>) -> KubernetesInventory {
    let namespace_id = Uuid::from_u128(0x00000000000000000000000000000201);
    let service_id = Uuid::from_u128(0x00000000000000000000000000000101);
    let workload_id = Uuid::from_u128(0x00000000000000000000000000000102);
    let pod_id = Uuid::from_u128(0x00000000000000000000000000000103);
    let node_id = Uuid::from_u128(0x00000000000000000000000000000202);
    let unassigned_id = Uuid::from_u128(0x00000000000000000000000000000104);
    KubernetesInventory {
        resources: vec![
            kubernetes_resource(
                namespace_id,
                scope,
                "Namespace",
                "prod",
                "uid-namespace-prod",
                KubernetesHealth::Unknown,
                BTreeMap::new(),
                None,
                None,
            ),
            kubernetes_resource(
                service_id,
                scope,
                "Service",
                "prod/checkout",
                "uid-service-checkout",
                KubernetesHealth::Unknown,
                BTreeMap::from([
                    ("app".into(), "checkout".into()),
                    ("team".into(), "platform".into()),
                ]),
                None,
                Some(BTreeMap::from([("app".into(), "checkout-api".into())])),
            ),
            kubernetes_resource(
                workload_id,
                scope,
                "Deployment",
                "prod/checkout-api",
                "uid-workload-checkout-api",
                KubernetesHealth::Healthy,
                BTreeMap::from([(String::from("app"), String::from("checkout-api"))]),
                Some(KubernetesReplicaSummary {
                    desired: 3,
                    ready: 3,
                    available: Some(3),
                }),
                None,
            ),
            kubernetes_resource_with_owner(
                pod_id,
                scope,
                "Pod",
                "prod/checkout-api-0",
                "uid-pod-checkout-api-0",
                KubernetesHealth::Healthy,
                BTreeMap::from([(String::from("app"), String::from("checkout-api"))]),
                Some(KubernetesOwner {
                    kind: "Deployment".into(),
                    name: "checkout-api".into(),
                    uid: Some("uid-workload-checkout-api".into()),
                }),
            ),
            kubernetes_resource(
                node_id,
                scope,
                "Node",
                "worker-a",
                "uid-node-worker-a",
                KubernetesHealth::Healthy,
                BTreeMap::from([(String::from("role"), String::from("worker"))]),
                None,
                None,
            ),
            kubernetes_resource(
                unassigned_id,
                scope,
                "Deployment",
                "prod/unassigned-worker",
                "uid-workload-unassigned-worker",
                KubernetesHealth::Healthy,
                BTreeMap::from([(String::from("app"), String::from("unassigned-worker"))]),
                Some(KubernetesReplicaSummary {
                    desired: 1,
                    ready: 1,
                    available: Some(1),
                }),
                None,
            ),
        ],
        availability: vec![],
        topology: vec![
            KubernetesTopologyEdge {
                from_kind: "Deployment".into(),
                from_name: "checkout-api".into(),
                to_kind: "Pod".into(),
                to_name: "prod/checkout-api-0".into(),
                relationship: "owns".into(),
            },
            KubernetesTopologyEdge {
                from_kind: "Service".into(),
                from_name: "prod/checkout".into(),
                to_kind: "Pod".into(),
                to_name: "prod/checkout-api-0".into(),
                relationship: "selects".into(),
            },
        ],
    }
}

fn gcp_inventory(scope: &ResourceScope, _generated_at: DateTime<Utc>) -> KubernetesInventory {
    let namespace_id = Uuid::from_u128(0x00000000000000000000000000000301);
    let service_id = Uuid::from_u128(0x00000000000000000000000000000302);
    let workload_id = Uuid::from_u128(0x00000000000000000000000000000303);
    let pod_id = Uuid::from_u128(0x00000000000000000000000000000304);
    KubernetesInventory {
        resources: vec![
            kubernetes_resource(
                namespace_id,
                scope,
                "Namespace",
                "staging",
                "uid-namespace-staging",
                KubernetesHealth::Unknown,
                BTreeMap::new(),
                None,
                None,
            ),
            kubernetes_resource(
                service_id,
                scope,
                "Service",
                "staging/catalog",
                "uid-service-catalog",
                KubernetesHealth::Unknown,
                BTreeMap::from([(String::from("app"), String::from("catalog-api"))]),
                None,
                Some(BTreeMap::from([("app".into(), "catalog-api".into())])),
            ),
            kubernetes_resource(
                workload_id,
                scope,
                "Deployment",
                "staging/catalog-api",
                "uid-workload-catalog-api",
                KubernetesHealth::Healthy,
                BTreeMap::from([(String::from("app"), String::from("catalog-api"))]),
                Some(KubernetesReplicaSummary {
                    desired: 2,
                    ready: 2,
                    available: Some(2),
                }),
                None,
            ),
            kubernetes_resource_with_owner(
                pod_id,
                scope,
                "Pod",
                "staging/catalog-api-0",
                "uid-pod-catalog-api-0",
                KubernetesHealth::Healthy,
                BTreeMap::from([(String::from("app"), String::from("catalog-api"))]),
                Some(KubernetesOwner {
                    kind: "Deployment".into(),
                    name: "catalog-api".into(),
                    uid: Some("uid-workload-catalog-api".into()),
                }),
            ),
        ],
        availability: vec![],
        topology: vec![
            KubernetesTopologyEdge {
                from_kind: "Deployment".into(),
                from_name: "catalog-api".into(),
                to_kind: "Pod".into(),
                to_name: "staging/catalog-api-0".into(),
                relationship: "owns".into(),
            },
            KubernetesTopologyEdge {
                from_kind: "Service".into(),
                from_name: "staging/catalog".into(),
                to_kind: "Pod".into(),
                to_name: "staging/catalog-api-0".into(),
                relationship: "selects".into(),
            },
        ],
    }
}

#[allow(clippy::too_many_arguments)]
fn kubernetes_resource(
    id: Uuid,
    scope: &ResourceScope,
    kind: &str,
    name: &str,
    native_id: &str,
    health: KubernetesHealth,
    labels: BTreeMap<String, String>,
    replicas: Option<KubernetesReplicaSummary>,
    service_selector: Option<BTreeMap<String, String>>,
) -> KubernetesResource {
    KubernetesResource {
        resource: Resource {
            id,
            environment_id: Uuid::from_u128(0x00000000000000000000000000000021),
            scope: scope.clone(),
            kind: kind.into(),
            name: name.into(),
            provider: Some("kubernetes".into()),
            native_id: Some(native_id.into()),
            labels,
            created_at: fixture_time(),
        },
        status: None,
        conditions: vec![],
        owner: None,
        service_selector,
        replicas,
        containers: vec![],
        health,
    }
}

#[allow(clippy::too_many_arguments)]
fn kubernetes_resource_with_owner(
    id: Uuid,
    scope: &ResourceScope,
    kind: &str,
    name: &str,
    native_id: &str,
    health: KubernetesHealth,
    labels: BTreeMap<String, String>,
    owner: Option<KubernetesOwner>,
) -> KubernetesResource {
    let mut resource =
        kubernetes_resource(id, scope, kind, name, native_id, health, labels, None, None);
    resource.owner = owner;
    resource
}

fn fixture_environment(
    environment_id: &str,
    name: &str,
    provider: Option<&str>,
    health: ConsoleHealthState,
    detail: &str,
    evidence_id: &str,
    scope: &ResourceScope,
) -> thalassa_domain::EnvironmentStatus {
    let evidence_id = evidence_id.to_string();
    let reference = DrillDownReference {
        source_query: format!("environment:{environment_id}"),
        scope: scope.clone(),
        time_window: None,
        evidence_ids: vec![evidence_id.clone()],
    };
    thalassa_domain::EnvironmentStatus {
        environment_id: environment_id.into(),
        name: name.into(),
        provider: provider.map(str::to_owned),
        health,
        status_detail: detail.into(),
        resource_count: CriticalNumber {
            key: format!("environment.{environment_id}.resource_count"),
            value: "4".into(),
            unit: NumberUnit::Count,
            evidence_ids: vec![evidence_id.clone()],
            drill_down: DrillDownTarget {
                destination: DrillDownDestination::EnvironmentStatus,
                evidence_ids: vec![evidence_id.clone()],
                filter_key: Some(environment_id.into()),
            },
            drill_down_reference: reference,
        },
        last_observed_at: fixture_time().to_rfc3339(),
        evidence_ids: vec![evidence_id.clone()],
        drill_down: DrillDownTarget {
            destination: DrillDownDestination::EnvironmentStatus,
            evidence_ids: vec![evidence_id],
            filter_key: Some(environment_id.into()),
        },
    }
}

fn fixture_alert() -> NormalizedAlert {
    NormalizedAlert {
        fingerprint: "alert-checkout-s1".into(),
        state: "firing".into(),
        starts_at: "2026-08-28T08:55:00Z".into(),
        ends_at: "2026-08-28T09:00:00Z".into(),
        labels: BTreeMap::from([
            ("alertname".into(), "CheckoutUnavailable".into()),
            ("environment".into(), "env-aws-prod".into()),
            ("namespace".into(), "prod".into()),
            ("service".into(), "checkout".into()),
        ]),
        annotations: BTreeMap::from([("summary".into(), "Checkout unavailable".into())]),
        generator_url: Some("https://prometheus.example/graph".into()),
        source: AlertSourceReference {
            connector_id: "alertmanager-prod".into(),
            endpoint: "/api/v2/alerts".into(),
        },
        resource_reference: ResourceReference::Resolved {
            namespace: "prod".into(),
            kind: "Service".into(),
            name: "checkout".into(),
        },
    }
}

fn fixture_incident(scope: &ResourceScope, resource_id: Uuid) -> IncidentQueueItem {
    let evidence_id = "evidence-topology-incident-checkout".to_string();
    IncidentQueueItem {
        id: "alert-checkout-s1".into(),
        title: "Checkout unavailable".into(),
        source_kind: QueueItemSourceKind::Alert,
        source_id: "alert-checkout-s1".into(),
        severity: ConsoleSeverity::S1,
        priority: Some(ConsolePriority::P1),
        status: QueueStatus::Detected,
        business_impact: BusinessImpact {
            level: thalassa_domain::ImpactLevel::Critical,
            summary: "Checkout requests are failing".into(),
            customer_scope: "production checkout customers".into(),
            service_criticality: "tier-0".into(),
            trajectory: thalassa_domain::ImpactTrajectory::Expanding,
            dimensions: thalassa_domain::ImpactDimensions::single_dimension(
                thalassa_domain::ImpactLevel::Critical,
                thalassa_domain::ImpactTrajectory::Expanding,
            ),
            evidence_ids: vec![evidence_id.clone()],
        },
        scope: scope.clone(),
        detected_at: "2026-08-28T08:55:00Z".into(),
        opened_at: "2026-08-28T08:55:00Z".into(),
        last_update: "2026-08-28T08:59:00Z".into(),
        affected_scope: scope.clone().resource(resource_id),
        evidence_ids: vec![evidence_id.clone()],
        drill_down: DrillDownTarget {
            destination: DrillDownDestination::IncidentQueue,
            evidence_ids: vec![evidence_id.clone()],
            filter_key: Some("alert-checkout-s1".into()),
        },
        drill_down_reference: DrillDownReference {
            source_query: "incident:alert-checkout-s1".into(),
            scope: scope.clone(),
            time_window: None,
            evidence_ids: vec![evidence_id],
        },
    }
}

fn fixture_edge(
    id: &str,
    upstream_node_id: &str,
    downstream_node_id: &str,
    evidence_id: &str,
    confidence: f64,
) -> TopologyEdge {
    let evidence_id = evidence_id.to_string();
    TopologyEdge {
        id: id.into(),
        upstream_node_id: upstream_node_id.into(),
        downstream_node_id: downstream_node_id.into(),
        kind: TopologyEdgeKind::DependsOn,
        provenance: vec![TopologyEdgeProvenance {
            source: TopologySourceKind::Fixture,
            source_key: "fixture:topology".into(),
            observed_at: Some(fixture_time().to_rfc3339()),
        }],
        confidence,
        metadata: BTreeMap::from([("relationship".into(), "depends_on".into())]),
        evidence_ids: vec![evidence_id.clone()],
        drill_down: DrillDownTarget {
            destination: DrillDownDestination::Evidence,
            evidence_ids: vec![evidence_id],
            filter_key: None,
        },
    }
}

fn fixture_source_status() -> Vec<SourceStatus> {
    vec![
        SourceStatus {
            source_key: "cloud".into(),
            state: SourceState::Fresh,
            reason: None,
            detail: None,
            observed_at: Some(fixture_time().to_rfc3339()),
            evidence_ids: vec![
                "evidence-topology-environment-aws".into(),
                "evidence-topology-environment-gcp".into(),
                "evidence-topology-cloud-checkout-rds".into(),
                "evidence-topology-cloud-checkout-rds-replica".into(),
                "evidence-topology-cloud-catalog-cluster".into(),
            ],
        },
        SourceStatus {
            source_key: "fixtures".into(),
            state: SourceState::Fresh,
            reason: None,
            detail: None,
            observed_at: Some(fixture_time().to_rfc3339()),
            evidence_ids: vec![
                "evidence-topology-edge-checkout-api".into(),
                "evidence-topology-edge-api-rds".into(),
                "evidence-topology-edge-rds-replica".into(),
                "evidence-topology-edge-replica-rds".into(),
                "evidence-topology-ownership-platform".into(),
                "evidence-topology-ownership-environment".into(),
                "evidence-topology-ownership-unassigned".into(),
                "evidence-topology-incident-checkout".into(),
                "evidence-topology-summary".into(),
            ],
        },
        SourceStatus {
            source_key: "kubernetes:env-aws-prod".into(),
            state: SourceState::Fresh,
            reason: None,
            detail: None,
            observed_at: Some(fixture_time().to_rfc3339()),
            evidence_ids: vec![
                "evidence-topology-k8s-namespace-prod".into(),
                "evidence-topology-k8s-service-checkout".into(),
                "evidence-topology-k8s-workload-checkout-api".into(),
                "evidence-topology-k8s-pod-checkout-api-0".into(),
                "evidence-topology-k8s-node-worker-a".into(),
                "evidence-topology-k8s-workload-unassigned-worker".into(),
            ],
        },
        SourceStatus {
            source_key: "kubernetes:env-gcp-staging".into(),
            state: SourceState::Fresh,
            reason: None,
            detail: None,
            observed_at: Some(fixture_time().to_rfc3339()),
            evidence_ids: vec![
                "evidence-topology-k8s-namespace-staging".into(),
                "evidence-topology-k8s-service-catalog".into(),
                "evidence-topology-k8s-workload-catalog-api".into(),
                "evidence-topology-k8s-pod-catalog-api-0".into(),
            ],
        },
        SourceStatus {
            source_key: "observability".into(),
            state: SourceState::Fresh,
            reason: None,
            detail: None,
            observed_at: Some(fixture_time().to_rfc3339()),
            evidence_ids: vec![
                "evidence-topology-alert-checkout".into(),
                "evidence-topology-metric-checkout".into(),
            ],
        },
    ]
}

fn fixture_evidence_set(scope: &ResourceScope) -> Vec<EvidenceRef> {
    let mut entries = Vec::new();
    let mut add = |id: &str, source_kind: EvidenceSourceKind, excerpt: &str| {
        entries.push(EvidenceRef {
            id: id.into(),
            source_kind,
            connector_id: Some("fixture-topology".into()),
            scope: scope.clone(),
            endpoint: "fixture://topology".into(),
            query: Some(id.into()),
            observed_at: fixture_time().to_rfc3339(),
            excerpt: excerpt.into(),
            native_url: None,
            redaction: EvidenceRedaction {
                classification_verified: true,
                redaction_verified: true,
                masked: false,
                unparsed: false,
            },
        });
    };

    add(
        "evidence-topology-environment-aws",
        EvidenceSourceKind::Cloud,
        "AWS production environment status",
    );
    add(
        "evidence-topology-environment-gcp",
        EvidenceSourceKind::Cloud,
        "GCP staging environment status",
    );
    add(
        "evidence-topology-cloud-checkout-rds",
        EvidenceSourceKind::Cloud,
        "checkout database resource",
    );
    add(
        "evidence-topology-cloud-checkout-rds-replica",
        EvidenceSourceKind::Cloud,
        "checkout database replica resource",
    );
    add(
        "evidence-topology-cloud-catalog-cluster",
        EvidenceSourceKind::Cloud,
        "catalog cluster resource",
    );
    add(
        "evidence-topology-k8s-namespace-prod",
        EvidenceSourceKind::Kubernetes,
        "production namespace",
    );
    add(
        "evidence-topology-k8s-service-checkout",
        EvidenceSourceKind::Kubernetes,
        "checkout service",
    );
    add(
        "evidence-topology-k8s-workload-checkout-api",
        EvidenceSourceKind::Kubernetes,
        "checkout API workload",
    );
    add(
        "evidence-topology-k8s-pod-checkout-api-0",
        EvidenceSourceKind::Kubernetes,
        "checkout API pod",
    );
    add(
        "evidence-topology-k8s-node-worker-a",
        EvidenceSourceKind::Kubernetes,
        "worker node",
    );
    add(
        "evidence-topology-k8s-workload-unassigned-worker",
        EvidenceSourceKind::Kubernetes,
        "unassigned worker workload",
    );
    add(
        "evidence-topology-k8s-namespace-staging",
        EvidenceSourceKind::Kubernetes,
        "staging namespace",
    );
    add(
        "evidence-topology-k8s-service-catalog",
        EvidenceSourceKind::Kubernetes,
        "catalog service",
    );
    add(
        "evidence-topology-k8s-workload-catalog-api",
        EvidenceSourceKind::Kubernetes,
        "catalog API workload",
    );
    add(
        "evidence-topology-k8s-pod-catalog-api-0",
        EvidenceSourceKind::Kubernetes,
        "catalog API pod",
    );
    add(
        "evidence-topology-alert-checkout",
        EvidenceSourceKind::Alertmanager,
        "checkout alert is firing",
    );
    add(
        "evidence-topology-metric-checkout",
        EvidenceSourceKind::Prometheus,
        "checkout request metric",
    );
    add(
        "evidence-topology-edge-checkout-api",
        EvidenceSourceKind::Fixture,
        "checkout to API structural dependency",
    );
    add(
        "evidence-topology-edge-api-rds",
        EvidenceSourceKind::Fixture,
        "API to database structural dependency",
    );
    add(
        "evidence-topology-edge-rds-replica",
        EvidenceSourceKind::Fixture,
        "database to replica structural dependency",
    );
    add(
        "evidence-topology-edge-replica-rds",
        EvidenceSourceKind::Fixture,
        "replica to database cycle edge",
    );
    add(
        "evidence-topology-ownership-platform",
        EvidenceSourceKind::Fixture,
        "platform ownership mapping",
    );
    add(
        "evidence-topology-ownership-environment",
        EvidenceSourceKind::Fixture,
        "environment ownership mapping",
    );
    add(
        "evidence-topology-ownership-unassigned",
        EvidenceSourceKind::Fixture,
        "unassigned ownership mapping",
    );
    add(
        "evidence-topology-incident-checkout",
        EvidenceSourceKind::Fixture,
        "queue item affected checkout root",
    );
    add(
        "evidence-topology-summary",
        EvidenceSourceKind::Fixture,
        "topology summary counts",
    );
    entries.sort_by(|left, right| left.id.cmp(&right.id));
    entries
}

#[allow(dead_code)]
fn evidence_ids_set(evidence: &[EvidenceRef]) -> BTreeSet<String> {
    evidence.iter().map(|item| item.id.clone()).collect()
}
