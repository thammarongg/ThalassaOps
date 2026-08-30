//! Local-first incident write model.
//!
//! The domain crate owns the Incident aggregate and every invariant; this
//! module owns only local persistence and, from Task 5 on, the application
//! services that resolve explicit triggers.  Nothing here performs a provider
//! request, reads a credential or writes an Incident on behalf of replay.

pub mod repository;

pub use repository::{IncidentCreationRecord, IncidentStoreError, SqliteIncidentRepository};
