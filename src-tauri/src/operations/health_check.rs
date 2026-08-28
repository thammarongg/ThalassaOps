//! Deterministic scheduled health-check evaluation.
//!
//! This module is intentionally a passive producer.  It exposes synchronous
//! evaluation for the Operations aggregation layer to drive; it does not
//! start an operating-system thread, spawn a Tokio task, install a timer, or
//! otherwise run automatically at application boot.

use chrono::{DateTime, Duration, SecondsFormat, Utc};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use thalassa_domain::{
    FixtureHealthCheck, HealthCheckAudit, HealthCheckOutcome, HealthCheckResult,
    HealthCheckSchedule, HealthCheckSource, ResourceScope,
};
use thalassa_policy::{DataClass, EgressDestination, EgressRequest, PolicyRuntime};

/// Time source used by the scheduler.  Tests can implement this trait with a
/// fixed instant; no health-check test needs to read the wall clock or sleep.
pub trait HealthCheckClock {
    fn now(&self) -> DateTime<Utc>;
}

impl HealthCheckClock for DateTime<Utc> {
    fn now(&self) -> DateTime<Utc> {
        *self
    }
}

impl<T: HealthCheckClock + ?Sized> HealthCheckClock for &T {
    fn now(&self) -> DateTime<Utc> {
        (*self).now()
    }
}

/// Fixed clock useful for deterministic callers and unit tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixedClock {
    instant: DateTime<Utc>,
}

impl FixedClock {
    pub fn new(instant: DateTime<Utc>) -> Self {
        Self { instant }
    }
}

impl HealthCheckClock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.instant
    }
}

/// Result of applying a schedule's interval, cooldown and enabled checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DueState {
    Disabled,
    NotDue,
    Cooldown,
    Due,
}

/// Errors fail closed so malformed definitions, denied scopes and missing
/// fixture data cannot be represented as a healthy check.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum HealthCheckError {
    #[error("health-check schedule is invalid")]
    InvalidSchedule,
    #[error("health-check timestamp is invalid")]
    InvalidTimestamp,
    #[error("health-check duration is invalid")]
    InvalidDuration,
    #[error("health-check fixture was not found")]
    FixtureNotFound,
    #[error("health-check policy denied scope")]
    PolicyDenied,
    #[error("health-check is already running")]
    AlreadyRunning,
    #[error("health-check scheduler state is unavailable")]
    SchedulerUnavailable,
    #[error("health-check schedule identifiers must be unique")]
    DuplicateSchedule,
}

/// Scope/capability seam used by a caller's existing policy authorization
/// path.  The default free function permits bounded fixture scopes, while an
/// aggregation or application caller can inject its established capability
/// check through [`run_due_checks_with_policy`].
pub trait HealthCheckPolicy {
    fn authorize(&self, scope: &ResourceScope) -> Result<(), HealthCheckError>;

    fn policy_version(&self) -> u64 {
        0
    }
}

/// Default policy for the pure fixture producer.  It still rejects an
/// unbounded target scope rather than silently skipping that check.
#[derive(Clone, Copy, Debug, Default)]
pub struct BoundedScopePolicy;

impl HealthCheckPolicy for BoundedScopePolicy {
    fn authorize(&self, scope: &ResourceScope) -> Result<(), HealthCheckError> {
        if scope.is_bounded() {
            Ok(())
        } else {
            Err(HealthCheckError::PolicyDenied)
        }
    }
}

impl HealthCheckPolicy for u64 {
    fn authorize(&self, scope: &ResourceScope) -> Result<(), HealthCheckError> {
        BoundedScopePolicy.authorize(scope)
    }

    fn policy_version(&self) -> u64 {
        *self
    }
}

impl HealthCheckPolicy for PolicyRuntime {
    fn authorize(&self, scope: &ResourceScope) -> Result<(), HealthCheckError> {
        if !scope.is_bounded()
            || !self
                .evaluate_egress(EgressRequest::verified(
                    DataClass::Internal,
                    EgressDestination::AuditLog,
                ))
                .is_allowed()
        {
            return Err(HealthCheckError::PolicyDenied);
        }
        Ok(())
    }
}

impl<T: HealthCheckPolicy + ?Sized> HealthCheckPolicy for &T {
    fn authorize(&self, scope: &ResourceScope) -> Result<(), HealthCheckError> {
        (*self).authorize(scope)
    }

    fn policy_version(&self) -> u64 {
        (*self).policy_version()
    }
}

/// A fixture lookup abstraction keeps the producer provider-neutral.  The
/// built-in map implementations accept a fixture key, schedule ID, or the
/// provider-neutral source key; no provider operation is performed here.
pub trait FixtureLookup {
    fn lookup(&self, schedule: &HealthCheckSchedule) -> Option<&FixtureHealthCheck>;
}

impl FixtureLookup for BTreeMap<String, FixtureHealthCheck> {
    fn lookup(&self, schedule: &HealthCheckSchedule) -> Option<&FixtureHealthCheck> {
        lookup_map(self, schedule)
    }
}

impl FixtureLookup for HashMap<String, FixtureHealthCheck> {
    fn lookup(&self, schedule: &HealthCheckSchedule) -> Option<&FixtureHealthCheck> {
        lookup_map(self, schedule)
    }
}

impl<T: FixtureLookup + ?Sized> FixtureLookup for &T {
    fn lookup(&self, schedule: &HealthCheckSchedule) -> Option<&FixtureHealthCheck> {
        (*self).lookup(schedule)
    }
}

impl<T: FixtureLookup + ?Sized> FixtureLookup for Arc<T> {
    fn lookup(&self, schedule: &HealthCheckSchedule) -> Option<&FixtureHealthCheck> {
        (**self).lookup(schedule)
    }
}

fn lookup_map<'a, M>(map: &'a M, schedule: &HealthCheckSchedule) -> Option<&'a FixtureHealthCheck>
where
    M: MapLookup,
{
    source_keys(&schedule.source)
        .iter()
        .flatten()
        .find_map(|key| map.get(key))
        .or_else(|| map.get(&schedule.id))
}

trait MapLookup {
    fn get(&self, key: &str) -> Option<&FixtureHealthCheck>;
}

impl MapLookup for BTreeMap<String, FixtureHealthCheck> {
    fn get(&self, key: &str) -> Option<&FixtureHealthCheck> {
        BTreeMap::get(self, key)
    }
}

impl MapLookup for HashMap<String, FixtureHealthCheck> {
    fn get(&self, key: &str) -> Option<&FixtureHealthCheck> {
        HashMap::get(self, key)
    }
}

fn source_keys(source: &HealthCheckSource) -> [Option<&str>; 2] {
    match source {
        HealthCheckSource::Connector {
            connector_id,
            probe_key,
        }
        | HealthCheckSource::Observability {
            connector_id,
            probe_key,
        } => [Some(probe_key.as_str()), Some(connector_id.as_str())],
        HealthCheckSource::Kubernetes {
            connector_id,
            resource_key,
        } => [Some(resource_key.as_str()), Some(connector_id.as_str())],
        HealthCheckSource::Fixture { fixture_key } => [Some(fixture_key.as_str()), None],
    }
}

/// Decide whether one schedule is disabled, not due, cooldown-suppressed or
/// ready to execute at the explicit clock instant.
pub fn is_due<C: HealthCheckClock>(
    schedule: &HealthCheckSchedule,
    clock: C,
) -> Result<DueState, HealthCheckError> {
    schedule
        .validate()
        .map_err(|_| HealthCheckError::InvalidSchedule)?;
    if !schedule.enabled {
        return Ok(DueState::Disabled);
    }

    let now = clock.now();
    if let Some(last_run_at) = schedule.last_run_at.as_deref() {
        let last_run_at = parse_timestamp(last_run_at)?;
        let interval = seconds(schedule.interval_seconds)?;
        let next_run_at = last_run_at
            .checked_add_signed(interval)
            .ok_or(HealthCheckError::InvalidTimestamp)?;
        if now < next_run_at {
            return Ok(DueState::NotDue);
        }
    }

    if schedule.cooldown_seconds > 0 {
        if let Some(last_signal_at) = schedule.last_signal_at.as_deref() {
            let last_signal_at = parse_timestamp(last_signal_at)?;
            let cooldown = seconds(schedule.cooldown_seconds)?;
            let cooldown_until = last_signal_at
                .checked_add_signed(cooldown)
                .ok_or(HealthCheckError::InvalidTimestamp)?;
            if now < cooldown_until {
                return Ok(DueState::Cooldown);
            }
        }
    }

    Ok(DueState::Due)
}

/// Evaluate all schedules at an explicit instant using the supplied policy
/// capability.  Passing a `u64` uses the fixture producer's bounded-scope
/// policy and records that value as the policy version.  The function returns
/// one result per schedule, including disabled, not-due and
/// cooldown-suppressed outcomes.
pub fn run_due_checks<C, F, P>(
    schedules: &[HealthCheckSchedule],
    fixtures: &F,
    clock: C,
    policy: P,
) -> Result<Vec<HealthCheckResult>, HealthCheckError>
where
    C: HealthCheckClock,
    F: FixtureLookup + Sync,
    P: HealthCheckPolicy,
{
    run_due_checks_with_policy(schedules, fixtures, clock, policy.policy_version(), &policy)
}

/// Evaluate all schedules with a caller-provided capability/policy gate.
/// Policy denial is returned as an explicit error; it is never converted into
/// a skipped or healthy result.
pub fn run_due_checks_with_policy<C, F, P>(
    schedules: &[HealthCheckSchedule],
    fixtures: &F,
    clock: C,
    policy_version: u64,
    policy: &P,
) -> Result<Vec<HealthCheckResult>, HealthCheckError>
where
    C: HealthCheckClock,
    F: FixtureLookup + Sync,
    P: HealthCheckPolicy,
{
    let active = Arc::new(Mutex::new(BTreeSet::new()));
    run_due_checks_internal(schedules, fixtures, clock, policy_version, policy, &active)
}

/// Passive scheduler façade for aggregation callers.  Constructing it does
/// not start work; callers explicitly invoke `run_due_checks` when building a
/// snapshot.  The in-flight set prevents a schedule ID from executing twice
/// concurrently when the same façade is shared across caller threads.
#[derive(Debug)]
pub struct HealthCheckScheduler<C> {
    clock: C,
    active: Arc<Mutex<BTreeSet<String>>>,
}

impl<C> HealthCheckScheduler<C>
where
    C: HealthCheckClock,
{
    pub fn new(clock: C) -> Self {
        Self {
            clock,
            active: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }

    pub fn is_due(&self, schedule: &HealthCheckSchedule) -> Result<DueState, HealthCheckError> {
        is_due(schedule, &self.clock)
    }

    pub fn run_due_checks<F>(
        &self,
        schedules: &[HealthCheckSchedule],
        fixtures: &F,
        policy_version: u64,
    ) -> Result<Vec<HealthCheckResult>, HealthCheckError>
    where
        F: FixtureLookup + Sync,
    {
        self.run_due_checks_with_policy(schedules, fixtures, policy_version, &BoundedScopePolicy)
    }

    pub fn run_due_checks_with_policy<F, P>(
        &self,
        schedules: &[HealthCheckSchedule],
        fixtures: &F,
        policy_version: u64,
        policy: &P,
    ) -> Result<Vec<HealthCheckResult>, HealthCheckError>
    where
        F: FixtureLookup + Sync,
        P: HealthCheckPolicy,
    {
        run_due_checks_internal(
            schedules,
            fixtures,
            &self.clock,
            policy_version,
            policy,
            &self.active,
        )
    }
}

fn run_due_checks_internal<C, F, P>(
    schedules: &[HealthCheckSchedule],
    fixtures: &F,
    clock: C,
    policy_version: u64,
    policy: &P,
    active: &Arc<Mutex<BTreeSet<String>>>,
) -> Result<Vec<HealthCheckResult>, HealthCheckError>
where
    C: HealthCheckClock,
    F: FixtureLookup + Sync,
    P: HealthCheckPolicy,
{
    let mut seen = BTreeSet::new();
    for schedule in schedules {
        if !seen.insert(schedule.id.clone()) {
            return Err(HealthCheckError::DuplicateSchedule);
        }
    }

    let now = clock.now();
    let mut schedule_order: Vec<&HealthCheckSchedule> = schedules.iter().collect();
    schedule_order.sort_by(|left, right| left.id.cmp(&right.id));
    schedule_order
        .into_iter()
        .map(|schedule| {
            let _guard = InFlight::acquire(active, &schedule.id)?;
            if schedule.enabled {
                policy.authorize(&schedule.scope)?;
            }
            let state = is_due(schedule, now)?;
            let fixture = if state == DueState::Due {
                Some(
                    fixtures
                        .lookup(schedule)
                        .ok_or(HealthCheckError::FixtureNotFound)?,
                )
            } else {
                None
            };
            result_for(schedule, state, fixture, now, policy_version)
        })
        .collect()
}

fn result_for(
    schedule: &HealthCheckSchedule,
    state: DueState,
    fixture: Option<&FixtureHealthCheck>,
    now: DateTime<Utc>,
    policy_version: u64,
) -> Result<HealthCheckResult, HealthCheckError> {
    let (outcome, duration_ms, evidence_id, cooldown_suppressed) = match state {
        DueState::Disabled => (HealthCheckOutcome::SkippedDisabled, 0, None, false),
        DueState::NotDue => (HealthCheckOutcome::SkippedNotDue, 0, None, false),
        DueState::Cooldown => (HealthCheckOutcome::SkippedCooldown, 0, None, true),
        DueState::Due => {
            let fixture = fixture.ok_or(HealthCheckError::FixtureNotFound)?;
            let timed_out = fixture.duration_ms > schedule.timeout_ms;
            let duration_ms = if timed_out {
                schedule.timeout_ms
            } else {
                fixture.duration_ms
            };
            let outcome = if timed_out {
                HealthCheckOutcome::TimedOut
            } else {
                fixture.outcome
            };
            (outcome, duration_ms, fixture.evidence_id.clone(), false)
        }
    };

    let started_at = format_timestamp(now);
    let completed_at = format_timestamp(
        now.checked_add_signed(milliseconds(duration_ms)?)
            .ok_or(HealthCheckError::InvalidTimestamp)?,
    );
    let run_id = stable_identifier(&["health-check-run", &schedule.id, &started_at]);
    let audit = HealthCheckAudit {
        run_id,
        schedule_id: schedule.id.clone(),
        triggered_by: "scheduler".into(),
        started_at,
        completed_at,
        duration_ms,
        scope: schedule.scope.clone(),
        source: schedule.source.clone(),
        outcome,
        cooldown_suppressed,
        policy_version,
    };
    Ok(HealthCheckResult {
        schedule_id: schedule.id.clone(),
        outcome,
        observed_at: format_timestamp(now),
        evidence_id,
        audit,
    })
}

/// Return the existing audit metadata carried by a health-check result.  The
/// aggregation layer can pass this through its established Audit retention
/// path without introducing a second audit record type or store.
pub fn audit_for(result: &HealthCheckResult) -> &HealthCheckAudit {
    &result.audit
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, HealthCheckError> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| HealthCheckError::InvalidTimestamp)
}

fn seconds(value: u64) -> Result<Duration, HealthCheckError> {
    if value > i64::MAX as u64 {
        return Err(HealthCheckError::InvalidDuration);
    }
    Ok(Duration::seconds(value as i64))
}

fn milliseconds(value: u64) -> Result<Duration, HealthCheckError> {
    if value > i64::MAX as u64 {
        return Err(HealthCheckError::InvalidDuration);
    }
    Ok(Duration::milliseconds(value as i64))
}

fn format_timestamp(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

fn stable_identifier(parts: &[&str]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for part in parts {
        for byte in part.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3_u64);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3_u64);
    }
    format!("health-check-run-{hash:016x}")
}

struct InFlight<'a> {
    active: &'a Mutex<BTreeSet<String>>,
    schedule_id: String,
}

impl<'a> InFlight<'a> {
    fn acquire(
        active: &'a Arc<Mutex<BTreeSet<String>>>,
        schedule_id: &str,
    ) -> Result<Self, HealthCheckError> {
        let mut active_ids = active
            .lock()
            .map_err(|_| HealthCheckError::SchedulerUnavailable)?;
        if !active_ids.insert(schedule_id.to_owned()) {
            return Err(HealthCheckError::AlreadyRunning);
        }
        Ok(Self {
            active,
            schedule_id: schedule_id.to_owned(),
        })
    }
}

impl Drop for InFlight<'_> {
    fn drop(&mut self) {
        if let Ok(mut active_ids) = self.active.lock() {
            active_ids.remove(&self.schedule_id);
        }
    }
}
