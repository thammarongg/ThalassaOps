// SPDX-License-Identifier: Apache-2.0

use serde_json::json;
use thalassa_domain::{
    BusinessImpact, ImpactDimensions, ImpactLevel, ImpactTrajectory, IncidentDisposition,
    IncidentError, IncidentListRequest, IncidentRoleRequest, IncidentSeverity, IncidentSourceKind,
    IncidentStatus, IncidentTimelinePage, IncidentTimelineRequest, IncidentTriggerInput,
};
use uuid::Uuid;

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

#[test]
fn incident_pagination_requests_reject_limits_cursors_and_zero_sequences() {
    let valid = IncidentListRequest {
        cursor: None,
        limit: 100,
    };
    assert!(valid.validate().is_ok());

    for limit in [0u16, 101u16] {
        let request = IncidentListRequest {
            cursor: None,
            limit,
        };
        assert!(matches!(
            request.validate(),
            Err(IncidentError::InvalidPagination)
        ));
    }

    let empty_cursor = IncidentListRequest {
        cursor: Some("   ".into()),
        limit: 10,
    };
    assert!(matches!(
        empty_cursor.validate(),
        Err(IncidentError::InvalidPagination)
    ));

    let control_cursor = IncidentListRequest {
        cursor: Some("cursor\nline".into()),
        limit: 10,
    };
    assert!(matches!(
        control_cursor.validate(),
        Err(IncidentError::InvalidPagination)
    ));

    let oversized_cursor = IncidentListRequest {
        cursor: Some("x".repeat(201)),
        limit: 10,
    };
    assert!(matches!(
        oversized_cursor.validate(),
        Err(IncidentError::InvalidPagination)
    ));

    let bounded_cursor = IncidentListRequest {
        cursor: Some("x".repeat(200)),
        limit: 1,
    };
    assert!(bounded_cursor.validate().is_ok());

    let zero_sequence = IncidentTimelineRequest {
        incident_id: Uuid::from_u128(0x21),
        after_sequence: Some(0),
        limit: 25,
    };
    assert!(matches!(
        zero_sequence.validate(),
        Err(IncidentError::InvalidEventSequence)
    ));

    let valid_timeline = IncidentTimelineRequest {
        incident_id: Uuid::from_u128(0x21),
        after_sequence: Some(1),
        limit: 1,
    };
    assert!(valid_timeline.validate().is_ok());
}

#[test]
fn incident_trigger_inputs_deserialize_exactly_the_six_tags() {
    for kind in [
        "alert",
        "anomaly",
        "scheduled_health_check",
        "vulnerability_finding",
    ] {
        let input: IncidentTriggerInput =
            serde_json::from_value(json!({ "kind": kind, "source_id": "source-1" })).unwrap();
        assert_eq!(
            serde_json::to_value(&input).unwrap(),
            json!({ "kind": kind, "source_id": "source-1" })
        );
    }

    let user_report: IncidentTriggerInput = serde_json::from_value(json!({
        "kind": "user_report",
        "reporter_id": "22222222-2222-4222-8222-222222222222",
        "observed_at": "2026-08-30T09:00:00Z",
        "summary": "customers report failures",
        "scope": { "resource_ids": [] }
    }))
    .unwrap();
    assert_eq!(
        serde_json::to_value(&user_report).unwrap()["kind"],
        json!("user_report")
    );

    let manual_report: IncidentTriggerInput = serde_json::from_value(json!({
        "kind": "manual_report",
        "observed_at": "2026-08-30T09:00:00Z",
        "summary": "checkout is down",
        "scope": { "resource_ids": [] }
    }))
    .unwrap();
    assert_eq!(
        serde_json::to_value(&manual_report).unwrap()["kind"],
        json!("manual_report")
    );

    assert!(serde_json::from_value::<IncidentTriggerInput>(json!({
        "kind": "correlation_candidate",
        "source_id": "candidate-1"
    }))
    .is_err());
}

#[test]
fn incident_requests_and_pages_keep_snake_case_wire_fields() {
    let timeline = IncidentTimelineRequest {
        incident_id: Uuid::from_u128(0x21),
        after_sequence: Some(3),
        limit: 25,
    };
    let value = serde_json::to_value(&timeline).unwrap();
    assert_eq!(
        value["incident_id"],
        json!(timeline.incident_id.to_string())
    );
    assert_eq!(value["after_sequence"], json!(3));
    assert_eq!(value["limit"], json!(25));
    assert_eq!(
        serde_json::from_value::<IncidentTimelineRequest>(value).unwrap(),
        timeline
    );

    let role_request = IncidentRoleRequest {
        incident_id: Uuid::from_u128(0x21),
        expected_version: 4,
        command: thalassa_domain::IncidentRoleCommand::Release {
            role: thalassa_domain::IncidentRole::Owner,
            principal_id: Uuid::from_u128(0x31),
        },
    };
    let value = serde_json::to_value(&role_request).unwrap();
    for field in ["incident_id", "expected_version", "command"] {
        assert!(value.get(field).is_some(), "{field} must stay snake_case");
    }

    let page = IncidentTimelinePage {
        incident_id: Uuid::from_u128(0x21),
        events: vec![],
        next_sequence: None,
    };
    let value = serde_json::to_value(&page).unwrap();
    for field in ["incident_id", "events", "next_sequence"] {
        assert!(value.get(field).is_some(), "{field} must stay snake_case");
    }
    assert_eq!(value["next_sequence"], json!(null));
}

#[test]
fn commented_event_wire_names_are_stable() {
    let payload =
        thalassa_domain::IncidentTimelinePayload::Commented(thalassa_domain::CommentedPayload {
            body: "note".into(),
        });
    let encoded = serde_json::to_value(&payload).expect("payload encodes");
    assert_eq!(encoded["kind"], json!("commented"));
    assert_eq!(encoded["data"]["body"], json!("note"));
    assert_eq!(
        serde_json::from_value::<thalassa_domain::IncidentTimelinePayload>(encoded).unwrap(),
        payload
    );

    assert_eq!(
        serde_json::to_value(thalassa_domain::IncidentEventKind::Commented).expect("kind encodes"),
        json!("commented")
    );
    assert_eq!(
        serde_json::from_value::<thalassa_domain::IncidentEventKind>(json!("commented")).unwrap(),
        thalassa_domain::IncidentEventKind::Commented
    );
}
