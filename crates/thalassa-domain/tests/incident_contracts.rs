// SPDX-License-Identifier: Apache-2.0

use serde_json::json;
use thalassa_domain::{
    BusinessImpact, ImpactDimensions, ImpactLevel, ImpactTrajectory, IncidentDisposition,
    IncidentError, IncidentSeverity, IncidentSourceKind, IncidentStatus,
};

fn impact(dimensions: ImpactDimensions) -> BusinessImpact {
    BusinessImpact {
        level: dimensions.highest_level(),
        summary: "Checkout unavailable".into(),
        customer_scope: "production customers".into(),
        service_criticality: "tier_0".into(),
        trajectory: dimensions.trajectory,
        dimensions,
        evidence_ids: vec!["evidence-checkout-alert".into()],
    }
}

#[test]
fn incident_source_kinds_have_exact_wire_values() {
    for (kind, wire) in [
        (IncidentSourceKind::Alert, "alert"),
        (IncidentSourceKind::Anomaly, "anomaly"),
        (IncidentSourceKind::UserReport, "user_report"),
        (
            IncidentSourceKind::ScheduledHealthCheck,
            "scheduled_health_check",
        ),
        (
            IncidentSourceKind::VulnerabilityFinding,
            "vulnerability_finding",
        ),
        (IncidentSourceKind::ManualReport, "manual_report"),
    ] {
        assert_eq!(serde_json::to_value(kind).unwrap(), json!(wire));
    }
}

#[test]
fn incident_status_severity_and_disposition_use_explicit_wire_values() {
    for (status, wire) in [
        (IncidentStatus::Detected, "detected"),
        (IncidentStatus::Triage, "triage"),
        (IncidentStatus::Investigating, "investigating"),
        (IncidentStatus::Mitigating, "mitigating"),
        (IncidentStatus::Monitoring, "monitoring"),
        (IncidentStatus::Resolved, "resolved"),
        (IncidentStatus::Closed, "closed"),
        (IncidentStatus::Reopened, "reopened"),
    ] {
        assert_eq!(serde_json::to_value(status).unwrap(), json!(wire));
    }
    for (severity, wire) in [
        (IncidentSeverity::S1, "S1"),
        (IncidentSeverity::S2, "S2"),
        (IncidentSeverity::S3, "S3"),
        (IncidentSeverity::S4, "S4"),
        (IncidentSeverity::S5, "S5"),
    ] {
        assert_eq!(serde_json::to_value(severity).unwrap(), json!(wire));
    }
    for (disposition, wire) in [
        (IncidentDisposition::Duplicate, "duplicate"),
        (IncidentDisposition::FalsePositive, "false_positive"),
        (IncidentDisposition::Suppressed, "suppressed"),
        (IncidentDisposition::Cancelled, "cancelled"),
        (IncidentDisposition::Informational, "informational"),
    ] {
        assert_eq!(serde_json::to_value(disposition).unwrap(), json!(wire));
    }
}

#[test]
fn highest_impact_dimension_derives_initial_severity() {
    let dimensions = ImpactDimensions {
        availability: ImpactLevel::High,
        customer_reach: ImpactLevel::Medium,
        business_criticality: ImpactLevel::High,
        data_integrity: ImpactLevel::None,
        security_privacy: ImpactLevel::None,
        financial_contractual: ImpactLevel::Low,
        trajectory: ImpactTrajectory::Stable,
        production: true,
    };
    assert_eq!(
        impact(dimensions).derive_severity().unwrap(),
        IncidentSeverity::S2
    );
}

#[test]
fn impact_ranking_maps_each_confirmed_level_to_the_severity_ladder() {
    for (level, expected) in [
        (ImpactLevel::Critical, IncidentSeverity::S1),
        (ImpactLevel::High, IncidentSeverity::S2),
        (ImpactLevel::Medium, IncidentSeverity::S3),
        (ImpactLevel::Low, IncidentSeverity::S4),
        (ImpactLevel::None, IncidentSeverity::S5),
    ] {
        let dimensions = ImpactDimensions {
            availability: level,
            customer_reach: ImpactLevel::None,
            business_criticality: ImpactLevel::None,
            data_integrity: ImpactLevel::None,
            security_privacy: ImpactLevel::None,
            financial_contractual: ImpactLevel::None,
            trajectory: ImpactTrajectory::Stable,
            production: false,
        };
        assert_eq!(impact(dimensions).derive_severity().unwrap(), expected);
    }
}

#[test]
fn rapidly_expanding_unknown_production_scope_is_at_least_s2() {
    let dimensions = ImpactDimensions {
        availability: ImpactLevel::Unknown,
        customer_reach: ImpactLevel::Unknown,
        business_criticality: ImpactLevel::High,
        data_integrity: ImpactLevel::None,
        security_privacy: ImpactLevel::None,
        financial_contractual: ImpactLevel::None,
        trajectory: ImpactTrajectory::Expanding,
        production: true,
    };
    assert_eq!(
        impact(dimensions).derive_severity().unwrap(),
        IncidentSeverity::S2
    );
}

#[test]
fn unknown_impact_stays_s5_unless_expanding_production_scope_is_rapid() {
    let dimensions = ImpactDimensions {
        availability: ImpactLevel::Unknown,
        customer_reach: ImpactLevel::Unknown,
        business_criticality: ImpactLevel::Unknown,
        data_integrity: ImpactLevel::Unknown,
        security_privacy: ImpactLevel::Unknown,
        financial_contractual: ImpactLevel::Unknown,
        trajectory: ImpactTrajectory::Stable,
        production: true,
    };
    assert_eq!(
        impact(dimensions).derive_severity().unwrap(),
        IncidentSeverity::S5
    );

    let expanding = ImpactDimensions {
        trajectory: ImpactTrajectory::Expanding,
        ..dimensions
    };
    assert_eq!(
        impact(expanding).derive_severity().unwrap(),
        IncidentSeverity::S2
    );
}

#[test]
fn business_impact_rejects_level_mismatch_and_missing_evidence() {
    let dimensions = ImpactDimensions {
        availability: ImpactLevel::High,
        customer_reach: ImpactLevel::Medium,
        business_criticality: ImpactLevel::High,
        data_integrity: ImpactLevel::None,
        security_privacy: ImpactLevel::None,
        financial_contractual: ImpactLevel::Low,
        trajectory: ImpactTrajectory::Stable,
        production: true,
    };
    let mut mismatched = impact(dimensions);
    mismatched.level = ImpactLevel::Low;
    assert!(matches!(
        mismatched.derive_severity(),
        Err(IncidentError::ImpactLevelMismatch)
    ));

    let mut without_evidence = impact(dimensions);
    without_evidence.evidence_ids.clear();
    assert!(matches!(
        without_evidence.derive_severity(),
        Err(IncidentError::InvalidEvidence)
    ));
}

#[test]
fn incident_text_rejects_secrets_controls_and_oversize_input() {
    assert!(thalassa_domain::validate_incident_text("ok\nline", 64).is_err());
    assert!(thalassa_domain::validate_incident_text("authorization: bearer abc", 64).is_err());
    assert!(thalassa_domain::validate_incident_text(&"x".repeat(65), 64).is_err());
    assert!(thalassa_domain::validate_incident_text("bounded safe summary", 64).is_ok());
}

#[test]
fn single_dimension_dimensions_match_their_primary_level() {
    for level in [
        ImpactLevel::Critical,
        ImpactLevel::High,
        ImpactLevel::Medium,
        ImpactLevel::Low,
        ImpactLevel::None,
    ] {
        let dimensions = ImpactDimensions::single_dimension(level, ImpactTrajectory::Unknown);
        assert_eq!(dimensions.highest_level(), level);
        assert!(dimensions.production);
    }
}

#[test]
fn unknown_single_dimension_stays_consistent_and_within_severity_floors() {
    let stable = ImpactDimensions::single_dimension(ImpactLevel::Unknown, ImpactTrajectory::Stable);
    assert_eq!(stable.highest_level(), ImpactLevel::Unknown);
    assert_eq!(
        impact(stable).derive_severity().unwrap(),
        IncidentSeverity::S5
    );

    let expanding =
        ImpactDimensions::single_dimension(ImpactLevel::Unknown, ImpactTrajectory::Expanding);
    assert_eq!(
        impact(expanding).derive_severity().unwrap(),
        IncidentSeverity::S2
    );
}
