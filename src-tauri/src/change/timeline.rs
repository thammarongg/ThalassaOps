//! Bounded, deterministic ordering for normalized change events.

use chrono::{DateTime, Utc};
use std::collections::BTreeSet;
use thalassa_domain::{
    ChangeError, ChangeEvent, ChangeEventId, ChangeTimeline, TimeWindow, MAX_CHANGE_LIMIT,
};

/// Build a half-open `[window.start, window.end)` timeline from normalized
/// events, retaining only the newest `limit` entries when truncation is needed.
pub fn build(
    events: &[ChangeEvent],
    window: &TimeWindow,
    limit: usize,
) -> Result<ChangeTimeline, ChangeError> {
    window.validate().map_err(|_| ChangeError::InvalidWindow)?;
    if limit == 0 || limit > MAX_CHANGE_LIMIT as usize {
        return Err(ChangeError::InvalidLimit);
    }

    let start = parse_timestamp(&window.start)?;
    let end = parse_timestamp(&window.end)?;
    let mut seen = BTreeSet::new();
    let mut selected = Vec::new();
    for event in events {
        event.validate()?;
        if !seen.insert(event.id) {
            return Err(ChangeError::DuplicateId);
        }
        let occurred_at = parse_timestamp(&event.occurred_at)?;
        if occurred_at >= start && occurred_at < end {
            selected.push((occurred_at, event.id));
        }
    }

    selected.sort();
    let truncated = selected.len() > limit;
    if truncated {
        let drop_count = selected.len() - limit;
        selected.drain(..drop_count);
    }
    let entry_ids = selected
        .into_iter()
        .map(|(_, id)| id)
        .collect::<Vec<ChangeEventId>>();

    Ok(ChangeTimeline {
        window: window.clone(),
        entry_ids,
        truncated,
    })
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, ChangeError> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| ChangeError::InvalidTimestamp)
}
