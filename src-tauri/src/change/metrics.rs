//! Deterministic counts over one change snapshot.
//!
//! Metrics mirror the Sprint 13 correlation summary shape with their own key
//! enum, and every metric is evidence-backed: a count whose contributing
//! events carry no evidence is omitted rather than rendered without a source,
//! exactly as the correlation summary omits an empty selection.

use std::collections::{BTreeMap, BTreeSet};

use thalassa_domain::{
    ChangeAssociation, ChangeEvent, ChangeEventId, ChangeMetric, ChangeMetricKey,
    ConsoleEvidenceId, DrillDownDestination, DrillDownReference, DrillDownTarget,
    EvidenceSourceKind, NumberUnit, ResourceScope,
};

/// Build the change metrics for the events inside the requested window.
///
/// `events` is the in-window event set the timeline was built from, so a
/// change outside the window never contributes to a count.
pub fn build(
    events: &[ChangeEvent],
    associations: &[ChangeAssociation],
    scope: &ResourceScope,
) -> Vec<ChangeMetric> {
    let mut metrics = Vec::new();
    push_metric(
        &mut metrics,
        ChangeMetricKey::ChangesInWindow,
        None,
        events,
        scope,
    );

    let associated_ids = associations
        .iter()
        .map(|association| association.change_id)
        .collect::<BTreeSet<ChangeEventId>>();
    let associated = events
        .iter()
        .filter(|event| associated_ids.contains(&event.id))
        .cloned()
        .collect::<Vec<_>>();
    push_metric(
        &mut metrics,
        ChangeMetricKey::AssociatedChanges,
        None,
        &associated,
        scope,
    );

    let mut by_source: BTreeMap<&str, Vec<ChangeEvent>> = BTreeMap::new();
    for event in events {
        by_source
            .entry(source_wire(event.source))
            .or_default()
            .push(event.clone());
    }
    for (_, source_events) in by_source {
        let source = source_events
            .first()
            .map(|event| event.source)
            .expect("a grouped source always has one event");
        push_metric(
            &mut metrics,
            ChangeMetricKey::ChangesBySource,
            Some(source),
            &source_events,
            scope,
        );
    }
    metrics
}

fn push_metric(
    metrics: &mut Vec<ChangeMetric>,
    key: ChangeMetricKey,
    source: Option<EvidenceSourceKind>,
    events: &[ChangeEvent],
    scope: &ResourceScope,
) {
    if events.is_empty() {
        return;
    }
    let evidence_ids = events
        .iter()
        .flat_map(|event| event.source_record.evidence_ids.iter().cloned())
        .collect::<BTreeSet<ConsoleEvidenceId>>();
    if evidence_ids.is_empty() {
        return;
    }
    let evidence_ids = evidence_ids.into_iter().collect::<Vec<_>>();
    let key_name = match (key, source) {
        (ChangeMetricKey::ChangesInWindow, _) => "changes_in_window".to_owned(),
        (ChangeMetricKey::AssociatedChanges, _) => "associated_changes".to_owned(),
        (ChangeMetricKey::ChangesBySource, Some(source)) => {
            format!("changes_by_source:{}", source_wire(source))
        }
        (ChangeMetricKey::ChangesBySource, None) => "changes_by_source".to_owned(),
    };
    metrics.push(ChangeMetric {
        key,
        source,
        value: events.len() as f64,
        unit: NumberUnit::Count,
        evidence_ids: evidence_ids.clone(),
        drill_down: DrillDownTarget {
            destination: DrillDownDestination::Evidence,
            evidence_ids: evidence_ids.clone(),
            filter_key: Some(format!("metric:{key_name}")),
        },
        drill_down_reference: DrillDownReference {
            source_query: format!("change:summary:{key_name}"),
            scope: scope.clone(),
            time_window: None,
            evidence_ids,
        },
    });
}

fn source_wire(source: EvidenceSourceKind) -> &'static str {
    match source {
        EvidenceSourceKind::GitHub => "github",
        EvidenceSourceKind::GitLab => "gitlab",
        EvidenceSourceKind::ArgoCd => "argo_cd",
        _ => "unknown",
    }
}
