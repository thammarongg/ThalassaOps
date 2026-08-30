//! Project a canonical change event onto the Sprint 11 console summary.
//!
//! `ChangeStreamItem` stays the Operations Console's summary shape; it is
//! derived here instead of being invented by the operations fixture module.
//! The projection only carries source-supplied identifiers across, so Rust
//! never emits a user-facing sentence: React renders the surrounding copy
//! from the typed fields and its locale keys.

use thalassa_domain::{ChangeEvent, ChangeStreamItem, DrillDownDestination, DrillDownTarget};

/// Derive the console change-stream summary for one canonical change event.
pub fn to_stream_item(event: &ChangeEvent) -> ChangeStreamItem {
    let id = event.id.to_string();
    ChangeStreamItem {
        source: event.source,
        occurred_at: event.occurred_at.clone(),
        kind: event.kind,
        summary: summary_for(event, &id),
        actor: event.actor.handle.clone(),
        target_resource: event.targets.first().map(|target| target.id.clone()),
        native_link: event.source_link.as_ref().map(|link| link.url.clone()),
        scope: event.scope.clone(),
        evidence_ids: event.evidence_ids.clone(),
        drill_down: DrillDownTarget {
            destination: DrillDownDestination::ChangeStream,
            evidence_ids: event.evidence_ids.clone(),
            filter_key: Some(id.clone()),
        },
        id,
    }
}

/// Return the most specific source-supplied identifier for the event.
///
/// The revision short ID, the provider's native ID and the event ID are all
/// identifiers supplied or derived from the retained record; none of them is
/// generated prose.
fn summary_for(event: &ChangeEvent, id: &str) -> String {
    event
        .revision
        .as_ref()
        .and_then(|revision| revision.short_id.clone())
        .or_else(|| event.source_record.native_id.clone())
        .unwrap_or_else(|| id.to_owned())
}
