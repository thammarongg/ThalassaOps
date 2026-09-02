// SPDX-License-Identifier: Apache-2.0

//! Sprint 15 exit criterion, proved end to end from committed fixtures:
//! "Incidents can be created from alerts, anomalies, user reports, scheduled
//! health checks, vulnerability findings and manual reports, then progress
//! through a validated state machine."

use std::sync::Arc;

use chrono::{DateTime, TimeZone, Utc};
use serde_json::{json, Value};
use tempfile::{tempdir, TempDir};
use thalassa_domain::{
    BusinessImpact, ClosedContext, EnterpriseIdentity, ImpactDimensions, ImpactLevel,
    ImpactTrajectory, Incident, IncidentCreateRequest, IncidentDisposition,
    IncidentDispositionCommand, IncidentDispositionRequest, IncidentRole,
    IncidentRoleAssignmentInput, IncidentRoleCommand, IncidentRoleRequest, IncidentSeverity,
    IncidentSourceKind, IncidentStatus, IncidentTimelinePage, IncidentTransition,
    IncidentTransitionRequest, IncidentTriggerInput, InvestigatingContext, Membership,
    MitigatingContext, MonitoringContext, Principal, PrincipalId, PrincipalKind, ReopenedContext,
    ResolvedContext, ResourceScope, TriageContext,
};
use thalassa_ipc::{Capability, CommandEnvelope, CommandName, IpcErrorCode};
use thalassa_policy::{DataClass, PolicyDocument, PolicyRuntime};
use thalassaops::app::{AppState, IpcResult};
use thalassaops::connectors::InMemoryCredentialStore;
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
const ACTOR: PrincipalId = Uuid::from_u128(0xa0);
const REPORTER: PrincipalId = Uuid::from_u128(0xa2);
const COMMANDER: PrincipalId = Uuid::from_u128(0xa1);
const POLICY_VERSION: u64 = 7;

const USER_REPORT: &str =
    include_str!("../../docs/superpowers/fixtures/2026-08-30-incident/user-report.json");
const MANUAL_REPORT: &str =
    include_str!("../../docs/superpowers/fixtures/2026-08-30-incident/manual-report.json");

fn workspace_scope() -> ResourceScope {
    ResourceScope::workspace(WORKSPACE, TEAM, ORGANIZATION)
}

fn environment_scope() -> ResourceScope {
    ResourceScope::environment(ENVIRONMENT, WORKSPACE, TEAM, ORGANIZATION)
}

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 28, 9, 0, 0).unwrap()
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

fn business_impact() -> BusinessImpact {
    impact_at(ImpactLevel::High, "Checkout unavailable for customers")
}

struct Acceptance {
    _directory: TempDir,
    service: IncidentService,
    next_request: u128,
}

fn seed_principals(database_path: &std::path::Path, workspace_id: Uuid, ids: &[PrincipalId]) {
    let connection = rusqlite::Connection::open(database_path).expect("the database opens");
    connection
        .execute_batch(include_str!("../migrations/0001_local_workspace.sql"))
        .expect("the identity schema applies");
    for principal_id in ids {
        let principal = Principal {
            id: *principal_id,
            kind: PrincipalKind::Local,
            display_name: format!("Principal {principal_id}"),
            identity: EnterpriseIdentity {
                subject: principal_id.to_string(),
                ..Default::default()
            },
            created_at: now(),
        };
        let membership = Membership::workspace_owner(*principal_id, workspace_id);
        connection
            .execute(
                "INSERT INTO principals (id, document_json) VALUES (?1, ?2)",
                rusqlite::params![
                    principal_id.to_string(),
                    serde_json::to_string(&principal).expect("principal serializes")
                ],
            )
            .expect("principal inserts");
        connection
            .execute(
                "INSERT INTO memberships (id, document_json) VALUES (?1, ?2)",
                rusqlite::params![
                    principal_id.to_string(),
                    serde_json::to_string(&membership).expect("membership serializes")
                ],
            )
            .expect("membership inserts");
    }
}

impl Acceptance {
    fn new() -> Self {
        let directory = tempdir().expect("temporary directory");
        let repository =
            SqliteIncidentRepository::open(&directory.path().join("incidents.sqlite3"))
                .expect("repository opens");
        seed_principals(
            &directory.path().join("incidents.sqlite3"),
            WORKSPACE,
            &[
                ACTOR,
                COMMANDER,
                Uuid::from_u128(0xa5),
                Uuid::from_u128(0xa6),
            ],
        );
        let mut records = SourceRecordStore::with_scope(environment_scope());
        let resolver = IncidentSourceResolver::replay(&environment_scope(), &mut records)
            .expect("the committed replay catalog resolves");
        // Fixture discipline: an empty replay association fails silently, so
        // assert every source-backed kind resolved before anything leans on
        // the catalog.
        for kind in [
            IncidentSourceKind::Alert,
            IncidentSourceKind::Anomaly,
            IncidentSourceKind::ScheduledHealthCheck,
            IncidentSourceKind::VulnerabilityFinding,
        ] {
            assert!(
                !resolver.signal_ids(kind).is_empty(),
                "the committed replay catalog must carry a {kind:?} signal"
            );
        }
        Self {
            _directory: directory,
            service: IncidentService::new(resolver, repository),
            next_request: 0x2000,
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

    fn source_input(&self, kind: IncidentSourceKind, index: usize) -> IncidentTriggerInput {
        let source_id = self
            .service
            .resolver()
            .signal_ids(kind)
            .get(index)
            .copied()
            .unwrap_or_else(|| panic!("the replay catalog carries a {kind:?} signal"))
            .to_string();
        match kind {
            IncidentSourceKind::Alert => IncidentTriggerInput::Alert { source_id },
            IncidentSourceKind::Anomaly => IncidentTriggerInput::Anomaly { source_id },
            IncidentSourceKind::ScheduledHealthCheck => {
                IncidentTriggerInput::ScheduledHealthCheck { source_id }
            }
            IncidentSourceKind::VulnerabilityFinding => {
                IncidentTriggerInput::VulnerabilityFinding { source_id }
            }
            other => panic!("{other:?} is not source backed"),
        }
    }

    fn create(&mut self, triggers: Vec<IncidentTriggerInput>) -> Incident {
        let context = self.context();
        self.service
            .create(
                &context,
                IncidentCreateRequest {
                    summary: "Checkout errors under investigation".into(),
                    triggers,
                    business_impact: business_impact(),
                    initial_roles: vec![IncidentRoleAssignmentInput {
                        role: IncidentRole::Owner,
                        principal_id: ACTOR,
                    }],
                },
            )
            .expect("explicit creation succeeds")
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

    fn timeline(&self, incident_id: Uuid) -> IncidentTimelinePage {
        self.service
            .timeline(&self.read_context(), incident_id, None, 100)
            .expect("timeline is readable")
    }
}

fn user_report_input() -> IncidentTriggerInput {
    let document: Value =
        serde_json::from_str(USER_REPORT).expect("the committed user report parses");
    IncidentTriggerInput::UserReport {
        reporter_id: REPORTER,
        observed_at: document["observed_at"]
            .as_str()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .expect("a valid observation time")
            .with_timezone(&Utc),
        summary: document["summary"]
            .as_str()
            .expect("a report summary")
            .to_owned(),
        scope: serde_json::from_value(document["scope"].clone()).expect("a valid scope"),
    }
}

fn manual_report_input() -> IncidentTriggerInput {
    let document: Value =
        serde_json::from_str(MANUAL_REPORT).expect("the committed manual report parses");
    IncidentTriggerInput::ManualReport {
        observed_at: document["observed_at"]
            .as_str()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .expect("a valid observation time")
            .with_timezone(&Utc),
        summary: document["summary"]
            .as_str()
            .expect("a report summary")
            .to_owned(),
        scope: serde_json::from_value(document["scope"].clone()).expect("a valid scope"),
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
fn sprint_15_exit_criterion_is_reachable_from_committed_fixtures() {
    let mut fixture = Acceptance::new();

    let mut created = Vec::new();
    for kind in [
        IncidentSourceKind::Alert,
        IncidentSourceKind::Anomaly,
        IncidentSourceKind::ScheduledHealthCheck,
        IncidentSourceKind::VulnerabilityFinding,
    ] {
        let input = fixture.source_input(kind, 0);
        created.push(fixture.create(vec![input]));
    }
    created.push(fixture.create(vec![user_report_input()]));
    created.push(fixture.create(vec![manual_report_input()]));

    assert_eq!(created.len(), 6);
    assert!(created
        .iter()
        .all(|incident| incident.status == IncidentStatus::Detected));
    assert!(created.iter().all(|incident| incident.version == 1));

    // A responder starting from a correlation candidate submits the selected
    // underlying signals; there is no correlation_candidate trigger kind.
    let selected = vec![
        fixture.source_input(IncidentSourceKind::Alert, 0),
        fixture.source_input(IncidentSourceKind::Anomaly, 0),
    ];
    let multi = fixture.create(selected);
    assert!(multi.trigger_ids.len() >= 2);
    let mut expected_signals = vec![
        fixture
            .service
            .resolver()
            .signal_ids(IncidentSourceKind::Alert)[0],
        fixture
            .service
            .resolver()
            .signal_ids(IncidentSourceKind::Anomaly)[0],
    ];
    expected_signals.sort();
    assert_eq!(multi.signal_ids, expected_signals);

    // The persisted trigger rows carry only supported source kinds; a
    // correlation candidate is never a trigger.
    let connection = rusqlite::Connection::open_with_flags(
        fixture._directory.path().join("incidents.sqlite3"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("the incident store opens read-only");
    let kinds: Vec<String> = connection
        .prepare(
            "SELECT source_kind FROM incident_trigger WHERE incident_id = ?1 \
             ORDER BY source_kind",
        )
        .expect("the trigger table is queryable")
        .query_map([multi.id.to_string()], |row| row.get(0))
        .expect("trigger rows are readable")
        .collect::<Result<_, _>>()
        .expect("trigger kinds decode as text");
    assert_eq!(kinds, ["alert", "anomaly"]);

    let incident = fixture.transition(&multi, triage());
    let incident = fixture.transition(&incident, investigating());
    let incident = fixture.transition(&incident, mitigating());
    let incident = fixture.transition(&incident, monitoring());
    let incident = fixture.transition(&incident, resolved());
    let closed_incident = fixture.transition(&incident, closed());
    assert_eq!(closed_incident.status, IncidentStatus::Closed);

    let reopened_incident = fixture.transition(&closed_incident, reopened());
    assert_eq!(reopened_incident.status, IncidentStatus::Reopened);
    let investigating_again = fixture.transition(&reopened_incident, investigating());
    assert_eq!(investigating_again.status, IncidentStatus::Investigating);

    let timeline = fixture.timeline(investigating_again.id);
    assert!(timeline
        .events
        .windows(2)
        .all(|pair| pair[0].sequence < pair[1].sequence));
    assert!(timeline.events.iter().all(|event| !event.actor_id.is_nil()
        && !event.request_id.is_nil()
        && event.policy_version == POLICY_VERSION));
    assert_eq!(fixture.service.incident_count().unwrap(), 7);
}

#[test]
fn every_disposition_is_recorded_without_transition_or_merge() {
    let mut fixture = Acceptance::new();
    let other = fixture.create(vec![fixture.source_input(IncidentSourceKind::Alert, 0)]);

    for (index, disposition) in [
        IncidentDisposition::Duplicate,
        IncidentDisposition::FalsePositive,
        IncidentDisposition::Suppressed,
        IncidentDisposition::Cancelled,
        IncidentDisposition::Informational,
    ]
    .into_iter()
    .enumerate()
    {
        let subject = fixture.create(vec![IncidentTriggerInput::ManualReport {
            observed_at: now(),
            summary: format!("Operator report number {index}"),
            scope: environment_scope(),
        }]);
        let duplicate_of = (disposition == IncidentDisposition::Duplicate).then_some(other.id);
        let expected = disposition.clone();
        let context = fixture.context();
        let applied = fixture
            .service
            .set_disposition(
                &context,
                IncidentDispositionRequest {
                    incident_id: subject.id,
                    expected_version: subject.version,
                    command: IncidentDispositionCommand {
                        disposition: Some(disposition),
                        duplicate_of_incident_id: duplicate_of,
                        reason: "Reviewed during triage handover".into(),
                    },
                },
            )
            .expect("disposition is accepted")
            .incident;

        assert_eq!(applied.disposition, Some(expected));
        assert_eq!(applied.status, IncidentStatus::Detected);
        assert_eq!(applied.duplicate_of_incident_id, duplicate_of);
    }

    // The duplicate target is untouched: a disposition never merges incidents.
    let stored = fixture
        .service
        .get(&fixture.read_context(), other.id)
        .expect("the duplicate target is readable");
    assert_eq!(stored, other);
}

#[test]
fn every_responder_role_can_be_held_and_one_principal_may_hold_several() {
    let mut fixture = Acceptance::new();
    let mut incident = fixture.create(vec![manual_report_input()]);
    // S1/S2 staffing: the incident is already S2 from its High availability
    // impact, and every responder role is assignable on it.
    assert_eq!(incident.derived_severity, IncidentSeverity::S2);

    for role in [
        IncidentRole::IncidentCommander,
        IncidentRole::TechnicalLead,
        IncidentRole::CommunicationsLead,
        IncidentRole::Approver,
        IncidentRole::ChangeOwner,
    ] {
        let context = fixture.context();
        incident = fixture
            .service
            .assign_role(
                &context,
                IncidentRoleRequest {
                    incident_id: incident.id,
                    expected_version: incident.version,
                    command: IncidentRoleCommand::Assign {
                        role,
                        principal_id: COMMANDER,
                    },
                },
            )
            .expect("assignment is accepted")
            .incident;
    }

    for stakeholder in [Uuid::from_u128(0xa5), Uuid::from_u128(0xa6)] {
        let context = fixture.context();
        incident = fixture
            .service
            .assign_role(
                &context,
                IncidentRoleRequest {
                    incident_id: incident.id,
                    expected_version: incident.version,
                    command: IncidentRoleCommand::Assign {
                        role: IncidentRole::Stakeholder,
                        principal_id: stakeholder,
                    },
                },
            )
            .expect("stakeholders may be many")
            .incident;
    }

    // Owner plus five exclusive roles plus two stakeholders.
    assert_eq!(incident.roles.len(), 8);
    assert_eq!(
        incident
            .roles
            .iter()
            .filter(|assignment| assignment.principal_id == COMMANDER)
            .count(),
        5,
        "one principal may hold several distinct roles"
    );

    let stored = fixture
        .service
        .get(&fixture.read_context(), incident.id)
        .expect("incident is readable");
    assert_eq!(stored, incident);
}

#[test]
fn a_stale_writer_changes_neither_state_nor_timeline() {
    let mut fixture = Acceptance::new();
    let incident = fixture.create(vec![manual_report_input()]);
    let triaged = fixture.transition(&incident, triage());

    let before_state = fixture
        .service
        .get(&fixture.read_context(), incident.id)
        .expect("incident is readable");
    let before_timeline = fixture.timeline(incident.id);

    let context = fixture.context();
    let stale = fixture.service.transition(
        &context,
        IncidentTransitionRequest {
            incident_id: incident.id,
            expected_version: incident.version,
            transition: triage(),
        },
    );
    assert!(matches!(
        stale,
        Err(IncidentServiceError::VersionConflict {
            expected: 1,
            actual: 2
        })
    ));

    assert_eq!(
        fixture
            .service
            .get(&fixture.read_context(), incident.id)
            .expect("incident is readable"),
        before_state
    );
    assert_eq!(fixture.timeline(incident.id), before_timeline);
    assert_eq!(triaged.version, 2);
}

fn ipc_state() -> (TempDir, AppState) {
    let directory = tempdir().expect("temporary directory");
    let state = AppState::open_with_credential_store(
        directory.path().join("thalassaops.sqlite"),
        Arc::new(InMemoryCredentialStore::default()),
    )
    .expect("app state opens");
    (directory, state)
}

fn ipc_create_envelope(state: &AppState) -> CommandEnvelope<Value> {
    CommandEnvelope {
        request_id: Uuid::new_v4(),
        command: CommandName::new("incident", "create").unwrap(),
        capability: Capability::IncidentWrite,
        scope: ResourceScope::default(),
        payload: json!({
            "summary": "Checkout errors reported by the on-call operator",
            "triggers": [{
                "kind": "manual_report",
                "observed_at": "2026-08-28T08:57:30Z",
                "summary": "Operator opening an incident for elevated checkout errors",
                "scope": {
                    "organization_id": state.bootstrap.organization.id,
                    "team_id": state.bootstrap.team.id,
                    "workspace_id": state.bootstrap.workspace.id,
                    "environment_id": null,
                    "resource_ids": []
                }
            }],
            "business_impact": {
                "level": "high",
                "summary": "Checkout unavailable for customers",
                "customer_scope": "production customers",
                "service_criticality": "tier-0",
                "trajectory": "stable",
                "dimensions": {
                    "availability": "high",
                    "customer_reach": "none",
                    "business_criticality": "none",
                    "data_integrity": "none",
                    "security_privacy": "none",
                    "financial_contractual": "none",
                    "trajectory": "stable",
                    "production": true
                },
                "evidence_ids": ["evidence-checkout"]
            },
            "initial_roles": [{
                "role": "owner",
                "principal_id": state.bootstrap.principal.id
            }]
        }),
    }
}

fn ipc_list_count(state: &AppState) -> usize {
    let envelope = CommandEnvelope {
        request_id: Uuid::new_v4(),
        command: CommandName::new("incident", "list").unwrap(),
        capability: Capability::IncidentRead,
        scope: ResourceScope::default(),
        payload: json!({ "cursor": null, "limit": 100 }),
    };
    match state.incident_list(envelope) {
        IpcResult::Ok { value, .. } => value.items.len(),
        IpcResult::Err { error, .. } => panic!("incident.list should succeed: {error:?}"),
    }
}

#[test]
fn a_policy_denied_write_changes_nothing() {
    let (_directory, mut state) = ipc_state();
    assert_eq!(ipc_list_count(&state), 0);

    state.policy = PolicyRuntime::load(
        PolicyDocument::baseline(14).with_audit_log_data_classes(vec![DataClass::Public]),
    )
    .expect("a restrictive policy loads");

    let denied = state.incident_create(ipc_create_envelope(&state));
    let IpcResult::Err { error, .. } = denied else {
        panic!("audit retention policy must deny the write")
    };
    assert_eq!(error.code, IpcErrorCode::PolicyDenied);
    assert_eq!(error.message, "incident audit retention policy denied");

    // Restore the baseline policy only to read back: nothing was written.
    state.policy = PolicyRuntime::load(PolicyDocument::baseline(14)).expect("baseline loads");
    assert_eq!(ipc_list_count(&state), 0);

    match state.incident_create(ipc_create_envelope(&state)) {
        IpcResult::Ok { .. } => {}
        IpcResult::Err { error, .. } => {
            panic!("baseline create must be accepted: {error:?}")
        }
    }
    assert_eq!(ipc_list_count(&state), 1);
}
