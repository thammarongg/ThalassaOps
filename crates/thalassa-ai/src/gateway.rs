use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Instant;

use thalassa_domain::{
    validate_model_request, ContentDeclaration, FailoverPermission, ModelAttempt,
    ModelAttemptOutcome, ModelDataClass, ModelRequest, ModelResponse, ProviderErrorReason,
};
use thalassa_policy::{
    DataClass, EgressDestination, EgressRequest, PolicyDecision, PolicyDenyReason, PolicyRuntime,
};
use thiserror::Error;

use crate::{
    BudgetLedger, BudgetRefusal, ProviderError, ProviderRegistry, ProviderSelection, RegistryError,
};

#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum GatewayError {
    #[error("model request is invalid: {0}")]
    InvalidRequest(#[from] thalassa_domain::ModelRequestError),
    #[error("model request names an unknown data class: {data_class}")]
    InvalidDataClass { data_class: ModelDataClass },
    #[error("provider selection failed: {0}")]
    Registry(#[from] RegistryError),
    #[error("policy denied model egress: {reason:?}")]
    PolicyDenied {
        reason: PolicyDenyReason,
        policy_version: u64,
    },
    #[error("budget refused: {0}")]
    Budget(#[from] BudgetRefusal),
    #[error("model request deadline exceeded")]
    DeadlineExceeded,
    #[error("model request was cancelled")]
    Cancelled,
    #[error("provider {provider_id}/{model_id} failed ({reason:?}): {message}")]
    Provider {
        provider_id: String,
        model_id: String,
        reason: ProviderErrorReason,
        message: String,
    },
}

#[derive(Debug, Error, Eq, PartialEq)]
#[error("{error}")]
pub struct GatewayFailure {
    #[source]
    pub error: GatewayError,
    pub attempts: Vec<ModelAttempt>,
}

#[derive(Clone)]
pub struct Gateway {
    registry: Arc<ProviderRegistry>,
    budget: Arc<Mutex<BudgetLedger>>,
    policy: PolicyRuntime,
}

impl Gateway {
    pub fn new(registry: ProviderRegistry, budget: BudgetLedger, policy: PolicyRuntime) -> Self {
        Self {
            registry: Arc::new(registry),
            budget: Arc::new(Mutex::new(budget)),
            policy,
        }
    }

    pub fn complete(
        &self,
        request: ModelRequest,
        deadline: Instant,
        cancel: &CancellationToken,
    ) -> Result<ModelResponse, GatewayFailure> {
        validate_model_request(&request)
            .map_err(GatewayError::InvalidRequest)
            .map_err(|error| failure(error, Vec::new()))?;
        let data_class = parse_data_class(&request.data_class).ok_or_else(|| {
            failure(
                GatewayError::InvalidDataClass {
                    data_class: request.data_class.clone(),
                },
                Vec::new(),
            )
        })?;
        check_request_state(deadline, cancel).map_err(|error| failure(error, Vec::new()))?;

        let initial = self
            .registry
            .select(&request.model)
            .map_err(GatewayError::Registry)
            .map_err(|error| failure(error, Vec::new()))?;
        let fallbacks = self.fallbacks(&request, &initial);
        let total_attempts = fallbacks.len() + 1;
        let mut attempts = Vec::new();

        for (index, selection) in std::iter::once(initial).chain(fallbacks).enumerate() {
            check_request_state(deadline, cancel)
                .map_err(|error| failure(error, attempts.clone()))?;
            self.check_policy(&request, data_class, &selection)
                .map_err(|error| failure(error, attempts.clone()))?;

            let prepared = self
                .budget
                .lock()
                .expect("gateway budget mutex poisoned")
                .prepare(&request, &selection.model)
                .map_err(GatewayError::Budget)
                .map_err(|error| failure(error, attempts.clone()))?;

            let provider_result = selection
                .provider
                .complete(&prepared.provider_request, deadline);

            if Instant::now() >= deadline {
                return Err(failure(GatewayError::DeadlineExceeded, attempts));
            }

            match provider_result {
                Ok(provider_response) => {
                    let usage = provider_response.usage;
                    self.budget
                        .lock()
                        .expect("gateway budget mutex poisoned")
                        .record_usage(usage);
                    attempts.push(ModelAttempt {
                        provider_id: selection.provider_id.clone(),
                        model_id: selection.model.id.clone(),
                        outcome: ModelAttemptOutcome::Answered,
                        usage: Some(usage),
                    });
                    let finish = if cancel.is_cancelled() {
                        thalassa_domain::ModelFinishReason::Cancelled
                    } else {
                        provider_response.finish
                    };
                    return Ok(ModelResponse {
                        request_id: request.request_id,
                        provider_id: selection.provider_id,
                        model_id: selection.model.id,
                        content: provider_response.content,
                        usage,
                        finish,
                        attempts,
                    });
                }
                Err(error) => {
                    attempts.push(ModelAttempt {
                        provider_id: selection.provider_id.clone(),
                        model_id: selection.model.id.clone(),
                        outcome: ModelAttemptOutcome::Failed(error.reason),
                        usage: None,
                    });
                    if error.reason == ProviderErrorReason::DeadlineExceeded {
                        return Err(failure(GatewayError::DeadlineExceeded, attempts));
                    }
                    if error.reason == ProviderErrorReason::Cancelled {
                        return Err(failure(GatewayError::Cancelled, attempts));
                    }
                    let can_fail_over = request.failover == FailoverPermission::Permitted
                        && is_failover_reason(error.reason)
                        && index + 1 < total_attempts;
                    if can_fail_over {
                        continue;
                    }
                    return Err(failure(
                        provider_error(selection.provider_id, selection.model.id, error),
                        attempts,
                    ));
                }
            }
        }

        Err(failure(GatewayError::Cancelled, attempts))
    }

    fn check_policy(
        &self,
        request: &ModelRequest,
        data_class: DataClass,
        selection: &ProviderSelection,
    ) -> Result<(), GatewayError> {
        let destination = if selection.provider.manifest().kind.is_local() {
            EgressDestination::LocalModel
        } else {
            EgressDestination::HostedAi
        };
        let (classification_verified, redaction_verified) = match request.declaration {
            ContentDeclaration::OperatorDeclared => (true, true),
        };
        let decision = self.policy.evaluate_egress(EgressRequest {
            data_class,
            destination,
            classification_verified,
            redaction_verified,
            contains_immutable_secret: false,
        });
        match decision {
            PolicyDecision::Allowed { .. } => Ok(()),
            PolicyDecision::Denied {
                reason,
                policy_version,
            } => Err(GatewayError::PolicyDenied {
                reason,
                policy_version,
            }),
        }
    }

    fn fallbacks(
        &self,
        request: &ModelRequest,
        initial: &ProviderSelection,
    ) -> Vec<ProviderSelection> {
        let mut candidates = match &request.model {
            thalassa_domain::ModelSelector::Capability(requirement) => {
                self.registry.fallback_selections(requirement)
            }
            thalassa_domain::ModelSelector::Explicit { model_id, .. } => self
                .registry
                .provider_order()
                .iter()
                .filter_map(|provider_id| {
                    let provider = self.registry.provider(provider_id)?;
                    let model = provider.manifest().model(model_id)?.clone();
                    Some(ProviderSelection {
                        provider_id: provider_id.clone(),
                        model,
                        provider,
                    })
                })
                .collect(),
        };
        candidates.retain(|selection| selection.provider_id != initial.provider_id);
        candidates
    }
}

fn failure(error: GatewayError, attempts: Vec<ModelAttempt>) -> GatewayFailure {
    GatewayFailure { error, attempts }
}

fn check_request_state(deadline: Instant, cancel: &CancellationToken) -> Result<(), GatewayError> {
    if cancel.is_cancelled() {
        return Err(GatewayError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(GatewayError::DeadlineExceeded);
    }
    Ok(())
}

fn parse_data_class(value: &str) -> Option<DataClass> {
    match value.trim().to_ascii_lowercase().as_str() {
        "public" => Some(DataClass::Public),
        "internal" => Some(DataClass::Internal),
        "confidential" => Some(DataClass::Confidential),
        "restricted" => Some(DataClass::Restricted),
        _ => None,
    }
}

fn is_failover_reason(reason: ProviderErrorReason) -> bool {
    matches!(
        reason,
        ProviderErrorReason::Unreachable
            | ProviderErrorReason::RateLimited
            | ProviderErrorReason::ModelUnavailable
    )
}

fn provider_error(provider_id: String, model_id: String, error: ProviderError) -> GatewayError {
    GatewayError::Provider {
        provider_id,
        model_id,
        reason: error.reason,
        message: error.message,
    }
}
