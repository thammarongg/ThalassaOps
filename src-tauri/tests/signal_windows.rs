// SPDX-License-Identifier: Apache-2.0

use thalassa_domain::{CorrelationRequest, CorrelationWindowState, TimeWindow};
use thalassaops::correlation::adapters::normalize_operational;
use thalassaops::correlation::window::{
    evaluate_window, request_with_clock, signal_membership, CorrelationWindowEvaluator, FixedClock,
    SignalWindowMembership,
};
use thalassaops::correlation::{correlation_fixture_catalog, SourceRecordStore};

fn fixture_signal(
    key: &str,
    observed_at: Option<&str>,
    ingested_at: Option<&str>,
) -> thalassa_domain::Signal {
    let mut fixture = correlation_fixture_catalog()
        .fixtures
        .into_iter()
        .find(|fixture| fixture.key == key)
        .expect("fixture exists");
    fixture.observed_at = observed_at.map(str::to_owned);
    fixture.ingested_at = ingested_at.map(str::to_owned);
    if observed_at.is_none() {
        if let Some(payload) = fixture.recorded_json.as_object_mut() {
            payload.remove("starts_at");
            payload.remove("startsAt");
        }
    }
    let mut records = SourceRecordStore::default();
    normalize_operational(&fixture, &mut records)
        .expect("fixture should normalize")
        .remove(0)
}

fn window() -> TimeWindow {
    TimeWindow {
        start: "2026-08-28T08:55:00Z".into(),
        end: "2026-08-28T09:00:00Z".into(),
    }
}

fn request(evaluated_at: &str, lateness: u64) -> CorrelationRequest {
    CorrelationRequest {
        window: window(),
        evaluated_at: evaluated_at.into(),
        allowed_lateness_seconds: lateness,
    }
}

#[test]
fn membership_is_start_inclusive_and_end_exclusive() {
    let request = request("2026-08-28T08:59:00Z", 60);
    let at_start = fixture_signal(
        "alert-checkout",
        Some("2026-08-28T08:55:00Z"),
        Some("2026-08-28T09:00:00Z"),
    );
    let at_end = fixture_signal(
        "alert-checkout",
        Some("2026-08-28T09:00:00Z"),
        Some("2026-08-28T09:00:00Z"),
    );
    assert_eq!(
        signal_membership(&at_start, &request).unwrap(),
        SignalWindowMembership::Eligible
    );
    assert_eq!(
        signal_membership(&at_end, &request).unwrap(),
        SignalWindowMembership::Outside
    );
}

#[test]
fn missing_and_future_observed_times_are_retained_but_ineligible() {
    let request = request("2026-08-28T08:59:00Z", 60);
    let missing = fixture_signal("alert-checkout", None, Some("2026-08-28T09:00:00Z"));
    let future = fixture_signal(
        "alert-checkout",
        Some("2026-08-28T09:01:00Z"),
        Some("2026-08-28T09:00:00Z"),
    );
    let assignment = evaluate_window(&request, &[missing.clone(), future.clone()], None).unwrap();
    assert!(assignment.eligible_signals.is_empty());
    assert!(assignment.missing_signal_ids.contains(&missing.id));
    assert!(assignment.outside_signal_ids.contains(&future.id));
    assert_eq!(assignment.retained_signal_ids().len(), 2);
}

#[test]
fn explicit_clock_builds_deterministic_evaluation_time_without_sleeping() {
    let clock = FixedClock::from_rfc3339("2026-08-28T08:59:00.123456Z").unwrap();
    let request = request_with_clock(window(), 60, &clock).unwrap();
    assert_eq!(request.evaluated_at, "2026-08-28T08:59:00.123456Z");
    assert_eq!(request.window, window());

    let evaluator = CorrelationWindowEvaluator::new(clock);
    assert_eq!(evaluator.request(window(), 60).unwrap(), request);
}

#[test]
fn watermark_and_state_follow_open_ready_and_finalized_boundaries() {
    let open = evaluate_window(&request("2026-08-28T08:59:59Z", 60), &[], None).unwrap();
    assert_eq!(open.window.state, CorrelationWindowState::Open);
    assert_eq!(open.window.watermark, "2026-08-28T08:58:59Z");

    let ready = evaluate_window(&request("2026-08-28T09:00:00Z", 60), &[], None).unwrap();
    assert_eq!(ready.window.state, CorrelationWindowState::ReadyToFinalize);

    let finalized = evaluate_window(&request("2026-08-28T09:01:00Z", 60), &[], None).unwrap();
    assert_eq!(finalized.window.state, CorrelationWindowState::Finalized);
}

#[test]
fn request_rejects_unbounded_window_and_lateness() {
    let too_wide = CorrelationRequest {
        window: TimeWindow {
            start: "2026-08-28T00:00:00Z".into(),
            end: "2026-08-29T00:00:01Z".into(),
        },
        evaluated_at: "2026-08-28T00:00:00Z".into(),
        allowed_lateness_seconds: 0,
    };
    assert!(evaluate_window(&too_wide, &[], None).is_err());
    assert!(evaluate_window(&request("2026-08-28T08:59:00Z", 21_601), &[], None).is_err());
}

#[test]
fn late_in_range_arrival_reopens_a_finalized_window() {
    let initial = fixture_signal(
        "alert-checkout",
        Some("2026-08-28T08:56:00Z"),
        Some("2026-08-28T08:57:00Z"),
    );
    let late = fixture_signal(
        "anomaly-checkout-errors",
        Some("2026-08-28T08:57:00Z"),
        Some("2026-08-28T09:02:00Z"),
    );
    let prior = evaluate_window(
        &request("2026-08-28T09:01:00Z", 60),
        std::slice::from_ref(&initial),
        None,
    )
    .unwrap();
    assert_eq!(prior.window.state, CorrelationWindowState::Finalized);

    let reopened = evaluate_window(
        &request("2026-08-28T09:02:00Z", 60),
        &[initial, late.clone()],
        Some(&prior.window),
    )
    .unwrap();
    assert_eq!(reopened.window.state, CorrelationWindowState::Reopened);
    assert_eq!(reopened.late_signal_ids, vec![late.id]);
    assert_eq!(reopened.eligible_signals.len(), 2);
}

#[test]
fn arrival_after_logical_finalization_is_late_even_if_prior_evaluation_was_later() {
    let initial = fixture_signal(
        "alert-checkout",
        Some("2026-08-28T08:56:00Z"),
        Some("2026-08-28T08:57:00Z"),
    );
    let late = fixture_signal(
        "anomaly-checkout-errors",
        Some("2026-08-28T08:57:00Z"),
        Some("2026-08-28T09:01:30Z"),
    );
    let prior = evaluate_window(
        &request("2026-08-28T09:05:00Z", 60),
        std::slice::from_ref(&initial),
        None,
    )
    .unwrap();
    let reopened = evaluate_window(
        &request("2026-08-28T09:06:00Z", 60),
        &[initial, late.clone()],
        Some(&prior.window),
    )
    .unwrap();
    assert_eq!(reopened.window.state, CorrelationWindowState::Reopened);
    assert_eq!(reopened.late_signal_ids, vec![late.id]);
}

#[test]
fn late_signal_at_end_edge_does_not_reopen_the_window() {
    let initial = fixture_signal(
        "alert-checkout",
        Some("2026-08-28T08:56:00Z"),
        Some("2026-08-28T08:57:00Z"),
    );
    let edge = fixture_signal(
        "anomaly-checkout-errors",
        Some("2026-08-28T09:00:00Z"),
        Some("2026-08-28T09:02:00Z"),
    );
    let prior = evaluate_window(
        &request("2026-08-28T09:01:00Z", 60),
        std::slice::from_ref(&initial),
        None,
    )
    .unwrap();
    let assignment = evaluate_window(
        &request("2026-08-28T09:02:00Z", 60),
        &[initial, edge],
        Some(&prior.window),
    )
    .unwrap();
    assert_eq!(assignment.window.state, CorrelationWindowState::Finalized);
    assert!(assignment.late_signal_ids.is_empty());
    assert_eq!(assignment.eligible_signals.len(), 1);
}

#[test]
fn eligible_signal_order_is_input_order_independent() {
    let first = fixture_signal(
        "alert-checkout",
        Some("2026-08-28T08:56:00Z"),
        Some("2026-08-28T08:57:00Z"),
    );
    let second = fixture_signal(
        "anomaly-checkout-errors",
        Some("2026-08-28T08:56:00Z"),
        Some("2026-08-28T08:58:00Z"),
    );
    let request = request("2026-08-28T08:59:00Z", 60);
    let left = evaluate_window(&request, &[first.clone(), second.clone()], None).unwrap();
    let right = evaluate_window(&request, &[second, first], None).unwrap();
    assert_eq!(left.eligible_signals, right.eligible_signals);
    assert_eq!(left.window, right.window);
}
