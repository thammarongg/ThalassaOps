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

fn created() -> thalassa_domain::Incident {
    let fixture = incident_create_fixture();
    thalassa_domain::Incident::create(
        fixture.command,
        fixture.actor_id,
        fixture.request_id,
        7,
        fixture.now,
    )
    .unwrap()
    .incident
}

fn triage() -> IncidentTransition {
    IncidentTransition::Triage(TriageContext {
        business_impact: business_impact(),
        owner: COMMANDER,
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
    step: IncidentTransition,
) -> Result<thalassa_domain::IncidentMutation, IncidentError> {
    incident.transition(incident.version, step, ACTOR, REQUEST, 7, now())
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
fn creation_starts_detected_at_version_one_and_attributes_event() {
    let fixture = incident_create_fixture();
    let result = thalassa_domain::Incident::create(
        fixture.command,
        fixture.actor_id,
        fixture.request_id,
        7,
        fixture.now,
    )
    .unwrap();
    assert_eq!(result.incident.status, IncidentStatus::Detected);
    assert_eq!(result.incident.version, 1);
    assert_eq!(result.incident.derived_severity, IncidentSeverity::S2);
    assert_eq!(result.incident.owning_team_id, TEAM);
    assert_eq!(result.incident.roles.len(), 1);
    assert_eq!(result.incident.roles[0].assigned_by, ACTOR);
    assert_eq!(result.incident.roles[0].assigned_at, fixture.now);
    assert_eq!(result.events[0].sequence, 1);
    assert_eq!(result.events[0].kind, IncidentEventKind::IncidentCreated);
    assert_eq!(result.events[0].actor_id, fixture.actor_id);
    assert_eq!(result.events[0].policy_version, 7);
    assert_eq!(result.events[0].request_id, fixture.request_id);
    assert_eq!(result.events.len(), 1);
}

#[test]
fn creation_rejects_missing_triggers_and_mismatched_scopes() {
    let mut fixture = incident_create_fixture();
    fixture.command.triggers.clear();
    assert!(matches!(
        thalassa_domain::Incident::create(
            fixture.command,
            fixture.actor_id,
            fixture.request_id,
            7,
            fixture.now
        ),
        Err(IncidentError::InvalidTrigger)
    ));

    let mut fixture = incident_create_fixture();
    fixture.command.triggers[0].scope =
        ResourceScope::workspace(OTHER_WORKSPACE, TEAM, ORGANIZATION);
    assert!(matches!(
        thalassa_domain::Incident::create(
            fixture.command,
            fixture.actor_id,
            fixture.request_id,
            7,
            fixture.now
        ),
        Err(IncidentError::InvalidScope)
    ));

    let mut fixture = incident_create_fixture();
    fixture.command.owning_team_id = STAKEHOLDER_ONE;
    assert!(matches!(
        thalassa_domain::Incident::create(
            fixture.command,
            fixture.actor_id,
            fixture.request_id,
            7,
            fixture.now
        ),
        Err(IncidentError::InvalidScope)
    ));
}

#[test]
fn transitions_reject_skips_and_repeats() {
    let incident = created();
    assert!(matches!(
        transition(&incident, resolved()),
        Err(IncidentError::InvalidTransition { .. })
    ));

    let triaged = transition(&incident, triage()).unwrap().incident;
    assert_eq!(triaged.status, IncidentStatus::Triage);
    assert!(matches!(
        transition(&triaged, triage()),
        Err(IncidentError::InvalidTransition { .. })
    ));
}

#[test]
fn transitions_require_typed_context_values() {
    let incident = created();

    let unchecked = IncidentTransition::Triage(TriageContext {
        business_impact: business_impact(),
        owner: COMMANDER,
        duplicate_checked: false,
    });
    assert!(matches!(
        transition(&incident, unchecked),
        Err(IncidentError::InvalidTransitionContext)
    ));

    let triaged = transition(&incident, triage()).unwrap().incident;
    assert_eq!(triaged.roles[0].role, IncidentRole::Owner);
    assert_eq!(triaged.roles[0].principal_id, COMMANDER);

    let empty_evidence = IncidentTransition::Investigating(thalassa_domain::InvestigatingContext {
        note: "investigating".into(),
        evidence_ids: vec![],
    });
    assert!(matches!(
        transition(&triaged, empty_evidence),
        Err(IncidentError::InvalidTransitionContext)
    ));

    let investigating = transition(&triaged, investigating()).unwrap().incident;
    let mitigating = transition(&investigating, mitigating()).unwrap().incident;
    let zero_window = IncidentTransition::Monitoring(thalassa_domain::MonitoringContext {
        verification_seconds: 0,
        success_criteria: "stable".into(),
        watch_owner: COMMANDER,
    });
    assert!(matches!(
        transition(&mitigating, zero_window),
        Err(IncidentError::InvalidTransitionContext)
    ));
    let oversized_window = IncidentTransition::Monitoring(thalassa_domain::MonitoringContext {
        verification_seconds: 86_401,
        success_criteria: "stable".into(),
        watch_owner: COMMANDER,
    });
    assert!(matches!(
        transition(&mitigating, oversized_window),
        Err(IncidentError::InvalidTransitionContext)
    ));

    let monitoring = transition(&mitigating, monitoring()).unwrap().incident;
    let future_end = IncidentTransition::Resolved(thalassa_domain::ResolvedContext {
        resolution_summary: "rolled back".into(),
        evidence_ids: vec!["evidence-resolution".into()],
        impact_ended_at: now() + chrono::Duration::hours(1),
    });
    assert!(matches!(
        transition(&monitoring, future_end),
        Err(IncidentError::InvalidTransitionContext)
    ));

    let resolved = transition(&monitoring, resolved()).unwrap().incident;
    let bare_reopen = IncidentTransition::Reopened(thalassa_domain::ReopenedContext {
        reason: "errors returned".into(),
        evidence_ids: vec![],
        recurrence_signal_id: None,
    });
    assert!(matches!(
        transition(&resolved, bare_reopen),
        Err(IncidentError::InvalidTransitionContext)
    ));
    assert!(transition(&resolved, reopened_with_signal()).is_ok());
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
    let mut expected_sequence = 1;
    for (index, step) in steps.iter().enumerate() {
        let mutation = current
            .transition(current.version, step.clone(), ACTOR, REQUEST, 9, now())
            .unwrap();
        expected_sequence += 1;
        assert_eq!(mutation.incident.version, index as u64 + 2);
        assert_eq!(mutation.events.len(), 1);
        assert_eq!(mutation.events[0].sequence, expected_sequence);
        assert_eq!(mutation.events[0].actor_id, ACTOR);
        assert_eq!(mutation.events[0].request_id, REQUEST);
        assert_eq!(mutation.events[0].policy_version, 9);
        current = mutation.incident;
    }
    assert_eq!(current.status, IncidentStatus::Investigating);
    assert!(current.signal_ids.contains(&RECURRENCE_SIGNAL));
}

#[test]
fn stale_versions_return_typed_conflicts() {
    let incident = created();
    let triaged = transition(&incident, triage()).unwrap().incident;
    assert!(matches!(
        triaged.transition(1, investigating(), ACTOR, REQUEST, 7, now()),
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
        incident.set_disposition(1, self_duplicate, ACTOR, REQUEST, 7, now()),
        Err(IncidentError::InvalidDuplicateReference)
    ));

    let misplaced_duplicate = IncidentDispositionCommand {
        disposition: Some(IncidentDisposition::FalsePositive),
        duplicate_of_incident_id: Some(OTHER_INCIDENT),
        reason: "noise".into(),
    };
    assert!(matches!(
        incident.set_disposition(1, misplaced_duplicate, ACTOR, REQUEST, 7, now()),
        Err(IncidentError::InvalidDisposition)
    ));

    let empty_reason = IncidentDispositionCommand {
        disposition: Some(IncidentDisposition::Informational),
        duplicate_of_incident_id: None,
        reason: String::new(),
    };
    assert!(matches!(
        incident.set_disposition(1, empty_reason, ACTOR, REQUEST, 7, now()),
        Err(IncidentError::UnsafeText)
    ));

    let duplicate = IncidentDispositionCommand {
        disposition: Some(IncidentDisposition::Duplicate),
        duplicate_of_incident_id: Some(OTHER_INCIDENT),
        reason: "tracked under the primary incident".into(),
    };
    let mutated = incident
        .set_disposition(1, duplicate, ACTOR, REQUEST, 7, now())
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
        .set_disposition(2, cleared, ACTOR, REQUEST, 7, now())
        .unwrap();
    assert_eq!(cleared.incident.disposition, None);
    assert_eq!(cleared.incident.duplicate_of_incident_id, None);
    assert_eq!(cleared.incident.status, IncidentStatus::Detected);
}

#[test]
fn role_cardinality_permits_stakeholders_and_enforces_replacement() {
    let incident = created();
    let assign = |incident: &thalassa_domain::Incident,
                  command: IncidentRoleCommand|
     -> Result<thalassa_domain::IncidentMutation, IncidentError> {
        incident.assign_role(incident.version, command, ACTOR, REQUEST, 7, now())
    };

    let commander = IncidentRoleCommand::Assign {
        role: IncidentRole::IncidentCommander,
        principal_id: COMMANDER,
    };
    let with_commander = assign(&incident, commander).unwrap().incident;

    let second_commander = IncidentRoleCommand::Assign {
        role: IncidentRole::IncidentCommander,
        principal_id: STAKEHOLDER_ONE,
    };
    assert!(matches!(
        assign(&with_commander, second_commander),
        Err(IncidentError::InvalidRole)
    ));

    let first_stakeholder = IncidentRoleCommand::Assign {
        role: IncidentRole::Stakeholder,
        principal_id: STAKEHOLDER_ONE,
    };
    let stakeholders = assign(&with_commander, first_stakeholder).unwrap().incident;
    let second_stakeholder = IncidentRoleCommand::Assign {
        role: IncidentRole::Stakeholder,
        principal_id: STAKEHOLDER_TWO,
    };
    let stakeholders = assign(&stakeholders, second_stakeholder).unwrap().incident;
    assert_eq!(
        stakeholders
            .roles
            .iter()
            .filter(|assignment| assignment.role == IncidentRole::Stakeholder)
            .count(),
        2
    );

    let duplicate_stakeholder = IncidentRoleCommand::Assign {
        role: IncidentRole::Stakeholder,
        principal_id: STAKEHOLDER_ONE,
    };
    assert!(matches!(
        assign(&stakeholders, duplicate_stakeholder),
        Err(IncidentError::InvalidRole)
    ));

    let replacement = IncidentRoleCommand::Replace {
        role: IncidentRole::IncidentCommander,
        principal_id: STAKEHOLDER_TWO,
    };
    let replaced = assign(&stakeholders, replacement).unwrap();
    let commander_assignment = replaced
        .incident
        .roles
        .iter()
        .find(|assignment| assignment.role == IncidentRole::IncidentCommander)
        .unwrap();
    assert_eq!(commander_assignment.principal_id, STAKEHOLDER_TWO);
    assert_eq!(replaced.events.len(), 1);
    assert_eq!(replaced.events[0].kind, IncidentEventKind::RoleChanged);

    let release = IncidentRoleCommand::Release {
        role: IncidentRole::IncidentCommander,
    };
    let released = assign(&replaced.incident, release).unwrap().incident;
    assert!(!released
        .roles
        .iter()
        .any(|assignment| assignment.role == IncidentRole::IncidentCommander));

    let release_again = IncidentRoleCommand::Release {
        role: IncidentRole::IncidentCommander,
    };
    assert!(matches!(
        assign(&released, release_again),
        Err(IncidentError::InvalidRole)
    ));
}

#[test]
fn severity_changes_recalculate_or_override_with_attribution() {
    let incident = created();

    let empty_reason = IncidentSeverityCommand::Override {
        selected: IncidentSeverity::S1,
        reason: String::new(),
        evidence_ids: vec!["evidence-severity".into()],
    };
    assert!(matches!(
        incident.set_severity(1, empty_reason, ACTOR, REQUEST, 7, now()),
        Err(IncidentError::UnsafeText)
    ));

    let override_command = IncidentSeverityCommand::Override {
        selected: IncidentSeverity::S1,
        reason: "customer impact is worse than the assessment".into(),
        evidence_ids: vec!["evidence-severity".into()],
    };
    let overridden = incident
        .set_severity(1, override_command, ACTOR, REQUEST, 7, now())
        .unwrap();
    let override_detail = overridden.incident.severity_override.clone().unwrap();
    assert_eq!(override_detail.derived, IncidentSeverity::S2);
    assert_eq!(override_detail.selected, IncidentSeverity::S1);
    assert_eq!(override_detail.actor_id, ACTOR);
    assert_eq!(
        overridden.events[0].kind,
        IncidentEventKind::SeverityChanged
    );

    let reassess = IncidentSeverityCommand::Reassess {
        business_impact: BusinessImpact {
            level: ImpactLevel::None,
            summary: "impact subsided".into(),
            customer_scope: "no customers affected".into(),
            service_criticality: "tier-3".into(),
            trajectory: ImpactTrajectory::Improving,
            dimensions: ImpactDimensions::single_dimension(
                ImpactLevel::None,
                ImpactTrajectory::Improving,
            ),
            evidence_ids: vec!["evidence-reassessed".into()],
        },
        reason: "impact subsided after rollback".into(),
    };
    let reassessed = overridden
        .incident
        .set_severity(2, reassess, ACTOR, REQUEST, 7, now())
        .unwrap();
    assert_eq!(reassessed.incident.derived_severity, IncidentSeverity::S5);
    assert_eq!(reassessed.incident.severity_override, None);
    assert_eq!(
        reassessed.events[0].kind,
        IncidentEventKind::SeverityChanged
    );
}

#[test]
fn transitions_round_trip_through_tagged_wire_values() {
    let value = serde_json::to_value(investigating()).unwrap();
    assert_eq!(value["target"], json!("investigating"));
    let decoded: IncidentTransition = serde_json::from_value(value).unwrap();
    assert_eq!(decoded, investigating());
}
