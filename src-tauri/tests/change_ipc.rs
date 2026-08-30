// SPDX-License-Identifier: Apache-2.0

use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::tempdir;
use thalassa_domain::{
    ChangeEvidenceRequest, ChangeRequest, ResourceScope, TimeWindow, MAX_CHANGE_LOOKBACK_SECONDS,
};
use thalassa_ipc::{Capability, CommandEnvelope, CommandName, IpcErrorCode};
use thalassa_policy::{DataClass, PolicyDocument, PolicyRuntime};
use thalassaops::app::{AppState, IpcResult};
use thalassaops::connectors::InMemoryCredentialStore;
use uuid::Uuid;

fn test_state() -> (tempfile::TempDir, AppState) {
    let directory = tempdir().unwrap();
    let state = AppState::open_with_credential_store(
        directory.path().join("thalassaops.sqlite"),
        Arc::new(InMemoryCredentialStore::default()),
    )
    .unwrap();
    (directory, state)
}

fn request() -> ChangeRequest {
    ChangeRequest {
        window: TimeWindow {
            start: "2026-08-28T08:00:00Z".into(),
            end: "2026-08-28T09:00:00Z".into(),
        },
        evaluated_at: "2026-08-28T09:00:00Z".into(),
        lookback_seconds: 3_600,
        limit: 50,
    }
}

fn envelope(verb: &str, capability: Capability, payload: Value) -> CommandEnvelope<Value> {
    CommandEnvelope {
        request_id: Uuid::new_v4(),
        command: CommandName::new("change", verb).unwrap(),
        capability,
        scope: ResourceScope::default(),
        payload,
    }
}

fn snapshot_envelope() -> CommandEnvelope<Value> {
    envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(request()).unwrap(),
    )
}

#[test]
fn change_snapshot_returns_a_validated_workspace_scoped_projection() {
    let (_directory, state) = test_state();

    let IpcResult::Ok { value, .. } = state.change_snapshot(snapshot_envelope()) else {
        panic!("change.snapshot should succeed")
    };

    assert_eq!(value.scope.workspace_id, Some(state.bootstrap.workspace.id));
    assert!(!value.events.is_empty());
    assert!(!value.timeline.entry_ids.is_empty());
    assert!(value.validate().is_ok());
}

#[test]
fn change_snapshot_requires_workspace_read_capability() {
    let (_directory, state) = test_state();

    let result = state.change_snapshot(envelope(
        "snapshot",
        Capability::ResourceRead,
        serde_json::to_value(request()).unwrap(),
    ));

    assert!(matches!(
        result,
        IpcResult::Err { error, .. } if error.code == IpcErrorCode::PermissionDenied
    ));
}

#[test]
fn a_bounded_envelope_scope_is_rejected_before_any_adapter_runs() {
    let (_directory, state) = test_state();
    let mut request_envelope = snapshot_envelope();
    request_envelope.scope = ResourceScope::workspace(
        Uuid::from_u128(21),
        Uuid::from_u128(22),
        Uuid::from_u128(23),
    );

    let result = state.change_snapshot(request_envelope);

    assert!(matches!(
        result,
        IpcResult::Err { error, .. } if error.code == IpcErrorCode::PermissionDenied
    ));
}

#[test]
fn lookback_above_the_cap_maps_to_invalid_request() {
    let (_directory, state) = test_state();
    let mut payload = request();
    payload.lookback_seconds = MAX_CHANGE_LOOKBACK_SECONDS + 1;

    let result = state.change_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(payload).unwrap(),
    ));

    assert!(matches!(
        result,
        IpcResult::Err { error, .. } if error.code == IpcErrorCode::InvalidRequest
    ));
}

#[test]
fn an_unknown_request_field_is_rejected_as_an_invalid_request() {
    let (_directory, state) = test_state();

    let result = state.change_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        json!({
            "window": { "start": "2026-08-28T08:00:00Z", "end": "2026-08-28T09:00:00Z" },
            "evaluated_at": "2026-08-28T09:00:00Z",
            "lookback_seconds": 3_600,
            "limit": 50,
            "connector_id": "github-prod"
        }),
    ));

    assert!(matches!(
        result,
        IpcResult::Err { error, .. } if error.code == IpcErrorCode::InvalidRequest
    ));
}

#[test]
fn every_evidence_id_in_the_snapshot_resolves_inside_it() {
    let (_directory, state) = test_state();
    let IpcResult::Ok {
        value: snapshot, ..
    } = state.change_snapshot(snapshot_envelope())
    else {
        panic!("change.snapshot should succeed")
    };

    let mut evidence_ids: Vec<String> = Vec::new();
    for event in &snapshot.events {
        evidence_ids.extend(event.evidence_ids.iter().cloned());
    }
    for association in &snapshot.associations {
        evidence_ids.extend(association.evidence_ids.iter().cloned());
    }
    for metric in &snapshot.metrics {
        evidence_ids.extend(metric.evidence_ids.iter().cloned());
    }
    evidence_ids.sort();
    evidence_ids.dedup();
    assert!(!evidence_ids.is_empty());

    let IpcResult::Ok {
        value: evidence, ..
    } = state.change_evidence(envelope(
        "evidence",
        Capability::ResourceRead,
        serde_json::to_value(ChangeEvidenceRequest {
            evidence_ids: evidence_ids.clone(),
        })
        .unwrap(),
    ))
    else {
        panic!("change.evidence should resolve snapshot evidence")
    };

    assert_eq!(evidence.len(), evidence_ids.len());
    for reference in &evidence {
        assert!(evidence_ids.contains(&reference.id));
        assert!(reference.redaction.classification_verified);
        assert!(reference.redaction.redaction_verified);
    }
}

#[test]
fn change_evidence_rejects_an_id_absent_from_the_current_snapshot() {
    let (_directory, state) = test_state();

    let result = state.change_evidence(envelope(
        "evidence",
        Capability::ResourceRead,
        serde_json::to_value(ChangeEvidenceRequest {
            evidence_ids: vec![
                "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
            ],
        })
        .unwrap(),
    ));

    assert!(matches!(
        result,
        IpcResult::Err { error, .. } if error.code == IpcErrorCode::NotFound
    ));
}

#[test]
fn commands_require_audit_retention_policy_before_source_work() {
    let (_directory, mut state) = test_state();
    state.policy = PolicyRuntime::load(
        PolicyDocument::baseline(14).with_audit_log_data_classes(vec![DataClass::Public]),
    )
    .unwrap();

    let result = state.change_snapshot(snapshot_envelope());

    assert!(matches!(
        result,
        IpcResult::Err { error, .. }
            if error.code == IpcErrorCode::PolicyDenied
                && error.message == "change audit retention policy denied"
    ));
}

#[test]
fn a_change_that_only_precedes_a_signal_is_listed_but_not_associated() {
    let (_directory, state) = test_state();
    let IpcResult::Ok {
        value: snapshot, ..
    } = state.change_snapshot(snapshot_envelope())
    else {
        panic!("change.snapshot should succeed")
    };

    let associated: Vec<_> = snapshot
        .associations
        .iter()
        .map(|association| association.change_id)
        .collect();
    assert!(!associated.is_empty());
    assert!(
        snapshot
            .timeline
            .entry_ids
            .iter()
            .any(|entry_id| !associated.contains(entry_id)),
        "temporal presence alone must not create an association"
    );
    for association in &snapshot.associations {
        assert!(association.lead_time_seconds >= 0.0);
        assert!(association.lead_time_seconds <= snapshot.lookback_seconds as f64);
    }
}

#[test]
fn descriptors_are_read_only_and_capability_scoped() {
    let snapshot = thalassa_ipc::change_snapshot_descriptor();
    let evidence = thalassa_ipc::change_evidence_descriptor();

    assert_eq!(snapshot.name.to_string(), "change.snapshot");
    assert_eq!(snapshot.required_capability, Capability::WorkspaceRead);
    assert_eq!(evidence.name.to_string(), "change.evidence");
    assert_eq!(evidence.required_capability, Capability::ResourceRead);
    assert!(!snapshot.scope.is_bounded());
    assert!(!evidence.scope.is_bounded());
}

#[test]
fn repeated_snapshots_are_byte_identical() {
    let (_directory, state) = test_state();

    let IpcResult::Ok { value: first, .. } = state.change_snapshot(snapshot_envelope()) else {
        panic!("change.snapshot should succeed")
    };
    let IpcResult::Ok { value: second, .. } = state.change_snapshot(snapshot_envelope()) else {
        panic!("change.snapshot should succeed")
    };

    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap()
    );
}

#[test]
fn the_snapshot_carries_no_credential_email_or_diff_body() {
    let (_directory, state) = test_state();
    let IpcResult::Ok {
        value: snapshot, ..
    } = state.change_snapshot(snapshot_envelope())
    else {
        panic!("change.snapshot should succeed")
    };

    let serialized = serde_json::to_string(&snapshot).unwrap();
    for marker in [
        "@example.invalid",
        "Bearer ",
        "private_token",
        "@@ -",
        "\"patch\"",
    ] {
        assert!(
            !serialized.contains(marker),
            "the snapshot must not carry {marker}"
        );
    }
}
