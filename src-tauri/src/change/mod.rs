//! Change intelligence module surface.
//!
//! The canonical change contracts live in `thalassa_domain`; this module only
//! re-exports them for backend callers and owns replay fixtures.

pub mod adapters;
pub mod association;
pub mod fixtures;
pub mod metrics;
pub mod normalize;
pub mod projection;
pub mod records;
pub mod timeline;

pub use normalize::NormalizationOutput;
pub use records::AdmittedRecord;

pub use thalassa_domain::{
    ChangeActor, ChangeActorKind, ChangeAssociation, ChangeDiffStat, ChangeError, ChangeEvent,
    ChangeEventId, ChangeEvidenceRequest, ChangeKind, ChangeLinkKind, ChangeMetric,
    ChangeMetricKey, ChangeOutcome, ChangeRepositoryRef, ChangeRequest, ChangeRevision,
    ChangeSnapshot, ChangeSourceLink, ChangeTimeline,
};
