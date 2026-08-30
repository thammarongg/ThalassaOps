//! Recorded observability fixtures used by deterministic producer tests.

use super::anomaly::parse_prometheus_fixture;
use super::model::{
    AnomalyCondition, AnomalyRule, ConsoleHealthState, ConsoleSeverity, CriticalNumber,
    DrillDownDestination, DrillDownReference, DrillDownTarget, EvidenceRedaction, EvidenceRef,
    EvidenceSourceKind, FixtureHealthCheck, HealthCheckOutcome, HealthCheckSchedule,
    HealthCheckSource, MetricFixture, MetricFixtureSample, MetricFixtureSource, NumberUnit,
    RateDirection, ResourceScope, SourceState, SourceStatus, StatusReason, ThresholdOperator,
};
use crate::change::{adapters as change_adapters, fixtures as change_fixtures};
use crate::correlation::SourceRecordStore;
use crate::observability::alertmanager::{
    AlertSourceReference, NormalizedAlert, ResourceReference,
};
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;
use thalassa_domain::{ChangeEvent, EnvironmentStatus};
use uuid::Uuid;

const CPU_FIXTURE: &str = include_str!(
    "../../../docs/superpowers/fixtures/2026-08-28-capture/prometheus/metric-cpu-prod.json"
);
const ERROR_RATE_FIXTURE: &str = include_str!(
    "../../../docs/superpowers/fixtures/2026-08-28-capture/prometheus/metric-error-rate-prod.json"
);

/// Minimal fixture catalog consumed by the anomaly producer and expanded by
/// later Operations Console producers.  The catalog is intentionally an
/// ordinary value so callers can replace it with recorded test data.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct FixtureCatalog {
    pub alerts: Vec<NormalizedAlert>,
    pub metrics: Vec<MetricFixture>,
    pub anomaly_rules: Vec<AnomalyRule>,
    pub health_checks: Vec<HealthCheckSchedule>,
    pub health_check_results: BTreeMap<String, FixtureHealthCheck>,
    pub changes: Vec<ChangeEvent>,
    pub environments: Vec<EnvironmentStatus>,
    pub source_status: Vec<SourceStatus>,
    pub evidence: Vec<EvidenceRef>,
}

/// Return the fixed evaluation timestamp used by Sprint 11 fixtures.
pub fn fixture_time() -> DateTime<Utc> {
    match DateTime::<Utc>::from_timestamp(1_787_907_600, 0) {
        Some(timestamp) => timestamp,
        None => DateTime::<Utc>::UNIX_EPOCH,
    }
}

/// Return a deterministic metric/rule catalog backed by recorded Prometheus
/// response files.  The defensive fallback keeps a committed fixture typo
/// from becoming a panic during application startup; malformed external
/// fixture input is still reported by `parse_prometheus_fixture`.
pub fn fixture_catalog() -> FixtureCatalog {
    let scope = fixture_scope();
    let metrics = vec![
        fixed_metric(
            "metric-cpu-prod",
            &scope,
            CPU_FIXTURE,
            "node_cpu_utilization",
            "node_cpu_utilization",
            &["70", "92"],
        ),
        fixed_metric(
            "metric-error-rate-prod",
            &scope,
            ERROR_RATE_FIXTURE,
            "checkout_error_rate",
            "checkout_error_rate",
            &["0.010", "0.080"],
        ),
    ];
    let anomaly_rules = vec![
        AnomalyRule {
            id: "rule-cpu-threshold".into(),
            name: "Production CPU utilization".into(),
            enabled: true,
            scope: scope.clone(),
            metric_key: "metric-cpu-prod".into(),
            condition: AnomalyCondition::Threshold {
                operator: ThresholdOperator::GreaterThan,
                threshold: "90".into(),
            },
            severity: ConsoleSeverity::S2,
            cooldown_seconds: 0,
        },
        AnomalyRule {
            id: "rule-error-rate-rise".into(),
            name: "Production checkout error-rate rise".into(),
            enabled: true,
            scope: scope.clone(),
            metric_key: "metric-error-rate-prod".into(),
            condition: AnomalyCondition::RateOfChange {
                direction: RateDirection::Increase,
                threshold_per_second: "0.0005".into(),
                window_seconds: 60,
            },
            severity: ConsoleSeverity::S2,
            cooldown_seconds: 0,
        },
    ];

    let health_checks = vec![
        HealthCheckSchedule {
            id: "check-api-health".into(),
            name: "API health".into(),
            enabled: true,
            scope: scope.clone(),
            source: HealthCheckSource::Fixture {
                fixture_key: "api-health".into(),
            },
            interval_seconds: 300,
            timeout_ms: 1_000,
            cooldown_seconds: 0,
            last_run_at: None,
            last_signal_at: None,
            defined_by: Some("fixture-catalog".into()),
            defined_at: Some("2026-08-28T08:00:00Z".into()),
            last_outcome: None,
        },
        HealthCheckSchedule {
            id: "check-db-health".into(),
            name: "Database health".into(),
            enabled: true,
            scope: scope.clone(),
            source: HealthCheckSource::Fixture {
                fixture_key: "db-health".into(),
            },
            interval_seconds: 60,
            timeout_ms: 250,
            cooldown_seconds: 600,
            last_run_at: Some("2026-08-28T08:58:00Z".into()),
            last_signal_at: Some("2026-08-28T08:59:30Z".into()),
            defined_by: Some("fixture-catalog".into()),
            defined_at: Some("2026-08-28T08:00:00Z".into()),
            last_outcome: Some(HealthCheckOutcome::Degraded),
        },
        HealthCheckSchedule {
            id: "check-worker-timeout".into(),
            name: "Worker health".into(),
            enabled: true,
            scope: scope.clone(),
            source: HealthCheckSource::Fixture {
                fixture_key: "worker-health".into(),
            },
            interval_seconds: 30,
            timeout_ms: 100,
            cooldown_seconds: 0,
            last_run_at: Some("2026-08-28T08:59:00Z".into()),
            last_signal_at: None,
            defined_by: Some("fixture-catalog".into()),
            defined_at: Some("2026-08-28T08:00:00Z".into()),
            last_outcome: None,
        },
    ];

    let health_check_results = BTreeMap::from([
        (
            String::from("api-health"),
            FixtureHealthCheck {
                outcome: HealthCheckOutcome::Healthy,
                duration_ms: 42,
                evidence_id: Some("evidence-check-api-health".into()),
            },
        ),
        (
            String::from("db-health"),
            FixtureHealthCheck {
                outcome: HealthCheckOutcome::Degraded,
                duration_ms: 80,
                evidence_id: Some("evidence-check-db-health".into()),
            },
        ),
        (
            String::from("worker-health"),
            FixtureHealthCheck {
                outcome: HealthCheckOutcome::Healthy,
                duration_ms: 250,
                evidence_id: Some("evidence-check-worker-timeout".into()),
            },
        ),
    ]);

    let observed_at = fixture_time().to_rfc3339();
    let alert_evidence = String::from("evidence-alert-checkout-s1");
    let api_evidence = String::from("evidence-check-api-health");
    let db_evidence = String::from("evidence-check-db-health");
    let worker_evidence = String::from("evidence-check-worker-timeout");
    let aws_evidence = String::from("evidence-env-aws-prod");
    let gcp_evidence = String::from("evidence-env-gcp-staging");
    let (changes, change_evidence) = fixture_change_events(&scope);
    let change_evidence_ids: Vec<String> = change_evidence
        .iter()
        .map(|evidence| evidence.id.clone())
        .collect();

    FixtureCatalog {
        alerts: vec![fixture_alert(&scope)],
        metrics,
        anomaly_rules,
        health_checks,
        health_check_results,
        changes,
        environments: vec![
            fixture_environment(
                "env-aws-prod",
                "AWS production",
                Some("aws"),
                ConsoleHealthState::Degraded,
                "one service is degraded",
                "3",
                aws_evidence.clone(),
                &scope,
            ),
            fixture_environment(
                "env-gcp-staging",
                "GCP staging",
                Some("gcp"),
                ConsoleHealthState::Healthy,
                "all connected resources are healthy",
                "2",
                gcp_evidence.clone(),
                &scope,
            ),
        ],
        source_status: vec![
            source_status(
                "alertmanager",
                SourceState::Fresh,
                None,
                None,
                Some(&observed_at),
                vec![alert_evidence.clone()],
            ),
            source_status(
                "prometheus",
                SourceState::Fresh,
                None,
                None,
                Some(&observed_at),
                Vec::new(),
            ),
            source_status(
                "health_checks",
                SourceState::Fresh,
                None,
                None,
                Some(&observed_at),
                vec![
                    api_evidence.clone(),
                    db_evidence.clone(),
                    worker_evidence.clone(),
                ],
            ),
            source_status(
                "cloud:aws-prod",
                SourceState::Fresh,
                None,
                None,
                Some(&observed_at),
                vec![aws_evidence.clone()],
            ),
            source_status(
                "cloud:gcp-staging",
                SourceState::Fresh,
                None,
                None,
                Some(&observed_at),
                vec![gcp_evidence.clone()],
            ),
            source_status(
                "changes",
                SourceState::Fresh,
                None,
                None,
                Some(&observed_at),
                change_evidence_ids,
            ),
        ],
        evidence: vec![
            fixture_evidence(
                alert_evidence,
                EvidenceSourceKind::Alertmanager,
                Some("alertmanager-prod"),
                "/api/v2/alerts",
                Some("active alerts"),
                "2026-08-28T08:55:00Z",
                "Checkout unavailable for production customers",
                &scope,
            ),
            fixture_evidence(
                api_evidence,
                EvidenceSourceKind::HealthCheck,
                Some("fixture-health"),
                "fixture://health-check",
                Some("api-health"),
                &observed_at,
                "API health check completed successfully",
                &scope,
            ),
            fixture_evidence(
                db_evidence,
                EvidenceSourceKind::HealthCheck,
                Some("fixture-health"),
                "fixture://health-check",
                Some("db-health"),
                &observed_at,
                "Database health check is in cooldown",
                &scope,
            ),
            fixture_evidence(
                worker_evidence,
                EvidenceSourceKind::HealthCheck,
                Some("fixture-health"),
                "fixture://health-check",
                Some("worker-health"),
                &observed_at,
                "Worker health probe exceeded its timeout",
                &scope,
            ),
            fixture_evidence(
                aws_evidence,
                EvidenceSourceKind::Cloud,
                Some("aws-prod"),
                "fixture://cloud/aws-prod",
                Some("environment status"),
                &observed_at,
                "AWS production has one degraded service",
                &scope,
            ),
            fixture_evidence(
                gcp_evidence,
                EvidenceSourceKind::Cloud,
                Some("gcp-staging"),
                "fixture://cloud/gcp-staging",
                Some("environment status"),
                &observed_at,
                "GCP staging resources are healthy",
                &scope,
            ),
        ]
        .into_iter()
        .chain(change_evidence)
        .collect(),
    }
}

/// Replay the committed change fixtures through a local, scoped source ledger.
///
/// The console summarizes the same canonical change events the change module
/// produces; it never invents one.  A replay failure yields an empty change
/// stream, which the aggregate reports honestly as an unavailable source,
/// rather than a panic during application startup.
fn fixture_change_events(scope: &ResourceScope) -> (Vec<ChangeEvent>, Vec<EvidenceRef>) {
    let mut store = SourceRecordStore::with_scope(scope.clone());
    let Ok(output) =
        change_adapters::replay_all(&mut store, scope, change_fixtures::fixture_clock())
    else {
        return (Vec::new(), Vec::new());
    };
    let evidence = store.evidence_refs().cloned().collect();
    (output.events, evidence)
}

fn fixed_metric(
    key: &str,
    scope: &ResourceScope,
    fixture_json: &str,
    fallback_label: &str,
    query: &str,
    fallback_values: &[&str],
) -> MetricFixture {
    if let Ok(mut metrics) = parse_prometheus_fixture(key, scope.clone(), fixture_json) {
        if let Some(metric) = metrics.pop() {
            return metric;
        }
    }

    let base_timestamp = fixture_time().timestamp() - 60;
    MetricFixture {
        key: key.into(),
        scope: scope.clone(),
        labels: BTreeMap::from([(String::from("__name__"), fallback_label.into())]),
        samples: fallback_values
            .iter()
            .enumerate()
            .map(|(index, value)| MetricFixtureSample {
                timestamp_seconds: base_timestamp + (index as i64 * 60),
                value: (*value).into(),
            })
            .collect(),
        source: MetricFixtureSource {
            connector_id: "prometheus-prod".into(),
            query: query.into(),
            endpoint: "/api/v1/query_range".into(),
        },
    }
}

fn fixture_scope() -> ResourceScope {
    ResourceScope::environment(
        Uuid::from_u128(0x00000000000000000000000000000011),
        Uuid::from_u128(0x00000000000000000000000000000012),
        Uuid::from_u128(0x00000000000000000000000000000013),
        Uuid::from_u128(0x00000000000000000000000000000014),
    )
}

fn fixture_alert(scope: &ResourceScope) -> NormalizedAlert {
    let labels = BTreeMap::from([
        (
            String::from("alertname"),
            String::from("CheckoutUnavailable"),
        ),
        (String::from("severity"), String::from("S1")),
        (String::from("impact"), String::from("critical")),
        (
            String::from("customer_scope"),
            String::from("production checkout customers"),
        ),
        (String::from("service_criticality"), String::from("tier-0")),
        (String::from("trajectory"), String::from("expanding")),
        (String::from("priority"), String::from("P1")),
        (String::from("service"), String::from("checkout")),
        (String::from("environment"), String::from("production")),
    ]);
    let annotations = BTreeMap::from([
        (
            String::from("summary"),
            String::from("Checkout unavailable"),
        ),
        (
            String::from("description"),
            String::from("Checkout requests are failing in production"),
        ),
    ]);
    let _ = scope;
    NormalizedAlert {
        fingerprint: "alert-checkout-s1".into(),
        state: "firing".into(),
        starts_at: "2026-08-28T08:55:00Z".into(),
        ends_at: "2026-08-28T09:00:00Z".into(),
        labels,
        annotations,
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

#[allow(clippy::too_many_arguments)]
fn fixture_environment(
    id: &str,
    name: &str,
    provider: Option<&str>,
    health: ConsoleHealthState,
    detail: &str,
    resource_count: &str,
    evidence_id: String,
    scope: &ResourceScope,
) -> EnvironmentStatus {
    let resource_number = fixture_number(
        format!("environment.{id}.resource_count"),
        resource_count,
        evidence_id.clone(),
        DrillDownDestination::EnvironmentStatus,
        Some(id),
        scope,
        "environment resources",
    );
    EnvironmentStatus {
        environment_id: id.into(),
        name: name.into(),
        provider: provider.map(str::to_owned),
        health,
        status_detail: detail.into(),
        resource_count: resource_number,
        last_observed_at: fixture_time().to_rfc3339(),
        evidence_ids: vec![evidence_id.clone()],
        drill_down: DrillDownTarget {
            destination: DrillDownDestination::EnvironmentStatus,
            evidence_ids: vec![evidence_id],
            filter_key: Some(id.into()),
        },
    }
}

fn source_status(
    source_key: &str,
    state: SourceState,
    reason: Option<StatusReason>,
    detail: Option<&str>,
    observed_at: Option<&str>,
    evidence_ids: Vec<String>,
) -> SourceStatus {
    SourceStatus {
        source_key: source_key.into(),
        state,
        reason,
        detail: detail.map(str::to_owned),
        observed_at: observed_at.map(str::to_owned),
        evidence_ids,
    }
}

#[allow(clippy::too_many_arguments)]
fn fixture_evidence(
    id: String,
    source_kind: EvidenceSourceKind,
    connector_id: Option<&str>,
    endpoint: &str,
    query: Option<&str>,
    observed_at: &str,
    excerpt: &str,
    scope: &ResourceScope,
) -> EvidenceRef {
    EvidenceRef {
        id,
        source_kind,
        connector_id: connector_id.map(str::to_owned),
        scope: scope.clone(),
        endpoint: endpoint.into(),
        query: query.map(str::to_owned),
        observed_at: observed_at.into(),
        excerpt: excerpt.into(),
        native_url: None,
        redaction: EvidenceRedaction {
            classification_verified: true,
            redaction_verified: true,
            masked: false,
            unparsed: false,
        },
    }
}

fn fixture_number(
    key: String,
    value: &str,
    evidence_id: String,
    destination: DrillDownDestination,
    filter_key: Option<&str>,
    scope: &ResourceScope,
    query: &str,
) -> CriticalNumber {
    let drill_down_reference = DrillDownReference {
        source_query: query.into(),
        scope: scope.clone(),
        time_window: None,
        evidence_ids: vec![evidence_id.clone()],
    };
    CriticalNumber {
        key,
        value: value.into(),
        unit: NumberUnit::Count,
        evidence_ids: vec![evidence_id.clone()],
        drill_down: DrillDownTarget {
            destination,
            evidence_ids: vec![evidence_id.clone()],
            filter_key: filter_key.map(str::to_owned),
        },
        drill_down_reference,
    }
}
