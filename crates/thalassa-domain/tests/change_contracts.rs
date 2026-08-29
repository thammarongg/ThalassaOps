// SPDX-License-Identifier: Apache-2.0

use serde_json::json;
use thalassa_domain::{
    ChangeActor, ChangeActorKind, ChangeDiffStat, ChangeEvent, ChangeKind, ChangeLinkKind,
    ChangeOutcome, ChangeSourceLink, CorrelationQualification, CorrelationReasonKind,
    EvidenceSourceKind, NumberUnit,
};

#[test]
fn change_source_kinds_use_stable_wire_values() {
    assert_eq!(
        serde_json::to_value(EvidenceSourceKind::GitHub).unwrap(),
        json!("github")
    );
    assert_eq!(
        serde_json::to_value(EvidenceSourceKind::GitLab).unwrap(),
        json!("gitlab")
    );
    assert_eq!(
        serde_json::to_value(EvidenceSourceKind::ArgoCd).unwrap(),
        json!("argo_cd")
    );
}

#[test]
fn change_kind_keeps_sprint_11_values_and_adds_sprint_14_values() {
    for (kind, wire) in [
        (ChangeKind::Deployment, "deployment"),
        (ChangeKind::Configuration, "configuration"),
        (ChangeKind::Maintenance, "maintenance"),
        (ChangeKind::Connector, "connector"),
        (ChangeKind::CodeCommit, "code_commit"),
        (ChangeKind::CodeMerge, "code_merge"),
        (ChangeKind::Sync, "sync"),
        (ChangeKind::Rollback, "rollback"),
    ] {
        assert_eq!(serde_json::to_value(kind).unwrap(), json!(wire));
    }
}

#[test]
fn preceding_change_reason_is_always_probable_structural() {
    assert_eq!(
        serde_json::to_value(CorrelationReasonKind::PrecedingChange).unwrap(),
        json!("preceding_change")
    );
    assert_eq!(
        serde_json::to_value(CorrelationQualification::ProbableStructural).unwrap(),
        json!("probable_structural")
    );
}

#[test]
fn diff_stat_rejects_non_finite_and_negative_counts() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
        let stat = ChangeDiffStat {
            files_changed: value,
            insertions: 0.0,
            deletions: 0.0,
            unit: NumberUnit::Count,
        };
        assert!(stat.validate().is_err(), "expected rejection for {value}");
    }
}

#[test]
fn source_link_rejects_query_strings_and_non_https() {
    for url in [
        "https://github.com/acme/api/commit/abc?token=secret",
        "http://github.com/acme/api/commit/abc",
        "https://user:pass@github.com/acme/api/commit/abc",
        "https://github.com/acme/api/commit/abc#frag",
    ] {
        let link = ChangeSourceLink {
            kind: ChangeLinkKind::Commit,
            url: url.to_string(),
        };
        assert!(
            link.validate(EvidenceSourceKind::GitHub).is_err(),
            "expected rejection for {url}"
        );
    }
}

#[test]
fn actor_handle_rejects_email_shaped_identity() {
    let actor = ChangeActor {
        kind: ChangeActorKind::Human,
        handle: Some("someone@example.com".to_string()),
    };
    assert!(actor.validate().is_err());
}

#[test]
fn change_event_requires_occurred_at_and_serializes_optional_fields_as_null() {
    let event: ChangeEvent =
        serde_json::from_str(include_str!("fixtures/change_event.json")).expect("fixture parses");
    assert!(event.validate().is_ok());
    let value = serde_json::to_value(&event).unwrap();
    assert!(value.get("occurred_at").unwrap().is_string());
    assert!(value.get("environment").unwrap().is_null());
    assert_eq!(event.outcome, ChangeOutcome::Succeeded);
}
