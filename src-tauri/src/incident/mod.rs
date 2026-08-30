//! Local-first incident write model.
//!
//! The domain crate owns the Incident aggregate and every invariant; this
//! module owns local trigger resolution, application services and persistence.
//! Nothing here performs a provider request, reads a credential or writes an
//! Incident on behalf of replay or a projection.

pub mod repository;
pub mod service;
pub mod source;

pub use repository::{IncidentCreationRecord, IncidentStoreError, SqliteIncidentRepository};
pub use service::{IncidentCommandContext, IncidentService, IncidentServiceError};
pub use source::{
    replay_incident_signals, source_kind_matches_signal, IncidentSourceResolver,
    ResolvedIncidentTrigger,
};
