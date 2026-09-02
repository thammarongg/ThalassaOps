// SPDX-License-Identifier: Apache-2.0

//! Task 7 proofs: every incident command authorizes descriptor, capability,
//! envelope scope, membership, workspace grant and permission before it parses
//! a payload or looks up a target, and no denial discloses whether a target
//! exists.

use std::sync::Arc;

use serde_json::{json, Value};
use tempfile::tempdir;
use thalassa_domain::{
    Incident, IncidentDisposition, IncidentMutation, IncidentPage, IncidentStatus,
    IncidentTimelinePage, MembershipRole, ResourceScope,
};
use thalassa_ipc::{Capability, CommandEnvelope, CommandName, IpcErrorCode};
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

fn envelope(verb: &str, capability: Capability, payload: Value) -> CommandEnvelope<Value> {
    CommandEnvelope {
        request_id: Uuid::new_v4(),
        command: CommandName::new("incident", verb).unwrap(),
        capability,
        scope: ResourceScope::default(),
        payload,
    }
}

fn business_impact() -> Value {
    json!({
        "level": "high",
        "summary": "Checkout unavailable for customers",
        "customer_scope": "production customers",
        "service_criticality": "tier-0",
        "trajectory": "stable",
        "dimensions": {
            "availability": "high",
            "customer_reach": "none",
            "business_criticality": "none",
            "data_integrity": "none",
            "security_privacy": "none",
            "financial_contractual": "none",
            "trajectory": "stable",
            "production": true
        },
        "evidence_ids": ["evidence-checkout"]
    })
}

fn create_payload(state: &AppState) -> Value {
    json!({
        "summary": "Checkout errors reported by the on-call operator",
        "triggers": [{
            "kind": "manual_report",
            "observed_at": "2026-08-28T08:57:30Z",
            "summary": "Operator opening an incident for elevated checkout errors",
            "scope": {
                "organization_id": state.bootstrap.organization.id,
                "team_id": state.bootstrap.team.id,
                "workspace_id": state.bootstrap.workspace.id,
                "environment_id": null,
                "resource_ids": []
            }
        }],
        "business_impact": business_impact(),
        "initial_roles": [{
            "role": "owner",
            "principal_id": state.bootstrap.principal.id
        }]
    })
}

fn create_envelope(state: &AppState) -> CommandEnvelope<Value> {
    envelope("create", Capability::IncidentWrite, create_payload(state))
}

fn created(state: &AppState) -> IncidentMutation {
    match state.incident_create(create_envelope(state)) {
        IpcResult::Ok { value, .. } => value,
        IpcResult::Err { error, .. } => panic!("incident.create should succeed: {error:?}"),
    }
}

fn error_of<T>(result: IpcResult<T>) -> thalassa_ipc::IpcError {
    match result {
        IpcResult::Err { error, .. } => error,
        IpcResult::Ok { .. } => panic!("the command should have been rejected"),
    }
}

#[test]
fn create_get_list_and_timeline_round_trip_through_ipc() {
    let (_directory, state) = test_state();
    let mutation = created(&state);
    assert_eq!(mutation.incident.status, IncidentStatus::Detected);
    assert_eq!(
        mutation.incident.scope.workspace_id,
        Some(state.bootstrap.workspace.id)
    );

    let IpcResult::Ok { value, .. }: IpcResult<Incident> = state.incident_get(envelope(
        "get",
        Capability::IncidentRead,
        json!({ "incident_id": mutation.incident.id }),
    )) else {
        panic!("incident.get should succeed")
    };
    assert_eq!(value, mutation.incident);

    let IpcResult::Ok { value, .. }: IpcResult<IncidentPage> = state.incident_list(envelope(
        "list",
        Capability::IncidentRead,
        json!({ "cursor": null, "limit": 10 }),
    )) else {
        panic!("incident.list should succeed")
    };
    assert_eq!(value.items.len(), 1);

    let IpcResult::Ok { value, .. }: IpcResult<IncidentTimelinePage> =
        state.incident_timeline(envelope(
            "timeline",
            Capability::IncidentRead,
            json!({
                "incident_id": mutation.incident.id,
                "after_sequence": null,
                "limit": 10
            }),
        ))
    else {
        panic!("incident.timeline should succeed")
    };
    assert_eq!(value.events, mutation.events);
}

#[test]
fn every_write_command_advances_the_lifecycle_under_ipc() {
    let (_directory, state) = test_state();
    let mutation = created(&state);

    let IpcResult::Ok { value: triaged, .. } = state.incident_transition(envelope(
        "transition",
        Capability::IncidentWrite,
        json!({
            "incident_id": mutation.incident.id,
            "expected_version": mutation.incident.version,
            "transition": {
                "target": "triage",
                "context": {
                    "business_impact": business_impact(),
                    "owner": state.bootstrap.principal.id,
                    "duplicate_checked": true
                }
            }
        }),
    )) else {
        panic!("incident.transition should succeed")
    };
    assert_eq!(triaged.incident.status, IncidentStatus::Triage);

    let IpcResult::Ok {
        value: dispositioned,
        ..
    } = state.incident_set_disposition(envelope(
        "set_disposition",
        Capability::IncidentWrite,
        json!({
            "incident_id": triaged.incident.id,
            "expected_version": triaged.incident.version,
            "command": {
                "disposition": "informational",
                "duplicate_of_incident_id": null,
                "reason": "Tracked for awareness while the rollout continues"
            }
        }),
    ))
    else {
        panic!("incident.set_disposition should succeed")
    };
    assert_eq!(
        dispositioned.incident.disposition,
        Some(IncidentDisposition::Informational)
    );
    assert_eq!(dispositioned.incident.status, IncidentStatus::Triage);

    let IpcResult::Ok {
        value: assigned, ..
    } = state.incident_assign_role(envelope(
        "assign_role",
        Capability::IncidentWrite,
        json!({
            "incident_id": dispositioned.incident.id,
            "expected_version": dispositioned.incident.version,
            "command": {
                "action": "assign",
                "details": {
                    "role": "incident_commander",
                    "principal_id": state.bootstrap.principal.id
                }
            }
        }),
    ))
    else {
        panic!("incident.assign_role should succeed")
    };

    let IpcResult::Ok { value: severe, .. } = state.incident_set_severity(envelope(
        "set_severity",
        Capability::IncidentWrite,
        json!({
            "incident_id": assigned.incident.id,
            "expected_version": assigned.incident.version,
            "command": {
                "action": "override",
                "details": {
                    "selected": "S1",
                    "reason": "Payment provider confirms a wider outage",
                    "evidence_ids": ["evidence-checkout"]
                }
            }
        }),
    )) else {
        panic!("incident.set_severity should succeed")
    };
    assert!(severe.incident.severity_override.is_some());
    assert_eq!(severe.incident.version, 5);
}

#[test]
fn operator_can_write_but_auditor_and_viewer_cannot() {
    for role in [
        MembershipRole::Owner,
        MembershipRole::Administrator,
        MembershipRole::Operator,
    ] {
        let (_directory, mut state) = test_state();
        state.bootstrap.membership.role = role.clone();
        assert!(
            matches!(
                state.incident_create(create_envelope(&state)),
                IpcResult::Ok { .. }
            ),
            "{role:?} manages incidents"
        );
    }

    for role in [MembershipRole::Viewer, MembershipRole::Auditor] {
        let (_directory, mut state) = test_state();
        state.bootstrap.membership.role = role.clone();
        assert_eq!(
            error_of(state.incident_create(create_envelope(&state))).code,
            IpcErrorCode::PermissionDenied,
            "{role:?} cannot manage incidents"
        );
    }
}

#[test]
fn readers_may_read_without_write_permission() {
    let (_directory, mut state) = test_state();
    let mutation = created(&state);

    for role in [MembershipRole::Viewer, MembershipRole::Auditor] {
        state.bootstrap.membership.role = role.clone();
        assert!(
            matches!(
                state.incident_get(envelope(
                    "get",
                    Capability::IncidentRead,
                    json!({ "incident_id": mutation.incident.id }),
                )),
                IpcResult::Ok { .. }
            ),
            "{role:?} reads incidents"
        );
    }
}

#[test]
fn unauthorized_write_does_not_disclose_incident_existence() {
    let (_directory, mut state) = test_state();
    let mutation = created(&state);
    state.bootstrap.membership.role = MembershipRole::Viewer;

    let missing_id = Uuid::new_v4();
    for incident_id in [missing_id, mutation.incident.id] {
        let error = error_of(state.incident_transition(envelope(
            "transition",
            Capability::IncidentWrite,
            json!({
                "incident_id": incident_id,
                "expected_version": 1,
                "transition": {
                    "target": "triage",
                    "context": {
                        "business_impact": business_impact(),
                        "owner": state.bootstrap.principal.id,
                        "duplicate_checked": true
                    }
                }
            }),
        )));
        assert_eq!(error.code, IpcErrorCode::PermissionDenied);
        let rendered = serde_json::to_string(&error).unwrap();
        assert!(
            !rendered.contains(&incident_id.to_string()),
            "a denial must not echo the target identifier"
        );
    }
}

#[test]
fn commands_require_their_exact_capability_and_an_unbounded_envelope() {
    let (_directory, state) = test_state();

    assert_eq!(
        error_of(state.incident_create(envelope(
            "create",
            Capability::IncidentRead,
            create_payload(&state)
        )))
        .code,
        IpcErrorCode::PermissionDenied
    );
    assert_eq!(
        error_of(state.incident_list(envelope(
            "list",
            Capability::WorkspaceRead,
            json!({ "cursor": null, "limit": 10 })
        )))
        .code,
        IpcErrorCode::PermissionDenied
    );

    let mut bounded = create_envelope(&state);
    bounded.scope = ResourceScope::workspace(
        state.bootstrap.workspace.id,
        state.bootstrap.team.id,
        state.bootstrap.organization.id,
    );
    assert_eq!(
        error_of(state.incident_create(bounded)).code,
        IpcErrorCode::PermissionDenied
    );

    let mut wrong_verb = create_envelope(&state);
    wrong_verb.command = CommandName::new("incident", "get").unwrap();
    assert_eq!(
        error_of(state.incident_create(wrong_verb)).code,
        IpcErrorCode::PermissionDenied
    );
}

#[test]
fn unknown_payload_keys_and_invalid_limits_are_rejected() {
    let (_directory, state) = test_state();
    let mutation = created(&state);

    let mut payload = create_payload(&state);
    payload["unexpected"] = json!("value");
    let error =
        error_of(state.incident_create(envelope("create", Capability::IncidentWrite, payload)));
    assert_eq!(error.code, IpcErrorCode::InvalidRequest);

    for limit in [0, 101] {
        assert_eq!(
            error_of(state.incident_list(envelope(
                "list",
                Capability::IncidentRead,
                json!({ "cursor": null, "limit": limit })
            )))
            .code,
            IpcErrorCode::InvalidRequest
        );
        assert_eq!(
            error_of(state.incident_timeline(envelope(
                "timeline",
                Capability::IncidentRead,
                json!({
                    "incident_id": mutation.incident.id,
                    "after_sequence": null,
                    "limit": limit
                })
            )))
            .code,
            IpcErrorCode::InvalidRequest
        );
    }
}

#[test]
fn a_missing_incident_is_not_found_and_a_stale_version_is_a_typed_conflict() {
    let (_directory, state) = test_state();
    let mutation = created(&state);

    assert_eq!(
        error_of(state.incident_get(envelope(
            "get",
            Capability::IncidentRead,
            json!({ "incident_id": Uuid::new_v4() })
        )))
        .code,
        IpcErrorCode::NotFound
    );

    let stale = json!({
        "incident_id": mutation.incident.id,
        "expected_version": 99,
        "transition": {
            "target": "triage",
            "context": {
                "business_impact": business_impact(),
                "owner": state.bootstrap.principal.id,
                "duplicate_checked": true
            }
        }
    });
    let error = error_of(state.incident_transition(envelope(
        "transition",
        Capability::IncidentWrite,
        stale,
    )));
    assert_eq!(error.code, IpcErrorCode::InvalidRequest);
    assert_eq!(error.details["reason"], json!("incident_version_conflict"));
}

#[test]
fn a_retried_create_envelope_returns_the_same_incident() {
    let (_directory, state) = test_state();
    let envelope = create_envelope(&state);

    let IpcResult::Ok { value: first, .. } = state.incident_create(envelope.clone()) else {
        panic!("incident.create should succeed")
    };
    let IpcResult::Ok { value: retried, .. } = state.incident_create(envelope) else {
        panic!("the retry should replay the stored incident")
    };
    assert_eq!(first, retried);

    let IpcResult::Ok { value, .. }: IpcResult<IncidentPage> = state.incident_list(envelope_list())
    else {
        panic!("incident.list should succeed")
    };
    assert_eq!(value.items.len(), 1);
}

fn envelope_list() -> CommandEnvelope<Value> {
    envelope(
        "list",
        Capability::IncidentRead,
        json!({ "cursor": null, "limit": 10 }),
    )
}
