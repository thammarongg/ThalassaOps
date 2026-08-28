// SPDX-License-Identifier: Apache-2.0

use std::sync::{mpsc, Arc, Barrier};
use std::thread;
use thalassa_domain::{FixtureHealthCheck, HealthCheckOutcome, HealthCheckSchedule};
use thalassa_policy::{DataClass, PolicyDocument, PolicyRuntime};
use thalassaops::operations::{
    audit_for, fixture_catalog, fixture_time, is_due, run_due_checks, run_due_checks_with_policy,
    DueState, FixedClock, FixtureLookup, HealthCheckError, HealthCheckPolicy, HealthCheckScheduler,
};

#[test]
fn scheduled_health_checks_classify_timeout_cooldown_and_audit_deterministically() {
    let catalog = fixture_catalog();
    let runs = run_due_checks(
        &catalog.health_checks,
        &catalog.health_check_results,
        fixture_time(),
        7,
    )
    .expect("fixture schedules should evaluate");

    let api = runs
        .iter()
        .find(|run| run.schedule_id == "check-api-health")
        .expect("api check should be present");
    assert_eq!(api.outcome, HealthCheckOutcome::Healthy);
    assert_eq!(api.audit.triggered_by, "scheduler");
    assert_eq!(api.audit.policy_version, 7);

    let db = runs
        .iter()
        .find(|run| run.schedule_id == "check-db-health")
        .expect("db check should be present");
    assert_eq!(db.outcome, HealthCheckOutcome::SkippedCooldown);
    assert!(db.audit.cooldown_suppressed);
    assert_eq!(db.audit.duration_ms, 0);

    let timeout = runs
        .iter()
        .find(|run| run.schedule_id == "check-worker-timeout")
        .expect("worker check should be present");
    assert_eq!(timeout.outcome, HealthCheckOutcome::TimedOut);
    assert_eq!(timeout.audit.duration_ms, 100);
}

#[test]
fn interval_selection_reports_due_not_due_and_disabled_without_running_a_fixture() {
    let catalog = fixture_catalog();
    let now = fixture_time();

    let mut not_due = catalog.health_checks[0].clone();
    not_due.last_run_at = Some("2026-08-28T08:59:00Z".into());
    assert_eq!(is_due(&not_due, FixedClock::new(now)), Ok(DueState::NotDue));

    let mut due = not_due.clone();
    due.last_run_at = Some("2026-08-28T08:55:00Z".into());
    assert_eq!(is_due(&due, FixedClock::new(now)), Ok(DueState::Due));

    let mut disabled = due;
    disabled.enabled = false;
    assert_eq!(
        is_due(&disabled, FixedClock::new(now)),
        Ok(DueState::Disabled)
    );
}

#[test]
fn result_and_audit_preserve_scope_and_have_stable_run_ids() {
    let catalog = fixture_catalog();
    let now = fixture_time();
    let first = run_due_checks(
        &catalog.health_checks,
        &catalog.health_check_results,
        FixedClock::new(now),
        7,
    )
    .unwrap();
    let second = run_due_checks(
        &catalog.health_checks,
        &catalog.health_check_results,
        FixedClock::new(now),
        7,
    )
    .unwrap();
    let mut reversed_schedules = catalog.health_checks.clone();
    reversed_schedules.reverse();
    let reversed = run_due_checks(
        &reversed_schedules,
        &catalog.health_check_results,
        FixedClock::new(now),
        7,
    )
    .unwrap();

    assert_eq!(first, second);
    assert_eq!(first, reversed);
    for result in &first {
        let schedule = catalog
            .health_checks
            .iter()
            .find(|schedule| schedule.id == result.schedule_id)
            .unwrap();
        assert_eq!(result.audit.scope, schedule.scope);
        assert_eq!(result.audit.schedule_id, result.schedule_id);
        assert_eq!(result.observed_at, "2026-08-28T09:00:00Z");
        assert_eq!(audit_for(result), &result.audit);
    }
}

struct DeniedPolicy;

impl HealthCheckPolicy for DeniedPolicy {
    fn authorize(&self, _scope: &thalassa_domain::ResourceScope) -> Result<(), HealthCheckError> {
        Err(HealthCheckError::PolicyDenied)
    }
}

#[test]
fn policy_denied_scope_is_an_explicit_error() {
    let catalog = fixture_catalog();
    let error = run_due_checks_with_policy(
        &catalog.health_checks,
        &catalog.health_check_results,
        fixture_time(),
        7,
        &DeniedPolicy,
    )
    .expect_err("a denied check must not be silently skipped");
    assert_eq!(error, HealthCheckError::PolicyDenied);
    assert!(error.to_string().contains("policy"));
}

#[test]
fn default_policy_refuses_an_unbounded_target_scope() {
    let catalog = fixture_catalog();
    let mut denied = catalog.health_checks[0].clone();
    denied.scope = thalassa_domain::ResourceScope::default();
    let error = run_due_checks(&[denied], &catalog.health_check_results, fixture_time(), 7)
        .expect_err("an unbounded scope must fail the policy gate");
    assert_eq!(error, HealthCheckError::PolicyDenied);
}

#[test]
fn existing_policy_runtime_must_allow_internal_audit_egress() {
    let catalog = fixture_catalog();
    let policy = PolicyRuntime::load(
        PolicyDocument::baseline(9).with_audit_log_data_classes(vec![DataClass::Public]),
    )
    .unwrap();
    let error = run_due_checks(
        &catalog.health_checks,
        &catalog.health_check_results,
        fixture_time(),
        policy,
    )
    .expect_err("health-check audit metadata must fail closed when audit egress is denied");
    assert_eq!(error, HealthCheckError::PolicyDenied);
}

#[test]
fn every_result_has_auditable_metadata_without_provider_secrets() {
    let catalog = fixture_catalog();
    let runs = run_due_checks(
        &catalog.health_checks,
        &catalog.health_check_results,
        fixture_time(),
        7,
    )
    .unwrap();
    assert_eq!(runs.len(), catalog.health_checks.len());
    for run in &runs {
        assert!(!run.audit.run_id.is_empty());
        assert_eq!(run.audit.triggered_by, "scheduler");
        assert_eq!(run.audit.policy_version, 7);
        assert!(run.audit.scope.is_bounded());
    }
    let serialized = serde_json::to_string(&runs).unwrap();
    for secret in ["authorization", "credential", "raw provider response"] {
        assert!(!serialized.to_ascii_lowercase().contains(secret));
    }
}

#[derive(Clone)]
struct BlockingFixtures {
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
    result: FixtureHealthCheck,
}

impl FixtureLookup for BlockingFixtures {
    fn lookup(&self, _schedule: &HealthCheckSchedule) -> Option<&FixtureHealthCheck> {
        self.entered.wait();
        self.release.wait();
        Some(&self.result)
    }
}

#[test]
fn shared_scheduler_refuses_concurrent_runs_for_the_same_schedule() {
    let catalog = fixture_catalog();
    let schedule = catalog.health_checks[0].clone();
    let schedules = Arc::new(vec![schedule]);
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let fixtures = Arc::new(BlockingFixtures {
        entered: entered.clone(),
        release: release.clone(),
        result: catalog.health_check_results["api-health"].clone(),
    });
    let scheduler = Arc::new(HealthCheckScheduler::new(FixedClock::new(fixture_time())));
    let (sender, receiver) = mpsc::channel();
    let first_scheduler = scheduler.clone();
    let first_schedules = schedules.clone();
    let first_fixtures = fixtures.clone();
    let first = thread::spawn(move || {
        first_scheduler.run_due_checks(&first_schedules, first_fixtures.as_ref(), 7)
    });

    entered.wait();
    let second_scheduler = scheduler.clone();
    let second_schedules = schedules.clone();
    let second_fixtures = fixtures.clone();
    thread::spawn(move || {
        sender
            .send(second_scheduler.run_due_checks(&second_schedules, second_fixtures.as_ref(), 7))
            .unwrap();
    });
    assert_eq!(
        receiver
            .recv()
            .unwrap()
            .expect_err("second run must refuse"),
        HealthCheckError::AlreadyRunning
    );
    release.wait();
    assert!(first.join().unwrap().is_ok());
}
