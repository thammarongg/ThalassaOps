// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeSet;

use serde_json::Value;
use thalassa_domain::{
    CandidateStatus, CorrelationRequest, EvidenceSourceKind, ResourceScope, SignalKind,
    SignalTarget, SignalTargetKind, SuppressionKind, TimeWindow,
};
use thalassaops::correlation::adapters::{normalize_operational, normalize_security};
use thalassaops::correlation::{
    correlate_signals, correlation_fixture_catalog, evaluate_suppression,
    maintenance_window_matches_signal, rule_matches_signal, CorrelationFixtureCatalog,
    CorrelationInput, SourceRecordStore,
};

fn request(evaluated_at: &str) -> CorrelationRequest {
    CorrelationRequest {
        window: TimeWindow {
            start: "2026-08-28T08:55:00Z".into(),
            end: "2026-08-28T09:00:00Z".into(),
        },
        evaluated_at: evaluated_at.into(),
        allowed_lateness_seconds: 300,
    }
}

fn fixture_signal(key: &str) -> thalassa_domain::Signal {
    let catalog = correlation_fixture_catalog();
    let fixture = catalog
        .fixtures
        .iter()
        .find(|fixture| fixture.key == key)
        .unwrap_or_else(|| panic!("fixture {key} exists"));
    let mut records = SourceRecordStore::default();
    if fixture.source_kind.is_security_source() {
        normalize_security(fixture, &mut records)
            .expect("security fixture should normalize")
            .remove(0)
    } else {
        normalize_operational(fixture, &mut records)
            .expect("operational fixture should normalize")
            .remove(0)
    }
}

fn policy() -> CorrelationFixtureCatalog {
    correlation_fixture_catalog()
}

fn input(
    scope: ResourceScope,
    signals: Vec<thalassa_domain::Signal>,
    evaluated_at: &str,
    policy: &CorrelationFixtureCatalog,
) -> CorrelationInput {
    let evidence = signals
        .iter()
        .flat_map(|signal| signal.evidence_ids.iter().map(|id| id.as_str()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|id| {
            correlation_fixture_catalog()
                .fixtures
                .iter()
                .flat_map(|fixture| fixture.evidence.iter())
                .find(|evidence| evidence.id == id)
                .unwrap_or_else(|| panic!("evidence {id} exists"))
                .clone()
        })
        .collect();
    CorrelationInput {
        generated_at: evaluated_at.into(),
        scope,
        request: request(evaluated_at),
        signals,
        source_status: Vec::new(),
        evidence,
        prior_window: None,
        suppression_rules: policy.suppression_rules.clone(),
        maintenance_windows: policy.maintenance_windows.clone(),
        policy_version: 13,
    }
}

#[test]
fn rules_require_enabled_scope_and_exact_optional_selectors() {
    let catalog = correlation_fixture_catalog();
    let signal = fixture_signal("suppression-rule-only");
    let rule = catalog
        .suppression_rules
        .iter()
        .find(|rule| rule.id == "rule-suppress-checkout-test")
        .expect("rule-only rule");
    assert!(rule_matches_signal(rule, &signal).unwrap());

    let mut disabled = rule.clone();
    disabled.enabled = false;
    assert!(!rule_matches_signal(&disabled, &signal).unwrap());

    let mut wrong_source = rule.clone();
    wrong_source.source = Some(EvidenceSourceKind::Alertmanager);
    assert!(!rule_matches_signal(&wrong_source, &signal).unwrap());

    let mut wrong_kind = rule.clone();
    wrong_kind.signal_kind = Some(SignalKind::Alert);
    assert!(!rule_matches_signal(&wrong_kind, &signal).unwrap());

    let mut wrong_target = rule.clone();
    wrong_target.target = Some(SignalTarget {
        kind: SignalTargetKind::Service,
        id: "service/other".into(),
    });
    assert!(!rule_matches_signal(&wrong_target, &signal).unwrap());

    let mut wildcard = rule.clone();
    wildcard.target = None;
    assert!(rule_matches_signal(&wildcard, &signal).unwrap());
}

#[test]
fn maintenance_matching_is_event_time_half_open_and_never_ingestion_time() {
    let catalog = correlation_fixture_catalog();
    let window = &catalog.maintenance_windows[0];
    let mut signal = fixture_signal("suppression-maintenance-only");
    assert!(maintenance_window_matches_signal(window, &signal).unwrap());

    signal.observed_at = Some(window.window.start.clone());
    assert!(maintenance_window_matches_signal(window, &signal).unwrap());
    signal.observed_at = Some(window.window.end.clone());
    assert!(!maintenance_window_matches_signal(window, &signal).unwrap());
    signal.observed_at = None;
    assert!(!maintenance_window_matches_signal(window, &signal).unwrap());

    let mut disabled = window.clone();
    disabled.enabled = false;
    assert!(!maintenance_window_matches_signal(&disabled, &signal).unwrap());
}

#[test]
fn all_matching_rules_and_windows_are_sorted_and_classified() {
    let catalog = correlation_fixture_catalog();
    let signal = fixture_signal("suppression-both-match");
    let state = evaluate_suppression(
        &signal,
        &catalog.suppression_rules,
        &catalog.maintenance_windows,
        "2026-08-28T09:00:00Z",
        13,
    )
    .unwrap();
    assert_eq!(state.kind, SuppressionKind::RuleAndMaintenanceWindow);
    assert_eq!(state.rule_ids, vec!["rule-suppress-checkout-deployment"]);
    assert_eq!(
        state.maintenance_window_ids,
        vec!["maintenance-checkout-release"]
    );
    assert_eq!(state.evaluated_at, "2026-08-28T09:00:00Z");
    assert_eq!(state.policy_version, 13);
}

#[test]
fn multiple_matching_rules_and_windows_are_retained_in_sorted_order() {
    let catalog = correlation_fixture_catalog();
    let signal = fixture_signal("suppression-both-match");
    let mut rules = catalog.suppression_rules.clone();
    let mut rule_low = rules[1].clone();
    rule_low.id = "rule-suppress-a".into();
    let mut rule_high = rules[1].clone();
    rule_high.id = "rule-suppress-z".into();
    rules.extend([rule_high, rule_low]);

    let mut windows = catalog.maintenance_windows.clone();
    let mut window_low = windows[0].clone();
    window_low.id = "maintenance-a".into();
    let mut window_high = windows[0].clone();
    window_high.id = "maintenance-z".into();
    windows.extend([window_high, window_low]);

    let state =
        evaluate_suppression(&signal, &rules, &windows, "2026-08-28T09:00:00Z", 13).unwrap();
    assert_eq!(
        state.rule_ids,
        vec![
            "rule-suppress-a",
            "rule-suppress-checkout-deployment",
            "rule-suppress-z"
        ]
    );
    assert_eq!(
        state.maintenance_window_ids,
        vec![
            "maintenance-a",
            "maintenance-checkout-release",
            "maintenance-z"
        ]
    );
}

#[test]
fn scope_and_target_selectors_never_widen_a_suppression_match() {
    let catalog = correlation_fixture_catalog();
    let signal = fixture_signal("suppression-both-match");
    let mut out_of_scope = catalog.suppression_rules[1].clone();
    out_of_scope.scope = ResourceScope::workspace(
        uuid::Uuid::from_u128(99),
        uuid::Uuid::from_u128(2),
        uuid::Uuid::from_u128(1),
    );
    assert!(!rule_matches_signal(&out_of_scope, &signal).unwrap());

    let mut wrong_window_target = catalog.maintenance_windows[0].clone();
    wrong_window_target.target = Some(SignalTarget {
        kind: SignalTargetKind::Service,
        id: "service/checkout".into(),
    });
    assert!(!maintenance_window_matches_signal(&wrong_window_target, &signal).unwrap());
}

#[test]
fn suppressed_signal_retains_source_identity_payload_and_evidence() {
    let catalog = correlation_fixture_catalog();
    let before = fixture_signal("suppression-both-match");
    let mut after = before.clone();
    let state = evaluate_suppression(
        &before,
        &catalog.suppression_rules,
        &catalog.maintenance_windows,
        "2026-08-28T09:00:00Z",
        13,
    )
    .unwrap();
    after.suppression = state;

    assert_eq!(after.id, before.id);
    assert_eq!(after.source, before.source);
    assert_eq!(after.source_record, before.source_record);
    assert_eq!(after.observed_at, before.observed_at);
    assert_eq!(after.ingested_at, before.ingested_at);
    assert_eq!(after.scope, before.scope);
    assert_eq!(after.targets, before.targets);
    assert_eq!(after.payload, before.payload);
    assert_eq!(after.dedup_key, before.dedup_key);
    assert_eq!(after.evidence_ids, before.evidence_ids);
    assert_eq!(
        after.suppression.kind,
        SuppressionKind::RuleAndMaintenanceWindow
    );
    let encoded = serde_json::to_value(&after).unwrap();
    assert_eq!(
        encoded["source_record"]["content_digest"],
        Value::String(before.source_record.content_digest)
    );
    assert!(encoded["payload"].to_string().contains("observed_value"));
}

#[test]
fn mixed_and_all_suppressed_components_have_explainable_statuses() {
    let policy = policy();
    let mixed_left = fixture_signal("suppression-mixed-alert");
    let mixed_right = fixture_signal("suppression-mixed-anomaly");
    let scope = mixed_left.scope.clone();
    let mixed = correlate_signals(
        input(
            scope.clone(),
            vec![mixed_left, mixed_right],
            "2026-08-28T09:00:00Z",
            &policy,
        ),
        &NoTopology,
    )
    .unwrap();
    assert_eq!(mixed.candidates.len(), 1);
    assert_eq!(mixed.candidates[0].status, CandidateStatus::Active);
    assert!(mixed
        .signals
        .iter()
        .any(|signal| signal.suppression.kind != SuppressionKind::NotSuppressed));

    let all_left = fixture_signal("suppression-all-maintenance");
    let all_right = fixture_signal("suppression-all-both");
    let all = correlate_signals(
        input(
            scope,
            vec![all_left, all_right],
            "2026-08-28T09:00:00Z",
            &policy,
        ),
        &NoTopology,
    )
    .unwrap();
    assert_eq!(all.candidates.len(), 1);
    assert_eq!(all.candidates[0].status, CandidateStatus::Suppressed);
}

#[test]
fn late_mixed_components_are_provisional_but_late_all_suppressed_stays_suppressed() {
    let policy = policy();
    let mixed_left = fixture_signal("suppression-mixed-alert");
    let mixed_right = fixture_signal("suppression-mixed-anomaly");
    let scope = mixed_left.scope.clone();
    let initial_input = input(
        scope.clone(),
        vec![mixed_left.clone(), mixed_right.clone()],
        "2026-08-28T09:05:00Z",
        &policy,
    );
    let initial = correlate_signals(initial_input, &NoTopology).unwrap();

    let mut late_mixed = mixed_left;
    late_mixed.id = uuid::Uuid::from_u128(0x101);
    late_mixed.observed_at = Some("2026-08-28T08:58:00Z".into());
    late_mixed.ingested_at = Some("2026-08-28T09:06:00Z".into());
    let mut reopened_input = input(
        scope.clone(),
        vec![mixed_right, late_mixed],
        "2026-08-28T09:06:00Z",
        &policy,
    );
    reopened_input.prior_window = Some(initial.window.clone());
    let reopened = correlate_signals(reopened_input, &NoTopology).unwrap();
    assert_eq!(
        reopened.window.state,
        thalassa_domain::CorrelationWindowState::Reopened
    );
    assert_eq!(reopened.candidates[0].status, CandidateStatus::Provisional);

    let all_left = fixture_signal("suppression-all-maintenance");
    let all_right = fixture_signal("suppression-all-both");
    let all_scope = all_left.scope.clone();
    let initial_all = correlate_signals(
        input(
            all_scope.clone(),
            vec![all_left.clone(), all_right.clone()],
            "2026-08-28T09:05:00Z",
            &policy,
        ),
        &NoTopology,
    )
    .unwrap();
    let mut late_all = all_left;
    late_all.id = uuid::Uuid::from_u128(0x102);
    late_all.observed_at = Some("2026-08-28T08:58:00Z".into());
    late_all.ingested_at = Some("2026-08-28T09:06:00Z".into());
    let mut reopened_all_input = input(
        all_scope,
        vec![all_right, late_all],
        "2026-08-28T09:06:00Z",
        &policy,
    );
    reopened_all_input.prior_window = Some(initial_all.window);
    let reopened_all = correlate_signals(reopened_all_input, &NoTopology).unwrap();
    assert_eq!(
        reopened_all.candidates[0].status,
        CandidateStatus::Suppressed
    );
}

#[test]
fn singleton_suppressed_signal_is_retained_without_inventing_candidate() {
    let policy = policy();
    let signal = fixture_signal("suppression-rule-only");
    let snapshot = correlate_signals(
        input(
            signal.scope.clone(),
            vec![signal],
            "2026-08-28T09:00:00Z",
            &policy,
        ),
        &NoTopology,
    )
    .unwrap();
    assert!(snapshot.candidates.is_empty());
    assert_eq!(snapshot.signals.len(), 1);
    assert_eq!(snapshot.signals[0].suppression.kind, SuppressionKind::Rule);
}

#[test]
fn suppression_metadata_does_not_create_an_incident_or_raw_audit_payload() {
    let policy = policy();
    let signal = fixture_signal("suppression-rule-only");
    let snapshot = correlate_signals(
        input(
            signal.scope.clone(),
            vec![signal],
            "2026-08-28T09:00:00Z",
            &policy,
        ),
        &NoTopology,
    )
    .unwrap();
    let encoded = serde_json::to_string(&snapshot.signals[0].suppression).unwrap();
    assert!(!encoded.contains("CVE-"));
    assert!(!encoded.contains("incident"));
    assert!(!encoded.contains("payload"));
}

struct NoTopology;

impl thalassaops::correlation::TopologyCorrelationResolver for NoTopology {
    fn relation(
        &self,
        _left: &SignalTarget,
        _right: &SignalTarget,
        _window: &thalassa_domain::CorrelationWindow,
    ) -> Result<Option<thalassa_domain::TopologyPath>, thalassa_domain::TopologyError> {
        Ok(None)
    }
}
