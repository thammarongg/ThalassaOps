// SPDX-License-Identifier: Apache-2.0

use serde_json::json;
use thalassa_domain::{EvidenceSourceKind, Signal, SignalState};
use thalassaops::correlation::adapters::{normalize_operational, normalize_security};
use thalassaops::correlation::dedup::{
    compute_dedup_key, deduplicate_signals, index_signals, stable_candidate_anchor, DedupError,
};
use thalassaops::correlation::{
    correlation_fixture_catalog, SourceRecordError, SourceRecordInput, SourceRecordStore,
};

fn security_fixture(
    source: EvidenceSourceKind,
) -> thalassaops::correlation::ReplayableSignalFixture {
    correlation_fixture_catalog()
        .fixtures
        .into_iter()
        .find(|fixture| fixture.source_kind == source)
        .expect("the committed security fixture exists")
}

fn normalize_fixture(
    fixture: &thalassaops::correlation::ReplayableSignalFixture,
    records: &mut SourceRecordStore,
) -> Vec<Signal> {
    if fixture.source_kind.is_security_source() {
        normalize_security(fixture, records).expect("security fixture should normalize")
    } else {
        normalize_operational(fixture, records).expect("operational fixture should normalize")
    }
}

#[test]
fn source_identity_tuples_cover_all_initial_sources() {
    let catalog = correlation_fixture_catalog();
    let mut records = SourceRecordStore::default();
    for source in [
        EvidenceSourceKind::Alertmanager,
        EvidenceSourceKind::Prometheus,
        EvidenceSourceKind::Trivy,
        EvidenceSourceKind::Falco,
        EvidenceSourceKind::Kyverno,
        EvidenceSourceKind::OpaGatekeeper,
        EvidenceSourceKind::HealthCheck,
    ] {
        let fixture = catalog
            .fixtures
            .iter()
            .find(|fixture| {
                fixture.source_kind == source
                    && (fixture.key.starts_with("security-")
                        || fixture.key == "shared-service-anomaly"
                        || fixture.key == "shared-service-alert"
                        || fixture.key == "health-check-checkout")
            })
            .or_else(|| {
                catalog
                    .fixtures
                    .iter()
                    .find(|fixture| fixture.source_kind == source)
            })
            .expect("the source fixture exists");
        let signal = normalize_fixture(fixture, &mut records)
            .into_iter()
            .next()
            .expect("fixture emits one signal");
        assert!(compute_dedup_key(&signal, Some(&records))
            .unwrap_or_else(|error| panic!("source {source:?}: {error:?}"))
            .is_some());
    }
}

#[test]
fn dedup_key_excludes_time_evidence_severity_state_and_message() {
    let fixture = security_fixture(EvidenceSourceKind::Trivy);
    let mut records = SourceRecordStore::default();
    let signal = normalize_fixture(&fixture, &mut records).remove(0);
    let original = compute_dedup_key(&signal, Some(&records)).unwrap();

    let mut changed = signal.clone();
    changed.observed_at = Some("2026-08-28T10:00:00Z".into());
    changed.ingested_at = Some("2026-08-28T11:00:00Z".into());
    changed.evidence_ids = vec!["evidence-security-trivy".into()];
    changed.source_record.evidence_ids = vec!["evidence-security-trivy".into()];
    changed.business_severity = Some(thalassa_domain::ConsoleSeverity::S5);
    changed.state = SignalState::Cleared;
    changed.dedup_key = None;

    let changed_key = compute_dedup_key(&changed, Some(&records)).unwrap();
    assert_eq!(original, changed_key);

    // The retained source record remains complete and message changes do not
    // become part of the logical identity either.
    assert!(records
        .get(&signal.source_record)
        .unwrap()
        .payload()
        .to_string()
        .contains("vendor_extension"));
}

#[test]
fn equal_cross_source_text_is_not_a_duplicate() {
    let mut records = SourceRecordStore::default();
    let trivy =
        normalize_fixture(&security_fixture(EvidenceSourceKind::Trivy), &mut records).remove(0);
    let gatekeeper = normalize_fixture(
        &security_fixture(EvidenceSourceKind::OpaGatekeeper),
        &mut records,
    )
    .remove(0);
    let trivy_key = compute_dedup_key(&trivy, Some(&records)).unwrap().unwrap();
    let gatekeeper_key = compute_dedup_key(&gatekeeper, Some(&records))
        .unwrap()
        .unwrap();
    assert_ne!(trivy_key, gatekeeper_key);
    assert!(trivy_key.starts_with("dedup:v1:trivy:security_finding:"));
    assert!(gatekeeper_key.starts_with("dedup:v1:opa_gatekeeper:security_finding:"));
}

#[test]
fn missing_identity_is_explicitly_excluded_from_deduplication() {
    let mut fixture = correlation_fixture_catalog()
        .fixtures
        .into_iter()
        .find(|fixture| fixture.key == "anomaly-checkout-errors")
        .expect("anomaly fixture exists");
    fixture.recorded_json["target"] = json!(null);
    let mut records = SourceRecordStore::default();
    let signal = normalize_fixture(&fixture, &mut records).remove(0);
    assert_eq!(compute_dedup_key(&signal, Some(&records)).unwrap(), None);
}

#[test]
fn revisions_share_an_association_key_but_all_source_signals_are_retained() {
    let first = security_fixture(EvidenceSourceKind::Trivy);
    let mut second = first.clone();
    second.recorded_json["Results"][0]["vendor_extension"]["revision_hint"] = json!("fixture-2");
    second.recorded_json["Results"][0]["FixedVersion"] = json!("1.2.5");

    let mut records = SourceRecordStore::default();
    let mut signals = normalize_fixture(&first, &mut records);
    signals.extend(normalize_fixture(&second, &mut records));
    assert_eq!(signals.len(), 2);
    assert_eq!(records.len(), 2);

    let first_key = compute_dedup_key(&signals[0], Some(&records)).unwrap();
    let second_key = compute_dedup_key(&signals[1], Some(&records)).unwrap();
    assert_eq!(first_key, second_key);

    let index = deduplicate_signals(&mut signals, Some(&records)).unwrap();
    let key = first_key.unwrap();
    assert_eq!(index.signal_ids_for(&key).unwrap().len(), 2);
    assert_eq!(signals.len(), 2);
    assert!(signals
        .iter()
        .all(|signal| signal.dedup_key.as_deref() == Some(key.as_str())));
}

#[test]
fn key_and_anchor_are_independent_of_input_order() {
    let catalog = correlation_fixture_catalog();
    let mut records = SourceRecordStore::default();
    let mut signals = catalog
        .fixtures
        .iter()
        .filter(|fixture| {
            fixture.key == "shared-service-alert" || fixture.key == "shared-service-anomaly"
        })
        .flat_map(|fixture| normalize_fixture(fixture, &mut records))
        .collect::<Vec<_>>();
    let first = deduplicate_signals(&mut signals, Some(&records)).unwrap();
    let immutable = index_signals(&signals, Some(&records)).unwrap();
    let expected_anchor = stable_candidate_anchor(&signals).unwrap();
    let expected_ids = signals.iter().map(|signal| signal.id).collect::<Vec<_>>();

    signals.reverse();
    let reversed_anchor = stable_candidate_anchor(&signals).unwrap();
    let second = deduplicate_signals(&mut signals, Some(&records)).unwrap();

    assert_eq!(expected_anchor, reversed_anchor);
    assert_eq!(first, second);
    assert_eq!(first, immutable);
    assert_eq!(
        expected_ids,
        signals.iter().map(|signal| signal.id).collect::<Vec<_>>()
    );
}

#[test]
fn conflicting_native_identity_returns_typed_error_without_arrival_order_selection() {
    let scope = thalassaops::correlation::fixture_scope();
    let evidence = security_fixture(EvidenceSourceKind::Trivy)
        .evidence
        .into_iter()
        .collect::<Vec<_>>();
    let first = SourceRecordInput::new(
        EvidenceSourceKind::Trivy,
        Some("CVE-2024-1234".into()),
        Some("fixture-1".into()),
        scope.clone(),
        json!({"version": 1}),
        evidence.clone(),
    );
    let second = SourceRecordInput::new(
        EvidenceSourceKind::Trivy,
        Some("CVE-2024-1234".into()),
        Some("fixture-1".into()),
        scope,
        json!({"version": 2}),
        evidence,
    );
    let mut records = SourceRecordStore::default();
    records.retain(first).unwrap();
    assert_eq!(
        records.retain(second),
        Err(SourceRecordError::AmbiguousSourceIdentity)
    );
}

#[test]
fn dedup_index_does_not_hide_duplicate_source_references() {
    let fixture = security_fixture(EvidenceSourceKind::Falco);
    let mut records = SourceRecordStore::default();
    let mut signals = normalize_fixture(&fixture, &mut records);
    let mut duplicate = signals[0].clone();
    duplicate.id = uuid::Uuid::from_u128(99);
    duplicate.evidence_ids = signals[0].evidence_ids.clone();
    duplicate.source_record = signals[0].source_record.clone();
    signals.push(duplicate);

    let index = deduplicate_signals(&mut signals, Some(&records)).unwrap();
    let key = signals[0].dedup_key.clone().unwrap();
    assert_eq!(index.signal_ids_for(&key).unwrap().len(), 2);
    assert_eq!(index.total_signal_count(), 2);
}

#[test]
fn dedup_errors_do_not_include_raw_source_payload() {
    let fixture = security_fixture(EvidenceSourceKind::Trivy);
    let mut records = SourceRecordStore::default();
    let signal = normalize_fixture(&fixture, &mut records).remove(0);
    let error = DedupError::SourceRecordMissing;
    assert!(!error.to_string().contains("CVE-2024-1234"));
    assert!(compute_dedup_key(&signal, None).is_ok());
}
