// SPDX-License-Identifier: Apache-2.0

use rusqlite::Connection;
use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::tempdir;
use thalassa_domain::{
    CorrelationEvidenceRequest, CorrelationRequest, EvidenceSourceKind, MembershipRole,
    MembershipStatus, Permission, ResourceScope, TimeWindow,
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

fn request() -> CorrelationRequest {
    CorrelationRequest {
        window: TimeWindow {
            start: "2026-08-28T08:55:00Z".into(),
            end: "2026-08-28T09:05:00Z".into(),
        },
        evaluated_at: "2026-08-28T09:00:00Z".into(),
        allowed_lateness_seconds: 300,
    }
}

fn envelope(verb: &str, capability: Capability, payload: Value) -> CommandEnvelope<Value> {
    CommandEnvelope {
        request_id: Uuid::new_v4(),
        command: CommandName::new("correlation", verb).unwrap(),
        capability,
        scope: ResourceScope::default(),
        payload,
    }
}

#[test]
fn snapshot_command_returns_a_valid_workspace_scoped_projection() {
    let (_directory, state) = test_state();
    let result = state.correlation_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(request()).unwrap(),
    ));

    let IpcResult::Ok { value, .. } = result else {
        panic!("correlation.snapshot should succeed: {result:?}")
    };
    assert_eq!(value.scope.workspace_id, Some(state.bootstrap.workspace.id));
    assert!(!value.signals.is_empty());
    assert!(value.validate().is_ok());
}

#[test]
fn snapshot_command_rejects_the_wrong_capability_before_request_work() {
    let (_directory, state) = test_state();
    let result = state.correlation_snapshot(envelope(
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
fn snapshot_command_rejects_malformed_payload_and_invalid_window() {
    let (_directory, state) = test_state();
    let malformed = state.correlation_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        json!({ "window": {} }),
    ));
    assert!(matches!(
        malformed,
        IpcResult::Err { error, .. } if error.code == IpcErrorCode::InvalidRequest
    ));

    let mut invalid = request();
    invalid.window.end = invalid.window.start.clone();
    let invalid_window = state.correlation_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(invalid).unwrap(),
    ));
    assert!(matches!(
        invalid_window,
        IpcResult::Err { error, .. }
            if error.code == IpcErrorCode::InvalidRequest
                && error.message == "correlation window is invalid"
    ));
}

#[test]
fn commands_reject_inactive_membership_mismatched_principal_and_missing_grant() {
    let (_directory, mut state) = test_state();
    state.bootstrap.membership.status = MembershipStatus::Suspended;
    let suspended = state.correlation_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(request()).unwrap(),
    ));
    assert!(matches!(
        suspended,
        IpcResult::Err { error, .. } if error.code == IpcErrorCode::PermissionDenied
    ));

    state.bootstrap.membership.status = MembershipStatus::Active;
    state.bootstrap.membership.principal_id = Uuid::new_v4();
    let principal_mismatch = state.correlation_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(request()).unwrap(),
    ));
    assert!(matches!(
        principal_mismatch,
        IpcResult::Err { error, .. } if error.code == IpcErrorCode::PermissionDenied
    ));

    state.bootstrap.membership.principal_id = state.bootstrap.principal.id;
    state.bootstrap.membership.scope = ResourceScope::workspace(
        Uuid::new_v4(),
        state.bootstrap.team.id,
        state.bootstrap.organization.id,
    );
    let missing_grant = state.correlation_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(request()).unwrap(),
    ));
    assert!(matches!(
        missing_grant,
        IpcResult::Err { error, .. } if error.code == IpcErrorCode::PermissionDenied
    ));
}

#[test]
fn bounded_envelope_scope_is_rejected_without_echoing_scope_identifiers() {
    let (_directory, state) = test_state();
    let mut request = envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(request()).unwrap(),
    );
    request.scope = ResourceScope::environment(
        Uuid::from_u128(0x1111),
        Uuid::from_u128(0x2222),
        Uuid::from_u128(0x3333),
        Uuid::from_u128(0x4444),
    );

    let IpcResult::Err { error, .. } = state.correlation_snapshot(request) else {
        panic!("bounded correlation scope must be denied")
    };
    assert_eq!(error.code, IpcErrorCode::PermissionDenied);
    assert_eq!(
        error.details,
        json!({ "required_command": "correlation.snapshot" })
    );
}

#[test]
fn evidence_command_returns_only_ids_emitted_by_the_current_snapshot() {
    let (_directory, state) = test_state();
    let snapshot = match state.correlation_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(request()).unwrap(),
    )) {
        IpcResult::Ok { value, .. } => value,
        IpcResult::Err { error, .. } => panic!("snapshot failed: {error:?}"),
    };
    let evidence_id = snapshot.evidence[0].id.clone();
    let result = state.correlation_evidence(envelope(
        "evidence",
        Capability::ResourceRead,
        serde_json::to_value(CorrelationEvidenceRequest {
            evidence_ids: vec![evidence_id.clone()],
        })
        .unwrap(),
    ));
    let IpcResult::Ok { value, .. } = result else {
        panic!("correlation.evidence should succeed")
    };
    assert_eq!(value.len(), 1);
    assert_eq!(value[0].id, evidence_id);
}

#[test]
fn candidate_members_retain_operational_and_security_source_evidence() {
    let (_directory, state) = test_state();
    let snapshot = match state.correlation_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(request()).unwrap(),
    )) {
        IpcResult::Ok { value, .. } => value,
        IpcResult::Err { error, .. } => panic!("snapshot failed: {error:?}"),
    };
    let evidence_by_id = snapshot
        .evidence
        .iter()
        .map(|reference| (reference.id.as_str(), reference))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut saw_operational = false;
    let mut saw_security = false;

    for candidate in &snapshot.candidates {
        for signal_id in &candidate.signal_ids {
            let signal = snapshot
                .signals
                .iter()
                .find(|signal| signal.id == *signal_id)
                .unwrap_or_else(|| panic!("candidate references missing signal {signal_id}"));
            assert!(candidate.evidence_ids.iter().all(|id| snapshot
                .evidence
                .iter()
                .any(|reference| &reference.id == id)));
            assert!(!signal.source_record.evidence_ids.is_empty());
            for evidence_id in &signal.source_record.evidence_ids {
                let evidence = evidence_by_id.get(evidence_id.as_str()).unwrap_or_else(|| {
                    panic!("source record references missing evidence {evidence_id}")
                });
                assert_eq!(evidence.source_kind, signal.source);
                assert_eq!(evidence.scope, signal.scope);
                assert!(signal.evidence_ids.contains(evidence_id));
                assert!(candidate.evidence_ids.contains(evidence_id));
            }

            match signal.source {
                EvidenceSourceKind::Alertmanager
                | EvidenceSourceKind::Prometheus
                | EvidenceSourceKind::HealthCheck => saw_operational = true,
                EvidenceSourceKind::Trivy
                | EvidenceSourceKind::Falco
                | EvidenceSourceKind::Kyverno
                | EvidenceSourceKind::OpaGatekeeper => saw_security = true,
                _ => {}
            }
        }
    }

    assert!(
        saw_operational,
        "no operational signal was retained in a candidate"
    );
    assert!(
        saw_security,
        "no security signal was retained in a candidate"
    );
}

#[test]
fn snapshot_retains_source_records_and_evidence_in_the_local_ledger() {
    let (directory, state) = test_state();
    let snapshot = match state.correlation_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(request()).unwrap(),
    )) {
        IpcResult::Ok { value, .. } => value,
        IpcResult::Err { error, .. } => panic!("snapshot failed: {error:?}"),
    };

    let connection = Connection::open(directory.path().join("thalassaops.sqlite")).unwrap();
    for signal in &snapshot.signals {
        let record_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM source_records WHERE source_kind = ?1 AND content_digest = ?2 AND COALESCE(revision, '') = COALESCE(?3, '')",
                rusqlite::params![
                    serde_json::to_string(&signal.source).unwrap().trim_matches('"'),
                    signal.source_record.content_digest,
                    signal.source_record.revision,
                ],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(record_count, 1, "source record was not retained");

        for evidence_id in &signal.source_record.evidence_ids {
            let evidence_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM source_record_evidence WHERE evidence_id = ?1",
                    [evidence_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(evidence_count, 1, "source evidence was not retained");
        }
    }
}

#[test]
fn evidence_command_rejects_wrong_capability_malformed_duplicate_and_unknown_ids() {
    let (_directory, state) = test_state();
    let wrong_capability = state.correlation_evidence(envelope(
        "evidence",
        Capability::WorkspaceRead,
        json!({ "evidence_ids": ["evidence-security-trivy"] }),
    ));
    assert!(matches!(
        wrong_capability,
        IpcResult::Err { error, .. } if error.code == IpcErrorCode::PermissionDenied
    ));

    let malformed = state.correlation_evidence(envelope(
        "evidence",
        Capability::ResourceRead,
        json!({ "evidence_ids": "not-an-array" }),
    ));
    assert!(matches!(
        malformed,
        IpcResult::Err { error, .. } if error.code == IpcErrorCode::InvalidRequest
    ));

    let duplicate = state.correlation_evidence(envelope(
        "evidence",
        Capability::ResourceRead,
        json!({ "evidence_ids": ["evidence-security-trivy", "evidence-security-trivy"] }),
    ));
    assert!(matches!(
        duplicate,
        IpcResult::Err { error, .. } if error.code == IpcErrorCode::InvalidRequest
    ));

    let unknown = state.correlation_evidence(envelope(
        "evidence",
        Capability::ResourceRead,
        json!({ "evidence_ids": ["evidence-not-emitted"] }),
    ));
    assert!(matches!(
        unknown,
        IpcResult::Err { error, .. }
            if error.code == IpcErrorCode::NotFound
                && error.message == "correlation evidence was not emitted by the snapshot"
    ));
}

#[test]
fn commands_require_audit_retention_policy_before_source_work() {
    let (_directory, mut state) = test_state();
    state.policy = PolicyRuntime::load(
        PolicyDocument::baseline(13).with_audit_log_data_classes(vec![DataClass::Public]),
    )
    .unwrap();

    let snapshot = state.correlation_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(request()).unwrap(),
    ));
    assert!(matches!(
        snapshot,
        IpcResult::Err { error, .. }
            if error.code == IpcErrorCode::PolicyDenied
                && error.message == "correlation audit retention policy denied"
    ));
}

#[test]
fn descriptors_are_read_only_and_capability_scoped() {
    let snapshot = thalassa_ipc::correlation_snapshot_descriptor();
    assert_eq!(snapshot.name.to_string(), "correlation.snapshot");
    assert_eq!(snapshot.required_capability, Capability::WorkspaceRead);
    assert_eq!(snapshot.required_permission, Permission::Read);
    assert!(!snapshot.scope.is_bounded());

    let evidence = thalassa_ipc::correlation_evidence_descriptor();
    assert_eq!(evidence.name.to_string(), "correlation.evidence");
    assert_eq!(evidence.required_capability, Capability::ResourceRead);
    assert_eq!(evidence.required_permission, Permission::Read);
    assert!(!evidence.scope.is_bounded());
}

#[test]
fn read_roles_can_view_snapshot_and_evidence() {
    let (_directory, mut state) = test_state();
    state.bootstrap.membership.role = MembershipRole::Viewer;
    let snapshot = state.correlation_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(request()).unwrap(),
    ));
    assert!(matches!(snapshot, IpcResult::Ok { .. }));

    state.bootstrap.membership.role = MembershipRole::Auditor;
    let evidence = state.correlation_evidence(envelope(
        "evidence",
        Capability::ResourceRead,
        json!({ "evidence_ids": ["evidence-security-trivy"] }),
    ));
    assert!(matches!(evidence, IpcResult::Ok { .. }));
}
