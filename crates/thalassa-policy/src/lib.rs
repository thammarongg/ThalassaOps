// SPDX-License-Identifier: Apache-2.0

//! Versioned policy data and the immutable safety guard used by all egress paths.

use serde::{Deserialize, Serialize};
use thalassa_domain::{ActionRiskClass, ExecutionMode, ResourceScope};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DataClass {
    Public,
    Internal,
    Confidential,
    Restricted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum EgressDestination {
    HostedAi,
    LocalModel,
    LocalStorage,
    Ui,
    ExternalIntegration,
    AuditLog,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EgressRequest {
    pub data_class: DataClass,
    pub destination: EgressDestination,
    pub classification_verified: bool,
    pub redaction_verified: bool,
    pub contains_immutable_secret: bool,
}
impl EgressRequest {
    pub fn new(data_class: DataClass, destination: EgressDestination) -> Self {
        Self {
            data_class,
            destination,
            classification_verified: false,
            redaction_verified: false,
            contains_immutable_secret: false,
        }
    }
    pub fn verified(data_class: DataClass, destination: EgressDestination) -> Self {
        Self {
            classification_verified: true,
            redaction_verified: true,
            ..Self::new(data_class, destination)
        }
    }
    pub fn with_immutable_secret(mut self, contains: bool) -> Self {
        self.contains_immutable_secret = contains;
        self
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PolicyDenyReason {
    UnverifiedClassificationOrRedaction,
    ImmutableRestrictedData,
    DataClassNotPermitted,
    PolicyAutoDisabled,
    PolicyAutoRequiresMutatingAction,
    ScopeNotPermitted,
    ActionBlocked,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PolicyDecision {
    Allowed {
        policy_version: u64,
    },
    Denied {
        reason: PolicyDenyReason,
        policy_version: u64,
    },
}
impl PolicyDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed { .. })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ActionPolicyRequest {
    pub risk_class: ActionRiskClass,
    pub execution_mode: ExecutionMode,
    pub scope: ResourceScope,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PolicyDocument {
    pub id: String,
    pub version: u64,
    pub hosted_ai_data_classes: Vec<DataClass>,
    pub local_model_data_classes: Vec<DataClass>,
    #[serde(default = "default_external_integration_data_classes")]
    pub external_integration_data_classes: Vec<DataClass>,
    #[serde(default = "default_audit_log_data_classes")]
    pub audit_log_data_classes: Vec<DataClass>,
    pub policy_auto_enabled: bool,
    pub policy_auto_scope: Option<ResourceScope>,
}

fn default_external_integration_data_classes() -> Vec<DataClass> {
    vec![
        DataClass::Public,
        DataClass::Internal,
        DataClass::Confidential,
    ]
}

fn default_audit_log_data_classes() -> Vec<DataClass> {
    vec![
        DataClass::Public,
        DataClass::Internal,
        DataClass::Confidential,
    ]
}

impl PolicyDocument {
    pub fn baseline(version: u64) -> Self {
        Self {
            id: "system-baseline".into(),
            version,
            hosted_ai_data_classes: vec![DataClass::Public],
            local_model_data_classes: vec![
                DataClass::Public,
                DataClass::Internal,
                DataClass::Confidential,
            ],
            external_integration_data_classes: default_external_integration_data_classes(),
            audit_log_data_classes: default_audit_log_data_classes(),
            policy_auto_enabled: false,
            policy_auto_scope: None,
        }
    }
    pub fn with_hosted_ai_data_classes(mut self, classes: Vec<DataClass>) -> Self {
        self.hosted_ai_data_classes = classes;
        self
    }
    pub fn with_external_integration_data_classes(mut self, classes: Vec<DataClass>) -> Self {
        self.external_integration_data_classes = classes;
        self
    }
    pub fn with_audit_log_data_classes(mut self, classes: Vec<DataClass>) -> Self {
        self.audit_log_data_classes = classes;
        self
    }
    pub fn enable_policy_auto(mut self, scope: ResourceScope) -> Self {
        self.policy_auto_enabled = true;
        self.policy_auto_scope = Some(scope);
        self
    }
    pub fn from_json(value: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(value)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PolicyLoadError {
    #[error("policy version must be greater than zero")]
    InvalidVersion,
    #[error("policy auto requires a scope")]
    PolicyAutoScopeRequired,
}

#[derive(Clone, Debug)]
pub struct PolicyRuntime {
    document: PolicyDocument,
}
impl PolicyRuntime {
    pub fn baseline() -> Self {
        Self {
            document: PolicyDocument::baseline(1),
        }
    }
    pub fn load(document: PolicyDocument) -> Result<Self, PolicyLoadError> {
        if document.version == 0 {
            return Err(PolicyLoadError::InvalidVersion);
        }
        if document.policy_auto_enabled
            && match document.policy_auto_scope.as_ref() {
                Some(scope) => !scope.is_bounded(),
                None => true,
            }
        {
            return Err(PolicyLoadError::PolicyAutoScopeRequired);
        }
        Ok(Self { document })
    }
    pub fn load_json(value: &str) -> Result<Self, PolicyRuntimeLoadError> {
        let document =
            PolicyDocument::from_json(value).map_err(PolicyRuntimeLoadError::InvalidDocument)?;
        Self::load(document).map_err(PolicyRuntimeLoadError::InvalidPolicy)
    }
    pub fn version(&self) -> u64 {
        self.document.version
    }
    pub fn document(&self) -> &PolicyDocument {
        &self.document
    }

    pub fn evaluate_egress(&self, request: EgressRequest) -> PolicyDecision {
        let version = self.version();
        if !request.classification_verified || !request.redaction_verified {
            return PolicyDecision::Denied {
                reason: PolicyDenyReason::UnverifiedClassificationOrRedaction,
                policy_version: version,
            };
        }
        if (request.contains_immutable_secret || request.data_class == DataClass::Restricted)
            && matches!(
                request.destination,
                EgressDestination::HostedAi
                    | EgressDestination::ExternalIntegration
                    | EgressDestination::AuditLog
            )
        {
            return PolicyDecision::Denied {
                reason: PolicyDenyReason::ImmutableRestrictedData,
                policy_version: version,
            };
        }
        let permitted = match request.destination {
            EgressDestination::HostedAi => self
                .document
                .hosted_ai_data_classes
                .contains(&request.data_class),
            EgressDestination::LocalModel => self
                .document
                .local_model_data_classes
                .contains(&request.data_class),
            EgressDestination::LocalStorage | EgressDestination::Ui => true,
            EgressDestination::ExternalIntegration => self
                .document
                .external_integration_data_classes
                .contains(&request.data_class),
            EgressDestination::AuditLog => self
                .document
                .audit_log_data_classes
                .contains(&request.data_class),
        };
        if permitted {
            PolicyDecision::Allowed {
                policy_version: version,
            }
        } else {
            PolicyDecision::Denied {
                reason: PolicyDenyReason::DataClassNotPermitted,
                policy_version: version,
            }
        }
    }

    pub fn evaluate_action(&self, request: ActionPolicyRequest) -> PolicyDecision {
        let version = self.version();
        if request.execution_mode == ExecutionMode::PolicyAuto
            && request.risk_class != ActionRiskClass::Mutating
        {
            return PolicyDecision::Denied {
                reason: PolicyDenyReason::PolicyAutoRequiresMutatingAction,
                policy_version: version,
            };
        }
        if request.execution_mode == ExecutionMode::PolicyAuto && !self.document.policy_auto_enabled
        {
            return PolicyDecision::Denied {
                reason: PolicyDenyReason::PolicyAutoDisabled,
                policy_version: version,
            };
        }
        if request.execution_mode == ExecutionMode::PolicyAuto {
            if let Some(scope) = &self.document.policy_auto_scope {
                if !scope.contains(&request.scope) {
                    return PolicyDecision::Denied {
                        reason: PolicyDenyReason::ScopeNotPermitted,
                        policy_version: version,
                    };
                }
            }
        }
        if request.risk_class == ActionRiskClass::Blocked {
            return PolicyDecision::Denied {
                reason: PolicyDenyReason::ActionBlocked,
                policy_version: version,
            };
        }
        PolicyDecision::Allowed {
            policy_version: version,
        }
    }
}

#[derive(Debug, Error)]
pub enum PolicyRuntimeLoadError {
    #[error("invalid policy document: {0}")]
    InvalidDocument(serde_json::Error),
    #[error("invalid policy: {0}")]
    InvalidPolicy(PolicyLoadError),
}
