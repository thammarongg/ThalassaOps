// SPDX-License-Identifier: Apache-2.0

use thalassa_domain::{ActionRiskClass, ExecutionMode, ResourceScope};
use thalassa_policy::*;

fn scope() -> ResourceScope {
    ResourceScope::workspace(uuid::Uuid::nil(), uuid::Uuid::nil(), uuid::Uuid::nil())
}

fn staging_scope() -> ResourceScope {
    ResourceScope::environment(
        uuid::Uuid::from_u128(1),
        uuid::Uuid::from_u128(2),
        uuid::Uuid::from_u128(3),
        uuid::Uuid::from_u128(4),
    )
}

fn production_scope() -> ResourceScope {
    ResourceScope::environment(
        uuid::Uuid::from_u128(5),
        uuid::Uuid::from_u128(2),
        uuid::Uuid::from_u128(3),
        uuid::Uuid::from_u128(4),
    )
}

#[test]
fn baseline_allows_public_hosted_ai_only_when_classification_and_redaction_are_verified() {
    let runtime = PolicyRuntime::baseline();
    let allowed = runtime.evaluate_egress(EgressRequest::verified(
        DataClass::Public,
        EgressDestination::HostedAi,
    ));
    assert!(allowed.is_allowed());

    let unverified = runtime.evaluate_egress(EgressRequest::new(
        DataClass::Public,
        EgressDestination::HostedAi,
    ));
    assert_eq!(
        unverified,
        PolicyDecision::Denied {
            reason: PolicyDenyReason::UnverifiedClassificationOrRedaction,
            policy_version: 1
        }
    );
}

#[test]
fn external_integration_egress_obeys_configured_data_classes() {
    let document =
        PolicyDocument::baseline(2).with_external_integration_data_classes(vec![DataClass::Public]);
    let runtime = PolicyRuntime::load(document).unwrap();

    assert_eq!(
        runtime.evaluate_egress(EgressRequest::verified(
            DataClass::Internal,
            EgressDestination::ExternalIntegration,
        )),
        PolicyDecision::Denied {
            reason: PolicyDenyReason::DataClassNotPermitted,
            policy_version: 2,
        }
    );
}

#[test]
fn external_integration_restrictions_do_not_restrict_audit_log_egress() {
    let document =
        PolicyDocument::baseline(2).with_external_integration_data_classes(vec![DataClass::Public]);
    let runtime = PolicyRuntime::load(document).unwrap();

    assert_eq!(
        runtime.evaluate_egress(EgressRequest::verified(
            DataClass::Internal,
            EgressDestination::AuditLog,
        )),
        PolicyDecision::Allowed { policy_version: 2 }
    );
}

#[test]
fn audit_log_data_classes_gate_audit_log_egress() {
    let document = PolicyDocument::baseline(2).with_audit_log_data_classes(vec![DataClass::Public]);
    let runtime = PolicyRuntime::load(document).unwrap();

    assert_eq!(
        runtime.evaluate_egress(EgressRequest::verified(
            DataClass::Internal,
            EgressDestination::AuditLog,
        )),
        PolicyDecision::Denied {
            reason: PolicyDenyReason::DataClassNotPermitted,
            policy_version: 2,
        }
    );
}

#[test]
fn immutable_secret_protection_cannot_be_overridden_by_document_policy() {
    let document =
        PolicyDocument::baseline(2).with_hosted_ai_data_classes(vec![DataClass::Restricted]);
    let runtime = PolicyRuntime::load(document).unwrap();
    let request = EgressRequest::verified(DataClass::Restricted, EgressDestination::HostedAi)
        .with_immutable_secret(true);
    assert_eq!(
        runtime.evaluate_egress(request),
        PolicyDecision::Denied {
            reason: PolicyDenyReason::ImmutableRestrictedData,
            policy_version: 2
        }
    );
    let restricted_without_secret_marker =
        EgressRequest::verified(DataClass::Restricted, EgressDestination::HostedAi);
    assert_eq!(
        runtime.evaluate_egress(restricted_without_secret_marker),
        PolicyDecision::Denied {
            reason: PolicyDenyReason::ImmutableRestrictedData,
            policy_version: 2
        }
    );
}

#[test]
fn policy_auto_requires_mutating_action_and_explicit_enablement() {
    let runtime = PolicyRuntime::baseline();
    let read_only = runtime.evaluate_action(ActionPolicyRequest {
        risk_class: ActionRiskClass::ReadOnly,
        execution_mode: ExecutionMode::PolicyAuto,
        scope: scope(),
    });
    assert_eq!(
        read_only,
        PolicyDecision::Denied {
            reason: PolicyDenyReason::PolicyAutoRequiresMutatingAction,
            policy_version: 1
        }
    );
    let mutating = runtime.evaluate_action(ActionPolicyRequest {
        risk_class: ActionRiskClass::Mutating,
        execution_mode: ExecutionMode::PolicyAuto,
        scope: scope(),
    });
    assert_eq!(
        mutating,
        PolicyDecision::Denied {
            reason: PolicyDenyReason::PolicyAutoDisabled,
            policy_version: 1
        }
    );
}

#[test]
fn policy_auto_scope_only_constrains_policy_auto_requests() {
    let runtime =
        PolicyRuntime::load(PolicyDocument::baseline(2).enable_policy_auto(staging_scope()))
            .unwrap();

    let approval = runtime.evaluate_action(ActionPolicyRequest {
        risk_class: ActionRiskClass::Mutating,
        execution_mode: ExecutionMode::Approval,
        scope: production_scope(),
    });
    assert_eq!(approval, PolicyDecision::Allowed { policy_version: 2 });

    let policy_auto = runtime.evaluate_action(ActionPolicyRequest {
        risk_class: ActionRiskClass::Mutating,
        execution_mode: ExecutionMode::PolicyAuto,
        scope: production_scope(),
    });
    assert_eq!(
        policy_auto,
        PolicyDecision::Denied {
            reason: PolicyDenyReason::ScopeNotPermitted,
            policy_version: 2
        }
    );
}

#[test]
fn blocked_actions_use_an_action_specific_deny_reason() {
    let runtime = PolicyRuntime::baseline();
    let blocked = runtime.evaluate_action(ActionPolicyRequest {
        risk_class: ActionRiskClass::Blocked,
        execution_mode: ExecutionMode::Approval,
        scope: scope(),
    });

    assert_eq!(
        blocked,
        PolicyDecision::Denied {
            reason: PolicyDenyReason::ActionBlocked,
            policy_version: 1
        }
    );
}

#[test]
fn policy_documents_are_versioned_and_reject_invalid_versions() {
    assert!(PolicyRuntime::load(PolicyDocument::baseline(0)).is_err());
    let runtime = PolicyRuntime::load(PolicyDocument::baseline(7)).unwrap();
    assert_eq!(runtime.version(), 7);
}

#[test]
fn policy_runtime_can_load_a_versioned_json_document() {
    let json = serde_json::to_string(&PolicyDocument::baseline(9)).unwrap();
    let runtime = PolicyRuntime::load_json(&json).unwrap();
    assert_eq!(runtime.version(), 9);
}

#[test]
fn policy_auto_requires_a_bounded_scope() {
    assert!(PolicyRuntime::load(
        PolicyDocument::baseline(3).enable_policy_auto(ResourceScope::default())
    )
    .is_err());
}
