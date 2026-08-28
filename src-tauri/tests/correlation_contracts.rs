// SPDX-License-Identifier: Apache-2.0

use thalassa_domain::EvidenceSourceKind;
use thalassaops::correlation::{correlation_fixture_catalog, fixture_time, FIXTURE_CLOCK};

#[test]
fn correlation_fixture_catalog_is_deterministic_and_source_preserving() {
    let first = correlation_fixture_catalog();
    let second = correlation_fixture_catalog();
    assert_eq!(first, second);
    assert_eq!(fixture_time().timestamp(), 1_787_907_600);
    assert_eq!(FIXTURE_CLOCK, "2026-08-28T09:00:00Z");
    assert!(first.validate().is_ok());

    let security = first.security_fixtures().collect::<Vec<_>>();
    assert!(security.len() >= 4);
    for source_kind in [
        EvidenceSourceKind::Trivy,
        EvidenceSourceKind::Falco,
        EvidenceSourceKind::Kyverno,
        EvidenceSourceKind::OpaGatekeeper,
    ] {
        assert!(security
            .iter()
            .any(|fixture| fixture.source_kind == source_kind));
    }
    for fixture in security {
        assert!(serde_json::to_string(&fixture.recorded_json)
            .unwrap()
            .contains("vendor_extension"));
        assert_eq!(fixture.evidence.len(), 1);
    }
}

#[test]
fn catalog_contains_operational_and_grouping_inputs() {
    let catalog = correlation_fixture_catalog();
    let keys = catalog
        .fixtures
        .iter()
        .map(|fixture| fixture.key.as_str())
        .collect::<Vec<_>>();
    assert!(keys.contains(&"alert-checkout"));
    assert!(keys.contains(&"anomaly-checkout-errors"));
    assert!(keys.contains(&"late-anomaly-checkout"));
    assert!(keys.contains(&"shared-service-alert"));
    assert!(keys.contains(&"shared-deployment-finding"));
    assert!(keys.contains(&"topology-left"));
    assert!(keys.contains(&"topology-right"));
    assert_eq!(catalog.suppression_rules.len(), 2);
    assert_eq!(catalog.maintenance_windows.len(), 1);
}
