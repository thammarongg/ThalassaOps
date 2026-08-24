// SPDX-License-Identifier: Apache-2.0

use thalassa_domain::{ActionRiskClass, ExecutionMode, ResourceScope};
use thalassa_policy::*;

fn scope() -> ResourceScope {
    ResourceScope::workspace(uuid::Uuid::nil(), uuid::Uuid::nil(), uuid::Uuid::nil())
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
