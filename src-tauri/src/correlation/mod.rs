//! Canonical signal/correlation contracts and deterministic replay inputs.
//!
//! The wire model lives in `thalassa-domain`; this module owns only the
//! internal fixture catalog and re-exports the domain types for backend
//! callers.  This module owns replay fixtures, source retention and the
//! operational adapter seam; later Sprint 13 tasks build correlation on top.

pub mod adapters;
pub mod aggregate;
pub mod dedup;
pub mod fixtures;
pub mod grouping;
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

pub use crate::topology::TopologyCorrelationResolver;
pub use aggregate::{aggregate_snapshot, assemble_snapshot, CorrelationInput};
pub use grouping::{
    build_signal_groups, group_signals, group_signals_in_scope, CorrelationComponent,
    CorrelationTopologyResolver, GroupingResult,
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

/// Run the pure Task 6 correlation pipeline over already normalized Signals.
/// Source adapters remain outside this function; their admitted evidence is
/// passed through the input and is closed by snapshot validation.
pub fn correlate_signals(
    input: CorrelationInput,
    resolver: &dyn TopologyCorrelationResolver,
) -> Result<CorrelationSnapshot, CorrelationError> {
    correlate_signals_inner(input, None, resolver)
}

/// Run correlation while deriving source-aware keys from the retained local
/// source-record ledger. This is the production adapter for callers that do
/// not rely on adapter-populated `Signal.dedup_key` values.
pub fn correlate_signals_with_records(
    input: CorrelationInput,
    records: &SourceRecordStore,
    resolver: &dyn TopologyCorrelationResolver,
) -> Result<CorrelationSnapshot, CorrelationError> {
    correlate_signals_inner(input, Some(records), resolver)
}

fn correlate_signals_inner(
    mut input: CorrelationInput,
    records: Option<&SourceRecordStore>,
    resolver: &dyn TopologyCorrelationResolver,
) -> Result<CorrelationSnapshot, CorrelationError> {
    let preparation = prepare_correlation(
        input.signals.clone(),
        &input.request,
        records,
        input.prior_window.as_ref(),
    )
    .map_err(|error| match error {
        CorrelationPreparationError::Dedup(error) => match error {
            DedupError::Signal(validation) => validation,
            DedupError::SourceRecordMissing => CorrelationError::CandidateReferenceMissing,
            DedupError::SourceMismatch => CorrelationError::SourceMismatch,
            DedupError::UnsafeIdentity => CorrelationError::InvalidId,
            DedupError::InvalidPayload | DedupError::MissingIdentity => {
                CorrelationError::InvalidPayload
            }
            DedupError::ConflictingNativeIdentity => CorrelationError::DuplicateId,
            DedupError::DuplicateSignal => CorrelationError::DuplicateId,
        },
        CorrelationPreparationError::Window(error) => match error {
            WindowError::InvalidRequest(validation) | WindowError::InvalidSignal(validation) => {
                validation
            }
            WindowError::InvalidTimestamp
            | WindowError::WatermarkOverflow
            | WindowError::WindowMismatch
            | WindowError::EvaluationBeforePrevious => CorrelationError::InvalidWindow,
        },
    })?;
    // Carry the canonical keys and deterministic Signal order produced by the
    // preparation phase into the snapshot projection.  This matters for
    // callers that rely on the retained source-record ledger rather than an
    // adapter-populated `dedup_key`.
    input.signals = preparation.signals.clone();
    let grouping = group_signals_in_scope(
        &preparation.window.eligible_signals,
        &input.scope,
        &preparation.window.window,
        resolver,
    )?;
    aggregate_snapshot(
        &input,
        &preparation.window.window,
        &grouping,
        &preparation.window.late_signal_ids,
    )
}

/// Alias for callers that use the domain term correlation projection.
pub fn correlate(
    input: CorrelationInput,
    resolver: &dyn TopologyCorrelationResolver,
) -> Result<CorrelationSnapshot, CorrelationError> {
    correlate_signals(input, resolver)
}

/// Alias for [`correlate_signals_with_records`].
pub fn correlate_with_records(
    input: CorrelationInput,
    records: &SourceRecordStore,
    resolver: &dyn TopologyCorrelationResolver,
) -> Result<CorrelationSnapshot, CorrelationError> {
    correlate_signals_with_records(input, records, resolver)
}

/// Alias for callers that name the output as a correlation snapshot.
pub fn build_correlation_snapshot(
    input: CorrelationInput,
    resolver: &dyn TopologyCorrelationResolver,
) -> Result<CorrelationSnapshot, CorrelationError> {
    correlate_signals(input, resolver)
}
