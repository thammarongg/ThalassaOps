//! Deterministic suppression and maintenance-window evaluation.
//!
//! Suppression is an annotation over an admitted [`Signal`], not a retention
//! filter.  This module therefore only computes the typed decision that is
//! carried by the Signal; callers keep the complete source record, payload,
//! deduplication identity and evidence unchanged.

use chrono::{DateTime, Utc};
use std::collections::BTreeSet;

use thalassa_domain::{
    CorrelationError, MaintenanceWindow, Signal, SuppressionKind, SuppressionRule, SuppressionState,
};

/// Return whether an enabled rule matches a Signal's admitted scope and
/// exact typed selectors.  An omitted target is a wildcard; it does not cause
/// a target to be inferred from a name or any other source field.
pub fn rule_matches_signal(
    rule: &SuppressionRule,
    signal: &Signal,
) -> Result<bool, CorrelationError> {
    validate_rule_definition(rule)?;
    signal.validate()?;
    Ok(rule.enabled
        && rule.scope.contains(&signal.scope)
        && rule.source.is_none_or(|source| source == signal.source)
        && rule
            .signal_kind
            .is_none_or(|signal_kind| signal_kind == signal.kind)
        && rule.target.as_ref().is_none_or(|target| {
            signal
                .targets
                .iter()
                .any(|signal_target| signal_target == target)
        }))
}

/// Return whether an enabled maintenance window matches a Signal.
///
/// The Signal's observed/event timestamp is the only timestamp considered for
/// membership.  Ingestion time is intentionally ignored, and a missing event
/// timestamp never enters an active window.  Window intervals are half-open:
/// `[start, end)`.
pub fn maintenance_window_matches_signal(
    window: &MaintenanceWindow,
    signal: &Signal,
) -> Result<bool, CorrelationError> {
    validate_maintenance_definition(window)?;
    signal.validate()?;
    if !window.enabled || !window.scope.contains(&signal.scope) {
        return Ok(false);
    }
    if let Some(target) = &window.target {
        if !signal
            .targets
            .iter()
            .any(|signal_target| signal_target == target)
        {
            return Ok(false);
        }
    }
    let Some(observed_at) = signal.observed_at.as_deref() else {
        return Ok(false);
    };
    let observed_at = parse_timestamp(observed_at)?;
    let start = parse_timestamp(&window.window.start)?;
    let end = parse_timestamp(&window.window.end)?;
    Ok(observed_at >= start && observed_at < end)
}

/// Compute the complete typed suppression decision for one Signal.
pub fn evaluate_suppression(
    signal: &Signal,
    rules: &[SuppressionRule],
    maintenance_windows: &[MaintenanceWindow],
    evaluated_at: &str,
    policy_version: u64,
) -> Result<SuppressionState, CorrelationError> {
    validate_policy(rules, maintenance_windows)?;
    parse_timestamp(evaluated_at)?;
    signal.validate()?;
    evaluate_suppression_unchecked(
        signal,
        rules,
        maintenance_windows,
        evaluated_at,
        policy_version,
    )
}

/// Evaluate all Signals atomically, replacing only their suppression state.
/// The temporary state vector means an invalid policy or Signal cannot leave a
/// partially evaluated input behind.
pub fn apply_suppression(
    signals: &mut [Signal],
    rules: &[SuppressionRule],
    maintenance_windows: &[MaintenanceWindow],
    evaluated_at: &str,
    policy_version: u64,
) -> Result<(), CorrelationError> {
    validate_policy(rules, maintenance_windows)?;
    parse_timestamp(evaluated_at)?;
    let states = signals
        .iter()
        .map(|signal| {
            signal.validate()?;
            evaluate_suppression_unchecked(
                signal,
                rules,
                maintenance_windows,
                evaluated_at,
                policy_version,
            )
        })
        .collect::<Result<Vec<_>, CorrelationError>>()?;
    for (signal, state) in signals.iter_mut().zip(states) {
        signal.suppression = state;
    }
    Ok(())
}

/// Alias for callers that use the Signal-first spelling.
pub fn evaluate_signal_suppression(
    signal: &Signal,
    rules: &[SuppressionRule],
    maintenance_windows: &[MaintenanceWindow],
    evaluated_at: &str,
    policy_version: u64,
) -> Result<SuppressionState, CorrelationError> {
    evaluate_suppression(
        signal,
        rules,
        maintenance_windows,
        evaluated_at,
        policy_version,
    )
}

/// Alias for callers that want to apply decisions to a retained Signal set.
pub fn apply_signal_suppression(
    signals: &mut [Signal],
    rules: &[SuppressionRule],
    maintenance_windows: &[MaintenanceWindow],
    evaluated_at: &str,
    policy_version: u64,
) -> Result<(), CorrelationError> {
    apply_suppression(
        signals,
        rules,
        maintenance_windows,
        evaluated_at,
        policy_version,
    )
}

/// Alias for the rule matching seam.
pub fn matches_rule(rule: &SuppressionRule, signal: &Signal) -> Result<bool, CorrelationError> {
    rule_matches_signal(rule, signal)
}

/// Alias for the maintenance matching seam.
pub fn matches_maintenance_window(
    window: &MaintenanceWindow,
    signal: &Signal,
) -> Result<bool, CorrelationError> {
    maintenance_window_matches_signal(window, signal)
}

fn evaluate_suppression_unchecked(
    signal: &Signal,
    rules: &[SuppressionRule],
    maintenance_windows: &[MaintenanceWindow],
    evaluated_at: &str,
    policy_version: u64,
) -> Result<SuppressionState, CorrelationError> {
    let mut rule_ids = rules
        .iter()
        .filter(|rule| rule_matches_signal_unchecked(rule, signal))
        .map(|rule| rule.id.clone())
        .collect::<Vec<_>>();
    let mut maintenance_window_ids = maintenance_windows
        .iter()
        .filter(|window| maintenance_window_matches_signal_unchecked(window, signal))
        .map(|window| window.id.clone())
        .collect::<Vec<_>>();
    rule_ids.sort();
    rule_ids.dedup();
    maintenance_window_ids.sort();
    maintenance_window_ids.dedup();
    let kind = match (rule_ids.is_empty(), maintenance_window_ids.is_empty()) {
        (true, true) => SuppressionKind::NotSuppressed,
        (false, true) => SuppressionKind::Rule,
        (true, false) => SuppressionKind::MaintenanceWindow,
        (false, false) => SuppressionKind::RuleAndMaintenanceWindow,
    };
    let state = SuppressionState {
        kind,
        rule_ids,
        maintenance_window_ids,
        evaluated_at: evaluated_at.to_owned(),
        policy_version,
    };
    state.validate()?;
    Ok(state)
}

fn validate_policy(
    rules: &[SuppressionRule],
    maintenance_windows: &[MaintenanceWindow],
) -> Result<(), CorrelationError> {
    let mut rule_ids = BTreeSet::new();
    for rule in rules {
        validate_rule_definition(rule)?;
        if !rule_ids.insert(rule.id.as_str()) {
            return Err(CorrelationError::DuplicateId);
        }
    }
    let mut window_ids = BTreeSet::new();
    for window in maintenance_windows {
        validate_maintenance_definition(window)?;
        if !window_ids.insert(window.id.as_str()) {
            return Err(CorrelationError::DuplicateId);
        }
    }
    Ok(())
}

fn validate_rule_definition(rule: &SuppressionRule) -> Result<(), CorrelationError> {
    rule.validate()?;
    if !rule.scope.is_bounded() {
        return Err(CorrelationError::ScopeMismatch);
    }
    Ok(())
}

fn validate_maintenance_definition(window: &MaintenanceWindow) -> Result<(), CorrelationError> {
    window.validate()?;
    if !window.scope.is_bounded() {
        return Err(CorrelationError::ScopeMismatch);
    }
    Ok(())
}

fn rule_matches_signal_unchecked(rule: &SuppressionRule, signal: &Signal) -> bool {
    rule.enabled
        && rule.scope.contains(&signal.scope)
        && rule.source.is_none_or(|source| source == signal.source)
        && rule
            .signal_kind
            .is_none_or(|signal_kind| signal_kind == signal.kind)
        && rule.target.as_ref().is_none_or(|target| {
            signal
                .targets
                .iter()
                .any(|signal_target| signal_target == target)
        })
}

fn maintenance_window_matches_signal_unchecked(
    window: &MaintenanceWindow,
    signal: &Signal,
) -> bool {
    if !window.enabled || !window.scope.contains(&signal.scope) {
        return false;
    }
    if let Some(target) = &window.target {
        if !signal
            .targets
            .iter()
            .any(|signal_target| signal_target == target)
        {
            return false;
        }
    }
    let Some(observed_at) = signal.observed_at.as_deref() else {
        return false;
    };
    let Ok(observed_at) = parse_timestamp(observed_at) else {
        return false;
    };
    let Ok(start) = parse_timestamp(&window.window.start) else {
        return false;
    };
    let Ok(end) = parse_timestamp(&window.window.end) else {
        return false;
    };
    observed_at >= start && observed_at < end
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, CorrelationError> {
    if value.trim().is_empty() {
        return Err(CorrelationError::InvalidTimestamp);
    }
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| CorrelationError::InvalidTimestamp)
}
