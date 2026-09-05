// SPDX-License-Identifier: Apache-2.0
//! Sprint 17 exit criterion: one model request contract is answered by a
//! hosted provider and by a local one with nothing changed but the selector.
//!
//! The adapters here are fixtures, as design section 16 requires — no test
//! performs a network call — but the gateway, registry, budget ledger and
//! policy runtime are the real ones.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use thalassa_ai::{
    BudgetBound, BudgetLedger, BudgetRefusal, CancellationToken, Gateway, GatewayError,
    ModelProvider, ProviderError, ProviderManifest, ProviderRegistry, ProviderRequest,
    ProviderResponse,
};
use thalassa_domain::{
    ContentDeclaration, FailoverPermission, ModelBudget, ModelDescriptor, ModelFinishReason,
    ModelMessage, ModelRequest, ModelResponse, ModelRole, ModelSelector, ModelUsage,
    ProviderHealth, ProviderKind,
};
use thalassa_policy::{DataClass, PolicyDenyReason, PolicyDocument, PolicyRuntime};
use uuid::Uuid;

const REQUEST_ID: Uuid = Uuid::from_u128(0x1707);
const ANSWER: &str = "restart the checkout deployment and watch the error rate";

/// A recorded-response adapter that also keeps what the gateway asked it, so a
/// test can prove both destinations received the same provider request.
struct FixtureProvider {
    manifest: ProviderManifest,
    response: ProviderResponse,
    received: Mutex<Vec<ProviderRequest>>,
}

impl FixtureProvider {
    fn new(
        id: &str,
        kind: ProviderKind,
        model_id: &str,
        usage: ModelUsage,
        pricing: Option<(u64, u64)>,
    ) -> Self {
        let model = ModelDescriptor::new(model_id, 8_192, 1_024, true);
        let model = match pricing {
            Some((input, output)) => model.with_pricing(input, output),
            None => model,
        };
        Self {
            manifest: ProviderManifest::new(id, kind).with_model(model),
            response: ProviderResponse {
                content: ANSWER.into(),
                usage,
                finish: ModelFinishReason::Complete,
            },
            received: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<ProviderRequest> {
        self.received.lock().unwrap().clone()
    }
}

impl ModelProvider for FixtureProvider {
    fn manifest(&self) -> &ProviderManifest {
        &self.manifest
    }

    fn complete(
        &self,
        request: &ProviderRequest,
        _deadline: Instant,
    ) -> Result<ProviderResponse, ProviderError> {
        self.received.lock().unwrap().push(request.clone());
        Ok(self.response.clone())
    }

    fn probe(&self, _deadline: Instant) -> Result<ProviderHealth, ProviderError> {
        Ok(ProviderHealth::Healthy)
    }
}

fn hosted_usage() -> ModelUsage {
    ModelUsage {
        input_tokens: 24,
        output_tokens: 11,
        cost_micros: Some(340),
    }
}

fn local_usage() -> ModelUsage {
    ModelUsage {
        input_tokens: 24,
        output_tokens: 11,
        cost_micros: None,
    }
}

/// The payload a caller sends to `ai.complete`, before a destination is chosen.
fn request_payload(data_class: &str, max_cost_micros: Option<u64>) -> Value {
    serde_json::to_value(ModelRequest {
        request_id: REQUEST_ID,
        instruction: Some("answer the on-call engineer".into()),
        messages: vec![ModelMessage {
            role: ModelRole::User,
            content: "what should I do about the checkout error rate?".into(),
        }],
        data_class: data_class.into(),
        declaration: ContentDeclaration::OperatorDeclared,
        budget: ModelBudget {
            max_input_tokens: Some(1_000),
            max_output_tokens: 64,
            max_cost_micros,
        },
        timeout_ms: 1_000,
        model: ModelSelector::Explicit {
            provider_id: "unset".into(),
            model_id: "unset".into(),
        },
        failover: FailoverPermission::Forbidden,
    })
    .unwrap()
}

fn hosted_provider() -> FixtureProvider {
    FixtureProvider::new(
        "hosted",
        ProviderKind::OpenAiCompatible,
        "hosted-model",
        hosted_usage(),
        Some((2_000, 4_000)),
    )
}

/// A local model carries no pricing, so it cannot honour a cost bound.
fn local_provider() -> FixtureProvider {
    FixtureProvider::new(
        "local",
        ProviderKind::Ollama,
        "local-model",
        local_usage(),
        None,
    )
}

/// Returns the same payload with only `model` replaced, and parses it the way
/// the `ai.complete` handler does.
fn addressed_to(payload: &Value, provider_id: &str, model_id: &str) -> (Value, ModelRequest) {
    let mut addressed = payload.clone();
    addressed.as_object_mut().unwrap().insert(
        "model".into(),
        serde_json::to_value(ModelSelector::Explicit {
            provider_id: provider_id.into(),
            model_id: model_id.into(),
        })
        .unwrap(),
    );
    let request = serde_json::from_value::<ModelRequest>(addressed.clone()).unwrap();
    (addressed, request)
}

fn without(value: &Value, fields: &[&str]) -> Value {
    let mut stripped = value.clone();
    let object = stripped.as_object_mut().unwrap();
    for field in fields {
        assert!(
            object.remove(*field).is_some(),
            "response has no field named {field}"
        );
    }
    stripped
}

fn gateway_over(
    hosted: &Arc<FixtureProvider>,
    local: &Arc<FixtureProvider>,
    policy: PolicyRuntime,
) -> Gateway {
    let mut registry = ProviderRegistry::new();
    registry
        .register_arc(hosted.clone() as Arc<dyn ModelProvider>)
        .unwrap();
    registry
        .register_arc(local.clone() as Arc<dyn ModelProvider>)
        .unwrap();
    Gateway::new(registry, BudgetLedger::new(), policy)
}

fn complete(gateway: &Gateway, request: ModelRequest) -> Result<ModelResponse, GatewayError> {
    gateway.complete(
        request,
        Instant::now() + Duration::from_secs(1),
        &CancellationToken::new(),
    )
}

fn restricted_locally() -> PolicyRuntime {
    let mut document = PolicyDocument::baseline(2);
    document
        .local_model_data_classes
        .push(DataClass::Restricted);
    PolicyRuntime::load(document).unwrap()
}

#[test]
fn one_request_contract_is_answered_by_a_hosted_provider_and_by_a_local_one() {
    let payload = request_payload("public", None);
    let (hosted_payload, hosted_request) = addressed_to(&payload, "hosted", "hosted-model");
    let (local_payload, local_request) = addressed_to(&payload, "local", "local-model");

    // Nothing but the destination differs between the two calls.
    assert_eq!(
        without(&hosted_payload, &["model"]),
        without(&local_payload, &["model"])
    );

    let hosted = Arc::new(hosted_provider());
    let local = Arc::new(local_provider());

    let gateway = gateway_over(&hosted, &local, PolicyRuntime::baseline());

    let hosted_answer = complete(&gateway, hosted_request).unwrap();
    let local_answer = complete(&gateway, local_request).unwrap();

    // The exit criterion, as an assertion: the two responses are identical
    // apart from provider_id, model_id, usage and attempts.
    let excluded = ["provider_id", "model_id", "usage", "attempts"];
    assert_eq!(
        without(&serde_json::to_value(&hosted_answer).unwrap(), &excluded),
        without(&serde_json::to_value(&local_answer).unwrap(), &excluded)
    );
    assert_eq!(hosted_answer.request_id, REQUEST_ID);
    assert_eq!(hosted_answer.content, ANSWER);
    assert_eq!(hosted_answer.finish, ModelFinishReason::Complete);

    // And the four excluded fields really are the provider-specific ones.
    assert_eq!(hosted_answer.provider_id, "hosted");
    assert_eq!(local_answer.provider_id, "local");
    assert_eq!(hosted_answer.model_id, "hosted-model");
    assert_eq!(local_answer.model_id, "local-model");
    assert_eq!(hosted_answer.usage, hosted_usage());
    assert_eq!(local_answer.usage, local_usage());
    assert_eq!(hosted_answer.attempts.len(), 1);
    assert_eq!(local_answer.attempts.len(), 1);
    assert_eq!(hosted_answer.attempts[0].provider_id, "hosted");
    assert_eq!(local_answer.attempts[0].provider_id, "local");

    // Each adapter was asked the same thing, down to the message body.
    let hosted_calls = hosted.calls();
    let local_calls = local.calls();
    assert_eq!(hosted_calls.len(), 1);
    assert_eq!(local_calls.len(), 1);
    assert_eq!(hosted_calls[0].request_id, local_calls[0].request_id);
    assert_eq!(hosted_calls[0].instruction, local_calls[0].instruction);
    assert_eq!(hosted_calls[0].messages, local_calls[0].messages);
    assert_eq!(
        hosted_calls[0].max_output_tokens,
        local_calls[0].max_output_tokens
    );
    assert_eq!(hosted_calls[0].model_id, "hosted-model");
    assert_eq!(local_calls[0].model_id, "local-model");
}

#[test]
fn restricted_content_is_refused_by_the_hosted_provider_and_answered_by_the_local_one() {
    let payload = request_payload("restricted", None);
    let (_, hosted_request) = addressed_to(&payload, "hosted", "hosted-model");
    let (_, local_request) = addressed_to(&payload, "local", "local-model");

    let hosted = Arc::new(hosted_provider());
    let local = Arc::new(local_provider());

    let gateway = gateway_over(&hosted, &local, restricted_locally());

    let denial = complete(&gateway, hosted_request).unwrap_err();
    assert_eq!(
        denial,
        GatewayError::PolicyDenied {
            reason: PolicyDenyReason::ImmutableRestrictedData,
            policy_version: 2,
        }
    );
    assert!(
        hosted.calls().is_empty(),
        "restricted content must not reach a hosted provider"
    );

    let answer = complete(&gateway, local_request).unwrap();
    assert_eq!(answer.provider_id, "local");
    assert_eq!(answer.content, ANSWER);
    assert_eq!(local.calls().len(), 1);
}

#[test]
fn a_cost_bound_is_honoured_by_the_priced_hosted_model_and_refused_by_the_unpriced_local_one() {
    // Design section 8: a model with no pricing cannot honour a cost bound, and
    // a request that sets one against such a provider is refused rather than
    // allowed unpriced. So a shared contract that carries `max_cost_micros` is
    // the one thing a caller cannot point at both destinations unchanged.
    let payload = request_payload("public", Some(5_000));
    let (_, hosted_request) = addressed_to(&payload, "hosted", "hosted-model");
    let (_, local_request) = addressed_to(&payload, "local", "local-model");

    let hosted = Arc::new(hosted_provider());
    let local = Arc::new(local_provider());
    let gateway = gateway_over(&hosted, &local, PolicyRuntime::baseline());

    let answer = complete(&gateway, hosted_request).unwrap();
    assert_eq!(answer.provider_id, "hosted");

    let refusal = complete(&gateway, local_request).unwrap_err();
    assert_eq!(
        refusal,
        GatewayError::Budget(BudgetRefusal {
            bound: BudgetBound::UnpricedCost,
            requested: 0,
            limit: Some(5_000),
        })
    );
    assert_eq!(
        local.calls().len(),
        0,
        "an unpriced model is refused before it is called"
    );
}
