//! Change intelligence module surface.
//!
//! The canonical change contracts live in `thalassa_domain`; this module only
//! re-exports them for backend callers and owns replay fixtures.

pub mod fixtures;
pub mod normalize;
pub mod records;

pub use normalize::NormalizedChange;
pub use records::AdmittedRecord;

pub use thalassa_domain::{
    ChangeActor, ChangeActorKind, ChangeAssociation, ChangeDiffStat, ChangeError, ChangeEvent,
    ChangeEventId, ChangeEvidenceRequest, ChangeKind, ChangeLinkKind, ChangeMetric,
    ChangeMetricKey, ChangeOutcome, ChangeRepositoryRef, ChangeRequest, ChangeRevision,
    ChangeSnapshot, ChangeSourceLink, ChangeTimeline,
};
