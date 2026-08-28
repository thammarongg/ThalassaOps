//! Canonical signal/correlation contracts and deterministic replay inputs.
//!
//! The wire model lives in `thalassa-domain`; this module owns only the
//! internal fixture catalog and re-exports the domain types for backend
//! callers.  This module owns replay fixtures, source retention and the
//! operational adapter seam; later Sprint 13 tasks build correlation on top.

pub mod adapters;
pub mod dedup;
pub mod fixtures;
pub mod source_records;
pub mod window;

use thiserror::Error;

pub use dedup::{
    build_dedup_index, canonical_identity, compute_dedup_key, deduplicate_signals, index_signals,
    source_aware_dedup_key, stable_candidate_anchor, CanonicalIdentity, DedupAssociation,
    DedupError, DedupIndex,
};
pub use window::{
    assign_window, build_window, evaluate_window, request_with_clock, signal_membership,
    CorrelationClock, CorrelationWindowEvaluator, EvaluationClock, FixedClock,
    SignalWindowMembership, WindowAssignment, WindowError,
};

pub use adapters::{SignalAdapter, SignalAdapterError};

pub use fixtures::{
    correlation_fixture_catalog, falco_fixture, fixture_scope, fixture_time, gatekeeper_fixture,
    kyverno_fixture, mixed_signal_fixture_catalog, security_fixture_for, trivy_fixture,
    CorrelationFixtureCatalog, ReplayableSignalFixture, FIXTURE_CLOCK,
};

pub use source_records::{
    RetainedSourceRecord, SourceRecord, SourceRecordError, SourceRecordInput, SourceRecordStore,
};

pub use thalassa_domain::{
    CandidateStatus, ConsoleEvidenceId, ConsoleSeverity, CorrelationCandidate, CorrelationError,
    CorrelationEvidenceRequest, CorrelationMetric, CorrelationMetricKey, CorrelationNumberField,
    CorrelationQualification, CorrelationReason, CorrelationReasonKind, CorrelationRequest,
    CorrelationSnapshot, CorrelationSummary, CorrelationWindow, CorrelationWindowState,
    DrillDownDestination, DrillDownReference, DrillDownTarget, EvidenceRedaction, EvidenceRef,
    EvidenceSourceKind, Exploitability, FindingAsset, FindingAssetKind, FindingSeverity,
    HealthCheckOutcome, MaintenanceWindow, MaintenanceWindowReason, NumberUnit, ResourceScope,
    Signal, SignalKind, SignalPayload, SignalState, SignalTarget, SignalTargetKind,
    SourceRecordRef, SourceState, SourceStatus, StatusReason, SuppressionKind, SuppressionRule,
    SuppressionState, TimeWindow, TopologyPath, VulnerabilityFinding,
};

/// Errors returned while composing the Task 5 deduplication and window
/// phases. Later grouping/suppression phases can add their own typed layers
/// without changing either pure module's error contract.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum CorrelationPreparationError {
    #[error("correlation deduplication failed")]
    Dedup(#[source] DedupError),
    #[error("correlation window assignment failed")]
    Window(#[source] WindowError),
}

impl From<DedupError> for CorrelationPreparationError {
    fn from(error: DedupError) -> Self {
        Self::Dedup(error)
    }
}

impl From<WindowError> for CorrelationPreparationError {
    fn from(error: WindowError) -> Self {
        Self::Window(error)
    }
}

/// Intermediate output consumed by the later exact-target grouping phase.
/// The full Signal vector remains intact; `window.eligible_signals` is the
/// event-time subset that grouping may inspect.
#[derive(Clone, Debug, PartialEq)]
pub struct CorrelationPreparation {
    pub signals: Vec<Signal>,
    pub dedup_index: DedupIndex,
    pub window: WindowAssignment,
}

impl CorrelationPreparation {
    pub fn stable_candidate_anchor(&self) -> Option<String> {
        stable_candidate_anchor(&self.window.eligible_signals)
    }
}

/// Compose source-aware deduplication and explicit event-time assignment in
/// the required order. No records are deleted and no wall-clock value is read.
pub fn prepare_correlation(
    mut signals: Vec<Signal>,
    request: &CorrelationRequest,
    records: Option<&SourceRecordStore>,
    prior_window: Option<&CorrelationWindow>,
) -> Result<CorrelationPreparation, CorrelationPreparationError> {
    let dedup_index = deduplicate_signals(&mut signals, records)?;
    let window = evaluate_window(request, &signals, prior_window)?;
    Ok(CorrelationPreparation {
        signals,
        dedup_index,
        window,
    })
}
