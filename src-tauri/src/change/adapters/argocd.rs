//! Argo CD replay adapter.

use chrono::{DateTime, Utc};
use thalassa_domain::{EvidenceSourceKind, ResourceScope};

use crate::change::{fixtures::ChangeFixture, normalize::NormalizationOutput};
use crate::correlation::SourceRecordStore;

use super::{admit_and_normalize, ReplayAdapter};

/// Zero-sized adapter for Argo CD sync and rollback payloads.
pub(crate) struct Adapter;

impl ReplayAdapter for Adapter {
    fn source_kind(&self) -> EvidenceSourceKind {
        EvidenceSourceKind::ArgoCd
    }

    fn replay(
        &self,
        fixture: ChangeFixture,
        store: &mut SourceRecordStore,
        scope: &ResourceScope,
        clock: DateTime<Utc>,
    ) -> Result<NormalizationOutput, thalassa_domain::ChangeError> {
        admit_and_normalize(self.source_kind(), fixture, store, scope, clock)
    }
}
