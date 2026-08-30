//! Re-exports of the canonical Operations Console domain contracts.
//!
//! The domain crate owns the wire model.  Keeping these re-exports under the
//! producer module gives backend callers a natural import path without
//! creating a second, subtly different representation of a console signal.

pub use thalassa_domain::{
    AnomalyCondition, AnomalyEvaluation, AnomalyEvaluationStatus, AnomalyRule, AnomalySignal,
    BusinessImpact, ChangeEvent, ChangeKind, ChangeStreamItem, ChangeStreamState,
    ChangeStreamStatus, ConsoleEvidenceId, ConsoleHealthState, ConsolePriority, ConsoleSeverity,
    ContributingScope, CriticalNumber, DrillDownDestination, DrillDownReference, DrillDownTarget,
    EnvironmentStatus, EvidenceRedaction, EvidenceRef, EvidenceSourceKind, FixtureHealthCheck,
    HealthCheckAudit, HealthCheckOutcome, HealthCheckResult, HealthCheckSchedule,
    HealthCheckSource, HealthSummary, ImpactLevel, ImpactTrajectory, IncidentQueueItem,
    MetricFixture, MetricFixtureSample, MetricFixtureSource, NumberUnit, OperationsEvidenceRequest,
    OperationsSnapshot, QueueItemSourceKind, QueueStatus, RateDirection, ResourceScope,
    SignalCount, SignalSummary, SourceState, SourceStatus, StatusReason, ThresholdOperator,
    TimeWindow, WidgetDefinition, WidgetId, WidgetSize,
};
