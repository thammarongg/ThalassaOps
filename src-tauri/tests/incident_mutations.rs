// SPDX-License-Identifier: Apache-2.0

//! Task 6 proofs: the full validated lifecycle, independent dispositions,
//! responder roles, optimistic concurrency and bounded, workspace-scoped reads.

use chrono::{DateTime, TimeZone, Utc};
use tempfile::TempDir;
use thalassa_domain::{
    BusinessImpact, ClosedContext, ImpactDimensions, ImpactLevel, ImpactTrajectory, Incident,
    IncidentCreateRequest, IncidentDisposition, IncidentDispositionCommand,
    IncidentDispositionRequest, IncidentEventKind, IncidentRole, IncidentRoleAssignmentInput,
    IncidentRoleCommand, IncidentRoleRequest, IncidentSeverity, IncidentSeverityCommand,
    IncidentSeverityRequest, IncidentSourceKind, IncidentStatus, IncidentTransition,
    IncidentTransitionRequest, IncidentTriggerInput, InvestigatingContext, MitigatingContext,
    MonitoringContext, PrincipalId, ReopenedContext, ResolvedContext, ResourceScope, TriageContext,
};
use thalassaops::correlation::SourceRecordStore;
use thalassaops::incident::{
    IncidentCommandContext, IncidentService, IncidentServiceError, IncidentSourceResolver,
    SqliteIncidentRepository,
};
use uuid::Uuid;

const ORGANIZATION: Uuid = Uuid::from_u128(0x11);
const TEAM: Uuid = Uuid::from_u128(0x12);
const WORKSPACE: Uuid = Uuid::from_u128(0x13);
const ENVIRONMENT: Uuid = Uuid::from_u128(0x14);
const OTHER_WORKSPACE: Uuid = Uuid::from_u128(0x99);
const ACTOR: PrincipalId = Uuid::from_u128(0xa0);
const COMMANDER: PrincipalId = Uuid::from_u128(0xa1);
const STAKEHOLDER: PrincipalId = Uuid::from_u128(0xa4);
const POLICY_VERSION: u64 = 7;

fn workspace_scope() -> ResourceScope {
    ResourceScope::workspace(WORKSPACE, TEAM, ORGANIZATION)
}

fn environment_scope() -> ResourceScope {
    ResourceScope::environment(ENVIRONMENT, WORKSPACE, TEAM, ORGANIZATION)
}

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 28, 9, 0, 0).unwrap()
}

fn later() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 28, 10, 0, 0).unwrap()
}

fn business_impact() -> BusinessImpact {
    impact_at(ImpactLevel::High, "Checkout unavailable for customers")
}

fn impact_at(level: ImpactLevel, summary: &str) -> BusinessImpact {
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
        evidence_ids: vec!["evidence-checkout".into()],
    }
}

struct Fixture {
    _directory: TempDir,
    service: IncidentService,
    next_request: u128,
}

impl Fixture {
    fn new() -> Self {
        let directory = TempDir::new().expect("temporary directory");
        let repository =
            SqliteIncidentRepository::open(&directory.path().join("incidents.sqlite3"))
                .expect("repository opens");
        let mut records = SourceRecordStore::with_scope(environment_scope());
        let resolver = IncidentSourceResolver::replay(&environment_scope(), &mut records)
            .expect("the committed replay catalog resolves");
        Self {
            _directory: directory,
            service: IncidentService::new(resolver, repository),
            next_request: 0x1000,
        }
    }

    fn context(&mut self) -> IncidentCommandContext {
        self.next_request += 1;
        IncidentCommandContext {
            workspace_scope: workspace_scope(),
            actor_id: ACTOR,
            policy_version: POLICY_VERSION,
            request_id: Uuid::from_u128(self.next_request),
            now: now(),
        }
    }

    fn read_context(&self) -> IncidentCommandContext {
        IncidentCommandContext {
            workspace_scope: workspace_scope(),
            actor_id: ACTOR,
            policy_version: POLICY_VERSION,
            request_id: Uuid::from_u128(0xffff),
            now: now(),
        }
    }

    fn create_incident(&mut self) -> Incident {
        let source_id = self
            .service
            .resolver()
            .signal_ids(IncidentSourceKind::Alert)
            .first()
            .copied()
            .expect("an alert signal is resolvable")
            .to_string();
        let context = self.context();
        self.service
            .create(
                &context,
                IncidentCreateRequest {
                    summary: "Checkout errors under investigation".into(),
                    triggers: vec![IncidentTriggerInput::Alert { source_id }],
                    business_impact: business_impact(),
                    initial_roles: vec![IncidentRoleAssignmentInput {
                        role: IncidentRole::Owner,
                        principal_id: ACTOR,
                    }],
                },
            )
            .expect("creation succeeds")
            .incident
    }

    fn transition(&mut self, incident: &Incident, transition: IncidentTransition) -> Incident {
        let context = self.context();
        self.service
            .transition(
                &context,
                IncidentTransitionRequest {
                    incident_id: incident.id,
                    expected_version: incident.version,
                    transition,
                },
            )
            .expect("transition is accepted")
            .incident
    }

    fn advance_to_closed(&mut self) -> Incident {
        let incident = self.create_incident();
        let incident = self.transition(&incident, triage());
        let incident = self.transition(&incident, investigating());
        let incident = self.transition(&incident, mitigating());
        let incident = self.transition(&incident, monitoring());
        let incident = self.transition(&incident, resolved());
        self.transition(&incident, closed())
    }
}

fn triage() -> IncidentTransition {
    IncidentTransition::Triage(TriageContext {
        business_impact: business_impact(),
        owner: ACTOR,
        duplicate_checked: true,
    })
}

fn investigating() -> IncidentTransition {
    IncidentTransition::Investigating(InvestigatingContext {
        note: "Comparing checkout error rate against the last deployment".into(),
        evidence_ids: vec!["evidence-checkout".into()],
    })
}

fn mitigating() -> IncidentTransition {
    IncidentTransition::Mitigating(MitigatingContext {
        action_description: "Rolling the checkout deployment back one revision".into(),
        executor: ACTOR,
        expected_impact: "Checkout error rate returns to baseline".into(),
    })
}

fn monitoring() -> IncidentTransition {
    IncidentTransition::Monitoring(MonitoringContext {
        verification_seconds: 900,
        success_criteria: "Checkout error rate stays under one percent".into(),
        watch_owner: ACTOR,
    })
}

fn resolved() -> IncidentTransition {
    IncidentTransition::Resolved(ResolvedContext {
        resolution_summary: "Checkout recovered after the rollback".into(),
        evidence_ids: vec!["evidence-checkout".into()],
        impact_ended_at: now(),
    })
}

fn closed() -> IncidentTransition {
    IncidentTransition::Closed(ClosedContext {
        closure_notes: "Follow-up filed for the missing checkout canary".into(),
        follow_up_ids: vec!["follow-up-checkout-canary".into()],
    })
}

fn reopened() -> IncidentTransition {
    IncidentTransition::Reopened(ReopenedContext {
        reason: "Checkout errors returned after the rollback window".into(),
        evidence_ids: vec!["evidence-checkout".into()],
        recurrence_signal_id: None,
    })
}

#[test]
fn service_progresses_full_lifecycle_and_persists_ordered_events() {
    let mut fixture = Fixture::new();
    let incident = fixture.advance_to_closed();
    assert_eq!(incident.status, IncidentStatus::Closed);
    assert_eq!(incident.version, 7);

    let timeline = fixture
        .service
        .timeline(&fixture.read_context(), incident.id, None, 100)
        .expect("timeline is readable");
    assert!(timeline
        .events
        .windows(2)
        .all(|pair| pair[0].sequence < pair[1].sequence));
    assert_eq!(timeline.events[0].kind, IncidentEventKind::IncidentCreated);
    assert_eq!(
        timeline.events.last().expect("a closing event").kind,
        IncidentEventKind::StatusTransitioned
    );
    assert!(timeline
        .events
        .iter()
        .all(|event| event.actor_id == ACTOR && event.policy_version == POLICY_VERSION));
}

#[test]
fn skipped_and_repeated_transitions_are_rejected() {
    let mut fixture = Fixture::new();
    let incident = fixture.create_incident();
    let context = fixture.context();

    let skipped = fixture.service.transition(
        &context,
        IncidentTransitionRequest {
            incident_id: incident.id,
            expected_version: incident.version,
            transition: investigating(),
        },
    );
    assert!(matches!(skipped, Err(IncidentServiceError::Domain(_))));

    let triaged = fixture.transition(&incident, triage());
    let context = fixture.context();
    let repeated = fixture.service.transition(
        &context,
        IncidentTransitionRequest {
            incident_id: triaged.id,
            expected_version: triaged.version,
            transition: triage(),
        },
    );
    assert!(matches!(repeated, Err(IncidentServiceError::Domain(_))));

    let stored = fixture
        .service
        .get(&fixture.read_context(), incident.id)
        .expect("incident is readable");
    assert_eq!(stored.status, IncidentStatus::Triage);
    assert_eq!(stored.version, 2);
}

#[test]
fn closed_can_reopen_but_stale_writer_cannot_mutate() {
    let mut fixture = Fixture::new();
    let closed = fixture.advance_to_closed();

    let context = fixture.context();
    let reopened = fixture
        .service
        .transition(
            &context,
            IncidentTransitionRequest {
                incident_id: closed.id,
                expected_version: closed.version,
                transition: reopened(),
            },
        )
        .expect("a closed incident can be reopened");
    assert_eq!(reopened.incident.status, IncidentStatus::Reopened);

    let context = fixture.context();
    let stale = fixture.service.set_disposition(
        &context,
        IncidentDispositionRequest {
            incident_id: closed.id,
            expected_version: closed.version,
            command: IncidentDispositionCommand {
                disposition: Some(IncidentDisposition::FalsePositive),
                duplicate_of_incident_id: None,
                reason: "Late review concluded this was a false positive".into(),
            },
        },
    );
    assert!(matches!(
        stale,
        Err(IncidentServiceError::VersionConflict {
            expected: 7,
            actual: 8
        })
    ));

    let stored = fixture
        .service
        .get(&fixture.read_context(), closed.id)
        .expect("incident is readable");
    assert_eq!(stored.status, IncidentStatus::Reopened);
    assert_eq!(stored.disposition, None);
}

#[test]
fn retrying_a_status_transition_replays_the_original_mutation() {
    let mut fixture = Fixture::new();
    let incident = fixture.create_incident();
    let context = fixture.context();
    let request = IncidentTransitionRequest {
        incident_id: incident.id,
        expected_version: incident.version,
        transition: triage(),
    };

    let first = fixture
        .service
        .transition(&context, request.clone())
        .expect("transition is accepted");
    let before_retry = fixture
        .service
        .timeline(&fixture.read_context(), incident.id, None, 100)
        .expect("timeline is readable");
    let retry_context = IncidentCommandContext {
        now: later(),
        ..context
    };

    let replayed = fixture
        .service
        .transition(&retry_context, request)
        .expect("the transition retry is replayed");

    assert_eq!(replayed, first);
    assert_eq!(replayed.incident.version, 2);
    assert_eq!(
        fixture
            .service
            .timeline(&fixture.read_context(), incident.id, None, 100)
            .expect("timeline is readable"),
        before_retry
    );
}

#[test]
fn retrying_a_severity_update_replays_the_original_mutation() {
    let mut fixture = Fixture::new();
    let incident = fixture.create_incident();
    let context = fixture.context();
    let request = IncidentSeverityRequest {
        incident_id: incident.id,
        expected_version: incident.version,
        command: IncidentSeverityCommand::Reassess {
            business_impact: impact_at(ImpactLevel::Critical, "Checkout unavailable everywhere"),
            reason: "Impact widened to every production region".into(),
        },
    };

    let first = fixture
        .service
        .set_severity(&context, request.clone())
        .expect("severity update is accepted");
    let before_retry = fixture
        .service
        .timeline(&fixture.read_context(), incident.id, None, 100)
        .expect("timeline is readable");
    let retry_context = IncidentCommandContext {
        now: later(),
        ..context
    };

    let replayed = fixture
        .service
        .set_severity(&retry_context, request)
        .expect("the severity retry is replayed");

    assert_eq!(replayed, first);
    assert_eq!(replayed.incident.version, 2);
    assert_eq!(
        fixture
            .service
            .timeline(&fixture.read_context(), incident.id, None, 100)
            .expect("timeline is readable"),
        before_retry
    );
}

#[test]
fn retrying_a_disposition_update_replays_the_original_mutation() {
    let mut fixture = Fixture::new();
    let incident = fixture.create_incident();
    let context = fixture.context();
    let request = IncidentDispositionRequest {
        incident_id: incident.id,
        expected_version: incident.version,
        command: IncidentDispositionCommand {
            disposition: Some(IncidentDisposition::Suppressed),
            duplicate_of_incident_id: None,
            reason: "Suppressed during the planned maintenance window".into(),
        },
    };

    let first = fixture
        .service
        .set_disposition(&context, request.clone())
        .expect("disposition update is accepted");
    let before_retry = fixture
        .service
        .timeline(&fixture.read_context(), incident.id, None, 100)
        .expect("timeline is readable");
    let retry_context = IncidentCommandContext {
        now: later(),
        ..context
    };

    let replayed = fixture
        .service
        .set_disposition(&retry_context, request)
        .expect("the disposition retry is replayed");

    assert_eq!(replayed, first);
    assert_eq!(replayed.incident.version, 2);
    assert_eq!(
        fixture
            .service
            .timeline(&fixture.read_context(), incident.id, None, 100)
            .expect("timeline is readable"),
        before_retry
    );
}

#[test]
fn retrying_a_role_assignment_replays_the_original_mutation() {
    let mut fixture = Fixture::new();
    let incident = fixture.create_incident();
    let context = fixture.context();
    let request = IncidentRoleRequest {
        incident_id: incident.id,
        expected_version: incident.version,
        command: IncidentRoleCommand::Assign {
            role: IncidentRole::IncidentCommander,
            principal_id: COMMANDER,
        },
    };

    let first = fixture
        .service
        .assign_role(&context, request.clone())
        .expect("role assignment is accepted");
    let before_retry = fixture
        .service
        .timeline(&fixture.read_context(), incident.id, None, 100)
        .expect("timeline is readable");
    let retry_context = IncidentCommandContext {
        now: later(),
        ..context
    };

    let replayed = fixture
        .service
        .assign_role(&retry_context, request)
        .expect("the role retry is replayed");

    assert_eq!(replayed, first);
    assert_eq!(replayed.incident.version, 2);
    assert_eq!(
        fixture
            .service
            .timeline(&fixture.read_context(), incident.id, None, 100)
            .expect("timeline is readable"),
        before_retry
    );
}

#[test]
fn a_different_request_id_at_a_stale_version_still_conflicts() {
    let mut fixture = Fixture::new();
    let incident = fixture.create_incident();
    let first_context = fixture.context();
    fixture
        .service
        .transition(
            &first_context,
            IncidentTransitionRequest {
                incident_id: incident.id,
                expected_version: incident.version,
                transition: triage(),
            },
        )
        .expect("transition is accepted");

    let stale_context = fixture.context();
    let result = fixture.service.transition(
        &stale_context,
        IncidentTransitionRequest {
            incident_id: incident.id,
            expected_version: incident.version,
            transition: triage(),
        },
    );

    assert!(matches!(
        result,
        Err(IncidentServiceError::VersionConflict {
            expected: 1,
            actual: 2
        })
    ));
}

#[test]
fn reusing_a_mutation_request_id_with_different_content_is_rejected() {
    let mut fixture = Fixture::new();
    let incident = fixture.create_incident();
    let context = fixture.context();
    let request = IncidentTransitionRequest {
        incident_id: incident.id,
        expected_version: incident.version,
        transition: triage(),
    };
    fixture
        .service
        .transition(&context, request)
        .expect("transition is accepted");

    let divergent_context = IncidentCommandContext {
        now: later(),
        ..context
    };
    let result = fixture.service.transition(
        &divergent_context,
        IncidentTransitionRequest {
            incident_id: incident.id,
            expected_version: incident.version,
            transition: IncidentTransition::Triage(TriageContext {
                business_impact: impact_at(
                    ImpactLevel::Critical,
                    "Checkout unavailable everywhere",
                ),
                owner: ACTOR,
                duplicate_checked: true,
            }),
        },
    );

    assert!(matches!(
        result,
        Err(IncidentServiceError::IdempotencyConflict)
    ));
}

#[test]
fn a_disposition_never_changes_status() {
    let mut fixture = Fixture::new();
    let incident = fixture.create_incident();
    let context = fixture.context();

    let dispositioned = fixture
        .service
        .set_disposition(
            &context,
            IncidentDispositionRequest {
                incident_id: incident.id,
                expected_version: incident.version,
                command: IncidentDispositionCommand {
                    disposition: Some(IncidentDisposition::Suppressed),
                    duplicate_of_incident_id: None,
                    reason: "Suppressed during the planned maintenance window".into(),
                },
            },
        )
        .expect("disposition is accepted");

    assert_eq!(
        dispositioned.incident.disposition,
        Some(IncidentDisposition::Suppressed)
    );
    assert_eq!(dispositioned.incident.status, IncidentStatus::Detected);
    assert_eq!(
        dispositioned.events[0].kind,
        IncidentEventKind::DispositionChanged
    );
}

#[test]
fn duplicate_disposition_rejects_an_unknown_incident_without_writing() {
    let mut fixture = Fixture::new();
    let incident = fixture.create_incident();
    let before_timeline = fixture
        .service
        .timeline(&fixture.read_context(), incident.id, None, 100)
        .expect("timeline is readable");
    let context = fixture.context();

    let result = fixture.service.set_disposition(
        &context,
        IncidentDispositionRequest {
            incident_id: incident.id,
            expected_version: incident.version,
            command: IncidentDispositionCommand {
                disposition: Some(IncidentDisposition::Duplicate),
                duplicate_of_incident_id: Some(Uuid::from_u128(0xdead)),
                reason: "This incident duplicates another report".into(),
            },
        },
    );

    assert!(matches!(result, Err(IncidentServiceError::NotFound)));
    assert_eq!(
        fixture
            .service
            .get(&fixture.read_context(), incident.id)
            .expect("incident is readable"),
        incident
    );
    assert_eq!(
        fixture
            .service
            .timeline(&fixture.read_context(), incident.id, None, 100)
            .expect("timeline is readable"),
        before_timeline
    );
}

#[test]
fn duplicate_disposition_rejects_an_incident_from_another_workspace() {
    let mut fixture = Fixture::new();
    let incident = fixture.create_incident();
    let mut other_context = fixture.context();
    other_context.workspace_scope = ResourceScope::workspace(OTHER_WORKSPACE, TEAM, ORGANIZATION);
    let other_incident = fixture
        .service
        .create(
            &other_context,
            IncidentCreateRequest {
                summary: "Checkout report from another workspace".into(),
                triggers: vec![IncidentTriggerInput::ManualReport {
                    observed_at: now(),
                    summary: "Checkout errors reported elsewhere".into(),
                    scope: ResourceScope::workspace(OTHER_WORKSPACE, TEAM, ORGANIZATION),
                }],
                business_impact: business_impact(),
                initial_roles: vec![],
            },
        )
        .expect("other-workspace creation succeeds")
        .incident;
    let before = fixture
        .service
        .get(&fixture.read_context(), incident.id)
        .expect("incident is readable");
    let context = fixture.context();

    let result = fixture.service.set_disposition(
        &context,
        IncidentDispositionRequest {
            incident_id: incident.id,
            expected_version: incident.version,
            command: IncidentDispositionCommand {
                disposition: Some(IncidentDisposition::Duplicate),
                duplicate_of_incident_id: Some(other_incident.id),
                reason: "This incident duplicates another report".into(),
            },
        },
    );

    assert!(matches!(result, Err(IncidentServiceError::NotFound)));
    assert_eq!(
        fixture
            .service
            .get(&fixture.read_context(), incident.id)
            .expect("incident is readable"),
        before
    );
}

#[test]
fn duplicate_disposition_accepts_an_incident_from_the_same_workspace() {
    let mut fixture = Fixture::new();
    let incident = fixture.create_incident();
    let target = fixture.create_incident();
    let context = fixture.context();

    let dispositioned = fixture
        .service
        .set_disposition(
            &context,
            IncidentDispositionRequest {
                incident_id: incident.id,
                expected_version: incident.version,
                command: IncidentDispositionCommand {
                    disposition: Some(IncidentDisposition::Duplicate),
                    duplicate_of_incident_id: Some(target.id),
                    reason: "This incident duplicates the second report".into(),
                },
            },
        )
        .expect("same-workspace duplicate is accepted");

    assert_eq!(
        dispositioned.incident.disposition,
        Some(IncidentDisposition::Duplicate)
    );
    assert_eq!(
        dispositioned.incident.duplicate_of_incident_id,
        Some(target.id)
    );
    assert_eq!(dispositioned.incident.version, 2);
}

#[test]
fn severity_reassessment_and_override_are_attributed() {
    let mut fixture = Fixture::new();
    let incident = fixture.create_incident();
    assert_eq!(incident.derived_severity, IncidentSeverity::S2);

    let context = fixture.context();
    let reassessed = fixture
        .service
        .set_severity(
            &context,
            IncidentSeverityRequest {
                incident_id: incident.id,
                expected_version: incident.version,
                command: IncidentSeverityCommand::Reassess {
                    business_impact: impact_at(
                        ImpactLevel::Critical,
                        "Checkout unavailable in every region",
                    ),
                    reason: "Impact widened to every production region".into(),
                },
            },
        )
        .expect("reassessment is accepted");
    assert_eq!(reassessed.incident.derived_severity, IncidentSeverity::S1);

    let context = fixture.context();
    let overridden = fixture
        .service
        .set_severity(
            &context,
            IncidentSeverityRequest {
                incident_id: reassessed.incident.id,
                expected_version: reassessed.incident.version,
                command: IncidentSeverityCommand::Override {
                    selected: IncidentSeverity::S2,
                    reason: "Traffic is already drained from the failing region".into(),
                    evidence_ids: vec!["evidence-checkout".into()],
                },
            },
        )
        .expect("override is accepted");

    let stored = fixture
        .service
        .get(&fixture.read_context(), incident.id)
        .expect("incident is readable");
    assert_eq!(stored, overridden.incident);
    let override_detail = stored.severity_override.expect("an override is recorded");
    assert_eq!(override_detail.actor_id, ACTOR);
    assert_eq!(override_detail.derived, IncidentSeverity::S1);
    assert_eq!(override_detail.selected, IncidentSeverity::S2);
}

#[test]
fn roles_are_assigned_replaced_and_released() {
    let mut fixture = Fixture::new();
    let incident = fixture.create_incident();

    let context = fixture.context();
    let assigned = fixture
        .service
        .assign_role(
            &context,
            IncidentRoleRequest {
                incident_id: incident.id,
                expected_version: incident.version,
                command: IncidentRoleCommand::Assign {
                    role: IncidentRole::IncidentCommander,
                    principal_id: COMMANDER,
                },
            },
        )
        .expect("assignment is accepted")
        .incident;

    let context = fixture.context();
    let replaced = fixture
        .service
        .assign_role(
            &context,
            IncidentRoleRequest {
                incident_id: assigned.id,
                expected_version: assigned.version,
                command: IncidentRoleCommand::Replace {
                    role: IncidentRole::IncidentCommander,
                    principal_id: STAKEHOLDER,
                },
            },
        )
        .expect("replacement is accepted")
        .incident;
    assert_eq!(
        replaced
            .roles
            .iter()
            .filter(|role| role.role == IncidentRole::IncidentCommander)
            .count(),
        1
    );

    let context = fixture.context();
    let released = fixture
        .service
        .assign_role(
            &context,
            IncidentRoleRequest {
                incident_id: replaced.id,
                expected_version: replaced.version,
                command: IncidentRoleCommand::Release {
                    role: IncidentRole::IncidentCommander,
                    principal_id: STAKEHOLDER,
                },
            },
        )
        .expect("release is accepted")
        .incident;

    let stored = fixture
        .service
        .get(&fixture.read_context(), incident.id)
        .expect("incident is readable");
    assert_eq!(stored, released);
    assert!(stored
        .roles
        .iter()
        .all(|role| role.role != IncidentRole::IncidentCommander));
    assert_eq!(stored.roles.len(), 1);
}

#[test]
fn reads_are_bounded_and_workspace_scoped() {
    let mut fixture = Fixture::new();
    let first = fixture.create_incident();
    let second = fixture.create_incident();
    let second = fixture.transition(&second, triage());

    let page = fixture
        .service
        .list(&fixture.read_context(), None, 1)
        .expect("list succeeds");
    assert_eq!(page.items.len(), 1);
    let cursor = page.next_cursor.expect("a continuation cursor");
    let next = fixture
        .service
        .list(&fixture.read_context(), Some(&cursor), 10)
        .expect("list succeeds");
    assert_eq!(next.items.len(), 1);
    assert_ne!(next.items[0].id, page.items[0].id);
    assert!([first.id, second.id].contains(&next.items[0].id));

    for limit in [0, 101] {
        assert!(matches!(
            fixture.service.list(&fixture.read_context(), None, limit),
            Err(IncidentServiceError::Store(_))
        ));
        assert!(matches!(
            fixture
                .service
                .timeline(&fixture.read_context(), first.id, None, limit),
            Err(IncidentServiceError::Store(_))
        ));
    }

    let other_workspace = IncidentCommandContext {
        workspace_scope: ResourceScope::workspace(OTHER_WORKSPACE, TEAM, ORGANIZATION),
        ..fixture.read_context()
    };
    assert!(matches!(
        fixture.service.get(&other_workspace, first.id),
        Err(IncidentServiceError::NotFound)
    ));
    assert!(matches!(
        fixture
            .service
            .timeline(&other_workspace, first.id, None, 10),
        Err(IncidentServiceError::NotFound)
    ));
    assert!(fixture
        .service
        .list(&other_workspace, None, 10)
        .expect("list succeeds")
        .items
        .is_empty());
}

#[test]
fn a_mutation_outside_the_workspace_is_not_found() {
    let mut fixture = Fixture::new();
    let incident = fixture.create_incident();
    let mut context = fixture.context();
    context.workspace_scope = ResourceScope::workspace(OTHER_WORKSPACE, TEAM, ORGANIZATION);

    assert!(matches!(
        fixture.service.transition(
            &context,
            IncidentTransitionRequest {
                incident_id: incident.id,
                expected_version: incident.version,
                transition: triage(),
            },
        ),
        Err(IncidentServiceError::NotFound)
    ));

    let stored = fixture
        .service
        .get(&fixture.read_context(), incident.id)
        .expect("incident is readable");
    assert_eq!(stored.status, IncidentStatus::Detected);
    assert_eq!(stored.version, 1);
}
