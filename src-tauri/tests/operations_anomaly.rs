use chrono::{TimeZone, Utc};
use serde_json::json;
use std::collections::BTreeMap;
use thalassa_domain::{
    AnomalyCondition, AnomalyEvaluationStatus, AnomalyRule, ConsoleSeverity, MetricFixture,
    MetricFixtureSample, MetricFixtureSource, RateDirection, ResourceScope, ThresholdOperator,
};
use thalassaops::operations::anomaly::{
    evaluate_rule, evaluate_rules, parse_prometheus_fixture, AnomalyError,
};
use thalassaops::operations::fixtures::{fixture_catalog, fixture_time};

fn scope() -> ResourceScope {
    ResourceScope::workspace(
        uuid::Uuid::from_u128(1),
        uuid::Uuid::from_u128(2),
        uuid::Uuid::from_u128(3),
    )
}

fn metric(samples: Vec<MetricFixtureSample>) -> MetricFixture {
    MetricFixture {
        key: "metric-test".into(),
        scope: scope(),
        labels: BTreeMap::from([(String::from("service"), String::from("checkout"))]),
        samples,
        source: MetricFixtureSource {
            connector_id: "prometheus-test".into(),
            query: "checkout_metric".into(),
            endpoint: "/api/v1/query_range".into(),
        },
    }
}

fn sample(timestamp_seconds: i64, value: &str) -> MetricFixtureSample {
    MetricFixtureSample {
        timestamp_seconds,
        value: value.into(),
    }
}

fn rule(condition: AnomalyCondition) -> AnomalyRule {
    AnomalyRule {
        id: "rule-test".into(),
        name: "Test rule".into(),
        enabled: true,
        scope: scope(),
        metric_key: "metric-test".into(),
        condition,
        severity: ConsoleSeverity::S2,
        cooldown_seconds: 0,
    }
}

fn at(seconds: i64) -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(seconds, 0)
        .single()
        .unwrap_or(chrono::DateTime::<Utc>::UNIX_EPOCH)
}

#[test]
fn fixture_threshold_and_rate_rules_emit_distinct_deterministic_signals() {
    let catalog = fixture_catalog();
    let first = evaluate_rules(&catalog.anomaly_rules, &catalog.metrics, fixture_time())
        .expect("fixed fixtures should evaluate");
    let second = evaluate_rules(&catalog.anomaly_rules, &catalog.metrics, fixture_time())
        .expect("fixed fixtures should evaluate");

    assert_eq!(first, second);
    let signals: Vec<_> = first.into_iter().filter_map(|item| item.signal).collect();
    assert_eq!(signals.len(), 2);
    assert_eq!(signals[0].rule_id, "rule-cpu-threshold");
    assert_eq!(signals[0].observed_value, 92.0);
    assert_eq!(signals[0].comparison_value, 90.0);
    assert_eq!(signals[1].rule_id, "rule-error-rate-rise");
    assert_eq!(signals[1].observed_value, 0.080);
    assert!(matches!(
        signals[1].condition,
        AnomalyCondition::RateOfChange {
            window_seconds: 60,
            ..
        }
    ));
    assert!(!signals[0].evidence_id.is_empty());
    assert!(!signals[0].id.is_empty());
}

#[test]
fn threshold_operators_respect_boundaries_and_emit_only_on_breach() {
    let now = 100;
    for (operator, boundary_value, boundary_status, breach_value, breach_status) in [
        (
            ThresholdOperator::GreaterThan,
            "100",
            AnomalyEvaluationStatus::NotTriggered,
            "101",
            AnomalyEvaluationStatus::Triggered,
        ),
        (
            ThresholdOperator::GreaterThanOrEqual,
            "100",
            AnomalyEvaluationStatus::Triggered,
            "99",
            AnomalyEvaluationStatus::NotTriggered,
        ),
        (
            ThresholdOperator::LessThan,
            "100",
            AnomalyEvaluationStatus::NotTriggered,
            "99",
            AnomalyEvaluationStatus::Triggered,
        ),
        (
            ThresholdOperator::LessThanOrEqual,
            "100",
            AnomalyEvaluationStatus::Triggered,
            "101",
            AnomalyEvaluationStatus::NotTriggered,
        ),
    ] {
        let condition = AnomalyCondition::Threshold {
            operator,
            threshold: "100".into(),
        };
        let threshold_rule = rule(condition);
        let equality = evaluate_rule(
            &threshold_rule,
            &metric(vec![sample(now, boundary_value)]),
            at(now),
        )
        .expect("valid threshold should evaluate");
        assert_eq!(equality.status, boundary_status);

        let above_or_below = evaluate_rule(
            &threshold_rule,
            &metric(vec![sample(now, breach_value)]),
            at(now),
        )
        .expect("valid threshold should evaluate");
        assert_eq!(above_or_below.status, breach_status);
    }
}

#[test]
fn threshold_no_breach_has_no_signal() {
    let threshold_rule = rule(AnomalyCondition::Threshold {
        operator: ThresholdOperator::GreaterThan,
        threshold: "90".into(),
    });
    let result = evaluate_rule(&threshold_rule, &metric(vec![sample(100, "90")]), at(100))
        .expect("valid threshold should evaluate");
    assert_eq!(result.status, AnomalyEvaluationStatus::NotTriggered);
    assert!(result.signal.is_none());
}

#[test]
fn rate_directions_use_exact_seconds_and_window_filtering() {
    let rising = rule(AnomalyCondition::RateOfChange {
        direction: RateDirection::Increase,
        threshold_per_second: "0.5".into(),
        window_seconds: 10,
    });
    let result = evaluate_rule(
        &rising,
        &metric(vec![sample(90, "1"), sample(95, "4"), sample(100, "6")]),
        at(100),
    )
    .expect("valid rate rule should evaluate");
    assert_eq!(result.status, AnomalyEvaluationStatus::Triggered);
    assert_eq!(
        result.signal.as_ref().map(|signal| signal.comparison_value),
        Some(0.5)
    );

    let flat = rule(AnomalyCondition::RateOfChange {
        direction: RateDirection::Increase,
        threshold_per_second: "0.1".into(),
        window_seconds: 10,
    });
    let result = evaluate_rule(
        &flat,
        &metric(vec![sample(90, "1"), sample(100, "1")]),
        at(100),
    )
    .expect("valid rate rule should evaluate");
    assert_eq!(result.status, AnomalyEvaluationStatus::NotTriggered);
    assert!(result.signal.is_none());
}

#[test]
fn rate_decrease_and_absolute_direction_are_supported() {
    let decrease = rule(AnomalyCondition::RateOfChange {
        direction: RateDirection::Decrease,
        threshold_per_second: "-0.5".into(),
        window_seconds: 60,
    });
    let result = evaluate_rule(
        &decrease,
        &metric(vec![sample(40, "10"), sample(60, "0")]),
        at(60),
    )
    .expect("valid decrease rule should evaluate");
    assert_eq!(result.status, AnomalyEvaluationStatus::Triggered);

    let absolute = rule(AnomalyCondition::RateOfChange {
        direction: RateDirection::Absolute,
        threshold_per_second: "0.4".into(),
        window_seconds: 60,
    });
    let result = evaluate_rule(
        &absolute,
        &metric(vec![sample(40, "10"), sample(60, "0")]),
        at(60),
    )
    .expect("valid absolute rule should evaluate");
    assert_eq!(result.status, AnomalyEvaluationStatus::Triggered);
}

#[test]
fn missing_empty_and_insufficient_series_are_honest_non_signals() {
    let threshold_rule = rule(AnomalyCondition::Threshold {
        operator: ThresholdOperator::GreaterThan,
        threshold: "1".into(),
    });
    let empty = evaluate_rule(&threshold_rule, &metric(Vec::new()), at(100))
        .expect("empty series should be a non-signal");
    assert_eq!(empty.status, AnomalyEvaluationStatus::InsufficientData);
    assert!(empty.signal.is_none());

    let rate_rule = rule(AnomalyCondition::RateOfChange {
        direction: RateDirection::Increase,
        threshold_per_second: "0.1".into(),
        window_seconds: 60,
    });
    let one_sample = evaluate_rule(&rate_rule, &metric(vec![sample(100, "2")]), at(100))
        .expect("one sample should be a non-signal");
    assert_eq!(one_sample.status, AnomalyEvaluationStatus::InsufficientData);

    let missing = evaluate_rules(&[threshold_rule], &[], at(100));
    assert!(matches!(missing, Err(AnomalyError::MetricNotFound(_))));
}

#[test]
fn invalid_values_and_malformed_prometheus_fixtures_return_errors() {
    let threshold_rule = rule(AnomalyCondition::Threshold {
        operator: ThresholdOperator::GreaterThan,
        threshold: "1".into(),
    });
    let malformed_value = evaluate_rule(
        &threshold_rule,
        &metric(vec![sample(100, "not-a-number")]),
        at(100),
    );
    assert!(matches!(
        malformed_value,
        Err(AnomalyError::InvalidSample(_))
    ));
    let non_finite_value =
        evaluate_rule(&threshold_rule, &metric(vec![sample(100, "NaN")]), at(100));
    assert!(matches!(
        non_finite_value,
        Err(AnomalyError::InvalidSample(_))
    ));

    let malformed_fixture = parse_prometheus_fixture(
        "metric-test",
        scope(),
        r#"{"status":"success","data": "not a query result"}"#,
    );
    assert!(matches!(
        malformed_fixture,
        Err(AnomalyError::MalformedFixture(_))
    ));
}

#[test]
fn duplicate_sample_timestamps_are_rejected_instead_of_ordered_by_input() {
    let threshold_rule = rule(AnomalyCondition::Threshold {
        operator: ThresholdOperator::GreaterThan,
        threshold: "1".into(),
    });
    let result = evaluate_rule(
        &threshold_rule,
        &metric(vec![sample(100, "2"), sample(100, "0")]),
        at(100),
    );

    assert!(matches!(result, Err(AnomalyError::InvalidSample(_))));
}

#[test]
fn invalid_duplicate_missing_ambiguous_and_out_of_scope_rules_are_rejected() {
    let base_rule = rule(AnomalyCondition::Threshold {
        operator: ThresholdOperator::GreaterThan,
        threshold: "1".into(),
    });
    let duplicate = evaluate_rules(
        &[base_rule.clone(), base_rule.clone()],
        &[metric(vec![sample(100, "2")])],
        at(100),
    );
    assert!(matches!(duplicate, Err(AnomalyError::DuplicateRuleId(_))));

    let mut other_metric = metric(vec![sample(100, "2")]);
    other_metric.key = "other-metric".into();
    let missing = evaluate_rules(std::slice::from_ref(&base_rule), &[other_metric], at(100));
    assert!(matches!(missing, Err(AnomalyError::MetricNotFound(_))));

    let ambiguous = evaluate_rules(
        std::slice::from_ref(&base_rule),
        &[
            metric(vec![sample(100, "2")]),
            metric(vec![sample(100, "3")]),
        ],
        at(100),
    );
    assert!(matches!(ambiguous, Err(AnomalyError::AmbiguousMetric(_))));

    let mut narrower_rule = base_rule;
    narrower_rule.scope = ResourceScope::environment(
        uuid::Uuid::from_u128(99),
        uuid::Uuid::from_u128(1),
        uuid::Uuid::from_u128(2),
        uuid::Uuid::from_u128(3),
    );
    let mismatch = evaluate_rule(&narrower_rule, &metric(vec![sample(100, "2")]), at(100));
    assert!(matches!(mismatch, Err(AnomalyError::ScopeMismatch(_))));
}

#[test]
fn signal_serialization_is_stable_and_contains_no_credentials() {
    let rule = rule(AnomalyCondition::Threshold {
        operator: ThresholdOperator::GreaterThan,
        threshold: "1".into(),
    });
    let input = metric(vec![sample(100, "2")]);
    let first = evaluate_rule(&rule, &input, at(100))
        .expect("valid rule should evaluate")
        .signal
        .expect("threshold should trigger");
    let second = evaluate_rule(&rule, &input, at(100))
        .expect("valid rule should evaluate")
        .signal
        .expect("threshold should trigger");
    assert_eq!(
        serde_json::to_vec(&first).expect("signal serializes"),
        serde_json::to_vec(&second).expect("signal serializes")
    );
    let serialized = serde_json::to_string(&first).expect("signal serializes");
    for forbidden in [
        "Authorization",
        "Bearer",
        "password",
        "credential",
        "secret",
    ] {
        assert!(!serialized.contains(forbidden), "{forbidden} leaked");
    }
    assert_eq!(
        serde_json::to_value(&first).expect("signal serializes")["condition"],
        json!({"threshold":{"operator":"gt","threshold":"1"}})
    );
}

#[test]
fn direct_prometheus_fixture_conversion_preserves_source_provenance() {
    let payload = r#"
    {
      "series": [
        {
          "labels": {"__name__": "checkout_metric"},
          "samples": [
            {"timestamp": 90.0, "value": "1"},
            {"timestamp": 100.0, "value": "2"}
          ]
        }
      ],
      "source": {
        "connector_id": "prometheus-test",
        "query": "checkout_metric",
        "endpoint": "/api/v1/query_range"
      }
    }
    "#;
    let fixtures = parse_prometheus_fixture("metric-test", scope(), payload)
        .expect("valid Prometheus fixture should parse");
    assert_eq!(fixtures.len(), 1);
    assert_eq!(fixtures[0].source.query, "checkout_metric");
    assert_eq!(fixtures[0].samples[1].timestamp_seconds, 100);
}
