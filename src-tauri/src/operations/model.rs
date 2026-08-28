//! Re-exports of the canonical Operations Console domain contracts.
//!
//! The domain crate owns the wire model.  Keeping these re-exports under the
//! producer module gives backend callers a natural import path without
//! creating a second, subtly different representation of a console signal.

pub use thalassa_domain::{
    AnomalyCondition, AnomalyEvaluation, AnomalyEvaluationStatus, AnomalyRule, AnomalySignal,
    ConsoleEvidenceId, ConsoleSeverity, MetricFixture, MetricFixtureSample, MetricFixtureSource,
    RateDirection, ResourceScope, ThresholdOperator,
};
