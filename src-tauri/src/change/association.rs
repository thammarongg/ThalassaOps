//! Structural associations between preceding changes and signal candidates.
//!
//! A change is eligible only when it precedes the earliest signal in a
//! candidate and falls inside the caller's bounded lookback.  Temporal
//! precedence is deliberately not enough: an exact target or a topology path
//! must also explain the association.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use thalassa_domain::{
    ChangeAssociation, ChangeError, ChangeEvent, CorrelationCandidate, CorrelationQualification,
    Signal, SignalTarget, TopologyPath, MAX_CHANGE_LOOKBACK_SECONDS,
};

use crate::topology::TopologyCorrelationResolver;

/// Associate preceding changes with correlation candidates.
///
/// Results are deterministic and ordered by `(candidate_id, change_id)`.
/// Exact target matches take precedence over topology resolution; topology
/// paths are considered only when no exact candidate grouping target matches.
pub fn associate(
    events: &[ChangeEvent],
    candidates: &[CorrelationCandidate],
    signals: &[Signal],
    lookback_seconds: f64,
    topology: &dyn TopologyCorrelationResolver,
) -> Result<Vec<ChangeAssociation>, ChangeError> {
    validate_lookback(lookback_seconds)?;

    let mut event_map = BTreeMap::new();
    for event in events {
        event.validate()?;
        if event_map.insert(event.id, event).is_some() {
            return Err(ChangeError::DuplicateId);
        }
    }

    let mut signal_map = BTreeMap::new();
    for signal in signals {
        if signal_map.insert(signal.id, signal).is_some() {
            return Err(ChangeError::DuplicateId);
        }
    }

    let mut candidate_ids = BTreeSet::new();
    let mut associations = Vec::new();
    for candidate in candidates {
        if !candidate_ids.insert(candidate.id.as_str()) {
            return Err(ChangeError::DuplicateId);
        }

        let mut candidate_signals = Vec::with_capacity(candidate.signal_ids.len());
        for signal_id in &candidate.signal_ids {
            candidate_signals.push(
                *signal_map
                    .get(signal_id)
                    .ok_or(ChangeError::CandidateReferenceMissing)?,
            );
        }

        let earliest_observed_at = candidate_signals
            .iter()
            .filter_map(|signal| signal.observed_at.as_deref())
            .map(parse_timestamp)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .min();
        let Some(earliest_observed_at) = earliest_observed_at else {
            continue;
        };

        // Topology candidates do not have grouping_targets. Their canonical
        // candidate targets are the targets carried by their referenced
        // signals, so retain those as the topology resolver's right-hand
        // endpoints. Exact matches remain constrained to grouping_targets.
        let mut candidate_targets = candidate.grouping_targets.clone();
        for signal in &candidate_signals {
            candidate_targets.extend(signal.targets.iter().cloned());
        }
        candidate_targets.sort_by(target_ordering);
        candidate_targets.dedup();

        for event in events {
            if !candidate.scope.contains(&event.scope) {
                continue;
            }
            let occurred_at = parse_timestamp(&event.occurred_at)?;
            if occurred_at >= earliest_observed_at {
                continue;
            }
            let lead_time_seconds = lead_time_seconds(occurred_at, earliest_observed_at);
            if lead_time_seconds > lookback_seconds {
                continue;
            }

            let exact_target = event.targets.iter().find(|event_target| {
                candidate
                    .grouping_targets
                    .iter()
                    .any(|candidate_target| candidate_target == *event_target)
            });

            let (target, topology_path_ids, topology_paths) = if let Some(target) = exact_target {
                (Some(target.clone()), Vec::new(), Vec::new())
            } else {
                topology_matches(event, &candidate_targets, &candidate.window, topology)
            };

            if target.is_none() && topology_path_ids.is_empty() {
                continue;
            }

            let mut evidence_ids = BTreeSet::new();
            evidence_ids.extend(event.evidence_ids.iter().cloned());
            for path in topology_paths {
                evidence_ids.extend(path.evidence_ids);
            }

            associations.push(ChangeAssociation {
                change_id: event.id,
                candidate_id: candidate.id.clone(),
                qualification: CorrelationQualification::ProbableStructural,
                lead_time_seconds,
                target,
                topology_path_ids,
                evidence_ids: evidence_ids.into_iter().collect(),
            });
        }
    }

    associations.sort_by(|left, right| {
        left.candidate_id
            .cmp(&right.candidate_id)
            .then_with(|| left.change_id.cmp(&right.change_id))
    });
    Ok(associations)
}

fn validate_lookback(lookback_seconds: f64) -> Result<(), ChangeError> {
    if !lookback_seconds.is_finite() {
        return Err(ChangeError::NonFiniteNumber);
    }
    if lookback_seconds < 0.0 {
        return Err(ChangeError::NegativeNumber);
    }
    if lookback_seconds > MAX_CHANGE_LOOKBACK_SECONDS as f64 {
        return Err(ChangeError::InvalidLookback);
    }
    Ok(())
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, ChangeError> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| ChangeError::InvalidTimestamp)
}

fn lead_time_seconds(occurred_at: DateTime<Utc>, observed_at: DateTime<Utc>) -> f64 {
    let duration = observed_at.signed_duration_since(occurred_at);
    // A duration that can pass the bounded lookback is filtered before this
    // helper is called, so nanoseconds cannot overflow for an eligible value.
    duration
        .num_nanoseconds()
        .map(|nanoseconds| nanoseconds as f64 / 1_000_000_000.0)
        .unwrap_or(f64::INFINITY)
}

fn topology_matches(
    event: &ChangeEvent,
    candidate_targets: &[SignalTarget],
    window: &thalassa_domain::CorrelationWindow,
    topology: &dyn TopologyCorrelationResolver,
) -> (Option<SignalTarget>, Vec<String>, Vec<TopologyPath>) {
    let mut paths_by_id = BTreeMap::<String, TopologyPath>::new();
    let mut ambiguous_path_ids = BTreeSet::new();
    for event_target in &event.targets {
        for candidate_target in candidate_targets {
            if event_target == candidate_target {
                continue;
            }
            let Ok(Some(path)) = topology.relation(event_target, candidate_target, window) else {
                continue;
            };
            if path.validate().is_err()
                || !path.node_ids.contains(&event_target.id)
                || !path.node_ids.contains(&candidate_target.id)
            {
                continue;
            }
            if ambiguous_path_ids.contains(&path.id) {
                continue;
            }
            if let Some(existing) = paths_by_id.get(&path.id) {
                if existing != &path {
                    paths_by_id.remove(&path.id);
                    ambiguous_path_ids.insert(path.id);
                }
                continue;
            }
            paths_by_id.insert(path.id.clone(), path);
        }
    }
    let topology_path_ids = paths_by_id.keys().cloned().collect::<Vec<_>>();
    let topology_paths = paths_by_id.into_values().collect::<Vec<_>>();
    (None, topology_path_ids, topology_paths)
}

fn target_ordering(left: &SignalTarget, right: &SignalTarget) -> std::cmp::Ordering {
    left.kind
        .cmp(&right.kind)
        .then_with(|| left.id.cmp(&right.id))
}
