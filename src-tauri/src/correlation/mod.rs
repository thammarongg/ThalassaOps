//! Canonical signal/correlation contracts and deterministic replay inputs.
//!
//! The wire model lives in `thalassa-domain`; this module owns only the
//! internal fixture catalog and re-exports the domain types for backend
//! callers.  This module owns replay fixtures, source retention and the
//! operational adapter seam; later Sprint 13 tasks build correlation on top.

pub mod adapters;
pub mod fixtures;
pub mod source_records;

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
