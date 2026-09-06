use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use thalassa_ai::{
    BudgetLedger, CancellationToken, Gateway, ModelProvider, ProviderError, ProviderManifest,
    ProviderRegistry, ProviderRequest, ProviderResponse,
};
use thalassa_domain::{
    ContentDeclaration, FailoverPermission, ModelAttempt, ModelAttemptOutcome, ModelBudget,
    ModelCapabilityRequirement, ModelDescriptor, ModelFinishReason, ModelMessage, ModelRequest,
    ModelRole, ModelSelector, ModelUsage, ProviderErrorReason, ProviderHealth, ProviderKind,
};
use thalassa_policy::{DataClass, PolicyDenyReason, PolicyDocument, PolicyRuntime};
use uuid::Uuid;

#[derive(Clone)]
enum Action {
    Respond(ProviderResponse),
    Fail(ProviderError),
    SleepThenFail(Duration, ProviderError),
    CancelThenRespond(CancellationToken, ProviderResponse),
}

struct FakeProvider {
    manifest: ProviderManifest,
    actions: Mutex<VecDeque<Action>>,
    calls: Arc<Mutex<usize>>,
}

impl FakeProvider {
    fn new(id: &str, kind: ProviderKind, action: Action) -> (Self, Arc<Mutex<usize>>) {
        let calls = Arc::new(Mutex::new(0));
        let provider = Self {
            manifest: ProviderManifest::new(id, kind)
                .with_model(ModelDescriptor::new("model", 8_192, 1_024, true)),
            actions: Mutex::new(VecDeque::from([action])),
            calls: Arc::clone(&calls),
        };
        (provider, calls)
    }
}

impl ModelProvider for FakeProvider {
    fn manifest(&self) -> &ProviderManifest {
        &self.manifest
    }

    fn complete(
        &self,
        _request: &ProviderRequest,
        _deadline: Instant,
    ) -> Result<ProviderResponse, ProviderError> {
        *self.calls.lock().unwrap() += 1;
        match self.actions.lock().unwrap().pop_front().unwrap() {
            Action::Respond(response) => Ok(response),
            Action::Fail(error) => Err(error),
            Action::SleepThenFail(duration, error) => {
                std::thread::sleep(duration);
                Err(error)
            }
            Action::CancelThenRespond(cancel, response) => {
                cancel.cancel();
                Ok(response)
            }
        }
    }

    fn probe(&self, _deadline: Instant) -> Result<ProviderHealth, ProviderError> {
        Ok(ProviderHealth::Healthy)
    }
}

fn response(content: &str) -> ProviderResponse {
    ProviderResponse {
        content: content.into(),
        usage: ModelUsage {
            input_tokens: 12,
            output_tokens: 7,
            cost_micros: Some(3),
        },
        finish: ModelFinishReason::Complete,
    }
}

fn request(
    selector: ModelSelector,
    data_class: &str,
    failover: FailoverPermission,
) -> ModelRequest {
    ModelRequest {
        request_id: Uuid::from_u128(17),
        instruction: Some("answer briefly".into()),
        messages: vec![ModelMessage {
            role: ModelRole::User,
            content: "hello".into(),
        }],
        data_class: data_class.into(),
        declaration: ContentDeclaration::OperatorDeclared,
        budget: ModelBudget {
            max_input_tokens: Some(100),
            max_output_tokens: 32,
            max_cost_micros: None,
        },
        timeout_ms: 1_000,
        model: selector,
        failover,
    }
}

fn capability_request(data_class: &str, failover: FailoverPermission) -> ModelRequest {
    request(
        ModelSelector::Capability(ModelCapabilityRequirement::default()),
        data_class,
        failover,
    )
}

fn gateway(registry: ProviderRegistry, policy: PolicyRuntime) -> Gateway {
    Gateway::new(registry, BudgetLedger::new(), policy)
}

fn calls(calls: &Arc<Mutex<usize>>) -> usize {
    *calls.lock().unwrap()
}

fn rate_limited() -> ProviderError {
    ProviderError::new(ProviderErrorReason::RateLimited, "fixture rate limit")
}

#[test]
fn restricted_hosted_request_is_denied_before_the_provider_is_called() {
    let (provider, call_count) = FakeProvider::new(
        "hosted",
        ProviderKind::OpenAiCompatible,
        Action::Respond(response("should not be returned")),
    );
    let mut registry = ProviderRegistry::new();
    registry.register(provider).unwrap();

    let error = gateway(registry, PolicyRuntime::baseline())
        .complete(
            request(
                ModelSelector::Explicit {
                    provider_id: "hosted".into(),
                    model_id: "model".into(),
                },
                "restricted",
                FailoverPermission::Forbidden,
            ),
            Instant::now() + Duration::from_secs(1),
            &CancellationToken::new(),
        )
        .unwrap_err();

    assert!(matches!(
        error.error,
        thalassa_ai::GatewayError::PolicyDenied {
            reason: PolicyDenyReason::ImmutableRestrictedData,
            policy_version: 1,
        }
    ));
    assert_eq!(calls(&call_count), 0);
}

#[test]
fn restricted_local_request_uses_the_local_policy_data_classes() {
    let (provider, call_count) = FakeProvider::new(
        "local",
        ProviderKind::Ollama,
        Action::Respond(response("local answer")),
    );
    let mut registry = ProviderRegistry::new();
    registry.register(provider).unwrap();
    let mut document = PolicyDocument::baseline(2);
    document
        .local_model_data_classes
        .push(DataClass::Restricted);
    let policy = PolicyRuntime::load(document).unwrap();

    let answer = gateway(registry, policy)
        .complete(
            request(
                ModelSelector::Explicit {
                    provider_id: "local".into(),
                    model_id: "model".into(),
                },
                "restricted",
                FailoverPermission::Forbidden,
            ),
            Instant::now() + Duration::from_secs(1),
            &CancellationToken::new(),
        )
        .unwrap();

    assert_eq!(answer.content, "local answer");
    assert_eq!(calls(&call_count), 1);
}

#[test]
fn deadline_covers_failover_and_never_starts_a_late_second_attempt() {
    let (first, first_calls) = FakeProvider::new(
        "first",
        ProviderKind::OpenAiCompatible,
        Action::SleepThenFail(Duration::from_millis(20), rate_limited()),
    );
    let (second, second_calls) = FakeProvider::new(
        "second",
        ProviderKind::OpenAiCompatible,
        Action::Respond(response("late answer")),
    );
    let mut registry = ProviderRegistry::new();
    registry.register(first).unwrap();
    registry.register(second).unwrap();
    registry.set_provider_order(["second"]).unwrap();

    let error = gateway(registry, PolicyRuntime::baseline())
        .complete(
            capability_request("public", FailoverPermission::Permitted),
            Instant::now() + Duration::from_millis(5),
            &CancellationToken::new(),
        )
        .unwrap_err();

    assert!(matches!(
        error.error,
        thalassa_ai::GatewayError::DeadlineExceeded
    ));
    assert_eq!(calls(&first_calls), 1);
    assert_eq!(calls(&second_calls), 0);
}

#[test]
fn cancellation_returns_the_reported_usage() {
    let cancel = CancellationToken::new();
    let (provider, call_count) = FakeProvider::new(
        "hosted",
        ProviderKind::OpenAiCompatible,
        Action::CancelThenRespond(cancel.clone(), response("partial answer")),
    );
    let mut registry = ProviderRegistry::new();
    registry.register(provider).unwrap();

    let answer = gateway(registry, PolicyRuntime::baseline())
        .complete(
            capability_request("public", FailoverPermission::Forbidden),
            Instant::now() + Duration::from_secs(1),
            &cancel,
        )
        .unwrap();

    assert_eq!(answer.finish, ModelFinishReason::Cancelled);
    assert_eq!(answer.usage.input_tokens, 12);
    assert_eq!(answer.usage.output_tokens, 7);
    assert_eq!(answer.usage.cost_micros, Some(3));
    assert_eq!(calls(&call_count), 1);
}

#[test]
fn forbidden_failover_returns_rate_limit_without_touching_the_next_provider() {
    let (first, first_calls) = FakeProvider::new(
        "first",
        ProviderKind::OpenAiCompatible,
        Action::Fail(rate_limited()),
    );
    let (second, second_calls) = FakeProvider::new(
        "second",
        ProviderKind::OpenAiCompatible,
        Action::Respond(response("backup")),
    );
    let mut registry = ProviderRegistry::new();
    registry.register(first).unwrap();
    registry.register(second).unwrap();
    registry.set_provider_order(["second"]).unwrap();

    let failure = gateway(registry, PolicyRuntime::baseline())
        .complete(
            capability_request("public", FailoverPermission::Forbidden),
            Instant::now() + Duration::from_secs(1),
            &CancellationToken::new(),
        )
        .unwrap_err();

    assert!(matches!(
        failure.error,
        thalassa_ai::GatewayError::Provider {
            reason: ProviderErrorReason::RateLimited,
            ..
        }
    ));
    assert_eq!(failure.attempts.len(), 1);
    assert_eq!(failure.attempts[0].usage, None);
    assert_eq!(calls(&first_calls), 1);
    assert_eq!(calls(&second_calls), 0);
}

#[test]
fn permitted_failover_returns_the_second_answer_and_both_attempts() {
    let (first, first_calls) = FakeProvider::new(
        "first",
        ProviderKind::OpenAiCompatible,
        Action::Fail(rate_limited()),
    );
    let (second, second_calls) = FakeProvider::new(
        "second",
        ProviderKind::OpenAiCompatible,
        Action::Respond(response("backup answer")),
    );
    let mut registry = ProviderRegistry::new();
    registry.register(first).unwrap();
    registry.register(second).unwrap();
    registry.set_provider_order(["second"]).unwrap();

    let answer = gateway(registry, PolicyRuntime::baseline())
        .complete(
            capability_request("public", FailoverPermission::Permitted),
            Instant::now() + Duration::from_secs(1),
            &CancellationToken::new(),
        )
        .unwrap();

    assert_eq!(answer.provider_id, "second");
    assert_eq!(answer.content, "backup answer");
    assert_eq!(
        answer.attempts,
        vec![
            ModelAttempt {
                provider_id: "first".into(),
                model_id: "model".into(),
                outcome: ModelAttemptOutcome::Failed(ProviderErrorReason::RateLimited),
                usage: None,
            },
            ModelAttempt {
                provider_id: "second".into(),
                model_id: "model".into(),
                outcome: ModelAttemptOutcome::Answered,
                usage: Some(response("backup answer").usage),
            },
        ]
    );
    assert_eq!(calls(&first_calls), 1);
    assert_eq!(calls(&second_calls), 1);
}

#[test]
fn unauthorized_failure_does_not_fail_over() {
    let (first, first_calls) = FakeProvider::new(
        "first",
        ProviderKind::OpenAiCompatible,
        Action::Fail(ProviderError::new(
            ProviderErrorReason::Unauthorized,
            "fixture unauthorized",
        )),
    );
    let (second, second_calls) = FakeProvider::new(
        "second",
        ProviderKind::OpenAiCompatible,
        Action::Respond(response("backup")),
    );
    let mut registry = ProviderRegistry::new();
    registry.register(first).unwrap();
    registry.register(second).unwrap();
    registry.set_provider_order(["second"]).unwrap();

    let error = gateway(registry, PolicyRuntime::baseline())
        .complete(
            capability_request("public", FailoverPermission::Permitted),
            Instant::now() + Duration::from_secs(1),
            &CancellationToken::new(),
        )
        .unwrap_err();

    assert!(matches!(
        error.error,
        thalassa_ai::GatewayError::Provider {
            reason: ProviderErrorReason::Unauthorized,
            ..
        }
    ));
    assert_eq!(calls(&first_calls), 1);
    assert_eq!(calls(&second_calls), 0);
}

#[test]
fn provider_absent_from_configured_order_is_not_a_fallback() {
    let (first, first_calls) = FakeProvider::new(
        "first",
        ProviderKind::OpenAiCompatible,
        Action::Fail(rate_limited()),
    );
    let (unlisted, unlisted_calls) = FakeProvider::new(
        "unlisted",
        ProviderKind::OpenAiCompatible,
        Action::Respond(response("unlisted answer")),
    );
    let mut registry = ProviderRegistry::new();
    registry.register(first).unwrap();
    registry.register(unlisted).unwrap();

    let error = gateway(registry, PolicyRuntime::baseline())
        .complete(
            capability_request("public", FailoverPermission::Permitted),
            Instant::now() + Duration::from_secs(1),
            &CancellationToken::new(),
        )
        .unwrap_err();

    assert!(matches!(
        error.error,
        thalassa_ai::GatewayError::Provider {
            reason: ProviderErrorReason::RateLimited,
            ..
        }
    ));
    assert_eq!(calls(&first_calls), 1);
    assert_eq!(calls(&unlisted_calls), 0);
}

#[test]
fn unknown_data_class_is_refused_before_the_provider_is_called() {
    let (provider, call_count) = FakeProvider::new(
        "hosted",
        ProviderKind::OpenAiCompatible,
        Action::Respond(response("should not be returned")),
    );
    let mut registry = ProviderRegistry::new();
    registry.register(provider).unwrap();

    let error = gateway(registry, PolicyRuntime::baseline())
        .complete(
            capability_request("not-a-data-class", FailoverPermission::Forbidden),
            Instant::now() + Duration::from_secs(1),
            &CancellationToken::new(),
        )
        .unwrap_err();

    assert!(matches!(
        error.error,
        thalassa_ai::GatewayError::InvalidDataClass { data_class } if data_class == "not-a-data-class"
    ));
    assert_eq!(calls(&call_count), 0);
}

#[test]
fn successful_request_has_exactly_one_answered_attempt() {
    let (provider, call_count) = FakeProvider::new(
        "hosted",
        ProviderKind::OpenAiCompatible,
        Action::Respond(response("answer")),
    );
    let mut registry = ProviderRegistry::new();
    registry.register(provider).unwrap();

    let answer = gateway(registry, PolicyRuntime::baseline())
        .complete(
            request(
                ModelSelector::Explicit {
                    provider_id: "hosted".into(),
                    model_id: "model".into(),
                },
                "public",
                FailoverPermission::Forbidden,
            ),
            Instant::now() + Duration::from_secs(1),
            &CancellationToken::new(),
        )
        .unwrap();

    assert_eq!(answer.attempts.len(), 1);
    assert_eq!(answer.attempts[0].outcome, ModelAttemptOutcome::Answered);
    assert_eq!(calls(&call_count), 1);
}
