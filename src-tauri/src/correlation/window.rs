//! Explicit event-time correlation windows and late-arrival policy.
//!
//! All timestamps used here come from the request or a caller-provided clock.
//! There is deliberately no wall-clock read, scheduler, sleep or ingestion
//! timestamp fallback: missing event time remains missing and is ineligible.

use chrono::{DateTime, Duration, Utc};
use std::borrow::Borrow;
use std::collections::BTreeMap;
use thalassa_domain::{
    CorrelationError, CorrelationRequest, CorrelationWindow, CorrelationWindowState, Signal,
    SignalId, TimeWindow,
};
use thiserror::Error;

/// Injectable evaluation clock used to build explicit requests in replay and
/// production callers. The correlation engine never constructs its own clock.
pub trait CorrelationClock {
    fn now(&self) -> DateTime<Utc>;
}

impl CorrelationClock for DateTime<Utc> {
    fn now(&self) -> DateTime<Utc> {
        *self
    }
}

impl<T: CorrelationClock + ?Sized> CorrelationClock for &T {
    fn now(&self) -> DateTime<Utc> {
        (*self).now()
    }
}

/// Deterministic clock implementation for fixtures and tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixedClock {
    now: DateTime<Utc>,
}

impl FixedClock {
    pub fn new(now: DateTime<Utc>) -> Self {
        Self { now }
    }

    pub fn from_rfc3339(value: &str) -> Result<Self, WindowError> {
        parse_timestamp(value).map(|now| Self { now })
    }

    pub fn now(&self) -> DateTime<Utc> {
        self.now
    }

    pub fn timestamp(&self) -> String {
        format_timestamp(self.now)
    }
}

impl CorrelationClock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.now()
    }
}

/// Small clock-backed adapter for callers that want to construct requests and
/// evaluate them through one injected dependency.
#[derive(Clone, Debug)]
pub struct CorrelationWindowEvaluator<C> {
    clock: C,
}

impl<C: CorrelationClock> CorrelationWindowEvaluator<C> {
    pub fn new(clock: C) -> Self {
        Self { clock }
    }

    pub fn request(
        &self,
        window: TimeWindow,
        allowed_lateness_seconds: u64,
    ) -> Result<CorrelationRequest, WindowError> {
        request_with_clock(window, allowed_lateness_seconds, &self.clock)
    }

    pub fn evaluate(
        &self,
        window: TimeWindow,
        allowed_lateness_seconds: u64,
        signals: &[Signal],
        prior: Option<&CorrelationWindow>,
    ) -> Result<WindowAssignment, WindowError> {
        let request = self.request(window, allowed_lateness_seconds)?;
        evaluate_window(&request, signals, prior)
    }
}

/// Alias that reads naturally at call sites that refer to request evaluation.
pub use CorrelationClock as EvaluationClock;

/// Typed failures from request validation and event-time assignment.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum WindowError {
    #[error("correlation request is invalid")]
    InvalidRequest(#[source] CorrelationError),
    #[error("signal failed correlation validation")]
    InvalidSignal(#[source] CorrelationError),
    #[error("signal timestamp is invalid")]
    InvalidTimestamp,
    #[error("correlation window does not match the prior evaluation")]
    WindowMismatch,
    #[error("evaluation time moved backwards")]
    EvaluationBeforePrevious,
    #[error("correlation watermark cannot be represented")]
    WatermarkOverflow,
}

/// Membership classification for one retained Signal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignalWindowMembership {
    Eligible,
    Late,
    MissingObservedAt,
    Outside,
}

/// Result of assigning retained Signals to one explicit event-time window.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowAssignment {
    pub window: CorrelationWindow,
    pub eligible_signals: Vec<Signal>,
    pub late_signal_ids: Vec<SignalId>,
    pub missing_signal_ids: Vec<SignalId>,
    pub outside_signal_ids: Vec<SignalId>,
    pub memberships: BTreeMap<SignalId, SignalWindowMembership>,
}

impl WindowAssignment {
    pub fn eligible_signal_ids(&self) -> Vec<SignalId> {
        self.eligible_signals
            .iter()
            .map(|signal| signal.id)
            .collect()
    }

    pub fn missing_observed_at_ids(&self) -> &[SignalId] {
        &self.missing_signal_ids
    }

    pub fn out_of_range_signal_ids(&self) -> &[SignalId] {
        &self.outside_signal_ids
    }

    pub fn retained_signal_ids(&self) -> Vec<SignalId> {
        let mut ids = self
            .eligible_signals
            .iter()
            .map(|signal| signal.id)
            .chain(self.missing_signal_ids.iter().copied())
            .chain(self.outside_signal_ids.iter().copied())
            .collect::<Vec<_>>();
        ids.sort();
        ids.dedup();
        ids
    }

    pub fn membership(&self, signal_id: SignalId) -> Option<SignalWindowMembership> {
        self.memberships.get(&signal_id).copied()
    }
}

/// Construct a request using an injected clock's explicit evaluation time.
pub fn request_with_clock<C: CorrelationClock + ?Sized, W: Borrow<TimeWindow>>(
    window: W,
    allowed_lateness_seconds: u64,
    clock: &C,
) -> Result<CorrelationRequest, WindowError> {
    let request = CorrelationRequest {
        window: window.borrow().clone(),
        evaluated_at: format_timestamp(clock.now()),
        allowed_lateness_seconds,
    };
    request
        .validate()
        .map_err(WindowError::InvalidRequest)
        .map(|()| request)
}

/// Build an explicit window for a request. `late_signal_ids` switches a
/// finalized evaluation to `Reopened` after a recompute.
pub fn build_window(
    request: &CorrelationRequest,
    late_signal_ids: &[SignalId],
) -> Result<CorrelationWindow, WindowError> {
    request.validate().map_err(WindowError::InvalidRequest)?;
    let end = parse_timestamp(&request.window.end)?;
    let evaluated_at = parse_timestamp(&request.evaluated_at)?;
    let watermark = evaluated_at
        .checked_sub_signed(Duration::seconds(request.allowed_lateness_seconds as i64))
        .ok_or(WindowError::WatermarkOverflow)?;
    let finalization_at = end
        .checked_add_signed(Duration::seconds(request.allowed_lateness_seconds as i64))
        .ok_or(WindowError::WatermarkOverflow)?;
    let state = if !late_signal_ids.is_empty() && evaluated_at >= finalization_at {
        CorrelationWindowState::Reopened
    } else if evaluated_at < end {
        CorrelationWindowState::Open
    } else if evaluated_at < finalization_at {
        CorrelationWindowState::ReadyToFinalize
    } else {
        CorrelationWindowState::Finalized
    };
    let mut late_signal_ids = late_signal_ids.to_vec();
    late_signal_ids.sort();
    late_signal_ids.dedup();
    let window = CorrelationWindow {
        range: request.window.clone(),
        evaluated_at: request.evaluated_at.clone(),
        watermark: format_timestamp(watermark),
        allowed_lateness_seconds: request.allowed_lateness_seconds,
        state,
    };
    window.validate().map_err(WindowError::InvalidRequest)?;
    Ok(window)
}

/// Evaluate Signal membership, ordering, watermark state and late reopen
/// semantics for a request. The optional prior window represents the previous
/// finalized evaluation of the same range.
pub fn evaluate_window(
    request: &CorrelationRequest,
    signals: &[Signal],
    prior: Option<&CorrelationWindow>,
) -> Result<WindowAssignment, WindowError> {
    request.validate().map_err(WindowError::InvalidRequest)?;
    if let Some(previous) = prior {
        previous.validate().map_err(WindowError::InvalidRequest)?;
        if previous.range != request.window
            || previous.allowed_lateness_seconds != request.allowed_lateness_seconds
        {
            return Err(WindowError::WindowMismatch);
        }
        let previous_evaluation = parse_timestamp(&previous.evaluated_at)?;
        let current_evaluation = parse_timestamp(&request.evaluated_at)?;
        if current_evaluation < previous_evaluation {
            return Err(WindowError::EvaluationBeforePrevious);
        }
    }

    let mut eligible_signals = Vec::new();
    let mut late_signal_ids = Vec::new();
    let mut missing_signal_ids = Vec::new();
    let mut outside_signal_ids = Vec::new();
    let mut memberships = BTreeMap::new();
    let previous_evaluation = prior
        .map(|window| parse_timestamp(&window.evaluated_at))
        .transpose()?;
    let current_evaluation = parse_timestamp(&request.evaluated_at)?;
    let finalization_at = parse_timestamp(&request.window.end)?
        .checked_add_signed(Duration::seconds(request.allowed_lateness_seconds as i64))
        .ok_or(WindowError::WatermarkOverflow)?;

    for signal in signals {
        signal.validate().map_err(WindowError::InvalidSignal)?;
        let membership = signal_membership(signal, request)?;
        let membership = if membership == SignalWindowMembership::Eligible
            && previous_evaluation.is_some_and(|previous| {
                current_evaluation > previous
                    && prior.is_some_and(|window| {
                        matches!(
                            window.state,
                            CorrelationWindowState::Finalized | CorrelationWindowState::Reopened
                        )
                    })
                    && signal
                        .ingested_at
                        .as_deref()
                        .and_then(|value| parse_timestamp(value).ok())
                        .is_some_and(|ingested| ingested > finalization_at)
            }) {
            late_signal_ids.push(signal.id);
            SignalWindowMembership::Late
        } else {
            membership
        };
        match membership {
            SignalWindowMembership::Eligible | SignalWindowMembership::Late => {
                eligible_signals.push(signal.clone())
            }
            SignalWindowMembership::MissingObservedAt => missing_signal_ids.push(signal.id),
            SignalWindowMembership::Outside => outside_signal_ids.push(signal.id),
        }
        memberships.insert(signal.id, membership);
    }

    eligible_signals.sort_by(signal_ordering);
    late_signal_ids.sort();
    late_signal_ids.dedup();
    missing_signal_ids.sort();
    outside_signal_ids.sort();
    let mut window = build_window(request, &late_signal_ids)?;
    if late_signal_ids.is_empty()
        && prior.is_some_and(|previous| previous.state == CorrelationWindowState::Reopened)
        && window.state == CorrelationWindowState::Finalized
    {
        // A reopened window remains visibly reopened until a later policy
        // layer records a new finalized baseline; it must not silently revert.
        window.state = CorrelationWindowState::Reopened;
    }
    Ok(WindowAssignment {
        window,
        eligible_signals,
        late_signal_ids,
        missing_signal_ids,
        outside_signal_ids,
        memberships,
    })
}

/// Alias for callers that name this phase by its primary operation.
pub fn assign_window(
    request: &CorrelationRequest,
    signals: &[Signal],
    prior: Option<&CorrelationWindow>,
) -> Result<WindowAssignment, WindowError> {
    evaluate_window(request, signals, prior)
}

/// Determine half-open event-time membership. Ingestion time is never used to
/// make an otherwise missing or out-of-range Signal eligible.
pub fn signal_membership(
    signal: &Signal,
    request: &CorrelationRequest,
) -> Result<SignalWindowMembership, WindowError> {
    request.validate().map_err(WindowError::InvalidRequest)?;
    signal.validate().map_err(WindowError::InvalidSignal)?;
    let Some(observed_at) = signal.observed_at.as_deref() else {
        return Ok(SignalWindowMembership::MissingObservedAt);
    };
    let observed_at = parse_timestamp(observed_at)?;
    let start = parse_timestamp(&request.window.start)?;
    let end = parse_timestamp(&request.window.end)?;
    if observed_at >= start && observed_at < end {
        Ok(SignalWindowMembership::Eligible)
    } else {
        Ok(SignalWindowMembership::Outside)
    }
}

fn signal_ordering(left: &Signal, right: &Signal) -> std::cmp::Ordering {
    observed_sort_key(left)
        .cmp(&observed_sort_key(right))
        .then_with(|| source_wire(left.source).cmp(source_wire(right.source)))
        .then_with(|| {
            left.source_record
                .content_digest
                .cmp(&right.source_record.content_digest)
        })
        .then_with(|| left.id.cmp(&right.id))
}

fn observed_sort_key(signal: &Signal) -> (bool, i64, u32, String) {
    let Some(value) = signal.observed_at.as_deref() else {
        return (true, 0, 0, String::new());
    };
    match parse_timestamp(value) {
        Ok(timestamp) => (
            false,
            timestamp.timestamp(),
            timestamp.timestamp_subsec_nanos(),
            value.to_owned(),
        ),
        Err(_) => (false, i64::MAX, u32::MAX, value.to_owned()),
    }
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, WindowError> {
    if value.trim().is_empty() {
        return Err(WindowError::InvalidTimestamp);
    }
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| WindowError::InvalidTimestamp)
}

fn format_timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
}

fn source_wire(source: thalassa_domain::EvidenceSourceKind) -> &'static str {
    match source {
        thalassa_domain::EvidenceSourceKind::Alertmanager => "alertmanager",
        thalassa_domain::EvidenceSourceKind::Prometheus => "prometheus",
        thalassa_domain::EvidenceSourceKind::Kubernetes => "kubernetes",
        thalassa_domain::EvidenceSourceKind::Cloud => "cloud",
        thalassa_domain::EvidenceSourceKind::HealthCheck => "health_check",
        thalassa_domain::EvidenceSourceKind::Fixture => "fixture",
        thalassa_domain::EvidenceSourceKind::Trivy => "trivy",
        thalassa_domain::EvidenceSourceKind::Falco => "falco",
        thalassa_domain::EvidenceSourceKind::Kyverno => "kyverno",
        thalassa_domain::EvidenceSourceKind::OpaGatekeeper => "opa_gatekeeper",
        thalassa_domain::EvidenceSourceKind::GitHub => "github",
        thalassa_domain::EvidenceSourceKind::GitLab => "gitlab",
        thalassa_domain::EvidenceSourceKind::ArgoCd => "argo_cd",
    }
}
