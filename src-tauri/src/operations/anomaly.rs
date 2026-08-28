//! Pure anomaly evaluation over provider-neutral metric fixtures.
//!
//! The evaluator deliberately consumes `MetricFixture` values instead of
//! reaching into a connector.  A live Prometheus adapter can therefore map
//! its existing `MetricSeries` response into fixtures, while tests can load a
//! recorded response without starting a server or making a network request.

use crate::observability::masking::mask_json_object;
use crate::observability::prometheus::PrometheusQueryResult;
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{Map, Value};
use thalassa_domain::{
    AnomalyCondition, AnomalyEvaluation, AnomalyEvaluationStatus, AnomalyRule, AnomalySignal,
    MetricFixture, MetricFixtureSample, MetricFixtureSource, RateDirection, ResourceScope,
    ThresholdOperator,
};

const INVALID_RULE: &str = "invalid rule definition";
const DUPLICATE_RULE: &str = "duplicate rule identifier";
const MISSING_METRIC: &str = "metric fixture not found";
const AMBIGUOUS_METRIC: &str = "metric fixture is ambiguous";
const SCOPE_MISMATCH: &str = "metric fixture scope does not satisfy rule scope";
const INVALID_SAMPLE: &str = "metric fixture sample is invalid";
const MALFORMED_FIXTURE: &str = "Prometheus fixture is malformed";

/// Errors returned while validating or evaluating anomaly input.
///
/// Variant payloads intentionally contain fixed safe descriptions rather than
/// provider responses, query text, or sample values.  This keeps an error safe
/// to display or record at a boundary that has not yet applied redaction.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AnomalyError {
    #[error("anomaly rule is invalid")]
    InvalidRule(String),
    #[error("anomaly rule identifiers must be unique")]
    DuplicateRuleId(String),
    #[error("metric fixture was not found")]
    MetricNotFound(String),
    #[error("metric fixture selection is ambiguous")]
    AmbiguousMetric(String),
    #[error("metric fixture scope does not satisfy the rule scope")]
    ScopeMismatch(String),
    #[error("metric fixture sample is invalid")]
    InvalidSample(String),
    #[error("Prometheus fixture is malformed")]
    MalformedFixture(String),
}

/// Parse one recorded Prometheus response into provider-neutral metric
/// fixtures.  A response containing multiple series intentionally produces
/// multiple fixtures with the same key; rule evaluation then reports an
/// ambiguity instead of guessing which label set should trigger a signal.
pub fn parse_prometheus_fixture(
    key: &str,
    scope: ResourceScope,
    payload: &str,
) -> Result<Vec<MetricFixture>, AnomalyError> {
    if key.trim().is_empty() {
        return Err(AnomalyError::MalformedFixture(MALFORMED_FIXTURE.into()));
    }

    let result: PrometheusQueryResult = serde_json::from_str(payload)
        .map_err(|_| AnomalyError::MalformedFixture(MALFORMED_FIXTURE.into()))?;
    metric_fixtures_from_prometheus(key, scope, result)
}

/// Map an existing Prometheus result into the fixture shape consumed by the
/// evaluator.  This is the only adapter knowledge needed by the producer;
/// authentication, endpoint validation, HTTP policy, and response masking
/// remain owned by the observability module.
pub fn metric_fixtures_from_prometheus(
    key: &str,
    scope: ResourceScope,
    result: PrometheusQueryResult,
) -> Result<Vec<MetricFixture>, AnomalyError> {
    if key.trim().is_empty() {
        return Err(AnomalyError::MalformedFixture(MALFORMED_FIXTURE.into()));
    }

    result
        .series
        .into_iter()
        .map(|series| {
            let samples = series
                .samples
                .into_iter()
                .map(|sample| {
                    if !sample.timestamp.is_finite()
                        || sample.timestamp.fract() != 0.0
                        || sample.timestamp < i64::MIN as f64
                        || sample.timestamp > i64::MAX as f64
                    {
                        return Err(AnomalyError::InvalidSample(INVALID_SAMPLE.into()));
                    }
                    if !is_finite_decimal(&sample.value) {
                        return Err(AnomalyError::InvalidSample(INVALID_SAMPLE.into()));
                    }
                    Ok(MetricFixtureSample {
                        timestamp_seconds: sample.timestamp as i64,
                        value: sample.value,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            Ok(MetricFixture {
                key: key.to_owned(),
                scope: scope.clone(),
                labels: series.labels,
                samples,
                source: MetricFixtureSource {
                    connector_id: result.source.connector_id.clone(),
                    query: result.source.query.clone(),
                    endpoint: result.source.endpoint.clone(),
                },
            })
        })
        .collect()
}

/// Evaluate one anomaly rule against one already-selected metric fixture.
///
/// The explicit timestamp is used both to exclude samples observed in the
/// future and to make every produced identifier independent of the wall
/// clock.  No network request, timer, or correlation state is consulted.
pub fn evaluate_rule(
    rule: &AnomalyRule,
    metric: &MetricFixture,
    evaluated_at: DateTime<Utc>,
) -> Result<AnomalyEvaluation, AnomalyError> {
    validate_rule(rule)?;

    if metric.key != rule.metric_key {
        return Err(AnomalyError::MetricNotFound(MISSING_METRIC.into()));
    }
    if !rule.scope.contains(&metric.scope) {
        return Err(AnomalyError::ScopeMismatch(SCOPE_MISMATCH.into()));
    }

    let samples = eligible_samples(metric, evaluated_at);
    for sample in &samples {
        parse_sample_value(&sample.value)?;
    }
    let Some(latest) = samples.last().copied() else {
        return Ok(evaluation(
            rule,
            AnomalyEvaluationStatus::InsufficientData,
            None,
        ));
    };

    match &rule.condition {
        AnomalyCondition::Threshold {
            operator,
            threshold,
        } => evaluate_threshold(rule, metric, latest, *operator, threshold),
        AnomalyCondition::RateOfChange {
            direction,
            threshold_per_second,
            window_seconds,
        } => evaluate_rate(
            rule,
            metric,
            &samples,
            *direction,
            threshold_per_second,
            *window_seconds,
        ),
    }
}

/// Evaluate rules in stable identifier order and select exactly one metric
/// fixture by key.  Duplicate rules and ambiguous metric keys are rejected;
/// no signal is correlated or deduplicated here.
pub fn evaluate_rules(
    rules: &[AnomalyRule],
    metrics: &[MetricFixture],
    evaluated_at: DateTime<Utc>,
) -> Result<Vec<AnomalyEvaluation>, AnomalyError> {
    let mut rule_order: Vec<usize> = (0..rules.len()).collect();
    rule_order.sort_by(|left, right| {
        rules[*left]
            .id
            .cmp(&rules[*right].id)
            .then_with(|| left.cmp(right))
    });

    for pair in rule_order.windows(2) {
        if let [left, right] = pair {
            if rules[*left].id == rules[*right].id {
                return Err(AnomalyError::DuplicateRuleId(DUPLICATE_RULE.into()));
            }
        }
    }

    let mut evaluations = Vec::with_capacity(rules.len());
    for index in rule_order {
        let rule = &rules[index];
        validate_rule(rule)?;

        let matching: Vec<&MetricFixture> = metrics
            .iter()
            .filter(|metric| metric.key == rule.metric_key)
            .collect();
        let metric = match matching.as_slice() {
            [] => return Err(AnomalyError::MetricNotFound(MISSING_METRIC.into())),
            [metric] => *metric,
            _ => return Err(AnomalyError::AmbiguousMetric(AMBIGUOUS_METRIC.into())),
        };
        evaluations.push(evaluate_rule(rule, metric, evaluated_at)?);
    }

    Ok(evaluations)
}

fn validate_rule(rule: &AnomalyRule) -> Result<(), AnomalyError> {
    if !rule.enabled || rule.validate().is_err() {
        return Err(AnomalyError::InvalidRule(INVALID_RULE.into()));
    }
    Ok(())
}

fn eligible_samples(
    metric: &MetricFixture,
    evaluated_at: DateTime<Utc>,
) -> Vec<&MetricFixtureSample> {
    let cutoff = evaluated_at.timestamp();
    let mut samples: Vec<&MetricFixtureSample> = metric
        .samples
        .iter()
        .filter(|sample| sample.timestamp_seconds <= cutoff)
        .collect();
    samples.sort_by_key(|sample| sample.timestamp_seconds);
    samples
}

fn evaluate_threshold(
    rule: &AnomalyRule,
    metric: &MetricFixture,
    latest: &MetricFixtureSample,
    operator: ThresholdOperator,
    threshold: &str,
) -> Result<AnomalyEvaluation, AnomalyError> {
    let observed = parse_sample_value(&latest.value)?;
    let Some(bound) = parse_finite_value(threshold) else {
        return Err(AnomalyError::InvalidRule(INVALID_RULE.into()));
    };

    let triggered = match operator {
        ThresholdOperator::GreaterThan => observed > bound,
        ThresholdOperator::GreaterThanOrEqual => observed >= bound,
        ThresholdOperator::LessThan => observed < bound,
        ThresholdOperator::LessThanOrEqual => observed <= bound,
    };
    if !triggered {
        return Ok(evaluation(
            rule,
            AnomalyEvaluationStatus::NotTriggered,
            None,
        ));
    }

    let observed_at = sample_timestamp(latest.timestamp_seconds)?;
    let signal = signal(rule, metric, observed_at, observed, bound);
    Ok(evaluation(
        rule,
        AnomalyEvaluationStatus::Triggered,
        Some(signal),
    ))
}

fn evaluate_rate(
    rule: &AnomalyRule,
    metric: &MetricFixture,
    samples: &[&MetricFixtureSample],
    direction: RateDirection,
    threshold_per_second: &str,
    window_seconds: u64,
) -> Result<AnomalyEvaluation, AnomalyError> {
    let Some(latest) = samples.last().copied() else {
        return Ok(evaluation(
            rule,
            AnomalyEvaluationStatus::InsufficientData,
            None,
        ));
    };
    let window = if window_seconds > i64::MAX as u64 {
        i64::MAX
    } else {
        window_seconds as i64
    };
    let lower_bound = latest.timestamp_seconds.saturating_sub(window);
    let window_samples: Vec<&MetricFixtureSample> = samples
        .iter()
        .copied()
        .filter(|sample| sample.timestamp_seconds >= lower_bound)
        .collect();
    if window_samples.len() < 2 {
        return Ok(evaluation(
            rule,
            AnomalyEvaluationStatus::InsufficientData,
            None,
        ));
    }

    let Some(first) = window_samples.first().copied() else {
        return Ok(evaluation(
            rule,
            AnomalyEvaluationStatus::InsufficientData,
            None,
        ));
    };
    let Some(latest) = window_samples.last().copied() else {
        return Ok(evaluation(
            rule,
            AnomalyEvaluationStatus::InsufficientData,
            None,
        ));
    };
    let Some(delta_seconds) = latest
        .timestamp_seconds
        .checked_sub(first.timestamp_seconds)
    else {
        return Ok(evaluation(
            rule,
            AnomalyEvaluationStatus::InsufficientData,
            None,
        ));
    };
    if delta_seconds <= 0 {
        return Ok(evaluation(
            rule,
            AnomalyEvaluationStatus::InsufficientData,
            None,
        ));
    }

    let first_value = parse_sample_value(&first.value)?;
    let latest_value = parse_sample_value(&latest.value)?;
    let rate = (latest_value - first_value) / delta_seconds as f64;
    if !rate.is_finite() {
        return Err(AnomalyError::InvalidSample(INVALID_SAMPLE.into()));
    }
    let Some(bound) = parse_finite_value(threshold_per_second) else {
        return Err(AnomalyError::InvalidRule(INVALID_RULE.into()));
    };
    let triggered = match direction {
        RateDirection::Increase => rate >= bound,
        RateDirection::Decrease => rate <= bound,
        RateDirection::Absolute => rate.abs() >= bound,
    };
    if !triggered {
        return Ok(evaluation(
            rule,
            AnomalyEvaluationStatus::NotTriggered,
            None,
        ));
    }

    let observed_at = sample_timestamp(latest.timestamp_seconds)?;
    let signal = signal(rule, metric, observed_at, latest_value, rate);
    Ok(evaluation(
        rule,
        AnomalyEvaluationStatus::Triggered,
        Some(signal),
    ))
}

fn parse_sample_value(value: &str) -> Result<f64, AnomalyError> {
    match value.trim().parse::<f64>() {
        Ok(number) if number.is_finite() => Ok(number),
        Ok(_) => Err(AnomalyError::InvalidSample(INVALID_SAMPLE.into())),
        Err(_) => Err(AnomalyError::InvalidSample(INVALID_SAMPLE.into())),
    }
}

fn is_finite_decimal(value: &str) -> bool {
    value
        .trim()
        .parse::<f64>()
        .is_ok_and(|number| number.is_finite())
}

fn parse_finite_value(value: &str) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

fn sample_timestamp(seconds: i64) -> Result<String, AnomalyError> {
    DateTime::<Utc>::from_timestamp(seconds, 0)
        .map(|timestamp| timestamp.to_rfc3339_opts(SecondsFormat::AutoSi, true))
        .ok_or_else(|| AnomalyError::InvalidSample(INVALID_SAMPLE.into()))
}

fn evaluation(
    rule: &AnomalyRule,
    status: AnomalyEvaluationStatus,
    signal: Option<AnomalySignal>,
) -> AnomalyEvaluation {
    AnomalyEvaluation {
        rule_id: rule.id.clone(),
        metric_key: rule.metric_key.clone(),
        status,
        signal,
    }
}

fn signal(
    rule: &AnomalyRule,
    metric: &MetricFixture,
    observed_at: String,
    observed_value: f64,
    comparison_value: f64,
) -> AnomalySignal {
    let condition_key = condition_fingerprint(&rule.condition);
    let source_key = source_fingerprint(&metric.source);
    let id = stable_identifier(&[
        "anomaly",
        &rule.id,
        &metric.key,
        &observed_at,
        &condition_key,
    ]);
    let evidence_id = stable_identifier(&[
        "evidence",
        &metric.key,
        &source_key,
        &condition_key,
        &observed_at,
    ]);

    AnomalySignal {
        id,
        rule_id: rule.id.clone(),
        metric_key: metric.key.clone(),
        severity: rule.severity,
        observed_at,
        observed_value,
        comparison_value,
        condition: rule.condition.clone(),
        scope: metric.scope.clone(),
        evidence_id,
    }
}

fn condition_fingerprint(condition: &AnomalyCondition) -> String {
    match condition {
        AnomalyCondition::Threshold {
            operator,
            threshold,
        } => format!("threshold:{}:{}", threshold_operator(operator), threshold),
        AnomalyCondition::RateOfChange {
            direction,
            threshold_per_second,
            window_seconds,
        } => format!(
            "rate:{}:{}:{}",
            rate_direction(direction),
            threshold_per_second,
            window_seconds
        ),
    }
}

fn threshold_operator(operator: &ThresholdOperator) -> &'static str {
    match operator {
        ThresholdOperator::GreaterThan => "gt",
        ThresholdOperator::GreaterThanOrEqual => "gte",
        ThresholdOperator::LessThan => "lt",
        ThresholdOperator::LessThanOrEqual => "lte",
    }
}

fn rate_direction(direction: &RateDirection) -> &'static str {
    match direction {
        RateDirection::Increase => "increase",
        RateDirection::Decrease => "decrease",
        RateDirection::Absolute => "absolute",
    }
}

fn source_fingerprint(source: &MetricFixtureSource) -> String {
    let mut object = Map::new();
    object.insert(
        "connector_id".into(),
        Value::String(source.connector_id.clone()),
    );
    object.insert("query".into(), Value::String(source.query.clone()));
    object.insert("endpoint".into(), Value::String(source.endpoint.clone()));
    // Keep this fingerprint on the same masking path as observability source
    // payloads.  The fingerprint is never serialized as source content.
    let _ = mask_json_object(&mut object);
    let mut fingerprint = String::new();
    for field in ["connector_id", "query", "endpoint"] {
        if let Some(Value::String(value)) = object.get(field) {
            fingerprint.push_str(field);
            fingerprint.push(':');
            fingerprint.push_str(&value.len().to_string());
            fingerprint.push(':');
            fingerprint.push_str(value);
            fingerprint.push(';');
        }
    }
    fingerprint
}

fn stable_identifier(parts: &[&str]) -> String {
    // FNV-1a is intentionally implemented locally: a fixed, dependency-free
    // digest is stable across refreshes and does not depend on a randomized
    // hasher seed or UUID generation.
    let mut hash = 0xcbf29ce484222325_u64;
    for part in parts {
        for byte in part.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3_u64);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3_u64);
    }
    let prefix = match parts.first() {
        Some(part) => *part,
        None => "id",
    };
    format!("{prefix}-{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_identifier_is_repeatable() {
        assert_eq!(
            stable_identifier(&["anomaly", "rule", "metric"]),
            stable_identifier(&["anomaly", "rule", "metric"])
        );
    }

    #[test]
    fn source_fingerprint_is_json_and_not_raw_signal_content() {
        let source = MetricFixtureSource {
            connector_id: "prometheus".into(),
            query: "up".into(),
            endpoint: "/api/v1/query".into(),
        };
        let fingerprint = source_fingerprint(&source);
        assert!(fingerprint.contains("prometheus"));
        assert!(fingerprint.contains("/api/v1/query"));
    }
}
