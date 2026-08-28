//! Recorded observability fixtures used by deterministic producer tests.

use super::anomaly::parse_prometheus_fixture;
use super::model::{
    AnomalyCondition, AnomalyRule, ConsoleSeverity, MetricFixture, MetricFixtureSample,
    MetricFixtureSource, RateDirection, ResourceScope, ThresholdOperator,
};
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;
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
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct FixtureCatalog {
    pub metrics: Vec<MetricFixture>,
    pub anomaly_rules: Vec<AnomalyRule>,
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
            scope,
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

    FixtureCatalog {
        metrics,
        anomaly_rules,
    }
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
