// SPDX-License-Identifier: Apache-2.0

use serde_json::json;
use thalassa_domain::{
    EvidenceRedaction, EvidenceRef, EvidenceSourceKind, FindingAssetKind, FindingSeverity,
    SignalKind, SignalPayload, SignalTargetKind, SourceRecordRef,
};
use thalassaops::correlation::adapters::{
    normalize_falco, normalize_gatekeeper, normalize_kyverno, normalize_security, normalize_trivy,
    FalcoAdapter, GatekeeperAdapter, KyvernoAdapter, SignalAdapter, TrivyAdapter,
};
use thalassaops::correlation::{correlation_fixture_catalog, fixture_scope, SourceRecordStore};

fn security_fixture(
    source: EvidenceSourceKind,
) -> thalassaops::correlation::ReplayableSignalFixture {
    correlation_fixture_catalog()
        .fixtures
        .into_iter()
        .find(|fixture| fixture.source_kind == source)
        .expect("the committed security fixture exists")
}

fn finding(signal: &thalassa_domain::Signal) -> &thalassa_domain::VulnerabilityFinding {
    let SignalPayload::SecurityFinding { finding } = &signal.payload else {
        panic!("security adapter must emit a security finding payload")
    };
    finding
}

fn evidence(id: &str, source: EvidenceSourceKind) -> EvidenceRef {
    EvidenceRef {
        id: id.into(),
        source_kind: source,
        connector_id: Some("fixture-catalog".into()),
        scope: fixture_scope(),
        endpoint: format!("fixture://security/{source:?}"),
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

#[test]
fn trivy_fixture_maps_to_a_source_preserving_container_finding() {
    let fixture = security_fixture(EvidenceSourceKind::Trivy);
    let mut records = SourceRecordStore::default();
    let signal = TrivyAdapter
        .normalize(&fixture, &mut records)
        .expect("Trivy fixture should normalize")
        .remove(0);
    let finding = finding(&signal);

    assert_eq!(signal.kind, SignalKind::SecurityFinding);
    assert_eq!(signal.source, EvidenceSourceKind::Trivy);
    assert_eq!(finding.source, EvidenceSourceKind::Trivy);
    assert_eq!(finding.asset.kind, FindingAssetKind::ContainerImage);
    assert_eq!(finding.asset.target.kind, SignalTargetKind::Resource);
    assert_eq!(finding.asset.target.id, "checkout:2026.08.28.1");
    assert_eq!(finding.severity, Some(FindingSeverity::High));
    assert_eq!(finding.cvss_score, Some(8.1));
    assert_eq!(finding.exploitability, None);
    assert_eq!(
        signal.source_record.native_id.as_deref(),
        Some(
            "vulnerability_id=CVE-2024-1234;package=libcheckout;path=;image=checkout:2026.08.28.1"
        )
    );
    assert_eq!(signal.source_record.revision.as_deref(), Some("fixture-1"));
    assert_eq!(
        signal.evidence_ids,
        fixture
            .evidence
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(finding.evidence_ids, signal.evidence_ids);
    assert!(records.get(&signal.source_record).is_some());
    assert!(signal.validate().is_ok());
}

#[test]
fn falco_fixture_maps_priority_and_exact_runtime_target() {
    let fixture = security_fixture(EvidenceSourceKind::Falco);
    let mut records = SourceRecordStore::default();
    let signal = normalize_falco(&fixture, &mut records)
        .expect("Falco fixture should normalize")
        .remove(0);
    let finding = finding(&signal);

    assert_eq!(finding.asset.kind, FindingAssetKind::RuntimeResource);
    assert_eq!(finding.asset.target.kind, SignalTargetKind::Resource);
    assert_eq!(finding.asset.target.id, "pod/prod/checkout-7d9c");
    assert_eq!(finding.severity, Some(FindingSeverity::Critical));
    assert_eq!(signal.observed_at.as_deref(), Some("2026-08-28T08:58:30Z"));
    assert_eq!(
        signal.source_record.native_id.as_deref(),
        Some("falco-event-1")
    );
    assert_eq!(finding.evidence_ids, signal.evidence_ids);
    assert_eq!(
        records.get(&signal.source_record).unwrap().payload()["output_fields"]["proc.name"],
        "checkout-worker"
    );
}

#[test]
fn kyverno_and_gatekeeper_fixtures_map_policy_subjects() {
    let mut records = SourceRecordStore::default();
    let kyverno = normalize_kyverno(&security_fixture(EvidenceSourceKind::Kyverno), &mut records)
        .expect("Kyverno fixture should normalize")
        .remove(0);
    let gatekeeper = normalize_gatekeeper(
        &security_fixture(EvidenceSourceKind::OpaGatekeeper),
        &mut records,
    )
    .expect("Gatekeeper fixture should normalize")
    .remove(0);

    assert_eq!(
        finding(&kyverno).asset.kind,
        FindingAssetKind::PolicySubject
    );
    assert_eq!(
        finding(&kyverno).asset.target.kind,
        SignalTargetKind::Deployment
    );
    assert_eq!(
        finding(&kyverno).asset.target.id,
        "deployment/prod/checkout"
    );
    assert_eq!(finding(&kyverno).severity, Some(FindingSeverity::High));
    assert_eq!(
        kyverno.source_record.native_id.as_deref(),
        Some(
            "policy=disallow-host-path;rule=host-path;namespace=prod;kind=Deployment;name=checkout;path=spec.template.spec.volumes[0].hostPath"
        )
    );
    assert!(records
        .get(&kyverno.source_record)
        .unwrap()
        .payload()
        .get("violation_path")
        .is_some());

    assert_eq!(
        finding(&gatekeeper).asset.kind,
        FindingAssetKind::PolicySubject
    );
    assert_eq!(
        finding(&gatekeeper).asset.target.kind,
        SignalTargetKind::Deployment
    );
    assert_eq!(
        finding(&gatekeeper).asset.target.id,
        "deployment/prod/checkout"
    );
    assert_eq!(finding(&gatekeeper).severity, Some(FindingSeverity::Medium));
    assert_eq!(
        gatekeeper.source_record.native_id.as_deref(),
        Some(
            "template=k8srequiredlabels;constraint=checkout-required-labels;namespace=prod;kind=Deployment;name=checkout;path=metadata.labels.service-tier"
        )
    );
    assert!(records
        .get(&gatekeeper.source_record)
        .unwrap()
        .payload()
        .get("vendor_extension")
        .is_some());
}

#[test]
fn kubernetes_security_targets_keep_namespace_in_exact_identity() {
    let prod = security_fixture(EvidenceSourceKind::Kyverno);
    let mut staging = prod.clone();
    staging.recorded_json["resource"]["namespace"] = json!("staging");
    staging.evidence[0].id = "evidence-security-kyverno-staging".into();

    let prod_signal = normalize_kyverno(&prod, &mut SourceRecordStore::default())
        .expect("production Kyverno fixture should normalize")
        .remove(0);
    let staging_signal = normalize_kyverno(&staging, &mut SourceRecordStore::default())
        .expect("staging Kyverno fixture should normalize")
        .remove(0);

    assert_eq!(
        finding(&prod_signal).asset.target.id,
        "deployment/prod/checkout"
    );
    assert_eq!(
        finding(&staging_signal).asset.target.id,
        "deployment/staging/checkout"
    );
    assert_ne!(
        finding(&prod_signal).asset.target,
        finding(&staging_signal).asset.target
    );

    let falco_prod = security_fixture(EvidenceSourceKind::Falco);
    let mut falco_staging = falco_prod.clone();
    falco_staging.recorded_json["target"]["namespace"] = json!("staging");
    falco_staging.evidence[0].id = "evidence-security-falco-staging".into();
    let prod_signal = normalize_falco(&falco_prod, &mut SourceRecordStore::default())
        .expect("production Falco fixture should normalize")
        .remove(0);
    let staging_signal = normalize_falco(&falco_staging, &mut SourceRecordStore::default())
        .expect("staging Falco fixture should normalize")
        .remove(0);
    assert_eq!(
        finding(&prod_signal).asset.target.id,
        "pod/prod/checkout-7d9c"
    );
    assert_eq!(
        finding(&staging_signal).asset.target.id,
        "pod/staging/checkout-7d9c"
    );
}

#[test]
fn repeated_policy_reports_keep_each_resource_revision() {
    let kyverno = security_fixture(EvidenceSourceKind::Kyverno);
    let mut kyverno_second = kyverno.clone();
    kyverno_second.recorded_json["resource"]["name"] = json!("payments");
    kyverno_second.evidence[0].id = "evidence-security-kyverno-payments".into();
    let mut records = SourceRecordStore::default();
    let first = normalize_kyverno(&kyverno, &mut records).expect("first Kyverno report");
    let second = normalize_kyverno(&kyverno_second, &mut records)
        .expect("second Kyverno report must not collide by policy name");
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_eq!(records.len(), 2);
    assert_ne!(
        first[0].source_record.native_id,
        second[0].source_record.native_id
    );

    let gatekeeper = security_fixture(EvidenceSourceKind::OpaGatekeeper);
    let mut gatekeeper_second = gatekeeper.clone();
    gatekeeper_second.recorded_json["resource"]["name"] = json!("payments");
    gatekeeper_second.evidence[0].id = "evidence-security-gatekeeper-payments".into();
    let mut records = SourceRecordStore::default();
    let first = normalize_gatekeeper(&gatekeeper, &mut records).expect("first Gatekeeper report");
    let second = normalize_gatekeeper(&gatekeeper_second, &mut records)
        .expect("second Gatekeeper report must not collide by constraint name");
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_eq!(records.len(), 2);
    assert_ne!(
        first[0].source_record.native_id,
        second[0].source_record.native_id
    );
}

#[test]
fn repeated_trivy_packages_keep_each_vulnerability_source_record() {
    let first = security_fixture(EvidenceSourceKind::Trivy);
    let mut second = first.clone();
    second.recorded_json["Results"][0]["PkgName"] = json!("libpayments");
    second.evidence[0].id = "evidence-security-trivy-payments".into();
    let mut records = SourceRecordStore::default();
    let first_signal = normalize_trivy(&first, &mut records)
        .expect("first Trivy result")
        .remove(0);
    let second_signal = normalize_trivy(&second, &mut records)
        .expect("same CVE in another package must not collide")
        .remove(0);
    assert_eq!(records.len(), 2);
    assert_ne!(
        first_signal.source_record.native_id,
        second_signal.source_record.native_id
    );
}

#[test]
fn security_dispatch_registers_all_four_sources() {
    let catalog = correlation_fixture_catalog();
    let mut records = SourceRecordStore::default();
    let mut signals = Vec::new();
    for source in [
        EvidenceSourceKind::Trivy,
        EvidenceSourceKind::Falco,
        EvidenceSourceKind::Kyverno,
        EvidenceSourceKind::OpaGatekeeper,
    ] {
        signals.extend(
            normalize_security(
                catalog
                    .fixtures
                    .iter()
                    .find(|fixture| fixture.source_kind == source)
                    .unwrap(),
                &mut records,
            )
            .unwrap(),
        );
    }
    assert_eq!(signals.len(), 4);
    assert_eq!(records.len(), 4);
    assert!(signals.iter().all(|signal| signal.validate().is_ok()));
}

#[test]
fn trivy_adapter_accepts_the_mixed_deployment_fixture_without_flattening_it() {
    let fixture = correlation_fixture_catalog()
        .fixtures
        .into_iter()
        .find(|fixture| fixture.key == "shared-deployment-finding")
        .expect("the mixed Trivy deployment fixture exists");
    let mut records = SourceRecordStore::default();
    let signal = normalize_trivy(&fixture, &mut records).unwrap().remove(0);
    let finding = finding(&signal);
    assert_eq!(finding.asset.kind, FindingAssetKind::ContainerImage);
    assert_eq!(finding.asset.target.kind, SignalTargetKind::Deployment);
    assert_eq!(finding.asset.target.id, "deployment/checkout");
    assert_eq!(signal.source_record.native_id.as_deref(), None);
    assert_eq!(
        records.get(&signal.source_record).unwrap().payload(),
        &fixture.recorded_json
    );
}

#[test]
fn trivy_missing_package_stays_normalized_without_a_fabricated_dedup_key() {
    let mut fixture = security_fixture(EvidenceSourceKind::Trivy);
    fixture.recorded_json["Results"][0]
        .as_object_mut()
        .expect("Trivy result is an object")
        .remove("PkgName");
    let mut records = SourceRecordStore::default();
    let signal = normalize_trivy(&fixture, &mut records)
        .expect("missing optional package does not invalidate the source record")
        .remove(0);

    assert_eq!(signal.dedup_key, None);
    assert!(records.get(&signal.source_record).is_some());
    assert!(signal.validate().is_ok());
}

#[test]
fn malformed_security_payload_is_a_typed_error_without_a_partial_signal() {
    let mut fixture = security_fixture(EvidenceSourceKind::Kyverno);
    fixture.recorded_json = json!({
        "policy": "disallow-host-path",
        "rule": "host-path",
        "result": "fail",
        "severity": ["high"],
        "resource": {"namespace": "prod", "kind": "Deployment", "name": "checkout"},
        "vendor_extension": {"capture": "synthetic"}
    });
    let mut records = SourceRecordStore::default();
    let result = KyvernoAdapter.normalize(&fixture, &mut records);
    assert!(result.is_err());
    assert!(result
        .as_ref()
        .unwrap_err()
        .to_string()
        .contains("malformed"));
}

#[test]
fn source_evidence_and_unknown_fields_survive_security_normalization() {
    let mut fixture = security_fixture(EvidenceSourceKind::OpaGatekeeper);
    fixture.evidence.push(evidence(
        "evidence-gatekeeper-extra",
        EvidenceSourceKind::OpaGatekeeper,
    ));
    fixture.recorded_json["vendor_extension"]["unknown"] = json!({"nested": true});
    let mut records = SourceRecordStore::default();
    let signal = normalize_security(&fixture, &mut records)
        .unwrap()
        .remove(0);
    let retained = records.get(&signal.source_record).unwrap();
    assert_eq!(
        retained.payload()["vendor_extension"]["unknown"]["nested"],
        true
    );
    assert_eq!(signal.source_record.evidence_ids.len(), 2);
    assert!(signal
        .source_record
        .evidence_ids
        .iter()
        .any(|id| id == "evidence-gatekeeper-extra"));
}

#[test]
fn unsupported_cvss_and_unsafe_targets_fail_closed() {
    let mut trivy = security_fixture(EvidenceSourceKind::Trivy);
    trivy.recorded_json["Results"][0]["CVSS"]["nvd"]["V3Score"] = json!(11.0);
    let mut records = SourceRecordStore::default();
    assert!(normalize_trivy(&trivy, &mut records).is_err());

    let mut falco = security_fixture(EvidenceSourceKind::Falco);
    falco.recorded_json["target"]["pod"] = json!("prod token");
    assert!(normalize_falco(&falco, &mut SourceRecordStore::default()).is_err());
}

#[test]
fn security_adapters_do_not_require_a_source_client_or_query() {
    let adapters: Vec<Box<dyn SignalAdapter>> = vec![
        Box::new(TrivyAdapter),
        Box::new(FalcoAdapter),
        Box::new(KyvernoAdapter),
        Box::new(GatekeeperAdapter),
    ];
    for (adapter, source) in adapters.into_iter().zip([
        EvidenceSourceKind::Trivy,
        EvidenceSourceKind::Falco,
        EvidenceSourceKind::Kyverno,
        EvidenceSourceKind::OpaGatekeeper,
    ]) {
        assert_eq!(adapter.source_kind(), source);
        let fixture = security_fixture(source);
        let mut records = SourceRecordStore::default();
        assert!(adapter.normalize(&fixture, &mut records).is_ok());
    }
}

#[test]
fn source_record_reference_has_evidence_closure_for_each_finding() {
    for source in [
        EvidenceSourceKind::Trivy,
        EvidenceSourceKind::Falco,
        EvidenceSourceKind::Kyverno,
        EvidenceSourceKind::OpaGatekeeper,
    ] {
        let fixture = security_fixture(source);
        let mut records = SourceRecordStore::default();
        let signal = normalize_security(&fixture, &mut records)
            .unwrap()
            .remove(0);
        let source_record: &SourceRecordRef = &signal.source_record;
        assert!(source_record
            .evidence_ids
            .iter()
            .all(|id| signal.evidence_ids.contains(id)));
        assert_eq!(finding(&signal).evidence_ids, signal.evidence_ids);
    }
}

#[test]
fn explicit_unknown_security_severity_is_not_dropped_and_absent_cvss_stays_none() {
    let mut fixture = security_fixture(EvidenceSourceKind::OpaGatekeeper);
    fixture.recorded_json["severity"] = json!("unknown");
    fixture.recorded_json["exploitability"] = json!("unknown");
    let mut records = SourceRecordStore::default();
    let signal = normalize_gatekeeper(&fixture, &mut records)
        .unwrap()
        .remove(0);
    let security_finding = finding(&signal);
    assert_eq!(security_finding.severity, Some(FindingSeverity::Unknown));
    assert_eq!(
        security_finding.exploitability,
        Some(thalassa_domain::Exploitability::Unknown)
    );
    assert_eq!(security_finding.cvss_score, None);

    let mut falco = security_fixture(EvidenceSourceKind::Falco);
    falco
        .recorded_json
        .as_object_mut()
        .unwrap()
        .remove("priority");
    let signal = normalize_falco(&falco, &mut SourceRecordStore::default())
        .unwrap()
        .remove(0);
    assert_eq!(finding(&signal).severity, None);
}
