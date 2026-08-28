//! Canonical signal/correlation contracts and deterministic replay inputs.
//!
//! The wire model lives in `thalassa-domain`; this module owns only the
//! internal fixture catalog and re-exports the domain types for backend
//! callers.  Adapter, retention and aggregation implementations are added by
//! later Sprint 13 tasks.

pub mod fixtures;

pub use fixtures::{
    correlation_fixture_catalog, fixture_scope, fixture_time, CorrelationFixtureCatalog,
    ReplayableSignalFixture, FIXTURE_CLOCK,
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
