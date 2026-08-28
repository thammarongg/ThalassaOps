//! Deterministic, provider-neutral producers for the Operations Console.
//!
//! Producers in this module consume explicit fixture/source results.  They do
//! not perform correlation, deduplication, incident creation, or network I/O;
//! those concerns belong to later aggregation and workflow layers.

pub mod anomaly;
pub mod fixtures;
pub mod model;

pub use anomaly::{
    evaluate_rule, evaluate_rules, metric_fixtures_from_prometheus, parse_prometheus_fixture,
    AnomalyError,
};
pub use fixtures::{fixture_catalog, fixture_time, FixtureCatalog};
