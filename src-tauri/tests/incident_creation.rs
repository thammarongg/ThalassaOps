// SPDX-License-Identifier: Apache-2.0

//! Task 5 proofs: an incident is created only from an explicit command, from
//! each of the six supported trigger kinds, and never as a side effect of
//! replay or trigger resolution.

use chrono::{DateTime, TimeZone, Utc};
use tempfile::TempDir;
use thalassa_domain::{
    BusinessImpact, ImpactDimensions, ImpactLevel, ImpactTrajectory, IncidentCreateRequest,
    IncidentRole, IncidentRoleAssignmentInput, IncidentSourceKind, IncidentStatus,
    IncidentTriggerInput, PrincipalId, ResourceScope,
};
use thalassaops::correlation::SourceRecordStore;
use thalassaops::incident::{
    replay_incident_signals, IncidentCommandContext, IncidentService, IncidentServiceError,
    IncidentSourceResolver, SqliteIncidentRepository,
};
use uuid::Uuid;

const ORGANIZATION: Uuid = Uuid::from_u128(0x11);
const TEAM: Uuid = Uuid::from_u128(0x12);
const WORKSPACE: Uuid = Uuid::from_u128(0x13);
const ENVIRONMENT: Uuid = Uuid::from_u128(0x14);
const OTHER_WORKSPACE: Uuid = Uuid::from_u128(0x99);
const ACTOR: PrincipalId = Uuid::from_u128(0xa0);
const REPORTER: PrincipalId = Uuid::from_u128(0xa2);
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

/// The committed report documents, parsed exactly as a client would submit
/// them, so the fixtures and the wire contract cannot drift apart.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct UserReportFixture {
    reporter_id: PrincipalId,
    observed_at: DateTime<Utc>,
    summary: String,
    scope: ResourceScope,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ManualReportFixture {
    observed_at: DateTime<Utc>,
    summary: String,
    scope: ResourceScope,
}

fn user_report_input() -> IncidentTriggerInput {
    let fixture: UserReportFixture =
        serde_json::from_str(USER_REPORT).expect("the committed user report parses");
    assert_eq!(fixture.reporter_id, REPORTER);
    IncidentTriggerInput::UserReport {
        reporter_id: fixture.reporter_id,
        observed_at: fixture.observed_at,
        summary: fixture.summary,
        scope: fixture.scope,
    }
}

fn manual_report_input() -> IncidentTriggerInput {
    let fixture: ManualReportFixture =
        serde_json::from_str(MANUAL_REPORT).expect("the committed manual report parses");
    IncidentTriggerInput::ManualReport {
        observed_at: fixture.observed_at,
        summary: fixture.summary,
        scope: fixture.scope,
    }
}

struct Fixture {
    _directory: TempDir,
    service: IncidentService,
}

fn service_fixture() -> Fixture {
    let directory = TempDir::new().expect("temporary directory");
    let repository = SqliteIncidentRepository::open(&directory.path().join("incidents.sqlite3"))
        .expect("repository opens");
    let mut records = SourceRecordStore::with_scope(environment_scope());
    let resolver = IncidentSourceResolver::replay(&environment_scope(), &mut records)
        .expect("the committed replay catalog resolves");
    Fixture {
        _directory: directory,
        service: IncidentService::new(resolver, repository),
    }
}

fn context(request_id: Uuid) -> IncidentCommandContext {
    IncidentCommandContext {
        workspace_scope: workspace_scope(),
        actor_id: ACTOR,
        policy_version: POLICY_VERSION,
        request_id,
        now: now(),
    }
}

fn request(triggers: Vec<IncidentTriggerInput>) -> IncidentCreateRequest {
    IncidentCreateRequest {
        summary: "Checkout errors under investigation".into(),
        triggers,
        business_impact: business_impact(),
        initial_roles: vec![IncidentRoleAssignmentInput {
            role: IncidentRole::Owner,
            principal_id: ACTOR,
        }],
    }
}

fn source_backed_inputs(service: &IncidentService) -> Vec<IncidentTriggerInput> {
    [
        IncidentSourceKind::Alert,
        IncidentSourceKind::Anomaly,
        IncidentSourceKind::ScheduledHealthCheck,
        IncidentSourceKind::VulnerabilityFinding,
    ]
    .into_iter()
    .map(|kind| {
        let source_id = service
            .resolver()
            .signal_ids(kind)
            .first()
            .copied()
            .unwrap_or_else(|| panic!("the replay catalog carries a {kind:?} signal"))
            .to_string();
        match kind {
            IncidentSourceKind::Alert => IncidentTriggerInput::Alert { source_id },
            IncidentSourceKind::Anomaly => IncidentTriggerInput::Anomaly { source_id },
            IncidentSourceKind::ScheduledHealthCheck => {
                IncidentTriggerInput::ScheduledHealthCheck { source_id }
            }
            _ => IncidentTriggerInput::VulnerabilityFinding { source_id },
        }
    })
    .collect()
}

#[test]
fn explicit_creation_resolves_all_six_source_kinds() {
    let mut fixture = service_fixture();
    let mut inputs = source_backed_inputs(&fixture.service);
    inputs.push(user_report_input());
    inputs.push(manual_report_input());
    assert_eq!(inputs.len(), 6);

    for (index, input) in inputs.into_iter().enumerate() {
        let request_id = Uuid::from_u128(0xb000 + index as u128);
        let result = fixture
            .service
            .create(&context(request_id), request(vec![input]))
            .expect("explicit creation succeeds");
        assert_eq!(result.incident.status, IncidentStatus::Detected);
        assert_eq!(result.incident.version, 1);
        assert_eq!(result.incident.trigger_ids.len(), 1);
        assert!(!result.incident.evidence_ids.is_empty());
        assert_eq!(result.events.len(), 2);
    }

    assert_eq!(fixture.service.incident_count().unwrap(), 6);
}

#[test]
fn one_incident_can_carry_several_selected_signals() {
    let mut fixture = service_fixture();
    let inputs = source_backed_inputs(&fixture.service);
    let created = fixture
        .service
        .create(&context(Uuid::from_u128(0xb1)), request(inputs))
        .expect("multi-trigger creation succeeds");

    assert_eq!(created.incident.trigger_ids.len(), 4);
    assert_eq!(created.incident.signal_ids.len(), 4);
    assert_eq!(fixture.service.incident_count().unwrap(), 1);
}

#[test]
fn replay_and_trigger_resolution_do_not_create_incidents() {
    let fixture = service_fixture();
    let mut records = SourceRecordStore::with_scope(environment_scope());
    let signals =
        replay_incident_signals(&environment_scope(), &mut records).expect("replay succeeds");
    assert!(
        !signals.is_empty(),
        "the committed replay catalog must normalize at least one signal"
    );

    for kind in [
        IncidentSourceKind::Alert,
        IncidentSourceKind::Anomaly,
        IncidentSourceKind::ScheduledHealthCheck,
        IncidentSourceKind::VulnerabilityFinding,
    ] {
        let ids = fixture.service.resolver().signal_ids(kind);
        assert!(!ids.is_empty(), "{kind:?} must be resolvable from replay");
        for id in ids {
            fixture
                .service
                .resolver()
                .resolve(kind, &id.to_string(), &workspace_scope())
                .expect("a replayed signal resolves");
        }
    }

    assert_eq!(fixture.service.incident_count().unwrap(), 0);
}

#[test]
fn mixed_scope_or_sensitive_report_fails_without_partial_incident() {
    let mut fixture = service_fixture();

    let mixed = request(vec![IncidentTriggerInput::ManualReport {
        observed_at: now(),
        summary: "Operator report from another workspace".into(),
        scope: ResourceScope::workspace(OTHER_WORKSPACE, TEAM, ORGANIZATION),
    }]);
    assert!(matches!(
        fixture
            .service
            .create(&context(Uuid::from_u128(0xc1)), mixed),
        Err(IncidentServiceError::ScopeMismatch)
    ));

    let secret = request(vec![IncidentTriggerInput::ManualReport {
        observed_at: now(),
        summary: "token=sk-live-example".into(),
        scope: environment_scope(),
    }]);
    assert!(matches!(
        fixture
            .service
            .create(&context(Uuid::from_u128(0xc2)), secret),
        Err(IncidentServiceError::SensitiveContent)
    ));

    assert_eq!(fixture.service.incident_count().unwrap(), 0);
}

#[test]
fn unknown_and_mismatched_sources_are_rejected_before_any_write() {
    let mut fixture = service_fixture();

    let unknown = request(vec![IncidentTriggerInput::Alert {
        source_id: Uuid::from_u128(0xdead).to_string(),
    }]);
    assert!(matches!(
        fixture
            .service
            .create(&context(Uuid::from_u128(0xc3)), unknown),
        Err(IncidentServiceError::UnknownSource)
    ));

    let alert_id = fixture
        .service
        .resolver()
        .signal_ids(IncidentSourceKind::Alert)[0]
        .to_string();
    let mismatched = request(vec![IncidentTriggerInput::Anomaly {
        source_id: alert_id,
    }]);
    assert!(matches!(
        fixture
            .service
            .create(&context(Uuid::from_u128(0xc4)), mismatched),
        Err(IncidentServiceError::SourceKindMismatch)
    ));

    let malformed = request(vec![IncidentTriggerInput::Alert {
        source_id: "not-a-signal-id".into(),
    }]);
    assert!(matches!(
        fixture
            .service
            .create(&context(Uuid::from_u128(0xc5)), malformed),
        Err(IncidentServiceError::UnknownSource)
    ));

    assert!(fixture
        .service
        .create(&context(Uuid::from_u128(0xc6)), request(Vec::new()))
        .is_err());

    assert_eq!(fixture.service.incident_count().unwrap(), 0);
}

#[test]
fn a_retried_creation_returns_the_same_incident() {
    let mut fixture = service_fixture();
    let inputs = source_backed_inputs(&fixture.service);
    let request_id = Uuid::from_u128(0xd1);

    let first = fixture
        .service
        .create(&context(request_id), request(inputs.clone()))
        .expect("first creation succeeds");
    let retried = fixture
        .service
        .create(&context(request_id), request(inputs))
        .expect("the retry replays the stored incident");

    assert_eq!(first, retried);
    assert_eq!(fixture.service.incident_count().unwrap(), 1);
}

#[test]
fn a_reused_request_id_with_different_content_is_rejected() {
    let mut fixture = service_fixture();
    let request_id = Uuid::from_u128(0xd2);
    let inputs = source_backed_inputs(&fixture.service);

    fixture
        .service
        .create(&context(request_id), request(inputs.clone()))
        .expect("first creation succeeds");

    let mut divergent = request(inputs);
    divergent.summary = "A different incident under the same request".into();
    assert!(matches!(
        fixture.service.create(&context(request_id), divergent),
        Err(IncidentServiceError::IdempotencyConflict)
    ));
    assert_eq!(fixture.service.incident_count().unwrap(), 1);
}

#[test]
fn a_source_without_any_observation_time_is_unresolvable() {
    let mut records = SourceRecordStore::with_scope(environment_scope());
    let mut signals =
        replay_incident_signals(&environment_scope(), &mut records).expect("replay succeeds");

    // Observation time is source data: it falls back to the ingest time and is
    // never replaced by the command clock.
    for signal in &mut signals {
        signal.observed_at = None;
    }
    let fallback = IncidentSourceResolver::from_signals(signals.clone()).expect("resolver builds");
    let alert = fallback.signal_ids(IncidentSourceKind::Alert)[0];
    let resolved = fallback
        .resolve(
            IncidentSourceKind::Alert,
            &alert.to_string(),
            &workspace_scope(),
        )
        .expect("ingest time stands in for a missing observation time");
    assert_eq!(
        resolved.observed_at.to_rfc3339(),
        "2026-08-28T09:00:00+00:00"
    );

    for signal in &mut signals {
        signal.ingested_at = None;
    }
    let unresolvable = IncidentSourceResolver::from_signals(signals).expect("resolver builds");
    assert!(matches!(
        unresolvable.resolve(
            IncidentSourceKind::Alert,
            &alert.to_string(),
            &workspace_scope()
        ),
        Err(IncidentServiceError::UnresolvableSource)
    ));
}
