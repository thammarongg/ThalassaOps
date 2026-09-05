use thalassa_domain::{
    validate_model_request, FailoverPermission, ModelBudget, ModelCapabilityRequirement,
    ModelMessage, ModelRequest, ModelRole, ModelSelector, MODEL_MESSAGE_MAXIMUM,
    MODEL_TIMEOUT_MAXIMUM_MS,
};
use uuid::Uuid;

fn request(messages: Vec<ModelMessage>) -> ModelRequest {
    ModelRequest {
        request_id: Uuid::from_u128(1),
        instruction: None,
        messages,
        data_class: "public".into(),
        budget: ModelBudget {
            max_input_tokens: Some(100),
            max_output_tokens: 32,
            max_cost_micros: None,
        },
        timeout_ms: 1_000,
        model: ModelSelector::Capability(ModelCapabilityRequirement::default()),
        failover: FailoverPermission::Forbidden,
    }
}

fn user_message(content: impl Into<String>) -> ModelMessage {
    ModelMessage {
        role: ModelRole::User,
        content: content.into(),
    }
}

#[test]
fn model_request_rejects_empty_messages() {
    assert!(validate_model_request(&request(vec![])).is_err());
}

#[test]
fn model_request_rejects_empty_or_whitespace_message_body() {
    assert!(validate_model_request(&request(vec![user_message("")])).is_err());
    assert!(validate_model_request(&request(vec![user_message("  \t\n")])).is_err());
}

#[test]
fn model_request_message_bound_counts_characters() {
    let content = "界".repeat(MODEL_MESSAGE_MAXIMUM + 1);
    assert!(validate_model_request(&request(vec![user_message(content)])).is_err());
}

#[test]
fn model_request_rejects_zero_or_excessive_timeout() {
    let mut zero = request(vec![user_message("hello")]);
    zero.timeout_ms = 0;
    assert!(validate_model_request(&zero).is_err());

    let mut excessive = request(vec![user_message("hello")]);
    excessive.timeout_ms = MODEL_TIMEOUT_MAXIMUM_MS + 1;
    assert!(validate_model_request(&excessive).is_err());
}

#[test]
fn model_request_rejects_zero_output_budget() {
    let mut request = request(vec![user_message("hello")]);
    request.budget.max_output_tokens = 0;
    assert!(validate_model_request(&request).is_err());
}

#[test]
fn model_request_accepts_a_cost_budget_without_provider_pricing() {
    let mut request = request(vec![user_message("hello")]);
    request.budget.max_cost_micros = Some(500);
    assert!(validate_model_request(&request).is_ok());
}
