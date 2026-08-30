//! Replay adapters for the committed GitHub, GitLab and Argo CD payloads.
//!
//! Provider modules own source dispatch while admission and normalization stay
//! shared.  The caller owns the scoped source-record store; replay never
//! acquires ambient persistence or invents a scope.

use chrono::{DateTime, Utc};
use thalassa_domain::{ChangeError, ChangeEvent, EvidenceSourceKind, ResourceScope, SourceStatus};

use crate::correlation::SourceRecordStore;

use super::{fixtures, normalize, records};

pub mod argocd;
pub mod github;
pub mod gitlab;

/// Events and typed source statuses produced by one deterministic replay.
#[derive(Clone, Debug, PartialEq)]
pub struct AdapterOutput {
    pub events: Vec<ChangeEvent>,
    pub statuses: Vec<SourceStatus>,
}

/// Common seam implemented by every supported source replay adapter.
pub trait ReplayAdapter {
    fn source_kind(&self) -> EvidenceSourceKind;

    fn replay(
        &self,
        fixture: fixtures::ChangeFixture,
        store: &mut SourceRecordStore,
        scope: &ResourceScope,
        clock: DateTime<Utc>,
    ) -> Result<normalize::NormalizationOutput, ChangeError>;
}

/// Replay every committed fixture through the caller-owned source ledger.
pub fn replay_all(
    store: &mut SourceRecordStore,
    scope: &ResourceScope,
    clock: DateTime<Utc>,
) -> Result<AdapterOutput, ChangeError> {
    replay_from(fixtures::catalog(), store, scope, clock)
}

/// Replay an explicit fixture sequence and return deterministic event order.
pub fn replay_from(
    fixtures: Vec<fixtures::ChangeFixture>,
    store: &mut SourceRecordStore,
    scope: &ResourceScope,
    clock: DateTime<Utc>,
) -> Result<AdapterOutput, ChangeError> {
    let mut events = Vec::with_capacity(fixtures.len());
    let mut statuses = Vec::new();
    for fixture in fixtures {
        let normalized = match fixture.source {
            EvidenceSourceKind::GitHub => github::Adapter.replay(fixture, store, scope, clock)?,
            EvidenceSourceKind::GitLab => gitlab::Adapter.replay(fixture, store, scope, clock)?,
            EvidenceSourceKind::ArgoCd => argocd::Adapter.replay(fixture, store, scope, clock)?,
            _ => return Err(ChangeError::SourceMismatch),
        };
        events.push(normalized.event);
        statuses.extend(normalized.statuses);
    }

    events.sort_by(|left, right| {
        let left_at = DateTime::parse_from_rfc3339(&left.occurred_at)
            .ok()
            .map(|timestamp| timestamp.with_timezone(&Utc));
        let right_at = DateTime::parse_from_rfc3339(&right.occurred_at)
            .ok()
            .map(|timestamp| timestamp.with_timezone(&Utc));
        left_at.cmp(&right_at).then_with(|| left.id.cmp(&right.id))
    });
    statuses.sort_by(|left, right| left.source_key.cmp(&right.source_key));

    Ok(AdapterOutput { events, statuses })
}

/// Admit one source payload and normalize it through the shared contracts.
pub(crate) fn admit_and_normalize(
    expected_source: EvidenceSourceKind,
    fixture: fixtures::ChangeFixture,
    store: &mut SourceRecordStore,
    scope: &ResourceScope,
    clock: DateTime<Utc>,
) -> Result<normalize::NormalizationOutput, ChangeError> {
    if fixture.source != expected_source {
        return Err(ChangeError::SourceMismatch);
    }
    let record = records::admit(store, fixture.payload, fixture.source, scope, clock)?;
    normalize::to_change_event(&record)
}
