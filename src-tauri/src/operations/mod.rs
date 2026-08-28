//! Deterministic, provider-neutral producers for the Operations Console.
//!
//! Producers in this module consume explicit fixture/source results.  They do
//! not perform correlation, deduplication, incident creation, or network I/O;
//! those concerns belong to later aggregation and workflow layers.

pub mod aggregate;
pub mod anomaly;
pub mod evidence;
pub mod fixtures;
pub mod health_check;
pub mod model;

pub use aggregate::{AggregationError, AggregationInput, OperationsAggregator};
pub use anomaly::{
    evaluate_rule, evaluate_rules, metric_fixtures_from_prometheus, parse_prometheus_fixture,
    AnomalyError,
};
pub use evidence::{EvidenceError, EvidenceStore};
pub use fixtures::{fixture_catalog, fixture_time, FixtureCatalog};
pub use health_check::{
    audit_for, is_due, run_due_checks, run_due_checks_with_policy, BoundedScopePolicy, DueState,
    FixedClock, FixtureLookup, HealthCheckClock, HealthCheckError, HealthCheckPolicy,
    HealthCheckScheduler,
};
pub use model::OperationsEvidenceRequest;
