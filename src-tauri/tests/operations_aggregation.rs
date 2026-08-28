// SPDX-License-Identifier: Apache-2.0

use thalassa_domain::{
    ChangeStreamState, ConsoleHealthState, ConsoleSeverity, ImpactLevel, QueueItemSourceKind,
    ResourceScope, SourceState, StatusReason,
};
use thalassaops::operations::{fixture_catalog, fixture_time, OperationsAggregator};

#[test]
fn full_fixture_console_is_business_impact_first_and_evidence_backed() {
    let snapshot = OperationsAggregator::from_fixture_catalog(fixture_catalog())
        .snapshot_at(fixture_time())
        .expect("the complete fixture catalog should aggregate");

    assert_eq!(snapshot.health_summary.state, ConsoleHealthState::Critical);
    assert_eq!(
        snapshot.health_summary.headline.level,
        ImpactLevel::Critical
    );
    assert_eq!(snapshot.incident_queue[0].severity, ConsoleSeverity::S1);
    assert!(snapshot
        .incident_queue
        .iter()
        .any(|item| item.source_kind == QueueItemSourceKind::Alert));
    assert!(snapshot
        .incident_queue
        .iter()
        .any(|item| item.source_kind == QueueItemSourceKind::Anomaly));
    assert_eq!(snapshot.signal_summary.active_alerts.value, "1");
    assert_eq!(snapshot.signal_summary.active_anomalies.value, "2");
    assert_eq!(snapshot.signal_summary.checks_timed_out.value, "1");
    assert_eq!(
        snapshot.change_stream_status.state,
        ChangeStreamState::Available
    );
    assert!(snapshot.validate().is_ok());
}

#[test]
fn healthy_console_has_no_attention_and_numbers_still_have_evidence() {
    let mut catalog = fixture_catalog();
    catalog.alerts.clear();
    catalog.anomaly_rules.clear();
    catalog.health_checks.clear();
    catalog.environments[0].health = ConsoleHealthState::Healthy;

    let snapshot = OperationsAggregator::from_fixture_catalog(catalog)
        .snapshot_at(fixture_time())
        .expect("empty signal sources should produce a healthy projection");

    assert_eq!(snapshot.health_summary.state, ConsoleHealthState::Healthy);
    assert!(snapshot.incident_queue.is_empty());
    assert_eq!(snapshot.signal_summary.active_alerts.value, "0");
    assert_eq!(snapshot.signal_summary.active_anomalies.value, "0");
    assert_eq!(snapshot.signal_summary.checks_due.value, "0");
    assert!(snapshot
        .critical_numbers()
        .iter()
        .all(|number| !number.evidence_ids.is_empty()));
    assert!(snapshot.validate().is_ok());
}

#[test]
fn failing_check_and_anomalies_remain_independent_queue_items() {
    let snapshot = OperationsAggregator::from_fixture_catalog(fixture_catalog())
        .snapshot_at(fixture_time())
        .expect("producer failures should not abort the projection");
    let source_kinds: Vec<_> = snapshot
        .incident_queue
        .iter()
        .map(|item| item.source_kind)
        .collect();

    assert_eq!(
        source_kinds
            .iter()
            .filter(|kind| **kind == QueueItemSourceKind::Anomaly)
            .count(),
        2
    );
    assert_eq!(
        source_kinds
            .iter()
            .filter(|kind| **kind == QueueItemSourceKind::ScheduledHealthCheck)
            .count(),
        1
    );
}

#[test]
fn unavailable_environment_only_degrades_its_tile_and_source_status() {
    let mut catalog = fixture_catalog();
    catalog.environments[0].health = ConsoleHealthState::Healthy;
    catalog.environments[1].health = ConsoleHealthState::Unknown;
    catalog.environments[1].status_detail = "environment unavailable".into();
    catalog.source_status.push(thalassa_domain::SourceStatus {
        source_key: "cloud:gcp-staging".into(),
        state: SourceState::Unavailable,
        reason: Some(StatusReason::Unreachable),
        detail: Some("connection failed".into()),
        observed_at: None,
        evidence_ids: vec!["evidence-env-gcp-staging".into()],
    });

    let snapshot = OperationsAggregator::from_fixture_catalog(catalog)
        .snapshot_at(fixture_time())
        .expect("one unavailable environment must not blank the snapshot");

    assert_eq!(snapshot.environments.len(), 2);
    assert_eq!(snapshot.environments[0].health, ConsoleHealthState::Healthy);
    assert_eq!(snapshot.environments[1].health, ConsoleHealthState::Unknown);
    assert_eq!(snapshot.health_summary.state, ConsoleHealthState::Critical);
    assert!(snapshot
        .source_status
        .iter()
        .any(|status| status.source_key == "cloud:gcp-staging"
            && status.reason == Some(StatusReason::Unreachable)));
    assert!(snapshot.validate().is_ok());
}

#[test]
fn empty_change_stream_is_explicitly_not_configured() {
    let mut catalog = fixture_catalog();
    catalog.changes.clear();
    catalog
        .source_status
        .retain(|status| !status.source_key.starts_with("changes"));

    let snapshot = OperationsAggregator::from_fixture_catalog(catalog)
        .snapshot_at(fixture_time())
        .expect("a missing change source should produce an honest empty stream");

    assert!(snapshot.changes.is_empty());
    assert_eq!(
        snapshot.change_stream_status.state,
        ChangeStreamState::Empty
    );
    assert_eq!(
        snapshot.change_stream_status.reason,
        Some(StatusReason::NotConfigured)
    );
}

#[test]
fn unavailable_change_source_is_not_reported_as_an_empty_success() {
    let mut catalog = fixture_catalog();
    catalog.changes.clear();
    let changes_status = catalog
        .source_status
        .iter_mut()
        .find(|status| status.source_key == "changes")
        .expect("the fixture should declare the change source");
    changes_status.state = SourceState::Unavailable;
    changes_status.reason = Some(StatusReason::Unreachable);

    let snapshot = OperationsAggregator::from_fixture_catalog(catalog)
        .snapshot_at(fixture_time())
        .expect("an unavailable change source should still produce a snapshot");

    assert_eq!(
        snapshot.change_stream_status.state,
        ChangeStreamState::Unavailable
    );
    assert_eq!(
        snapshot.change_stream_status.reason,
        Some(StatusReason::Unreachable)
    );
    assert!(snapshot.validate().is_ok());
}

#[test]
fn unverified_evidence_degrades_sources_without_invalidating_zero_counts() {
    let mut catalog = fixture_catalog();
    for evidence in &mut catalog.evidence {
        evidence.redaction.classification_verified = false;
    }

    let snapshot = OperationsAggregator::from_fixture_catalog(catalog)
        .snapshot_at(fixture_time())
        .expect("unverified source evidence should not blank the console");

    assert_eq!(snapshot.signal_summary.active_alerts.value, "0");
    assert!(snapshot
        .source_status
        .iter()
        .any(|status| status.state == SourceState::Unverified));
    assert!(snapshot.validate().is_ok());
}

#[test]
fn queue_order_is_stable_when_source_records_are_reversed() {
    let catalog = fixture_catalog();
    let expected = OperationsAggregator::from_fixture_catalog(catalog.clone())
        .snapshot_at(fixture_time())
        .unwrap()
        .incident_queue
        .into_iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();

    let mut reversed = catalog;
    reversed.alerts.reverse();
    reversed.anomaly_rules.reverse();
    reversed.health_checks.reverse();
    let actual = OperationsAggregator::from_fixture_catalog(reversed)
        .snapshot_at(fixture_time())
        .unwrap()
        .incident_queue
        .into_iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();

    assert_eq!(actual, expected);
}

#[test]
fn malformed_records_are_skipped_without_panicking_or_erasing_valid_sources() {
    let mut catalog = fixture_catalog();
    let mut malformed_alert = catalog.alerts[0].clone();
    malformed_alert.fingerprint = "malformed-alert".into();
    malformed_alert.starts_at = "not-a-timestamp".into();
    catalog.alerts.push(malformed_alert);

    let mut malformed_schedule = catalog.health_checks[0].clone();
    malformed_schedule.id = "malformed-check".into();
    malformed_schedule.interval_seconds = 0;
    catalog.health_checks.push(malformed_schedule);

    let snapshot = OperationsAggregator::from_fixture_catalog(catalog)
        .snapshot_at(fixture_time())
        .expect("malformed source records should degrade independently");

    assert!(snapshot
        .incident_queue
        .iter()
        .any(|item| item.source_kind == QueueItemSourceKind::Alert
            && item.source_id == "alert-checkout-s1"));
    assert!(snapshot.source_status.iter().any(|status| {
        status.source_key == "alertmanager" && status.state == SourceState::Unverified
    }));
    assert!(snapshot.source_status.iter().any(|status| {
        status.source_key == "health_checks" && status.state == SourceState::Unverified
    }));
    assert!(snapshot.validate().is_ok());
}

#[test]
fn out_of_scope_changes_do_not_admit_evidence_before_the_scope_filter() {
    let mut catalog = fixture_catalog();
    let mut foreign_change = catalog.changes[0].clone();
    foreign_change.id = "foreign-change".into();
    foreign_change.scope = ResourceScope::environment(
        uuid::Uuid::from_u128(99),
        uuid::Uuid::from_u128(98),
        uuid::Uuid::from_u128(97),
        uuid::Uuid::from_u128(96),
    );
    foreign_change.evidence_ids.clear();
    catalog.changes.push(foreign_change);

    let snapshot = OperationsAggregator::from_fixture_catalog(catalog)
        .snapshot_at(fixture_time())
        .expect("foreign records should be skipped without invalidating the snapshot");

    assert!(snapshot
        .changes
        .iter()
        .all(|change| change.id != "foreign-change"));
    assert!(snapshot
        .evidence
        .iter()
        .all(|evidence| evidence.id != "evidence-change-foreign-change"));
}

#[test]
fn unavailable_source_without_verified_evidence_reports_unknown_not_critical() {
    let mut catalog = fixture_catalog();
    catalog.alerts.clear();
    catalog.anomaly_rules.clear();
    catalog.health_checks.clear();
    for environment in &mut catalog.environments {
        environment.health = ConsoleHealthState::Healthy;
    }
    catalog.source_status.push(thalassa_domain::SourceStatus {
        source_key: "missing-source".into(),
        state: SourceState::Unavailable,
        reason: Some(StatusReason::Unreachable),
        detail: Some("source did not return a verifiable result".into()),
        observed_at: None,
        evidence_ids: Vec::new(),
    });

    let snapshot = OperationsAggregator::from_fixture_catalog(catalog)
        .snapshot_at(fixture_time())
        .expect("missing source evidence should leave an honest snapshot");

    assert_eq!(snapshot.health_summary.state, ConsoleHealthState::Unknown);
}

#[test]
fn fresh_source_without_records_is_not_reported_as_healthy() {
    let mut catalog = fixture_catalog();
    catalog.alerts.clear();
    catalog.metrics.clear();
    catalog.anomaly_rules.clear();
    catalog.health_checks.clear();
    for environment in &mut catalog.environments {
        environment.health = ConsoleHealthState::Healthy;
    }

    let snapshot = OperationsAggregator::from_fixture_catalog(catalog)
        .snapshot_at(fixture_time())
        .expect("empty source data should leave an honest snapshot");

    assert_eq!(snapshot.health_summary.state, ConsoleHealthState::Unknown);
    assert!(snapshot.source_status.iter().any(|status| {
        status.source_key == "prometheus" && status.state == SourceState::Unverified
    }));
}

#[test]
fn malformed_environment_values_are_unverified_instead_of_fabricated() {
    let mut catalog = fixture_catalog();
    catalog.environments[0].resource_count.value = "not-a-number".into();
    catalog.environments[1].last_observed_at = "not-a-timestamp".into();

    let snapshot = OperationsAggregator::from_fixture_catalog(catalog)
        .snapshot_at(fixture_time())
        .expect("malformed environment records should degrade independently");

    assert!(snapshot.environments.is_empty());
    assert!(snapshot.source_status.iter().any(|status| {
        status.source_key == "environment_status"
            && status.state == SourceState::Unverified
            && status.reason == Some(StatusReason::Unknown)
    }));
}

#[test]
fn duplicate_projection_records_are_skipped_as_ambiguous() {
    let mut catalog = fixture_catalog();

    let mut duplicate_alert = catalog.alerts[0].clone();
    duplicate_alert.annotations.insert(
        "summary".into(),
        "Conflicting checkout alert summary".into(),
    );
    catalog.alerts.push(duplicate_alert);

    let mut duplicate_rule = catalog.anomaly_rules[0].clone();
    duplicate_rule.name = "Conflicting CPU rule".into();
    catalog.anomaly_rules.push(duplicate_rule);

    let mut duplicate_schedule = catalog.health_checks[0].clone();
    duplicate_schedule.name = "Conflicting API check".into();
    catalog.health_checks.push(duplicate_schedule);

    let mut duplicate_change = catalog.changes[0].clone();
    duplicate_change.summary = "Conflicting deployment summary".into();
    catalog.changes.push(duplicate_change);

    let mut duplicate_environment = catalog.environments[0].clone();
    duplicate_environment.name = "Conflicting AWS environment".into();
    catalog.environments.push(duplicate_environment);

    let snapshot = OperationsAggregator::from_fixture_catalog(catalog)
        .snapshot_at(fixture_time())
        .expect("ambiguous source records should not abort the projection");

    assert!(snapshot.incident_queue.iter().all(|item| !matches!(
        item.title.as_str(),
        "Checkout unavailable"
            | "Conflicting checkout alert summary"
            | "Production CPU utilization"
            | "Conflicting CPU rule"
            | "API health"
            | "Conflicting API check"
    )));
    assert!(snapshot
        .changes
        .iter()
        .all(|change| change.id != "change-payment-deploy"));
    assert!(snapshot
        .environments
        .iter()
        .all(|environment| environment.environment_id != "env-aws-prod"));
    for source_key in [
        "alertmanager",
        "prometheus",
        "health_checks",
        "changes",
        "environment_status",
    ] {
        assert!(snapshot.source_status.iter().any(|status| {
            status.source_key == source_key && status.state == SourceState::Unverified
        }));
    }
}
