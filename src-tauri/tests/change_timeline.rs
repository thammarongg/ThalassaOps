use chrono::DateTime;
use thalassa_domain::{ChangeError, ChangeEvent, TimeWindow};
use thalassaops::change::{adapters, fixtures, timeline};
mod change_support;

use change_support::{fixture_scope, memory_store};

fn replayed_events() -> Vec<ChangeEvent> {
    let scope = fixture_scope();
    let mut store = memory_store(scope.clone());
    adapters::replay_all(&mut store, &scope, fixtures::fixture_clock())
        .expect("fixtures replay")
        .events
}

fn window() -> TimeWindow {
    TimeWindow {
        start: "2026-08-29T08:00:00Z".into(),
        end: "2026-08-29T10:00:00Z".into(),
    }
}

#[test]
fn window_is_half_open_on_the_end_boundary() {
    let mut events = replayed_events();
    events[0].occurred_at = window().start.clone();
    events[1].occurred_at = window().end.clone();

    let timeline = timeline::build(&events[..2], &window(), 10).expect("window builds");

    assert_eq!(timeline.entry_ids, vec![events[0].id]);
}

#[test]
fn entries_are_ordered_by_occurred_at_then_id() {
    let mut events = replayed_events();
    let same_timestamp = "2026-08-29T08:30:00Z";
    events[0].occurred_at = same_timestamp.into();
    events[1].occurred_at = same_timestamp.into();
    let mut expected = vec![events[0].id, events[1].id];
    expected.sort();

    let timeline = timeline::build(&events[..2], &window(), 10).expect("window builds");

    assert_eq!(timeline.entry_ids, expected);
}

#[test]
fn exceeding_the_limit_drops_oldest_entries_and_sets_truncated() {
    let events = replayed_events();
    let mut expected = events.clone();
    expected.sort_by(|left, right| {
        let left_at = DateTime::parse_from_rfc3339(&left.occurred_at).unwrap();
        let right_at = DateTime::parse_from_rfc3339(&right.occurred_at).unwrap();
        left_at.cmp(&right_at).then_with(|| left.id.cmp(&right.id))
    });
    let expected = expected[expected.len() - 3..]
        .iter()
        .map(|event| event.id)
        .collect::<Vec<_>>();

    let timeline = timeline::build(&events, &window(), 3).expect("window builds");

    assert_eq!(timeline.entry_ids.len(), 3);
    assert!(timeline.truncated);
    assert_eq!(timeline.entry_ids, expected);
}

#[test]
fn an_invalid_window_is_a_typed_error() {
    let invalid = TimeWindow {
        start: "2026-08-29T09:00:00Z".into(),
        end: "2026-08-29T09:00:00Z".into(),
    };

    assert_eq!(
        timeline::build(&[], &invalid, 3),
        Err(ChangeError::InvalidWindow)
    );
}
