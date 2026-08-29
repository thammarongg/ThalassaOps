// SPDX-License-Identifier: Apache-2.0

use rusqlite::Connection;
use serde_json::json;
use tempfile::tempdir;
use thalassa_domain::{
    AnomalyCondition, AnomalySignal, EvidenceRedaction, EvidenceRef, EvidenceSourceKind,
    HealthCheckAudit, HealthCheckOutcome, HealthCheckResult, ResourceScope, SignalKind,
    SignalState, SignalTargetKind, ThresholdOperator,
};
use thalassaops::app::AppState;
use thalassaops::correlation::adapters::{
    normalize_operational, OperationalAdapter, SignalAdapter, SignalAdapterError,
};
use thalassaops::correlation::{
    correlation_fixture_catalog, fixture_scope, SourceRecordError, SourceRecordInput,
    SourceRecordStore,
};
use thalassaops::observability::alertmanager::{
    AlertSourceReference, NormalizedAlert, ResourceReference,
};
use uuid::Uuid;

fn evidence(id: &str, source_kind: EvidenceSourceKind, scope: ResourceScope) -> EvidenceRef {
    EvidenceRef {
        id: id.into(),
        source_kind,
        connector_id: Some("fixture-catalog".into()),
        scope,
        endpoint: "fixture://operational/test".into(),
        query: Some("recorded fixture".into()),
        observed_at: "2026-08-28T09:00:00Z".into(),
        excerpt: "synthetic source evidence".into(),
        native_url: None,
        redaction: EvidenceRedaction {
            classification_verified: true,
            redaction_verified: true,
            masked: false,
            unparsed: false,
        },
    }
}

fn fixture(
    key: &str,
    source_kind: EvidenceSourceKind,
    recorded_json: serde_json::Value,
    observed_at: Option<&str>,
    evidence_id: &str,
) -> thalassaops::correlation::ReplayableSignalFixture {
    let scope = fixture_scope();
    thalassaops::correlation::ReplayableSignalFixture {
        key: key.into(),
        source_kind,
        scope: scope.clone(),
        recorded_json,
        observed_at: observed_at.map(str::to_owned),
        ingested_at: Some("2026-08-28T09:00:00Z".into()),
        evidence: vec![evidence(evidence_id, source_kind, scope)],
    }
}

#[test]
fn alert_is_normalized_without_losing_unknown_source_fields() {
    let source = correlation_fixture_catalog()
        .fixtures
        .into_iter()
        .find(|fixture| fixture.key == "shared-service-alert")
        .expect("the shared service alert fixture exists");
    let mut records = SourceRecordStore::default();

    let signals = normalize_operational(&source, &mut records).expect("alert should normalize");
    assert_eq!(signals.len(), 1);
    let signal = &signals[0];
    assert_eq!(signal.kind, SignalKind::Alert);
    assert_eq!(signal.source, EvidenceSourceKind::Alertmanager);
    assert_eq!(signal.state, SignalState::Active);
    assert_eq!(signal.targets[0].kind, SignalTargetKind::Service);
    assert!(signal.source_record.native_id.is_some());
    assert!(signal.source_record.content_digest.starts_with("sha256:"));

    let retained = records
        .get(&signal.source_record)
        .expect("source record should be reachable");
    assert_eq!(
        retained.redacted_payload["vendor_extension"]["capture"],
        "synthetic"
    );
    assert_eq!(retained.evidence_ids, signal.source_record.evidence_ids);
    assert_eq!(
        retained.redacted_payload["target"]["id"],
        "service/checkout"
    );
}

#[test]
fn anomaly_preserves_finite_values_condition_and_target() {
    let source = correlation_fixture_catalog()
        .fixtures
        .into_iter()
        .find(|fixture| fixture.key == "shared-service-anomaly")
        .expect("the shared service anomaly fixture exists");
    let mut records = SourceRecordStore::default();

    let signal = OperationalAdapter::new(EvidenceSourceKind::Prometheus)
        .normalize(&source, &mut records)
        .expect("anomaly should normalize")
        .remove(0);
    assert_eq!(signal.kind, SignalKind::Anomaly);
    assert_eq!(signal.state, SignalState::Active);
    assert_eq!(signal.targets[0].id, "service/checkout");
    match signal.payload {
        thalassa_domain::SignalPayload::Anomaly {
            observed_value,
            comparison_value,
            condition,
        } => {
            assert_eq!(observed_value, 120.0);
            assert_eq!(comparison_value, 100.0);
            assert!(matches!(condition, AnomalyCondition::Threshold { .. }));
        }
        payload => panic!("unexpected payload: {payload:?}"),
    }
    assert_eq!(records.len(), 1);
}

#[test]
fn anomaly_without_a_target_has_no_grouping_identity() {
    let source = fixture(
        "anomaly-without-target",
        EvidenceSourceKind::Prometheus,
        json!({
            "rule_id": "rule-checkout-errors",
            "metric_key": "checkout_error_rate",
            "observed_value": 0.08,
            "comparison_value": 0.05,
            "condition": {"threshold": {"operator": "gte", "threshold": "0.05"}},
            "vendor_extension": {"capture": "synthetic"}
        }),
        Some("2026-08-28T08:59:00Z"),
        "evidence-anomaly-without-target",
    );
    let mut records = SourceRecordStore::default();

    let signal = normalize_operational(&source, &mut records)
        .unwrap()
        .remove(0);
    assert!(signal.targets.is_empty());
    assert!(signal.dedup_key.is_none());
}

#[test]
fn anomaly_without_a_condition_is_rejected_without_inventing_an_operator() {
    let mut source = fixture(
        "anomaly-without-condition",
        EvidenceSourceKind::Prometheus,
        json!({
            "rule_id": "rule-checkout-errors",
            "metric_key": "checkout_error_rate",
            "observed_value": 0.08,
            "comparison_value": 0.05,
            "vendor_extension": {"capture": "synthetic"}
        }),
        Some("2026-08-28T08:59:00Z"),
        "evidence-anomaly-without-condition",
    );
    source
        .recorded_json
        .as_object_mut()
        .expect("test payload is an object")
        .remove("condition");
    let mut records = SourceRecordStore::default();

    assert_eq!(
        normalize_operational(&source, &mut records),
        Err(SignalAdapterError::MalformedPayload)
    );
    assert_eq!(records.len(), 1, "the complete source remains retained");
}

#[test]
fn skipped_health_check_is_retained_with_explicit_skipped_outcome() {
    let source = correlation_fixture_catalog()
        .fixtures
        .into_iter()
        .find(|fixture| fixture.key == "health-check-skipped")
        .expect("the skipped health fixture exists");
    let mut records = SourceRecordStore::default();

    let signal = normalize_operational(&source, &mut records)
        .expect("health check should normalize")
        .remove(0);
    assert_eq!(signal.kind, SignalKind::HealthCheck);
    assert_eq!(signal.state, SignalState::Observed);
    assert!(matches!(
        signal.payload,
        thalassa_domain::SignalPayload::HealthCheck {
            outcome: HealthCheckOutcome::SkippedCooldown
        }
    ));
    assert_eq!(signal.targets.len(), 0);
    assert!(records.get(&signal.source_record).is_some());
}

#[test]
fn absent_alert_time_and_severity_do_not_create_defaults() {
    let source = fixture(
        "alert-without-optional-facts",
        EvidenceSourceKind::Alertmanager,
        json!({
            "fingerprint": "alert-no-optional-facts",
            "state": "resolved",
            "labels": {},
            "vendor_extension": {"capture": "synthetic"}
        }),
        None,
        "evidence-alert-no-optional-facts",
    );
    let mut records = SourceRecordStore::default();
    let signal = normalize_operational(&source, &mut records)
        .expect("missing optional facts remain an admitted signal")
        .remove(0);

    assert_eq!(signal.state, SignalState::Cleared);
    assert_eq!(signal.observed_at, None);
    assert_eq!(signal.business_severity, None);
    assert!(signal.targets.is_empty());
    assert_eq!(signal.source_record.revision, None);
    assert!(signal.dedup_key.is_some());
}

#[test]
fn record_is_rejected_before_retention_when_evidence_is_unverified_or_out_of_scope() {
    let scope = fixture_scope();
    let mut unverified = evidence(
        "evidence-unverified",
        EvidenceSourceKind::Alertmanager,
        scope.clone(),
    );
    unverified.redaction.redaction_verified = false;
    let input = SourceRecordInput::from_fixture(
        &fixture(
            "unverified",
            EvidenceSourceKind::Alertmanager,
            json!({"fingerprint": "unsafe-admission"}),
            Some("2026-08-28T09:00:00Z"),
            "ignored",
        ),
        Some("unsafe-admission".into()),
        None,
    )
    .with_evidence(vec![unverified]);
    let mut records = SourceRecordStore::with_scope(scope.clone());
    assert!(matches!(
        records.retain(input),
        Err(SourceRecordError::InvalidEvidence)
    ));
    assert_eq!(records.len(), 0);

    let outside = ResourceScope::workspace(
        uuid::Uuid::from_u128(101),
        uuid::Uuid::from_u128(102),
        uuid::Uuid::from_u128(103),
    );
    let mut out_of_scope = fixture(
        "out-of-scope",
        EvidenceSourceKind::Alertmanager,
        json!({"fingerprint": "out-of-scope"}),
        Some("2026-08-28T09:00:00Z"),
        "evidence-out-of-scope",
    );
    out_of_scope.scope = outside.clone();
    out_of_scope.evidence[0].scope = outside;
    assert!(matches!(
        normalize_operational(&out_of_scope, &mut SourceRecordStore::with_scope(scope)),
        Err(SignalAdapterError::Source(SourceRecordError::ScopeMismatch))
    ));
}

#[test]
fn source_identity_conflicts_are_rejected_and_identical_replays_are_idempotent() {
    let scope = fixture_scope();
    let first = SourceRecordInput::new(
        EvidenceSourceKind::Alertmanager,
        Some("same-alert".into()),
        Some("revision-1".into()),
        scope.clone(),
        json!({"state": "firing", "vendor_extension": {"capture": "one"}}),
        vec![evidence(
            "evidence-first",
            EvidenceSourceKind::Alertmanager,
            scope.clone(),
        )],
    );
    let identical = SourceRecordInput::new(
        EvidenceSourceKind::Alertmanager,
        Some("same-alert".into()),
        Some("revision-1".into()),
        scope.clone(),
        json!({"vendor_extension": {"capture": "one"}, "state": "firing"}),
        vec![evidence(
            "evidence-replay",
            EvidenceSourceKind::Alertmanager,
            scope.clone(),
        )],
    );
    let conflicting = SourceRecordInput::new(
        EvidenceSourceKind::Alertmanager,
        Some("same-alert".into()),
        Some("revision-1".into()),
        scope.clone(),
        json!({"state": "resolved"}),
        vec![evidence(
            "evidence-conflicting",
            EvidenceSourceKind::Alertmanager,
            scope,
        )],
    );
    let mut records = SourceRecordStore::default();
    let first_ref = records.retain(first).expect("first record should retain");
    let replay_ref = records
        .retain(identical)
        .expect("identical replay should retain");
    assert_eq!(first_ref.content_digest, replay_ref.content_digest);
    assert_eq!(records.len(), 1);
    assert!(matches!(
        records.retain(conflicting),
        Err(SourceRecordError::AmbiguousSourceIdentity)
    ));
}

#[test]
fn evidence_ids_cannot_alias_different_retained_evidence() {
    let scope = fixture_scope();
    let first_evidence = evidence(
        "evidence-collision",
        EvidenceSourceKind::Alertmanager,
        scope.clone(),
    );
    let mut conflicting_evidence = first_evidence.clone();
    conflicting_evidence.excerpt = "different source evidence".into();

    let first = SourceRecordInput::new(
        EvidenceSourceKind::Alertmanager,
        Some("first-alert".into()),
        None,
        scope.clone(),
        json!({"state": "firing"}),
        vec![first_evidence],
    );
    let second = SourceRecordInput::new(
        EvidenceSourceKind::Alertmanager,
        Some("second-alert".into()),
        None,
        scope,
        json!({"state": "resolved"}),
        vec![conflicting_evidence],
    );
    let mut store = SourceRecordStore::default();
    store.retain(first).unwrap();
    assert_eq!(
        store.retain(second),
        Err(SourceRecordError::DuplicateEvidence)
    );
    assert_eq!(store.len(), 1);
}

#[test]
fn source_record_masks_sensitive_values_but_rejects_unsafe_identity() {
    let scope = fixture_scope();
    let mut records = SourceRecordStore::default();
    let input = SourceRecordInput::new(
        EvidenceSourceKind::Alertmanager,
        Some("safe-alert".into()),
        None,
        scope.clone(),
        json!({
            "fingerprint": "safe-alert",
            "password": "do-not-retain",
            "vendor_extension": {"capture": "synthetic"}
        }),
        vec![evidence(
            "evidence-masked",
            EvidenceSourceKind::Alertmanager,
            scope,
        )],
    );
    let retained = records.retain(input).expect("sensitive fields are masked");
    let record = records.get(&retained).unwrap();
    assert_eq!(record.redacted_payload["password"], "<REDACTED>");
    assert_eq!(
        record.redacted_payload["vendor_extension"]["capture"],
        "synthetic"
    );

    let unsafe_input = SourceRecordInput::new(
        EvidenceSourceKind::Alertmanager,
        Some("arn:unsafe".into()),
        None,
        fixture_scope(),
        json!({"fingerprint": "arn:unsafe"}),
        vec![evidence(
            "evidence-unsafe-id",
            EvidenceSourceKind::Alertmanager,
            fixture_scope(),
        )],
    );
    assert!(matches!(
        records.retain(unsafe_input),
        Err(SourceRecordError::UnsafeIdentity)
    ));
}

#[test]
fn forbidden_data_never_enters_the_source_record_or_typed_error() {
    let scope = fixture_scope();
    let input = SourceRecordInput::new(
        EvidenceSourceKind::Alertmanager,
        Some("safe-alert".into()),
        None,
        scope.clone(),
        json!({
            "fingerprint": "safe-alert",
            "account_id": "123456789012",
            "vendor_extension": {"capture": "synthetic"}
        }),
        vec![evidence(
            "evidence-forbidden",
            EvidenceSourceKind::Alertmanager,
            scope,
        )],
    );
    let mut records = SourceRecordStore::default();
    let error = records
        .retain(input)
        .expect_err("account identifiers fail closed");
    assert_eq!(error, SourceRecordError::InvalidPayload);
    assert!(!error.to_string().contains("123456789012"));
    assert!(records.is_empty());

    let numeric_identity = SourceRecordInput::new(
        EvidenceSourceKind::Alertmanager,
        Some("123456789012".into()),
        None,
        fixture_scope(),
        json!({"state": "firing"}),
        vec![evidence(
            "evidence-numeric-identity",
            EvidenceSourceKind::Alertmanager,
            fixture_scope(),
        )],
    );
    assert_eq!(
        records.retain(numeric_identity),
        Err(SourceRecordError::UnsafeIdentity)
    );
}

#[test]
fn app_migration_registers_the_append_only_source_record_table() {
    let directory = tempdir().unwrap();
    let database_path = directory.path().join("signals.sqlite");
    AppState::open(&database_path).expect("app bootstrap applies local migrations");

    let connection = Connection::open(database_path).unwrap();
    let table: String = connection
        .query_row(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'source_records'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(table, "source_records");
    let migration: i64 = connection
        .query_row(
            "SELECT version FROM schema_migrations WHERE version = 3",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(migration, 3);
    let evidence_migration: i64 = connection
        .query_row(
            "SELECT version FROM schema_migrations WHERE version = 4",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(evidence_migration, 4);
}

#[test]
fn every_existing_operational_fixture_normalizes_through_one_adapter() {
    let catalog = correlation_fixture_catalog();
    let mut records = SourceRecordStore::default();
    let mut normalized = Vec::new();
    for fixture in catalog.fixtures.iter().filter(|fixture| {
        matches!(
            fixture.source_kind,
            EvidenceSourceKind::Alertmanager
                | EvidenceSourceKind::Prometheus
                | EvidenceSourceKind::HealthCheck
        )
    }) {
        normalized.extend(
            normalize_operational(fixture, &mut records)
                .unwrap_or_else(|error| panic!("{} failed: {error}", fixture.key)),
        );
    }
    assert_eq!(normalized.len(), 15);
    assert_eq!(records.len(), 15);
    assert!(normalized.iter().all(|signal| signal.validate().is_ok()));
}

#[test]
fn sqlite_source_record_store_round_trips_the_complete_retained_record() {
    let directory = tempdir().unwrap();
    let database_path = directory.path().join("source-records.sqlite");
    let scope = fixture_scope();
    let retained_evidence = evidence(
        "evidence-sqlite",
        EvidenceSourceKind::Alertmanager,
        scope.clone(),
    );
    let input = SourceRecordInput::new(
        EvidenceSourceKind::Alertmanager,
        Some("sqlite-alert".into()),
        Some("revision-1".into()),
        scope.clone(),
        json!({
            "state": "firing",
            "vendor_extension": {"unknown": [1, 2, 3]}
        }),
        vec![retained_evidence.clone()],
    );
    let (reference, payload) = {
        let connection = Connection::open(&database_path).unwrap();
        let mut store = SourceRecordStore::with_connection(connection).unwrap();
        let reference = store.retain(input).unwrap();
        let payload = store.get(&reference).unwrap().redacted_payload.clone();
        (reference, payload)
    };

    let connection = Connection::open(database_path).unwrap();
    let store = SourceRecordStore::with_connection(connection).unwrap();
    let retained = store.get(&reference).expect("row survives reload");
    assert_eq!(retained.redacted_payload, payload);
    assert_eq!(retained.native_id.as_deref(), Some("sqlite-alert"));
    assert_eq!(retained.evidence_ids, vec!["evidence-sqlite"]);
    assert_eq!(
        store.evidence_for_record(&reference).unwrap(),
        vec![retained_evidence]
    );
}

#[test]
fn sqlite_source_record_store_rejects_tampered_evidence_on_reload() {
    let directory = tempdir().unwrap();
    let database_path = directory.path().join("source-records.sqlite");
    let scope = fixture_scope();
    let retained_evidence = evidence(
        "evidence-reload-safety",
        EvidenceSourceKind::Alertmanager,
        scope.clone(),
    );
    let input = SourceRecordInput::new(
        EvidenceSourceKind::Alertmanager,
        Some("reload-safety-alert".into()),
        None,
        scope,
        json!({"state": "firing"}),
        vec![retained_evidence],
    );
    {
        let connection = Connection::open(&database_path).unwrap();
        let mut store = SourceRecordStore::with_connection(connection).unwrap();
        store.retain(input).unwrap();
    }

    let connection = Connection::open(&database_path).unwrap();
    let mut tampered: serde_json::Value = connection
        .query_row(
            "SELECT evidence_json FROM source_record_evidence WHERE evidence_id = ?1",
            ["evidence-reload-safety"],
            |row| row.get::<_, String>(0),
        )
        .map(|json| serde_json::from_str(&json).unwrap())
        .unwrap();
    tampered["excerpt"] = json!("token=must-not-load");
    connection
        .execute(
            "UPDATE source_record_evidence SET evidence_json = ?1 WHERE evidence_id = ?2",
            rusqlite::params![
                serde_json::to_string(&tampered).unwrap(),
                "evidence-reload-safety"
            ],
        )
        .unwrap();

    match SourceRecordStore::with_connection(connection) {
        Err(error) => assert_eq!(error, SourceRecordError::InvalidEvidence),
        Ok(_) => panic!("tampered evidence must not be loaded"),
    }
}

#[test]
fn sqlite_source_record_store_rejects_cross_scope_evidence_id_rebinding() {
    let directory = tempdir().unwrap();
    let database_path = directory.path().join("source-records.sqlite");
    let first_scope = ResourceScope::workspace(
        Uuid::from_u128(11),
        Uuid::from_u128(12),
        Uuid::from_u128(13),
    );
    let second_scope = ResourceScope::workspace(
        Uuid::from_u128(21),
        Uuid::from_u128(22),
        Uuid::from_u128(23),
    );
    let first_input = SourceRecordInput::new(
        EvidenceSourceKind::Alertmanager,
        Some("first-alert".into()),
        None,
        first_scope.clone(),
        json!({"state": "firing", "target": "first"}),
        vec![evidence(
            "evidence-rebound",
            EvidenceSourceKind::Alertmanager,
            first_scope,
        )],
    );
    {
        let connection = Connection::open(&database_path).unwrap();
        let mut store =
            SourceRecordStore::with_connection_and_scope(connection, first_input.scope.clone())
                .unwrap();
        store.retain(first_input).unwrap();
    }

    let second_input = SourceRecordInput::new(
        EvidenceSourceKind::Alertmanager,
        Some("second-alert".into()),
        None,
        second_scope.clone(),
        json!({"state": "firing", "target": "second"}),
        vec![evidence(
            "evidence-rebound",
            EvidenceSourceKind::Alertmanager,
            second_scope,
        )],
    );
    let connection = Connection::open(database_path).unwrap();
    let mut store =
        SourceRecordStore::with_connection_and_scope(connection, second_input.scope.clone())
            .unwrap();
    assert_eq!(
        store.retain(second_input),
        Err(SourceRecordError::DuplicateEvidence)
    );
}

#[test]
fn sqlite_source_record_store_does_not_update_an_out_of_scope_identity_row() {
    let directory = tempdir().unwrap();
    let database_path = directory.path().join("source-records.sqlite");
    let first_scope = ResourceScope::workspace(
        Uuid::from_u128(31),
        Uuid::from_u128(32),
        Uuid::from_u128(33),
    );
    let second_scope = ResourceScope::workspace(
        Uuid::from_u128(41),
        Uuid::from_u128(42),
        Uuid::from_u128(43),
    );
    let first_input = SourceRecordInput::new(
        EvidenceSourceKind::Alertmanager,
        Some("first-scope-alert".into()),
        None,
        first_scope.clone(),
        json!({"state": "firing", "target": "same-content"}),
        vec![evidence(
            "evidence-first-scope",
            EvidenceSourceKind::Alertmanager,
            first_scope.clone(),
        )],
    );
    {
        let connection = Connection::open(&database_path).unwrap();
        let mut store =
            SourceRecordStore::with_connection_and_scope(connection, first_input.scope.clone())
                .unwrap();
        store.retain(first_input).unwrap();
    }

    let second_input = SourceRecordInput::new(
        EvidenceSourceKind::Alertmanager,
        Some("second-scope-alert".into()),
        None,
        second_scope.clone(),
        json!({"target": "same-content", "state": "firing"}),
        vec![evidence(
            "evidence-second-scope",
            EvidenceSourceKind::Alertmanager,
            second_scope,
        )],
    );
    let connection = Connection::open(&database_path).unwrap();
    let mut store =
        SourceRecordStore::with_connection_and_scope(connection, second_input.scope.clone())
            .unwrap();
    assert_eq!(
        store.retain(second_input),
        Err(SourceRecordError::ScopeMismatch)
    );

    let connection = Connection::open(database_path).unwrap();
    let stored_scope: String = connection
        .query_row("SELECT scope FROM source_records", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        serde_json::from_str::<ResourceScope>(&stored_scope).unwrap(),
        first_scope
    );
}

#[test]
fn alert_dedup_uses_safe_target_when_native_fingerprint_is_absent() {
    let source = fixture(
        "alert-target-only",
        EvidenceSourceKind::Alertmanager,
        json!({
            "state": "firing",
            "target": {"kind": "service", "id": "service/checkout"},
            "vendor_extension": {"capture": "synthetic"}
        }),
        Some("2026-08-28T08:59:00Z"),
        "evidence-alert-target-only",
    );
    let mut records = SourceRecordStore::default();
    let signal = normalize_operational(&source, &mut records)
        .unwrap()
        .remove(0);
    assert!(signal.source_record.native_id.is_none());
    assert!(signal.dedup_key.is_some());
}

#[test]
fn typed_sprint_11_values_use_the_same_source_preserving_adapter() {
    let scope = fixture_scope();
    let alert_fixture = fixture(
        "typed-alert",
        EvidenceSourceKind::Alertmanager,
        json!({"vendor_extension": {"unknown": true}}),
        Some("2026-08-28T08:55:00Z"),
        "evidence-typed-alert",
    );
    let alert = NormalizedAlert {
        fingerprint: "typed-alert-fingerprint".into(),
        state: "firing".into(),
        starts_at: "2026-08-28T08:55:00Z".into(),
        ends_at: "2026-08-28T09:05:00Z".into(),
        labels: [("severity".into(), "S2".into())].into_iter().collect(),
        annotations: Default::default(),
        generator_url: None,
        source: AlertSourceReference {
            connector_id: "fixture-catalog".into(),
            endpoint: "fixture://operational/test".into(),
        },
        resource_reference: ResourceReference::Resolved {
            namespace: "prod".into(),
            kind: "Service".into(),
            name: "checkout".into(),
        },
    };
    let mut records = SourceRecordStore::default();
    let alert_signal =
        thalassaops::correlation::adapters::normalize_alert(&alert, &alert_fixture, &mut records)
            .unwrap()
            .remove(0);
    assert_eq!(
        alert_signal.business_severity,
        Some(thalassa_domain::ConsoleSeverity::S2)
    );
    assert_eq!(alert_signal.targets[0].kind, SignalTargetKind::Service);

    let anomaly_fixture = fixture(
        "typed-anomaly",
        EvidenceSourceKind::Prometheus,
        json!({"target": {"kind": "service", "id": "service/checkout"}}),
        Some("2026-08-28T08:59:00Z"),
        "evidence-typed-anomaly",
    );
    let anomaly = AnomalySignal {
        id: "typed-anomaly-id".into(),
        rule_id: "typed-rule".into(),
        metric_key: "typed-metric".into(),
        severity: thalassa_domain::ConsoleSeverity::S3,
        observed_at: "2026-08-28T08:59:00Z".into(),
        observed_value: 3.0,
        comparison_value: 2.0,
        condition: AnomalyCondition::Threshold {
            operator: ThresholdOperator::GreaterThan,
            threshold: "2".into(),
        },
        scope: scope.clone(),
        evidence_id: "evidence-typed-anomaly".into(),
    };
    let anomaly_signal = thalassaops::correlation::adapters::normalize_anomaly(
        &anomaly,
        &anomaly_fixture,
        &mut records,
    )
    .unwrap()
    .remove(0);
    assert_eq!(
        anomaly_signal.business_severity,
        Some(thalassa_domain::ConsoleSeverity::S3)
    );
    assert_eq!(anomaly_signal.kind, SignalKind::Anomaly);

    let health_fixture = fixture(
        "typed-health",
        EvidenceSourceKind::HealthCheck,
        json!({"vendor_extension": {"unknown": true}}),
        Some("2026-08-28T08:59:30Z"),
        "evidence-typed-health",
    );
    let health = HealthCheckResult {
        schedule_id: "typed-schedule".into(),
        outcome: HealthCheckOutcome::TimedOut,
        observed_at: "2026-08-28T08:59:30Z".into(),
        evidence_id: Some("evidence-typed-health".into()),
        audit: HealthCheckAudit {
            run_id: "typed-run".into(),
            schedule_id: "typed-schedule".into(),
            triggered_by: "fixture".into(),
            started_at: "2026-08-28T08:59:30Z".into(),
            completed_at: "2026-08-28T08:59:30Z".into(),
            duration_ms: 10,
            scope,
            source: thalassa_domain::HealthCheckSource::Fixture {
                fixture_key: "typed-health".into(),
            },
            outcome: HealthCheckOutcome::TimedOut,
            cooldown_suppressed: false,
            policy_version: 13,
        },
    };
    let health_signal = thalassaops::correlation::adapters::normalize_health_check(
        &health,
        &health_fixture,
        &mut records,
    )
    .unwrap()
    .remove(0);
    assert_eq!(health_signal.state, SignalState::Unknown);
}
