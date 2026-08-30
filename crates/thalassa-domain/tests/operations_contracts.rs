// SPDX-License-Identifier: Apache-2.0

use serde::{de::DeserializeOwned, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use thalassa_domain::*;

fn scope() -> ResourceScope {
    ResourceScope::workspace(uuid::Uuid::nil(), uuid::Uuid::nil(), uuid::Uuid::nil())
}

fn assert_round_trip<T>(value: T)
where
    T: DeserializeOwned + PartialEq + Serialize + std::fmt::Debug,
{
    let encoded = serde_json::to_value(&value).expect("contract must serialize");
    let decoded: T = serde_json::from_value(encoded).expect("contract must deserialize");
    assert_eq!(decoded, value);
}

fn impact() -> BusinessImpact {
    BusinessImpact {
        level: ImpactLevel::High,
        summary: "Checkout is degraded for one region".into(),
        customer_scope: "customers in eu-west".into(),
        service_criticality: "tier-0".into(),
        trajectory: ImpactTrajectory::Stable,
        dimensions: ImpactDimensions::single_dimension(ImpactLevel::High, ImpactTrajectory::Stable),
        evidence_ids: vec!["evidence-checkout-degraded".into()],
    }
}

fn drill_down() -> DrillDownTarget {
    DrillDownTarget {
        destination: DrillDownDestination::Evidence,
        evidence_ids: vec!["evidence-1".into()],
        filter_key: Some("checkout".into()),
    }
}

fn critical_number() -> CriticalNumber {
    CriticalNumber {
        key: "active_alerts".into(),
        value: "2".into(),
        unit: NumberUnit::Count,
        evidence_ids: vec!["evidence-1".into()],
        drill_down: drill_down(),
        drill_down_reference: DrillDownReference {
            source_query: "active_alerts".into(),
            scope: scope(),
            time_window: None,
            evidence_ids: vec!["evidence-1".into()],
        },
    }
}

#[test]
fn operations_contracts_round_trip_through_json() {
    let mut labels = BTreeMap::new();
    labels.insert("service".into(), "checkout".into());

    let evidence = EvidenceRef {
        id: "evidence-1".into(),
        source_kind: EvidenceSourceKind::Prometheus,
        connector_id: Some("prometheus-prod".into()),
        scope: scope(),
        endpoint: "/api/v1/query_range".into(),
        query: Some("rate(http_requests_total[5m])".into()),
        observed_at: "2026-08-28T09:00:00Z".into(),
        excerpt: "checkout error rate is elevated".into(),
        native_url: Some("https://grafana.example/d/checkout".into()),
        redaction: EvidenceRedaction {
            classification_verified: true,
            redaction_verified: true,
            masked: false,
            unparsed: false,
        },
    };
    let evidence_id = evidence.id.clone();
    let number = critical_number();
    let business_impact = impact();
    let queue = IncidentQueueItem {
        id: "queue-1".into(),
        title: "Checkout degraded".into(),
        source_kind: QueueItemSourceKind::Alert,
        source_id: "alert-1".into(),
        severity: ConsoleSeverity::S2,
        priority: None,
        status: QueueStatus::Investigating,
        business_impact: business_impact.clone(),
        scope: scope(),
        detected_at: "2026-08-28T08:55:00Z".into(),
        opened_at: "2026-08-28T08:55:00Z".into(),
        last_update: "2026-08-28T08:59:00Z".into(),
        affected_scope: scope(),
        evidence_ids: vec![evidence_id.clone()],
        drill_down: drill_down(),
        drill_down_reference: DrillDownReference {
            source_query: "rate(http_requests_total[5m])".into(),
            scope: scope(),
            time_window: None,
            evidence_ids: vec![evidence_id.clone()],
        },
    };
    assert_eq!(
        serde_json::to_value(&queue).unwrap()["priority"],
        json!(null)
    );
    let metric = MetricFixture {
        key: "checkout_error_rate".into(),
        scope: scope(),
        labels,
        samples: vec![MetricFixtureSample {
            timestamp_seconds: 1_756_356_000,
            value: "0.08".into(),
        }],
        source: MetricFixtureSource {
            connector_id: "prometheus-prod".into(),
            query: "checkout_error_rate".into(),
            endpoint: "/api/v1/query".into(),
        },
    };
    let condition = AnomalyCondition::Threshold {
        operator: ThresholdOperator::GreaterThanOrEqual,
        threshold: "0.05".into(),
    };
    let rule = AnomalyRule {
        id: "rule-checkout-errors".into(),
        name: "Checkout errors".into(),
        enabled: true,
        scope: scope(),
        metric_key: metric.key.clone(),
        condition: condition.clone(),
        severity: ConsoleSeverity::S2,
        cooldown_seconds: 300,
    };
    let signal = AnomalySignal {
        id: "signal-1".into(),
        rule_id: rule.id.clone(),
        metric_key: metric.key.clone(),
        severity: ConsoleSeverity::S2,
        observed_at: "2026-08-28T09:00:00Z".into(),
        observed_value: 0.08,
        comparison_value: 0.05,
        condition,
        scope: scope(),
        evidence_id: evidence_id.clone(),
    };
    let schedule = HealthCheckSchedule {
        id: "check-checkout".into(),
        name: "Checkout health".into(),
        enabled: true,
        scope: scope(),
        source: HealthCheckSource::Fixture {
            fixture_key: "checkout-health".into(),
        },
        interval_seconds: 300,
        timeout_ms: 1_000,
        cooldown_seconds: 30,
        last_run_at: Some("2026-08-28T08:55:00Z".into()),
        last_signal_at: None,
        defined_by: Some("operator-1".into()),
        defined_at: Some("2026-08-28T08:00:00Z".into()),
        last_outcome: Some(HealthCheckOutcome::Healthy),
    };
    let check = FixtureHealthCheck {
        outcome: HealthCheckOutcome::Healthy,
        duration_ms: 42,
        evidence_id: Some(evidence_id.clone()),
    };
    let audit = HealthCheckAudit {
        run_id: "run-1".into(),
        schedule_id: schedule.id.clone(),
        triggered_by: "scheduler".into(),
        started_at: "2026-08-28T09:00:00Z".into(),
        completed_at: "2026-08-28T09:00:00Z".into(),
        duration_ms: 42,
        scope: scope(),
        source: schedule.source.clone(),
        outcome: check.outcome,
        cooldown_suppressed: false,
        policy_version: 7,
    };
    let result = HealthCheckResult {
        schedule_id: schedule.id.clone(),
        outcome: check.outcome,
        observed_at: "2026-08-28T09:00:00Z".into(),
        evidence_id: check.evidence_id.clone(),
        audit,
    };
    let change = ChangeStreamItem {
        id: "change-1".into(),
        source: EvidenceSourceKind::ArgoCd,
        occurred_at: "2026-08-28T08:50:00Z".into(),
        kind: ChangeKind::Deployment,
        summary: "Deploy checkout 2026.08.28.1".into(),
        actor: Some("release-bot".into()),
        target_resource: Some("deployment/checkout".into()),
        native_link: Some("https://argocd.example/app/checkout".into()),
        scope: scope(),
        evidence_ids: vec![evidence_id.clone()],
        drill_down: drill_down(),
    };
    let environment = EnvironmentStatus {
        environment_id: "env-prod".into(),
        name: "Production".into(),
        provider: Some("aws".into()),
        health: ConsoleHealthState::Degraded,
        status_detail: "one service is degraded".into(),
        resource_count: number.clone(),
        last_observed_at: "2026-08-28T09:00:00Z".into(),
        evidence_ids: vec![evidence_id.clone()],
        drill_down: drill_down(),
    };
    let source_status = SourceStatus {
        source_key: "prometheus-prod".into(),
        state: SourceState::Fresh,
        reason: None,
        detail: None,
        observed_at: Some("2026-08-28T09:00:00Z".into()),
        evidence_ids: vec![evidence_id.clone()],
    };
    let widgets = vec![WidgetDefinition {
        id: WidgetId::HealthSummary,
        title_key: "operations.health_summary".into(),
        default_order: 0,
        default_size: WidgetSize::Wide,
        required: true,
    }];
    let snapshot = OperationsSnapshot {
        generated_at: "2026-08-28T09:00:00Z".into(),
        scope: scope(),
        source_status: vec![source_status],
        health_summary: HealthSummary {
            state: ConsoleHealthState::Degraded,
            headline: business_impact,
            attention: number.clone(),
            impacted_services: number.clone(),
            active_by_severity: vec![number.clone()],
            environments_by_state: vec![number.clone()],
            contributing_scopes: vec![ContributingScope {
                scope: scope(),
                impact: ImpactLevel::High,
                summary: "Checkout is degraded".into(),
                evidence_ids: vec![evidence_id.clone()],
            }],
        },
        incident_queue: vec![queue],
        signal_summary: SignalSummary {
            active_alerts: number.clone(),
            active_anomalies: number.clone(),
            checks_due: number.clone(),
            checks_timed_out: number,
            by_source: vec![SignalCount {
                source_kind: QueueItemSourceKind::Alert,
                count: critical_number(),
            }],
        },
        changes: vec![change],
        change_stream_status: ChangeStreamStatus {
            state: ChangeStreamState::Available,
            reason: None,
            detail: None,
        },
        environments: vec![environment],
        evidence: vec![evidence],
        widget_registry: widgets,
    };

    assert_round_trip(snapshot.clone());
    assert_round_trip(metric);
    assert_round_trip(rule);
    assert_round_trip(signal);
    assert_round_trip(schedule);
    assert_round_trip(result);
    assert_round_trip(OperationsEvidenceRequest {
        evidence_ids: vec![evidence_id],
    });

    let mut invalid_queue_reference = snapshot.clone();
    invalid_queue_reference.incident_queue[0]
        .drill_down
        .evidence_ids = vec!["evidence-does-not-exist".into()];
    assert!(invalid_queue_reference.validate().is_err());

    let mut mismatched_queue_reference = snapshot.clone();
    let mut second_evidence = mismatched_queue_reference.evidence[0].clone();
    second_evidence.id = "evidence-2".into();
    second_evidence.endpoint = "fixture://different-source".into();
    mismatched_queue_reference.evidence.push(second_evidence);
    mismatched_queue_reference.incident_queue[0]
        .drill_down
        .evidence_ids = vec!["evidence-2".into()];
    assert!(mismatched_queue_reference.validate().is_err());

    let mut duplicate_evidence = snapshot;
    let mut duplicate = duplicate_evidence.evidence[0].clone();
    duplicate.id = "evidence-duplicate-content".into();
    duplicate_evidence.evidence.push(duplicate);
    assert!(duplicate_evidence.validate().is_err());
}

#[test]
fn unparsed_evidence_cannot_claim_that_sensitive_fields_were_masked() {
    let mut evidence = EvidenceRef {
        id: "evidence-1".into(),
        source_kind: EvidenceSourceKind::Fixture,
        connector_id: None,
        scope: scope(),
        endpoint: "fixture://operations".into(),
        query: None,
        observed_at: "2026-08-28T09:00:00Z".into(),
        excerpt: "unparsed source".into(),
        native_url: None,
        redaction: EvidenceRedaction {
            classification_verified: true,
            redaction_verified: true,
            masked: true,
            unparsed: true,
        },
    };
    let mut snapshot = OperationsSnapshot {
        generated_at: "2026-08-28T09:00:00Z".into(),
        scope: scope(),
        source_status: Vec::new(),
        health_summary: HealthSummary {
            state: ConsoleHealthState::Healthy,
            headline: BusinessImpact {
                level: ImpactLevel::None,
                summary: "none".into(),
                customer_scope: "none".into(),
                service_criticality: "none".into(),
                trajectory: ImpactTrajectory::Improving,
                dimensions: ImpactDimensions::single_dimension(
                    ImpactLevel::None,
                    ImpactTrajectory::Improving,
                ),
                evidence_ids: vec!["evidence-1".into()],
            },
            attention: critical_number(),
            impacted_services: critical_number(),
            active_by_severity: Vec::new(),
            environments_by_state: Vec::new(),
            contributing_scopes: Vec::new(),
        },
        incident_queue: Vec::new(),
        signal_summary: SignalSummary {
            active_alerts: critical_number(),
            active_anomalies: critical_number(),
            checks_due: critical_number(),
            checks_timed_out: critical_number(),
            by_source: Vec::new(),
        },
        changes: Vec::new(),
        change_stream_status: ChangeStreamStatus::default(),
        environments: Vec::new(),
        evidence: vec![evidence.clone()],
        widget_registry: Vec::new(),
    };
    assert!(snapshot.validate().is_err());

    evidence.redaction.masked = false;
    snapshot.evidence = vec![evidence];
    assert!(snapshot.validate().is_ok());
}

#[test]
fn every_operations_enum_uses_an_explicit_symmetric_wire_value() {
    macro_rules! assert_wire_values {
        ($type:ty, $( $variant:expr => $wire:expr ),+ $(,)?) => {
            $(
                assert_eq!(serde_json::to_value($variant).unwrap(), json!($wire));
                assert_eq!(
                    serde_json::from_value::<$type>(json!($wire)).unwrap(),
                    $variant
                );
            )+
        };
    }

    assert_wire_values!(
        EvidenceSourceKind,
        EvidenceSourceKind::Alertmanager => "alertmanager",
        EvidenceSourceKind::Prometheus => "prometheus",
        EvidenceSourceKind::Kubernetes => "kubernetes",
        EvidenceSourceKind::Cloud => "cloud",
        EvidenceSourceKind::HealthCheck => "health_check",
        EvidenceSourceKind::Fixture => "fixture",
    );
    assert_wire_values!(
        DrillDownDestination,
        DrillDownDestination::Evidence => "evidence",
        DrillDownDestination::IncidentQueue => "incident_queue",
        DrillDownDestination::SignalSummary => "signal_summary",
        DrillDownDestination::ChangeStream => "change_stream",
        DrillDownDestination::EnvironmentStatus => "environment_status",
    );
    assert_wire_values!(
        NumberUnit,
        NumberUnit::Count => "count",
        NumberUnit::Percentage => "percentage",
        NumberUnit::Milliseconds => "milliseconds",
        NumberUnit::Seconds => "seconds",
    );
    assert_wire_values!(
        ConsoleHealthState,
        ConsoleHealthState::Healthy => "healthy",
        ConsoleHealthState::Degraded => "degraded",
        ConsoleHealthState::Critical => "critical",
        ConsoleHealthState::Unknown => "unknown",
    );
    assert_wire_values!(
        ImpactLevel,
        ImpactLevel::Critical => "critical",
        ImpactLevel::High => "high",
        ImpactLevel::Medium => "medium",
        ImpactLevel::Low => "low",
        ImpactLevel::None => "none",
        ImpactLevel::Unknown => "unknown",
    );
    assert_wire_values!(
        ConsoleSeverity,
        ConsoleSeverity::S1 => "S1",
        ConsoleSeverity::S2 => "S2",
        ConsoleSeverity::S3 => "S3",
        ConsoleSeverity::S4 => "S4",
        ConsoleSeverity::S5 => "S5",
    );
    assert_wire_values!(
        ConsolePriority,
        ConsolePriority::P1 => "P1",
        ConsolePriority::P2 => "P2",
        ConsolePriority::P3 => "P3",
        ConsolePriority::P4 => "P4",
        ConsolePriority::P5 => "P5",
    );
    assert_wire_values!(
        ImpactTrajectory,
        ImpactTrajectory::Expanding => "expanding",
        ImpactTrajectory::Stable => "stable",
        ImpactTrajectory::Improving => "improving",
        ImpactTrajectory::Unknown => "unknown",
    );
    assert_wire_values!(
        QueueItemSourceKind,
        QueueItemSourceKind::Alert => "alert",
        QueueItemSourceKind::Anomaly => "anomaly",
        QueueItemSourceKind::ScheduledHealthCheck => "scheduled_health_check",
        QueueItemSourceKind::FixtureIncident => "fixture_incident",
    );
    assert_wire_values!(
        QueueStatus,
        QueueStatus::Detected => "detected",
        QueueStatus::Triage => "triage",
        QueueStatus::Investigating => "investigating",
        QueueStatus::Mitigating => "mitigating",
        QueueStatus::Monitoring => "monitoring",
    );
    assert_wire_values!(
        ThresholdOperator,
        ThresholdOperator::GreaterThan => "gt",
        ThresholdOperator::GreaterThanOrEqual => "gte",
        ThresholdOperator::LessThan => "lt",
        ThresholdOperator::LessThanOrEqual => "lte",
    );
    assert_wire_values!(
        RateDirection,
        RateDirection::Increase => "increase",
        RateDirection::Decrease => "decrease",
        RateDirection::Absolute => "absolute",
    );
    assert_wire_values!(
        AnomalyEvaluationStatus,
        AnomalyEvaluationStatus::Triggered => "triggered",
        AnomalyEvaluationStatus::NotTriggered => "not_triggered",
        AnomalyEvaluationStatus::InsufficientData => "insufficient_data",
    );
    assert_wire_values!(
        HealthCheckOutcome,
        HealthCheckOutcome::Healthy => "healthy",
        HealthCheckOutcome::Degraded => "degraded",
        HealthCheckOutcome::Unavailable => "unavailable",
        HealthCheckOutcome::TimedOut => "timed_out",
        HealthCheckOutcome::SkippedNotDue => "skipped_not_due",
        HealthCheckOutcome::SkippedCooldown => "skipped_cooldown",
        HealthCheckOutcome::SkippedDisabled => "skipped_disabled",
    );
    assert_wire_values!(
        ChangeKind,
        ChangeKind::Deployment => "deployment",
        ChangeKind::Configuration => "configuration",
        ChangeKind::Maintenance => "maintenance",
        ChangeKind::Connector => "connector",
    );
    assert_wire_values!(
        SourceState,
        SourceState::Fresh => "fresh",
        SourceState::Stale => "stale",
        SourceState::Unavailable => "unavailable",
        SourceState::Unverified => "unverified",
    );
    assert_wire_values!(
        StatusReason,
        StatusReason::NotConfigured => "not_configured",
        StatusReason::Unreachable => "unreachable",
        StatusReason::TimedOut => "timed_out",
        StatusReason::PolicyDenied => "policy_denied",
        StatusReason::NoDataInWindow => "no_data_in_window",
        StatusReason::Unknown => "unknown",
    );
    assert_wire_values!(
        ChangeStreamState,
        ChangeStreamState::Available => "available",
        ChangeStreamState::Empty => "empty",
        ChangeStreamState::Unavailable => "unavailable",
    );
    assert_wire_values!(
        WidgetId,
        WidgetId::HealthSummary => "health_summary",
        WidgetId::IncidentQueue => "incident_queue",
        WidgetId::SignalSummary => "signal_summary",
        WidgetId::ChangeStream => "change_stream",
        WidgetId::EnvironmentStatus => "environment_status",
    );
    assert_wire_values!(
        WidgetSize,
        WidgetSize::Compact => "compact",
        WidgetSize::Standard => "standard",
        WidgetSize::Wide => "wide",
    );
}

#[test]
fn severity_and_priority_order_from_highest_to_lowest_impact() {
    let mut severities = vec![
        ConsoleSeverity::S4,
        ConsoleSeverity::S1,
        ConsoleSeverity::S3,
        ConsoleSeverity::S2,
        ConsoleSeverity::S5,
    ];
    severities.sort();
    assert_eq!(
        severities,
        vec![
            ConsoleSeverity::S1,
            ConsoleSeverity::S2,
            ConsoleSeverity::S3,
            ConsoleSeverity::S4,
            ConsoleSeverity::S5,
        ]
    );

    let mut priorities = vec![
        ConsolePriority::P4,
        ConsolePriority::P1,
        ConsolePriority::P3,
        ConsolePriority::P5,
        ConsolePriority::P2,
    ];
    priorities.sort();
    assert_eq!(
        priorities,
        vec![
            ConsolePriority::P1,
            ConsolePriority::P2,
            ConsolePriority::P3,
            ConsolePriority::P4,
            ConsolePriority::P5,
        ]
    );
}

#[test]
fn invalid_anomaly_rules_are_rejected_without_accepting_non_finite_thresholds() {
    let invalid = AnomalyRule {
        id: " ".into(),
        name: "CPU threshold".into(),
        enabled: true,
        scope: scope(),
        metric_key: "cpu".into(),
        condition: AnomalyCondition::Threshold {
            operator: ThresholdOperator::GreaterThan,
            threshold: "NaN".into(),
        },
        severity: ConsoleSeverity::S3,
        cooldown_seconds: 0,
    };

    assert!(invalid.validate().is_err());
    assert!(invalid.is_valid().is_err());
}

#[test]
fn invalid_health_check_definitions_are_rejected_before_evaluation() {
    let invalid = HealthCheckSchedule {
        id: "check-1".into(),
        name: "API".into(),
        enabled: true,
        scope: scope(),
        source: HealthCheckSource::Fixture {
            fixture_key: "api".into(),
        },
        interval_seconds: 0,
        timeout_ms: 0,
        cooldown_seconds: 0,
        last_run_at: Some("not-a-timestamp".into()),
        last_signal_at: None,
        defined_by: None,
        defined_at: None,
        last_outcome: None,
    };

    assert!(invalid.validate().is_err());
    assert!(invalid.is_valid().is_err());
}

#[test]
fn critical_numbers_reject_missing_evidence_or_source_query() {
    let mut missing_evidence = critical_number();
    missing_evidence.evidence_ids.clear();
    assert!(missing_evidence.validate().is_err());

    let mut missing_query = critical_number();
    missing_query.drill_down_reference.source_query.clear();
    assert!(missing_query.validate().is_err());
}

#[test]
fn richer_console_types_keep_numbers_tied_to_scope_query_window_and_evidence() {
    let reference = DrillDownReference {
        source_query: "sum(rate(http_requests_total[5m]))".into(),
        scope: scope(),
        time_window: Some(TimeWindow {
            start: "2026-08-28T08:55:00Z".into(),
            end: "2026-08-28T09:00:00Z".into(),
        }),
        evidence_ids: vec!["evidence-1".into()],
    };
    assert_round_trip(reference.clone());

    let contributing_scope = ContributingScope {
        scope: scope(),
        impact: ImpactLevel::Critical,
        summary: "Checkout is unavailable".into(),
        evidence_ids: vec!["evidence-1".into()],
    };
    let summary = HealthSummary {
        state: ConsoleHealthState::Critical,
        headline: BusinessImpact {
            level: ImpactLevel::Critical,
            summary: "Checkout outage".into(),
            customer_scope: "all customers".into(),
            service_criticality: "tier-0".into(),
            trajectory: ImpactTrajectory::Expanding,
            dimensions: ImpactDimensions::single_dimension(
                ImpactLevel::Critical,
                ImpactTrajectory::Expanding,
            ),
            evidence_ids: vec!["evidence-1".into()],
        },
        attention: critical_number(),
        impacted_services: critical_number(),
        active_by_severity: vec![critical_number()],
        environments_by_state: vec![critical_number()],
        contributing_scopes: vec![contributing_scope],
    };
    assert_round_trip(summary.clone());
    assert_eq!(summary.overall_posture(), ConsoleHealthState::Critical);
    assert_eq!(summary.impact_tier(), ImpactLevel::Critical);

    let config = WidgetConfig {
        id: WidgetId::HealthSummary,
        kind: WidgetKind::HealthSummary,
        visible: true,
        order: 0,
        options: BTreeMap::from([(String::from("size"), json!("wide"))]),
    };
    assert_round_trip(config);
    assert_eq!(
        curated_default_layout()
            .into_iter()
            .map(|widget| widget.id)
            .collect::<Vec<_>>(),
        vec![
            WidgetId::HealthSummary,
            WidgetId::IncidentQueue,
            WidgetId::SignalSummary,
            WidgetId::ChangeStream,
            WidgetId::EnvironmentStatus,
        ]
    );
}
