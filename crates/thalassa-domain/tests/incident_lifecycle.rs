// SPDX-License-Identifier: Apache-2.0

use chrono::{TimeZone, Utc};
use serde_json::json;
use thalassa_domain::{
    BusinessImpact, ImpactDimensions, ImpactLevel, ImpactTrajectory, IncidentCreateCommand,
    IncidentDisposition, IncidentDispositionCommand, IncidentError, IncidentEventKind,
    IncidentReport, IncidentRole, IncidentRoleAssignment, IncidentRoleCommand, IncidentSeverity,
    IncidentSeverityCommand, IncidentSourceKind, IncidentStatus, IncidentTransition,
    IncidentTrigger, PrincipalId, ResourceScope, TriageContext,
};

const ACTOR: uuid::Uuid = uuid::Uuid::from_u128(0xa0);
const REQUEST: uuid::Uuid = uuid::Uuid::from_u128(0xb0);
const TEAM: uuid::Uuid = uuid::Uuid::from_u128(0xc0);
const WORKSPACE: uuid::Uuid = uuid::Uuid::from_u128(0xd0);
const OTHER_WORKSPACE: uuid::Uuid = uuid::Uuid::from_u128(0xd1);
const ORGANIZATION: uuid::Uuid = uuid::Uuid::from_u128(0xe0);
const OTHER_INCIDENT: uuid::Uuid = uuid::Uuid::from_u128(0xf0);
const COMMANDER: PrincipalId = uuid::Uuid::from_u128(0xa1);
const STAKEHOLDER_ONE: PrincipalId = uuid::Uuid::from_u128(0xa2);
const STAKEHOLDER_TWO: PrincipalId = uuid::Uuid::from_u128(0xa3);
const RECURRENCE_SIGNAL: uuid::Uuid = uuid::Uuid::from_u128(0x516);

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 30, 9, 0, 0).unwrap()
}

fn scope() -> ResourceScope {
    ResourceScope::workspace(WORKSPACE, TEAM, ORGANIZATION)
}

fn business_impact() -> BusinessImpact {
    BusinessImpact {
        level: ImpactLevel::High,
        summary: "Checkout unavailable for customers".into(),
        customer_scope: "production customers".into(),
        service_criticality: "tier-0".into(),
        trajectory: ImpactTrajectory::Stable,
        dimensions: ImpactDimensions {
            availability: ImpactLevel::High,
            customer_reach: ImpactLevel::Medium,
            business_criticality: ImpactLevel::Medium,
            data_integrity: ImpactLevel::None,
            security_privacy: ImpactLevel::None,
            financial_contractual: ImpactLevel::Low,
            trajectory: ImpactTrajectory::Stable,
            production: true,
        },
        evidence_ids: vec!["evidence-checkout".into()],
    }
}

fn impact_with_level(level: ImpactLevel, summary: &str, evidence: &str) -> BusinessImpact {
    BusinessImpact {
        level,
        summary: summary.into(),
        customer_scope: "production customers".into(),
        service_criticality: "tier-0".into(),
        trajectory: ImpactTrajectory::Stable,
        dimensions: ImpactDimensions {
            availability: level,
            customer_reach: ImpactLevel::None,
            business_criticality: ImpactLevel::None,
            data_integrity: ImpactLevel::None,
            security_privacy: ImpactLevel::None,
            financial_contractual: ImpactLevel::None,
            trajectory: ImpactTrajectory::Stable,
            production: true,
        },
        evidence_ids: vec![evidence.into()],
    }
}

fn trigger() -> IncidentTrigger {
    IncidentTrigger {
        id: uuid::Uuid::from_u128(0x110),
        source_kind: IncidentSourceKind::ManualReport,
        source_id: "manual-report-1".into(),
        source_record_digest: None,
        scope: scope(),
        observed_at: now(),
        signal_id: None,
        evidence_ids: vec!["evidence-manual-report".into()],
        report: Some(IncidentReport {
            reporter_id: Some(ACTOR),
            summary: "Checkout is returning errors".into(),
        }),
    }
}

fn alert_trigger() -> IncidentTrigger {
    IncidentTrigger {
        id: uuid::Uuid::from_u128(0x111),
        source_kind: IncidentSourceKind::Alert,
        source_id: "alert-checkout".into(),
        source_record_digest: Some("sha256:abcdef0123456789".into()),
        scope: scope(),
        observed_at: now(),
        signal_id: None,
        evidence_ids: vec!["evidence-alert-checkout".into()],
        report: None,
    }
}

fn initial_roles() -> Vec<IncidentRoleAssignment> {
    vec![IncidentRoleAssignment {
        role: IncidentRole::Owner,
        principal_id: ACTOR,
        assigned_by: ACTOR,
        assigned_at: now(),
    }]
}

struct CreateFixture {
    command: IncidentCreateCommand,
    actor_id: PrincipalId,
    request_id: uuid::Uuid,
    now: chrono::DateTime<Utc>,
}

fn incident_create_fixture() -> CreateFixture {
    CreateFixture {
        command: IncidentCreateCommand {
            summary: "Checkout unavailable".into(),
            scope: scope(),
            owning_team_id: TEAM,
            triggers: vec![trigger()],
            business_impact: business_impact(),
            initial_roles: initial_roles(),
        },
        actor_id: ACTOR,
        request_id: REQUEST,
        now: now(),
    }
}

fn create_result(
    fixture: CreateFixture,
) -> Result<thalassa_domain::IncidentMutation, IncidentError> {
    thalassa_domain::Incident::create(
        fixture.command,
        fixture.actor_id,
        fixture.request_id,
        7,
        fixture.now,
    )
}

fn created() -> thalassa_domain::Incident {
    create_result(incident_create_fixture()).unwrap().incident
}

fn triage() -> IncidentTransition {
    triage_with(COMMANDER)
}

fn triage_with(owner: PrincipalId) -> IncidentTransition {
    IncidentTransition::Triage(TriageContext {
        business_impact: business_impact(),
        owner,
        duplicate_checked: true,
    })
}

fn investigating() -> IncidentTransition {
    IncidentTransition::Investigating(thalassa_domain::InvestigatingContext {
        note: "checkout pods are restarting".into(),
        evidence_ids: vec!["evidence-investigation".into()],
    })
}

fn mitigating() -> IncidentTransition {
    IncidentTransition::Mitigating(thalassa_domain::MitigatingContext {
        action_description: "roll back the checkout deployment".into(),
        executor: COMMANDER,
        expected_impact: "error rate returns to baseline".into(),
    })
}

fn monitoring() -> IncidentTransition {
    IncidentTransition::Monitoring(thalassa_domain::MonitoringContext {
        verification_seconds: 3_600,
        success_criteria: "error rate stays below one percent".into(),
        watch_owner: COMMANDER,
    })
}

fn resolved() -> IncidentTransition {
    IncidentTransition::Resolved(thalassa_domain::ResolvedContext {
        resolution_summary: "deployment rolled back".into(),
        evidence_ids: vec!["evidence-resolution".into()],
        impact_ended_at: now(),
    })
}

fn closed() -> IncidentTransition {
    IncidentTransition::Closed(thalassa_domain::ClosedContext {
        closure_notes: "verified over the monitoring window".into(),
        follow_up_ids: vec!["follow-up-1".into()],
    })
}

fn reopened_with_signal() -> IncidentTransition {
    IncidentTransition::Reopened(thalassa_domain::ReopenedContext {
        reason: "checkout errors returned after the window".into(),
        evidence_ids: vec![],
        recurrence_signal_id: Some(RECURRENCE_SIGNAL),
    })
}

fn transition(
    incident: &thalassa_domain::Incident,
    first_event_sequence: u64,
    step: IncidentTransition,
) -> Result<thalassa_domain::IncidentMutation, IncidentError> {
    incident.transition(
        incident.version,
        first_event_sequence,
        step,
        ACTOR,
        REQUEST,
        7,
        now(),
    )
}

#[test]
fn lifecycle_accepts_only_canonical_edges() {
    let allowed = [
        (IncidentStatus::Detected, IncidentStatus::Triage),
        (IncidentStatus::Triage, IncidentStatus::Investigating),
        (IncidentStatus::Investigating, IncidentStatus::Mitigating),
        (IncidentStatus::Mitigating, IncidentStatus::Monitoring),
        (IncidentStatus::Monitoring, IncidentStatus::Resolved),
        (IncidentStatus::Resolved, IncidentStatus::Closed),
        (IncidentStatus::Monitoring, IncidentStatus::Reopened),
        (IncidentStatus::Resolved, IncidentStatus::Reopened),
        (IncidentStatus::Closed, IncidentStatus::Reopened),
        (IncidentStatus::Reopened, IncidentStatus::Investigating),
    ];
    for (from, to) in allowed {
        assert!(
            IncidentTransition::edge_allowed(from, to),
            "{from:?} -> {to:?}"
        );
    }
    assert!(!IncidentTransition::edge_allowed(
        IncidentStatus::Detected,
        IncidentStatus::Resolved
    ));
    assert!(!IncidentTransition::edge_allowed(
        IncidentStatus::Triage,
        IncidentStatus::Triage
    ));
    assert!(!IncidentTransition::edge_allowed(
        IncidentStatus::Detected,
        IncidentStatus::Reopened
    ));
}

#[test]
fn creation_starts_detected_at_version_one_and_attributes_two_events() {
    let result = create_result(incident_create_fixture()).unwrap();
    assert_eq!(result.incident.status, IncidentStatus::Detected);
    assert_eq!(result.incident.version, 1);
    assert_eq!(result.incident.derived_severity, IncidentSeverity::S2);
    assert_eq!(result.incident.owning_team_id, TEAM);
    assert_eq!(result.incident.roles.len(), 1);
    assert_eq!(result.incident.roles[0].assigned_by, ACTOR);
    assert_eq!(result.incident.roles[0].assigned_at, now());
    assert_eq!(result.incident.evidence_ids, {
        let mut ids = vec![
            "evidence-checkout".to_string(),
            "evidence-manual-report".to_string(),
        ];
        ids.sort();
        ids
    });
    assert_eq!(result.events.len(), 2);
    assert_eq!(result.events[0].sequence, 1);
    assert_eq!(result.events[0].kind, IncidentEventKind::IncidentCreated);
    assert_eq!(result.events[0].actor_id, ACTOR);
    assert_eq!(result.events[0].policy_version, 7);
    assert_eq!(result.events[0].request_id, REQUEST);
    assert_eq!(result.events[1].sequence, 2);
    assert_eq!(result.events[1].kind, IncidentEventKind::TriggersAttached);
    assert_eq!(result.events[1].actor_id, ACTOR);
    assert_eq!(result.events[1].request_id, REQUEST);
}

#[test]
fn creation_rejects_unresolvable_or_unsafe_commands() {
    let mut fixture = incident_create_fixture();
    fixture.command.triggers.clear();
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidTrigger)
    ));

    let mut fixture = incident_create_fixture();
    fixture.command.triggers[0].scope =
        ResourceScope::workspace(OTHER_WORKSPACE, TEAM, ORGANIZATION);
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidScope)
    ));

    let mut fixture = incident_create_fixture();
    fixture.command.owning_team_id = STAKEHOLDER_ONE;
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidScope)
    ));

    let mut fixture = incident_create_fixture();
    fixture.actor_id = uuid::Uuid::nil();
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidId)
    ));

    let mut fixture = incident_create_fixture();
    fixture.request_id = uuid::Uuid::nil();
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidId)
    ));

    let mut fixture = incident_create_fixture();
    fixture.command.owning_team_id = uuid::Uuid::nil();
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidId)
    ));

    let mut fixture = incident_create_fixture();
    fixture.command.scope.workspace_id = Some(uuid::Uuid::nil());
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidId)
    ));

    let mut fixture = incident_create_fixture();
    fixture.command.initial_roles[0].principal_id = uuid::Uuid::nil();
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidId)
    ));
}

#[test]
fn creation_validates_trigger_provenance_and_attribution() {
    let without_report = IncidentTrigger {
        report: None,
        ..trigger()
    };
    let mut fixture = incident_create_fixture();
    fixture.command.triggers = vec![without_report];
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidTrigger)
    ));

    let nil_reporter = IncidentTrigger {
        report: Some(IncidentReport {
            reporter_id: Some(uuid::Uuid::nil()),
            summary: "checkout down".into(),
        }),
        ..trigger()
    };
    let mut fixture = incident_create_fixture();
    fixture.command.triggers = vec![nil_reporter];
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidId)
    ));

    let foreign_reporter = IncidentTrigger {
        report: Some(IncidentReport {
            reporter_id: Some(COMMANDER),
            summary: "checkout down".into(),
        }),
        ..trigger()
    };
    let mut fixture = incident_create_fixture();
    fixture.command.triggers = vec![foreign_reporter];
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidId)
    ));

    let unsafe_report = IncidentTrigger {
        report: Some(IncidentReport {
            reporter_id: Some(ACTOR),
            summary: "authorization: bearer abc".into(),
        }),
        ..trigger()
    };
    let mut fixture = incident_create_fixture();
    fixture.command.triggers = vec![unsafe_report];
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::UnsafeText)
    ));

    let unattributed_user_report = IncidentTrigger {
        source_kind: IncidentSourceKind::UserReport,
        report: None,
        ..trigger()
    };
    let mut fixture = incident_create_fixture();
    fixture.command.triggers = vec![unattributed_user_report];
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidTrigger)
    ));

    let alert_with_report = IncidentTrigger {
        report: Some(IncidentReport {
            reporter_id: Some(ACTOR),
            summary: "checkout down".into(),
        }),
        ..alert_trigger()
    };
    let mut fixture = incident_create_fixture();
    fixture.command.triggers = vec![alert_with_report];
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidTrigger)
    ));

    let mut no_evidence = alert_trigger();
    no_evidence.evidence_ids = vec![];
    let mut fixture = incident_create_fixture();
    fixture.command.triggers = vec![no_evidence];
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidTrigger)
    ));

    let mut nil_signal = alert_trigger();
    nil_signal.signal_id = Some(uuid::Uuid::nil());
    let mut fixture = incident_create_fixture();
    fixture.command.triggers = vec![nil_signal];
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidId)
    ));

    let mut nil_trigger_id = alert_trigger();
    nil_trigger_id.id = uuid::Uuid::nil();
    let mut fixture = incident_create_fixture();
    fixture.command.triggers = vec![nil_trigger_id];
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidId)
    ));

    let mut fixture = incident_create_fixture();
    fixture.command.triggers = vec![alert_trigger(), alert_trigger()];
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidTrigger)
    ));

    let same_source_other_kind = IncidentTrigger {
        source_kind: IncidentSourceKind::Anomaly,
        ..alert_trigger()
    };
    let mut fixture = incident_create_fixture();
    fixture.command.triggers = vec![alert_trigger(), same_source_other_kind];
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidTrigger)
    ));

    let user_report = IncidentTrigger {
        id: uuid::Uuid::from_u128(0x112),
        source_kind: IncidentSourceKind::UserReport,
        report: Some(IncidentReport {
            reporter_id: Some(COMMANDER),
            summary: "customers report failures".into(),
        }),
        ..trigger()
    };
    let mut fixture = incident_create_fixture();
    fixture.command.triggers = vec![trigger(), user_report];
    assert!(create_result(fixture).is_ok());
}

#[test]
fn transitions_reject_skips_and_repeats() {
    let incident = created();
    assert!(matches!(
        transition(&incident, 3, resolved()),
        Err(IncidentError::InvalidTransition { .. })
    ));

    let triaged = transition(&incident, 3, triage()).unwrap().incident;
    assert_eq!(triaged.status, IncidentStatus::Triage);
    assert!(matches!(
        transition(&triaged, 4, triage()),
        Err(IncidentError::InvalidTransition { .. })
    ));
}

#[test]
fn transitions_require_typed_context_values() {
    let incident = created();

    let nil_owner = IncidentTransition::Triage(TriageContext {
        business_impact: business_impact(),
        owner: uuid::Uuid::nil(),
        duplicate_checked: true,
    });
    assert!(matches!(
        transition(&incident, 3, nil_owner),
        Err(IncidentError::InvalidTransitionContext)
    ));

    let unchecked = IncidentTransition::Triage(TriageContext {
        business_impact: business_impact(),
        owner: COMMANDER,
        duplicate_checked: false,
    });
    assert!(matches!(
        transition(&incident, 3, unchecked),
        Err(IncidentError::InvalidTransitionContext)
    ));

    let triaged = transition(&incident, 3, triage()).unwrap().incident;
    assert_eq!(triaged.roles[0].role, IncidentRole::Owner);
    assert_eq!(triaged.roles[0].principal_id, COMMANDER);

    let empty_evidence = IncidentTransition::Investigating(thalassa_domain::InvestigatingContext {
        note: "investigating".into(),
        evidence_ids: vec![],
    });
    assert!(matches!(
        transition(&triaged, 4, empty_evidence),
        Err(IncidentError::InvalidTransitionContext)
    ));

    let investigating = transition(&triaged, 4, investigating()).unwrap().incident;
    let nil_executor = IncidentTransition::Mitigating(thalassa_domain::MitigatingContext {
        action_description: "roll back".into(),
        executor: uuid::Uuid::nil(),
        expected_impact: "errors drop".into(),
    });
    assert!(matches!(
        transition(&investigating, 5, nil_executor),
        Err(IncidentError::InvalidTransitionContext)
    ));

    let mitigating = transition(&investigating, 5, mitigating())
        .unwrap()
        .incident;
    let nil_watch_owner = IncidentTransition::Monitoring(thalassa_domain::MonitoringContext {
        verification_seconds: 3_600,
        success_criteria: "stable".into(),
        watch_owner: uuid::Uuid::nil(),
    });
    assert!(matches!(
        transition(&mitigating, 6, nil_watch_owner),
        Err(IncidentError::InvalidTransitionContext)
    ));
    let zero_window = IncidentTransition::Monitoring(thalassa_domain::MonitoringContext {
        verification_seconds: 0,
        success_criteria: "stable".into(),
        watch_owner: COMMANDER,
    });
    assert!(matches!(
        transition(&mitigating, 6, zero_window),
        Err(IncidentError::InvalidTransitionContext)
    ));
    let oversized_window = IncidentTransition::Monitoring(thalassa_domain::MonitoringContext {
        verification_seconds: 86_401,
        success_criteria: "stable".into(),
        watch_owner: COMMANDER,
    });
    assert!(matches!(
        transition(&mitigating, 6, oversized_window),
        Err(IncidentError::InvalidTransitionContext)
    ));

    let monitoring = transition(&mitigating, 6, monitoring()).unwrap().incident;
    let before_creation = IncidentTransition::Resolved(thalassa_domain::ResolvedContext {
        resolution_summary: "rolled back".into(),
        evidence_ids: vec!["evidence-resolution".into()],
        impact_ended_at: now() - chrono::Duration::hours(1),
    });
    assert!(matches!(
        transition(&monitoring, 7, before_creation),
        Err(IncidentError::InvalidTransitionContext)
    ));
    let future_end = IncidentTransition::Resolved(thalassa_domain::ResolvedContext {
        resolution_summary: "rolled back".into(),
        evidence_ids: vec!["evidence-resolution".into()],
        impact_ended_at: now() + chrono::Duration::hours(1),
    });
    assert!(matches!(
        transition(&monitoring, 7, future_end),
        Err(IncidentError::InvalidTransitionContext)
    ));

    let resolved = transition(&monitoring, 7, resolved()).unwrap().incident;
    let no_follow_ups = IncidentTransition::Closed(thalassa_domain::ClosedContext {
        closure_notes: "verified".into(),
        follow_up_ids: vec![],
    });
    assert!(matches!(
        transition(&resolved, 8, no_follow_ups),
        Err(IncidentError::InvalidTransitionContext)
    ));
    let duplicated_follow_ups = IncidentTransition::Closed(thalassa_domain::ClosedContext {
        closure_notes: "verified".into(),
        follow_up_ids: vec!["follow-up-1".into(), "follow-up-1".into()],
    });
    assert!(matches!(
        transition(&resolved, 8, duplicated_follow_ups),
        Err(IncidentError::InvalidTransitionContext)
    ));
    let unsafe_follow_ups = IncidentTransition::Closed(thalassa_domain::ClosedContext {
        closure_notes: "verified".into(),
        follow_up_ids: vec!["follow up with spaces".into()],
    });
    assert!(matches!(
        transition(&resolved, 8, unsafe_follow_ups),
        Err(IncidentError::InvalidTransitionContext)
    ));

    let closed = transition(&resolved, 8, closed()).unwrap().incident;
    let bare_reopen = IncidentTransition::Reopened(thalassa_domain::ReopenedContext {
        reason: "errors returned".into(),
        evidence_ids: vec![],
        recurrence_signal_id: None,
    });
    assert!(matches!(
        transition(&closed, 9, bare_reopen),
        Err(IncidentError::InvalidTransitionContext)
    ));
    let nil_signal_reopen = IncidentTransition::Reopened(thalassa_domain::ReopenedContext {
        reason: "errors returned".into(),
        evidence_ids: vec![],
        recurrence_signal_id: Some(uuid::Uuid::nil()),
    });
    assert!(matches!(
        transition(&closed, 9, nil_signal_reopen),
        Err(IncidentError::InvalidTransitionContext)
    ));
    assert!(transition(&closed, 9, reopened_with_signal()).is_ok());
}

#[test]
fn triage_emits_unique_ordered_multi_event_audit() {
    let incident = created();
    let changed_impact = impact_with_level(
        ImpactLevel::Medium,
        "checkout degraded but available",
        "evidence-triage",
    );
    let triage = IncidentTransition::Triage(TriageContext {
        business_impact: changed_impact,
        owner: COMMANDER,
        duplicate_checked: true,
    });
    let mutation = transition(&incident, 3, triage).unwrap();

    let kinds: Vec<IncidentEventKind> = mutation.events.iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        vec![
            IncidentEventKind::StatusTransitioned,
            IncidentEventKind::SeverityChanged,
            IncidentEventKind::RoleChanged,
        ]
    );
    let sequences: Vec<u64> = mutation.events.iter().map(|e| e.sequence).collect();
    assert_eq!(sequences, vec![3, 4, 5]);
    assert_eq!(mutation.incident.version, 2);
    assert_eq!(mutation.incident.derived_severity, IncidentSeverity::S3);
    assert!(mutation
        .incident
        .evidence_ids
        .contains(&"evidence-triage".to_string()));
    assert!(mutation
        .incident
        .evidence_ids
        .contains(&"evidence-checkout".to_string()));
    let owner = mutation
        .incident
        .roles
        .iter()
        .find(|assignment| assignment.role == IncidentRole::Owner)
        .unwrap();
    assert_eq!(owner.principal_id, COMMANDER);

    let severity_event = &mutation.events[1];
    let thalassa_domain::IncidentTimelinePayload::SeverityChanged(payload) =
        &severity_event.payload
    else {
        panic!("expected severity payload");
    };
    assert_eq!(payload.previous_impact.level, ImpactLevel::High);
    assert_eq!(payload.current_impact.level, ImpactLevel::Medium);
    assert_eq!(payload.previous_severity, IncidentSeverity::S2);
    assert_eq!(payload.current_severity, IncidentSeverity::S3);

    let role_event = &mutation.events[2];
    let thalassa_domain::IncidentTimelinePayload::RoleChanged(role_payload) = &role_event.payload
    else {
        panic!("expected role payload");
    };
    assert_eq!(role_payload.previous_principal_ids, vec![ACTOR]);
    assert_eq!(role_payload.current_principal_id, Some(COMMANDER));

    let sequences: Vec<u64> = mutation.events.iter().map(|e| e.sequence).collect();
    let mut sorted = sequences.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sequences, sorted);
}

#[test]
fn unchanged_triage_context_emits_only_the_transition() {
    let incident = created();
    let mutation = transition(&incident, 3, triage_with(ACTOR)).unwrap();
    assert_eq!(mutation.events.len(), 1);
    assert_eq!(
        mutation.events[0].kind,
        IncidentEventKind::StatusTransitioned
    );
    assert_eq!(mutation.events[0].sequence, 3);
}

#[test]
fn sequences_stay_unique_across_creation_and_multi_event_mutations() {
    let incident = created();
    let mut sequences: Vec<u64> = vec![1, 2];

    let first = transition(&incident, 3, triage()).unwrap();
    sequences.extend(first.events.iter().map(|event| event.sequence));

    let investigating = first
        .incident
        .transition(
            first.incident.version,
            first.events.last().unwrap().sequence + 1,
            investigating(),
            ACTOR,
            REQUEST,
            7,
            now(),
        )
        .unwrap();
    sequences.extend(investigating.events.iter().map(|event| event.sequence));

    let mut sorted = sequences.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sequences.len(), sorted.len());
    assert_eq!(sorted.first(), Some(&1));
}

#[test]
fn lifecycle_walk_increments_version_and_sequence_with_attribution() {
    let mut current = created();
    let steps = [
        triage(),
        investigating(),
        mitigating(),
        monitoring(),
        resolved(),
        closed(),
        reopened_with_signal(),
        investigating(),
    ];
    let mut expected_sequence = 3;
    for (index, step) in steps.iter().enumerate() {
        let mutation = current
            .transition(
                current.version,
                expected_sequence,
                step.clone(),
                ACTOR,
                REQUEST,
                9,
                now(),
            )
            .unwrap();
        assert_eq!(mutation.incident.version, index as u64 + 2);
        assert!(
            !mutation.events.is_empty(),
            "every accepted transition emits at least one event"
        );
        assert_eq!(
            mutation.events.first().unwrap().kind,
            IncidentEventKind::StatusTransitioned
        );
        let kinds: Vec<IncidentEventKind> = mutation.events.iter().map(|e| e.kind).collect();
        if index == 0 {
            assert_eq!(
                kinds,
                vec![
                    IncidentEventKind::StatusTransitioned,
                    IncidentEventKind::RoleChanged
                ]
            );
        } else {
            assert_eq!(kinds, vec![IncidentEventKind::StatusTransitioned]);
        }
        for event in &mutation.events {
            assert_eq!(event.sequence, expected_sequence);
            assert_eq!(event.actor_id, ACTOR);
            assert_eq!(event.request_id, REQUEST);
            assert_eq!(event.policy_version, 9);
            expected_sequence += 1;
        }
        current = mutation.incident;
    }
    assert_eq!(current.status, IncidentStatus::Investigating);
    assert!(current.signal_ids.contains(&RECURRENCE_SIGNAL));
}

#[test]
fn stale_versions_return_typed_conflicts() {
    let incident = created();
    let triaged = transition(&incident, 3, triage()).unwrap().incident;
    assert!(matches!(
        triaged.transition(1, 4, investigating(), ACTOR, REQUEST, 7, now()),
        Err(IncidentError::VersionConflict {
            expected: 1,
            actual: 2
        })
    ));
}

#[test]
fn dispositions_stay_independent_of_status_and_duplicate_rules() {
    let incident = created();

    let self_duplicate = IncidentDispositionCommand {
        disposition: Some(IncidentDisposition::Duplicate),
        duplicate_of_incident_id: Some(incident.id),
        reason: "same incident".into(),
    };
    assert!(matches!(
        incident.set_disposition(1, 3, self_duplicate, ACTOR, REQUEST, 7, now()),
        Err(IncidentError::InvalidDuplicateReference)
    ));

    let nil_duplicate = IncidentDispositionCommand {
        disposition: Some(IncidentDisposition::Duplicate),
        duplicate_of_incident_id: Some(uuid::Uuid::nil()),
        reason: "unresolved reference".into(),
    };
    assert!(matches!(
        incident.set_disposition(1, 3, nil_duplicate, ACTOR, REQUEST, 7, now()),
        Err(IncidentError::InvalidId)
    ));

    let misplaced_duplicate = IncidentDispositionCommand {
        disposition: Some(IncidentDisposition::FalsePositive),
        duplicate_of_incident_id: Some(OTHER_INCIDENT),
        reason: "noise".into(),
    };
    assert!(matches!(
        incident.set_disposition(1, 3, misplaced_duplicate, ACTOR, REQUEST, 7, now()),
        Err(IncidentError::InvalidDisposition)
    ));

    let empty_reason = IncidentDispositionCommand {
        disposition: Some(IncidentDisposition::Informational),
        duplicate_of_incident_id: None,
        reason: String::new(),
    };
    assert!(matches!(
        incident.set_disposition(1, 3, empty_reason, ACTOR, REQUEST, 7, now()),
        Err(IncidentError::UnsafeText)
    ));

    let duplicate = IncidentDispositionCommand {
        disposition: Some(IncidentDisposition::Duplicate),
        duplicate_of_incident_id: Some(OTHER_INCIDENT),
        reason: "tracked under the primary incident".into(),
    };
    let mutated = incident
        .set_disposition(1, 3, duplicate, ACTOR, REQUEST, 7, now())
        .unwrap();
    assert_eq!(mutated.incident.status, IncidentStatus::Detected);
    assert_eq!(mutated.incident.version, 2);
    assert_eq!(
        mutated.incident.disposition,
        Some(IncidentDisposition::Duplicate)
    );
    assert_eq!(
        mutated.incident.duplicate_of_incident_id,
        Some(OTHER_INCIDENT)
    );
    assert_eq!(mutated.events.len(), 1);
    assert_eq!(
        mutated.events[0].kind,
        IncidentEventKind::DispositionChanged
    );
    assert_eq!(mutated.events[0].sequence, 3);
    assert_eq!(
        mutated.events[0].reason,
        Some("tracked under the primary incident".into())
    );

    let cleared = IncidentDispositionCommand {
        disposition: None,
        duplicate_of_incident_id: None,
        reason: "duplicate review found distinct impact".into(),
    };
    let cleared = mutated
        .incident
        .set_disposition(2, 4, cleared, ACTOR, REQUEST, 7, now())
        .unwrap();
    assert_eq!(cleared.incident.disposition, None);
    assert_eq!(cleared.incident.duplicate_of_incident_id, None);
    assert_eq!(cleared.incident.status, IncidentStatus::Detected);
}

#[test]
fn role_cardinality_requires_exact_replace_and_release() {
    let incident = created();
    let mut sequence = 3;
    let mut apply = |incident: &thalassa_domain::Incident,
                     command: IncidentRoleCommand|
     -> Result<thalassa_domain::IncidentMutation, IncidentError> {
        let mutation = incident.assign_role(
            incident.version,
            sequence,
            command,
            ACTOR,
            REQUEST,
            7,
            now(),
        );
        sequence += 1;
        mutation
    };

    let commander = IncidentRoleCommand::Assign {
        role: IncidentRole::IncidentCommander,
        principal_id: COMMANDER,
    };
    let with_commander = apply(&incident, commander).unwrap();
    assert_eq!(with_commander.events[0].sequence, 3);
    let thalassa_domain::IncidentTimelinePayload::RoleChanged(assign_payload) =
        &with_commander.events[0].payload
    else {
        panic!("expected role payload");
    };
    assert_eq!(
        assign_payload.previous_principal_ids,
        Vec::<PrincipalId>::new()
    );
    assert_eq!(assign_payload.current_principal_id, Some(COMMANDER));

    let second_commander = IncidentRoleCommand::Assign {
        role: IncidentRole::IncidentCommander,
        principal_id: STAKEHOLDER_ONE,
    };
    assert!(matches!(
        apply(&with_commander.incident, second_commander),
        Err(IncidentError::InvalidRole)
    ));

    let nil_principal = IncidentRoleCommand::Assign {
        role: IncidentRole::TechnicalLead,
        principal_id: uuid::Uuid::nil(),
    };
    assert!(matches!(
        apply(&with_commander.incident, nil_principal),
        Err(IncidentError::InvalidId)
    ));

    let first_stakeholder = IncidentRoleCommand::Assign {
        role: IncidentRole::Stakeholder,
        principal_id: STAKEHOLDER_ONE,
    };
    let stakeholders = apply(&with_commander.incident, first_stakeholder)
        .unwrap()
        .incident;
    let second_stakeholder = IncidentRoleCommand::Assign {
        role: IncidentRole::Stakeholder,
        principal_id: STAKEHOLDER_TWO,
    };
    let stakeholders = apply(&stakeholders, second_stakeholder).unwrap().incident;
    assert_eq!(
        stakeholders
            .roles
            .iter()
            .filter(|assignment| assignment.role == IncidentRole::Stakeholder)
            .count(),
        2
    );

    let replace_stakeholder = IncidentRoleCommand::Replace {
        role: IncidentRole::Stakeholder,
        principal_id: COMMANDER,
    };
    assert!(matches!(
        apply(&stakeholders, replace_stakeholder),
        Err(IncidentError::InvalidRole)
    ));

    let duplicate_stakeholder = IncidentRoleCommand::Assign {
        role: IncidentRole::Stakeholder,
        principal_id: STAKEHOLDER_ONE,
    };
    assert!(matches!(
        apply(&stakeholders, duplicate_stakeholder),
        Err(IncidentError::InvalidRole)
    ));

    let replacement = IncidentRoleCommand::Replace {
        role: IncidentRole::IncidentCommander,
        principal_id: STAKEHOLDER_TWO,
    };
    let replaced = apply(&stakeholders, replacement).unwrap();
    let commander_assignment = replaced
        .incident
        .roles
        .iter()
        .find(|assignment| assignment.role == IncidentRole::IncidentCommander)
        .unwrap();
    assert_eq!(commander_assignment.principal_id, STAKEHOLDER_TWO);
    assert_eq!(replaced.events.len(), 1);
    assert_eq!(replaced.events[0].kind, IncidentEventKind::RoleChanged);
    let thalassa_domain::IncidentTimelinePayload::RoleChanged(replace_payload) =
        &replaced.events[0].payload
    else {
        panic!("expected role payload");
    };
    assert_eq!(replace_payload.previous_principal_ids, vec![COMMANDER]);
    assert_eq!(replace_payload.current_principal_id, Some(STAKEHOLDER_TWO));

    let release_stakeholder_one = IncidentRoleCommand::Release {
        role: IncidentRole::Stakeholder,
        principal_id: STAKEHOLDER_ONE,
    };
    let released = apply(&replaced.incident, release_stakeholder_one)
        .unwrap()
        .incident;
    let remaining: Vec<_> = released
        .roles
        .iter()
        .filter(|assignment| assignment.role == IncidentRole::Stakeholder)
        .map(|assignment| assignment.principal_id)
        .collect();
    assert_eq!(remaining, vec![STAKEHOLDER_TWO]);

    let release_unassigned = IncidentRoleCommand::Release {
        role: IncidentRole::TechnicalLead,
        principal_id: COMMANDER,
    };
    assert!(matches!(
        apply(&released, release_unassigned),
        Err(IncidentError::InvalidRole)
    ));

    let release_commander_again = IncidentRoleCommand::Release {
        role: IncidentRole::IncidentCommander,
        principal_id: COMMANDER,
    };
    assert!(matches!(
        apply(&released, release_commander_again),
        Err(IncidentError::InvalidRole)
    ));
}

#[test]
fn severity_changes_recalculate_override_and_close_evidence() {
    let incident = created();

    let empty_reason = IncidentSeverityCommand::Override {
        selected: IncidentSeverity::S1,
        reason: String::new(),
        evidence_ids: vec!["evidence-severity".into()],
    };
    assert!(matches!(
        incident.set_severity(1, 3, empty_reason, ACTOR, REQUEST, 7, now()),
        Err(IncidentError::UnsafeText)
    ));

    let override_command = IncidentSeverityCommand::Override {
        selected: IncidentSeverity::S1,
        reason: "customer impact is worse than the assessment".into(),
        evidence_ids: vec!["evidence-severity".into()],
    };
    let overridden = incident
        .set_severity(1, 3, override_command, ACTOR, REQUEST, 7, now())
        .unwrap();
    let override_detail = overridden.incident.severity_override.clone().unwrap();
    assert_eq!(override_detail.derived, IncidentSeverity::S2);
    assert_eq!(override_detail.selected, IncidentSeverity::S1);
    assert_eq!(override_detail.actor_id, ACTOR);
    assert_eq!(overridden.events.len(), 1);
    assert_eq!(
        overridden.events[0].kind,
        IncidentEventKind::SeverityChanged
    );
    assert_eq!(overridden.events[0].sequence, 3);
    assert!(overridden
        .incident
        .evidence_ids
        .contains(&"evidence-severity".to_string()));
    let thalassa_domain::IncidentTimelinePayload::SeverityChanged(override_payload) =
        &overridden.events[0].payload
    else {
        panic!("expected severity payload");
    };
    assert_eq!(override_payload.previous_severity, IncidentSeverity::S2);
    assert_eq!(override_payload.current_severity, IncidentSeverity::S1);
    assert_eq!(
        override_payload.previous_impact,
        override_payload.current_impact
    );
    assert_eq!(
        override_payload.override_detail,
        Some(override_detail.clone())
    );

    let reassess = IncidentSeverityCommand::Reassess {
        business_impact: impact_with_level(
            ImpactLevel::None,
            "impact subsided",
            "evidence-reassessed",
        ),
        reason: "impact subsided after rollback".into(),
    };
    let reassessed = overridden
        .incident
        .set_severity(2, 4, reassess, ACTOR, REQUEST, 7, now())
        .unwrap();
    assert_eq!(reassessed.incident.derived_severity, IncidentSeverity::S5);
    assert_eq!(reassessed.incident.severity_override, None);
    assert!(reassessed
        .incident
        .evidence_ids
        .contains(&"evidence-reassessed".to_string()));
    let thalassa_domain::IncidentTimelinePayload::SeverityChanged(reassess_payload) =
        &reassessed.events[0].payload
    else {
        panic!("expected severity payload");
    };
    assert_eq!(reassess_payload.previous_impact.level, ImpactLevel::High);
    assert_eq!(reassess_payload.current_impact.level, ImpactLevel::None);
    assert_eq!(reassess_payload.previous_severity, IncidentSeverity::S1);
    assert_eq!(reassess_payload.current_severity, IncidentSeverity::S5);

    let same_severity_reassess = IncidentSeverityCommand::Reassess {
        business_impact: impact_with_level(
            ImpactLevel::None,
            "impact still contained",
            "evidence-rechecked",
        ),
        reason: "rechecked with no severity change".into(),
    };
    let unchanged = reassessed
        .incident
        .set_severity(3, 5, same_severity_reassess, ACTOR, REQUEST, 7, now())
        .unwrap();
    assert_eq!(unchanged.incident.derived_severity, IncidentSeverity::S5);
    assert_eq!(unchanged.events.len(), 1);
    assert_eq!(unchanged.events[0].kind, IncidentEventKind::SeverityChanged);
    let thalassa_domain::IncidentTimelinePayload::SeverityChanged(unchanged_payload) =
        &unchanged.events[0].payload
    else {
        panic!("expected severity payload");
    };
    assert_eq!(unchanged_payload.previous_severity, IncidentSeverity::S5);
    assert_eq!(unchanged_payload.current_severity, IncidentSeverity::S5);
    assert_eq!(unchanged_payload.previous_impact.summary, "impact subsided");
    assert_eq!(
        unchanged_payload.current_impact.summary,
        "impact still contained"
    );
}
#[test]
fn rejects_zero_and_overflowing_first_event_sequences() {
    let incident = created();

    let override_command = IncidentSeverityCommand::Override {
        selected: IncidentSeverity::S1,
        reason: "worse than assessed".into(),
        evidence_ids: vec!["evidence-severity".into()],
    };
    let disposition = IncidentDispositionCommand {
        disposition: Some(IncidentDisposition::Informational),
        duplicate_of_incident_id: None,
        reason: "noise only".into(),
    };
    let assignment = IncidentRoleCommand::Assign {
        role: IncidentRole::TechnicalLead,
        principal_id: COMMANDER,
    };

    assert!(matches!(
        incident.transition(1, 0, triage(), ACTOR, REQUEST, 7, now()),
        Err(IncidentError::InvalidEventSequence)
    ));
    assert!(matches!(
        incident.set_severity(1, 0, override_command.clone(), ACTOR, REQUEST, 7, now()),
        Err(IncidentError::InvalidEventSequence)
    ));
    assert!(matches!(
        incident.set_disposition(1, 0, disposition.clone(), ACTOR, REQUEST, 7, now()),
        Err(IncidentError::InvalidEventSequence)
    ));
    assert!(matches!(
        incident.assign_role(1, 0, assignment, ACTOR, REQUEST, 7, now()),
        Err(IncidentError::InvalidEventSequence)
    ));
    assert_eq!(incident.version, 1);

    let expanding_triage = IncidentTransition::Triage(TriageContext {
        business_impact: impact_with_level(
            ImpactLevel::Medium,
            "checkout degraded but available",
            "evidence-triage",
        ),
        owner: COMMANDER,
        duplicate_checked: true,
    });
    assert!(matches!(
        incident.transition(1, u64::MAX, expanding_triage, ACTOR, REQUEST, 7, now()),
        Err(IncidentError::InvalidEventSequence)
    ));

    let boundary_disposition = incident
        .set_disposition(1, u64::MAX, disposition, ACTOR, REQUEST, 7, now())
        .unwrap();
    assert_eq!(boundary_disposition.events.len(), 1);
    assert_eq!(boundary_disposition.events[0].sequence, u64::MAX);

    let boundary_triage = incident
        .transition(1, u64::MAX, triage_with(ACTOR), ACTOR, REQUEST, 7, now())
        .unwrap();
    assert_eq!(boundary_triage.events.len(), 1);
    assert_eq!(boundary_triage.events[0].sequence, u64::MAX);
}

#[test]
fn creation_requires_workspace_bounded_scopes_with_valid_ids() {
    let mut fixture = incident_create_fixture();
    fixture.command.scope.organization_id = None;
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidScope)
    ));

    let mut fixture = incident_create_fixture();
    fixture.command.scope.workspace_id = None;
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidScope)
    ));

    let mut fixture = incident_create_fixture();
    fixture.command.scope.organization_id = Some(uuid::Uuid::nil());
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidId)
    ));

    let mut fixture = incident_create_fixture();
    fixture.command.scope.environment_id = Some(uuid::Uuid::nil());
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidId)
    ));

    let mut fixture = incident_create_fixture();
    fixture.command.scope.resource_ids = vec![uuid::Uuid::nil()];
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidId)
    ));

    let mut teamless = trigger();
    teamless.scope.team_id = None;
    let mut fixture = incident_create_fixture();
    fixture.command.triggers = vec![teamless];
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidScope)
    ));

    let mut nil_resource = trigger();
    nil_resource.scope.resource_ids = vec![uuid::Uuid::nil()];
    let mut fixture = incident_create_fixture();
    fixture.command.triggers = vec![nil_resource];
    assert!(matches!(
        create_result(fixture),
        Err(IncidentError::InvalidId)
    ));

    let mut narrowed = trigger();
    narrowed.scope.environment_id = Some(uuid::Uuid::from_u128(0x220));
    narrowed.scope.resource_ids = vec![uuid::Uuid::from_u128(0x221)];
    let mut fixture = incident_create_fixture();
    fixture.command.triggers = vec![narrowed];
    assert!(create_result(fixture).is_ok());
}

#[test]
fn triage_replaces_assessment_and_clears_stale_override() {
    let incident = created();
    let override_command = IncidentSeverityCommand::Override {
        selected: IncidentSeverity::S1,
        reason: "worse than assessed".into(),
        evidence_ids: vec!["evidence-override".into()],
    };
    let overridden = incident
        .set_severity(1, 3, override_command, ACTOR, REQUEST, 7, now())
        .unwrap();
    assert_eq!(overridden.incident.current_severity(), IncidentSeverity::S1);

    let triage = IncidentTransition::Triage(TriageContext {
        business_impact: impact_with_level(
            ImpactLevel::Medium,
            "checkout degraded but available",
            "evidence-triage",
        ),
        owner: COMMANDER,
        duplicate_checked: true,
    });
    let mutation = overridden
        .incident
        .transition(2, 4, triage, ACTOR, REQUEST, 7, now())
        .unwrap();

    assert_eq!(mutation.incident.version, 3);
    assert_eq!(mutation.incident.severity_override, None);
    assert_eq!(mutation.incident.derived_severity, IncidentSeverity::S3);
    assert_eq!(mutation.incident.current_severity(), IncidentSeverity::S3);
    let kinds: Vec<IncidentEventKind> = mutation.events.iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        vec![
            IncidentEventKind::StatusTransitioned,
            IncidentEventKind::SeverityChanged,
            IncidentEventKind::RoleChanged,
        ]
    );
    let sequences: Vec<u64> = mutation.events.iter().map(|e| e.sequence).collect();
    assert_eq!(sequences, vec![4, 5, 6]);
    let thalassa_domain::IncidentTimelinePayload::SeverityChanged(payload) =
        &mutation.events[1].payload
    else {
        panic!("expected severity payload");
    };
    assert_eq!(payload.previous_severity, IncidentSeverity::S1);
    assert_eq!(payload.current_severity, IncidentSeverity::S3);
    assert_eq!(payload.override_detail, None);
    assert_eq!(payload.previous_impact.level, ImpactLevel::High);
    assert_eq!(payload.current_impact.level, ImpactLevel::Medium);
    assert!(mutation
        .incident
        .evidence_ids
        .contains(&"evidence-override".to_string()));
    assert!(mutation
        .incident
        .evidence_ids
        .contains(&"evidence-triage".to_string()));
}

#[test]
fn transitions_round_trip_through_tagged_wire_values() {
    let value = serde_json::to_value(investigating()).unwrap();
    assert_eq!(value["target"], json!("investigating"));
    let decoded: IncidentTransition = serde_json::from_value(value).unwrap();
    assert_eq!(decoded, investigating());
}
