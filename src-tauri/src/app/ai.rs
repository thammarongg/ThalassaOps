use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use thalassa_ai::{
    BudgetBound, BudgetLedger, Gateway, GatewayError, ModelProvider, ProviderManifest,
    ProviderRegistry,
};
use thalassa_domain::{
    ModelRequest, ProviderErrorReason, ProviderHealth, ProviderKind, ResourceScope,
};
use thalassa_ipc::{
    ai_cancel_descriptor, ai_complete_descriptor, ai_configure_provider_descriptor,
    ai_probe_descriptor, ai_providers_descriptor, ai_set_provider_order_descriptor,
    CommandDescriptor, CommandEnvelope, IpcError, IpcErrorCode,
};
use uuid::Uuid;

use super::{membership_role_grants_permission, AppState, IpcResult};
use crate::ai::config::{ProviderConfigError, ProviderConfiguration, ProviderSummary};
use crate::ai::providers::{
    anthropic::AnthropicProvider, local::LocalProvider, openai_compatible::OpenAiCompatibleProvider,
};
use crate::connectors::CredentialStore;

pub const AI_COMPLETE_TAURI_COMMAND: &str = "ai_complete";
pub const AI_COMPLETE_ENVELOPE_COMMAND: &str = "ai.complete";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AiEmptyRequest {}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiConfigureProviderRequest {
    pub id: String,
    pub kind: ProviderKind,
    pub endpoint: String,
    pub models: Vec<thalassa_ai::ModelDescriptor>,
    #[serde(default)]
    pub credential: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiSetProviderOrderRequest {
    pub provider_order: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiProbeRequest {
    pub provider_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiCancelRequest {
    pub request_id: Uuid,
}

impl From<AiConfigureProviderRequest> for ProviderConfiguration {
    fn from(request: AiConfigureProviderRequest) -> Self {
        Self {
            id: request.id,
            kind: request.kind,
            endpoint: request.endpoint,
            models: request.models,
        }
    }
}

impl AppState {
    pub fn ai_providers(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<Vec<ProviderSummary>> {
        let descriptor = ai_providers_descriptor();
        if let Err(error) = self.authorize_ai(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }
        if parse_payload::<AiEmptyRequest>(envelope.payload).is_err() {
            return IpcResult::Err {
                ok: false,
                error: invalid_ai_request("ai_invalid_payload"),
            };
        }
        let providers = self
            .ai_config
            .lock()
            .expect("AI provider configuration mutex poisoned")
            .providers();
        self.finish_ai(providers)
    }

    pub fn ai_configure_provider(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<ProviderSummary> {
        let descriptor = ai_configure_provider_descriptor();
        if let Err(error) = self.authorize_ai(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }
        let request = match parse_payload::<AiConfigureProviderRequest>(envelope.payload) {
            Ok(request) => request,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let configuration = request.clone().into();
        let result = self
            .ai_config
            .lock()
            .expect("AI provider configuration mutex poisoned")
            .configure_provider(configuration, request.credential.as_deref());
        match result {
            Ok(summary) => self.finish_ai(summary),
            Err(error) => IpcResult::Err {
                ok: false,
                error: provider_config_error(error),
            },
        }
    }

    pub fn ai_set_provider_order(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<Vec<String>> {
        let descriptor = ai_set_provider_order_descriptor();
        if let Err(error) = self.authorize_ai(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }
        let request = match parse_payload::<AiSetProviderOrderRequest>(envelope.payload) {
            Ok(request) => request,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let result = self
            .ai_config
            .lock()
            .expect("AI provider configuration mutex poisoned")
            .set_provider_order(request.provider_order);
        match result {
            Ok(order) => self.finish_ai(order),
            Err(error) => IpcResult::Err {
                ok: false,
                error: provider_config_error(error),
            },
        }
    }

    pub fn ai_probe(&self, envelope: CommandEnvelope<Value>) -> IpcResult<ProviderSummary> {
        let descriptor = ai_probe_descriptor();
        if let Err(error) = self.authorize_ai(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }
        let request = match parse_payload::<AiProbeRequest>(envelope.payload) {
            Ok(request) => request,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let provider = match self.provider(&request.provider_id) {
            Ok(provider) => provider,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let health = match provider.probe(Instant::now() + Duration::from_secs(30)) {
            Ok(health) => health,
            Err(error) => {
                let health = health_for_provider_error(error.reason);
                let _ = self
                    .ai_config
                    .lock()
                    .expect("AI provider configuration mutex poisoned")
                    .set_health(&request.provider_id, health);
                return IpcResult::Err {
                    ok: false,
                    error: provider_error(error.reason),
                };
            }
        };
        let summary = self
            .ai_config
            .lock()
            .expect("AI provider configuration mutex poisoned")
            .set_health(&request.provider_id, health);
        match summary {
            Ok(summary) => self.finish_ai(summary),
            Err(error) => IpcResult::Err {
                ok: false,
                error: provider_config_error(error),
            },
        }
    }

    pub fn ai_complete(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<thalassa_domain::ModelResponse> {
        let descriptor = ai_complete_descriptor();
        if let Err(error) = self.authorize_ai(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }
        let request = match parse_payload::<ModelRequest>(envelope.payload) {
            Ok(request) => request,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let request_id = request.request_id;
        let cancellation = thalassa_ai::CancellationToken::new();
        {
            let mut cancellations = self
                .ai_cancellations
                .lock()
                .expect("AI cancellation mutex poisoned");
            if !cancellations
                .insert(request_id, cancellation.clone())
                .is_none()
            {
                return IpcResult::Err {
                    ok: false,
                    error: invalid_ai_request("ai_request_in_flight"),
                };
            }
        }

        let result = self.complete_model(request, cancellation);
        self.ai_cancellations
            .lock()
            .expect("AI cancellation mutex poisoned")
            .remove(&request_id);
        match result {
            Ok(response) => self.finish_ai(response),
            Err(error) => IpcResult::Err {
                ok: false,
                error: gateway_error(error),
            },
        }
    }

    pub fn ai_cancel(&self, envelope: CommandEnvelope<Value>) -> IpcResult<Value> {
        let descriptor = ai_cancel_descriptor();
        if let Err(error) = self.authorize_ai(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }
        let request = match parse_payload::<AiCancelRequest>(envelope.payload) {
            Ok(request) => request,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let cancellation = self
            .ai_cancellations
            .lock()
            .expect("AI cancellation mutex poisoned")
            .get(&request.request_id)
            .cloned();
        let Some(cancellation) = cancellation else {
            return IpcResult::Err {
                ok: false,
                error: IpcError::new(
                    IpcErrorCode::NotFound,
                    "AI request was not found",
                    json!({ "reason": "ai_request_not_found" }),
                ),
            };
        };
        cancellation.cancel();
        self.finish_ai(json!({ "request_id": request.request_id }))
    }

    fn authorize_ai(
        &self,
        envelope: &CommandEnvelope<Value>,
        descriptor: &CommandDescriptor,
    ) -> Result<(), IpcError> {
        let workspace_scope = ResourceScope::workspace(
            self.bootstrap.workspace.id,
            self.bootstrap.team.id,
            self.bootstrap.organization.id,
        );
        if envelope.command != descriptor.name
            || envelope.capability != descriptor.required_capability
            || envelope.scope.is_bounded()
            || !descriptor.scope.contains(&envelope.scope)
            || self.bootstrap.membership.status != thalassa_domain::MembershipStatus::Active
            || self.bootstrap.membership.principal_id != self.bootstrap.principal.id
            || !self.bootstrap.membership.grants(&workspace_scope)
            || !membership_role_grants_permission(
                &self.bootstrap.membership.role,
                &descriptor.required_permission,
            )
        {
            return Err(IpcError::new(
                IpcErrorCode::PermissionDenied,
                "permission denied",
                json!({ "required_command": descriptor.name.to_string() }),
            ));
        }
        Ok(())
    }

    fn finish_ai<T>(&self, value: T) -> IpcResult<T> {
        if self
            .policy
            .evaluate_egress(thalassa_policy::EgressRequest::verified(
                thalassa_policy::DataClass::Internal,
                thalassa_policy::EgressDestination::Ui,
            ))
            .is_allowed()
        {
            IpcResult::Ok { ok: true, value }
        } else {
            IpcResult::Err {
                ok: false,
                error: IpcError::new(
                    IpcErrorCode::PolicyDenied,
                    "policy denied AI response",
                    json!({ "reason": "ui_egress_denied" }),
                ),
            }
        }
    }

    fn complete_model(
        &self,
        request: ModelRequest,
        cancellation: thalassa_ai::CancellationToken,
    ) -> Result<thalassa_domain::ModelResponse, GatewayError> {
        let registry = self.build_registry().map_err(GatewayError::Registry)?;
        let gateway = Gateway::new(registry, BudgetLedger::new(), self.policy.clone());
        gateway.complete(
            request.clone(),
            Instant::now() + Duration::from_millis(request.timeout_ms),
            &cancellation,
        )
    }

    fn provider(&self, provider_id: &str) -> Result<Arc<dyn ModelProvider>, IpcError> {
        let configuration = self
            .ai_config
            .lock()
            .expect("AI provider configuration mutex poisoned")
            .configuration(provider_id)
            .ok_or_else(|| {
                IpcError::new(
                    IpcErrorCode::NotFound,
                    "AI provider was not found",
                    json!({ "reason": "ai_provider_not_found" }),
                )
            })?;
        let credentials = self
            .ai_config
            .lock()
            .expect("AI provider configuration mutex poisoned")
            .credential_store();
        provider_from_configuration(configuration, credentials.as_ref()).map_err(|error| {
            IpcError::new(
                IpcErrorCode::InvalidRequest,
                "AI provider configuration is invalid",
                json!({ "reason": error }),
            )
        })
    }

    fn build_registry(&self) -> Result<ProviderRegistry, thalassa_ai::RegistryError> {
        let (configurations, provider_order, credentials) = {
            let configuration = self
                .ai_config
                .lock()
                .expect("AI provider configuration mutex poisoned");
            (
                configuration.configurations(),
                configuration.provider_order().to_vec(),
                configuration.credential_store(),
            )
        };
        let mut registry = ProviderRegistry::new();
        for configuration in configurations {
            let provider = provider_from_configuration(configuration, credentials.as_ref())
                .map_err(|_| thalassa_ai::RegistryError::NoMatchingProvider)?;
            registry.register_arc(provider)?;
        }
        registry.set_provider_order(provider_order)?;
        Ok(registry)
    }
}

fn provider_from_configuration(
    configuration: ProviderConfiguration,
    credentials: &dyn CredentialStore,
) -> Result<Arc<dyn ModelProvider>, String> {
    let manifest = ProviderManifest {
        id: configuration.id,
        kind: configuration.kind,
        models: configuration.models,
    };
    let metadata = json!({ "endpoint": configuration.endpoint });
    match manifest.kind {
        ProviderKind::OpenAiCompatible => {
            OpenAiCompatibleProvider::new(manifest, metadata, credentials)
                .map(|provider| Arc::new(provider) as Arc<dyn ModelProvider>)
                .map_err(|error| error.to_string())
        }
        ProviderKind::Anthropic => AnthropicProvider::new(manifest, metadata, credentials)
            .map(|provider| Arc::new(provider) as Arc<dyn ModelProvider>)
            .map_err(|error| error.to_string()),
        ProviderKind::Ollama | ProviderKind::Vllm => LocalProvider::new(manifest, metadata)
            .map(|provider| Arc::new(provider) as Arc<dyn ModelProvider>)
            .map_err(|error| error.to_string()),
    }
}

fn parse_payload<T: DeserializeOwned>(payload: Value) -> Result<T, IpcError> {
    serde_json::from_value(payload).map_err(|_| invalid_ai_request("ai_invalid_payload"))
}

fn invalid_ai_request(reason: &str) -> IpcError {
    IpcError::new(
        IpcErrorCode::InvalidRequest,
        "AI request was rejected",
        json!({ "reason": reason }),
    )
}

fn provider_config_error(error: ProviderConfigError) -> IpcError {
    match error {
        ProviderConfigError::ProviderNotFound { .. } => IpcError::new(
            IpcErrorCode::NotFound,
            "AI provider was not found",
            json!({ "reason": "ai_provider_not_found" }),
        ),
        ProviderConfigError::UnknownProviderInOrder { .. } => {
            invalid_ai_request("ai_provider_order_unknown_provider")
        }
        ProviderConfigError::DuplicateProviderInOrder { .. } => {
            invalid_ai_request("ai_provider_order_duplicate_provider")
        }
        ProviderConfigError::InvalidConfiguration { .. } => {
            invalid_ai_request("ai_provider_configuration_invalid")
        }
        ProviderConfigError::CredentialStore { .. } => IpcError::new(
            IpcErrorCode::InternalError,
            "AI credential storage failed",
            json!({ "reason": "ai_credential_store_failed" }),
        ),
    }
}

fn gateway_error(error: GatewayError) -> IpcError {
    match error {
        GatewayError::InvalidRequest(_) => invalid_ai_request("model_request_invalid"),
        GatewayError::InvalidDataClass { .. } => invalid_ai_request("model_data_class_invalid"),
        GatewayError::Registry(_) => invalid_ai_request("ai_provider_selection_failed"),
        GatewayError::PolicyDenied {
            reason,
            policy_version,
        } => IpcError::new(
            IpcErrorCode::PolicyDenied,
            "AI provider egress was denied",
            json!({
                "reason": serde_json::to_value(reason).expect("policy reason is serializable"),
                "policy_version": policy_version,
            }),
        ),
        GatewayError::Budget(refusal) => IpcError::new(
            IpcErrorCode::InvalidRequest,
            "AI request budget was refused",
            json!({
                "reason": "budget_refused",
                "bound": budget_bound_name(refusal.bound),
                "requested": refusal.requested,
                "limit": refusal.limit,
            }),
        ),
        GatewayError::DeadlineExceeded => invalid_ai_request("ai_deadline_exceeded"),
        GatewayError::Cancelled => invalid_ai_request("ai_request_cancelled"),
        GatewayError::Provider {
            reason,
            provider_id: _,
            model_id: _,
            message: _,
        } => provider_error(reason),
    }
}

fn provider_error(reason: ProviderErrorReason) -> IpcError {
    IpcError::new(
        IpcErrorCode::ConnectorUnavailable,
        "AI provider request failed",
        json!({
            "reason": serde_json::to_value(reason).expect("provider reason is serializable"),
        }),
    )
}

fn health_for_provider_error(reason: ProviderErrorReason) -> ProviderHealth {
    match reason {
        ProviderErrorReason::Unauthorized => ProviderHealth::Unauthorized,
        ProviderErrorReason::ModelUnavailable => ProviderHealth::ModelUnavailable,
        ProviderErrorReason::RateLimited => ProviderHealth::RateLimited,
        ProviderErrorReason::BudgetExhausted => ProviderHealth::BudgetExhausted,
        ProviderErrorReason::Unreachable
        | ProviderErrorReason::MalformedResponse
        | ProviderErrorReason::InvalidRequest
        | ProviderErrorReason::DeadlineExceeded
        | ProviderErrorReason::Cancelled => ProviderHealth::Unreachable,
    }
}

fn budget_bound_name(bound: BudgetBound) -> &'static str {
    match bound {
        BudgetBound::MaxInputTokens => "max_input_tokens",
        BudgetBound::MaxOutputTokens => "max_output_tokens",
        BudgetBound::MaxCostMicros => "max_cost_micros",
        BudgetBound::WindowInputTokens => "window_input_tokens",
        BudgetBound::WindowOutputTokens => "window_output_tokens",
        BudgetBound::WindowCostMicros => "window_cost_micros",
        BudgetBound::UnpricedCost => "unpriced_cost",
    }
}
