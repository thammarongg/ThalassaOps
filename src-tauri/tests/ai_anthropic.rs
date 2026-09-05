use serde_json::Value;
use thalassa_ai::{ProviderManifest, ProviderRequest};
use thalassa_domain::{
    ModelDescriptor, ModelFinishReason, ModelMessage, ModelRole, ProviderErrorReason, ProviderKind,
};
use thalassaops::ai::providers::anthropic::{AnthropicProvider, AnthropicProviderError};
use thalassaops::connectors::{CredentialStore, InMemoryCredentialStore};
use uuid::Uuid;

fn manifest() -> ProviderManifest {
    ProviderManifest::new("anthropic-fixture", ProviderKind::Anthropic).with_model(
        ModelDescriptor::new("claude-fixture", 16_384, 4_096, true).with_pricing(3_000, 15_000),
    )
}

fn provider(endpoint: &str) -> AnthropicProvider {
    let store = InMemoryCredentialStore::default();
    store
        .set("provider/anthropic-fixture", "test-key-not-real")
        .unwrap();
    AnthropicProvider::new(
        manifest(),
        serde_json::json!({"endpoint": endpoint}),
        &store,
    )
    .unwrap()
}

fn request() -> ProviderRequest {
    ProviderRequest {
        request_id: Uuid::from_u128(0x7001),
        model_id: "claude-fixture".into(),
        instruction: Some("fixture system instruction".into()),
        messages: vec![ModelMessage {
            role: ModelRole::User,
            content: "fixture question".into(),
        }],
        max_output_tokens: 37,
    }
}

fn fixture(name: &str) -> Value {
    match name {
        "request" => {
            serde_json::from_str(include_str!("../src/ai/fixtures/anthropic/request.json")).unwrap()
        }
        "end_turn" => {
            serde_json::from_str(include_str!("../src/ai/fixtures/anthropic/end_turn.json"))
                .unwrap()
        }
        "max_tokens" => {
            serde_json::from_str(include_str!("../src/ai/fixtures/anthropic/max_tokens.json"))
                .unwrap()
        }
        "unauthorized" => serde_json::from_str(include_str!(
            "../src/ai/fixtures/anthropic/unauthorized.json"
        ))
        .unwrap(),
        "not_found" => {
            serde_json::from_str(include_str!("../src/ai/fixtures/anthropic/not_found.json"))
                .unwrap()
        }
        "rate_limited" => serde_json::from_str(include_str!(
            "../src/ai/fixtures/anthropic/rate_limited.json"
        ))
        .unwrap(),
        "server_error" => serde_json::from_str(include_str!(
            "../src/ai/fixtures/anthropic/server_error.json"
        ))
        .unwrap(),
        "missing_usage" => serde_json::from_str(include_str!(
            "../src/ai/fixtures/anthropic/missing_usage.json"
        ))
        .unwrap(),
        _ => panic!("unknown fixture"),
    }
}

#[test]
fn anthropic_request_body_keeps_system_instruction_top_level_and_carries_output_limit() {
    let body = provider("https://api.example.test/v1/messages")
        .request_body(&request())
        .unwrap();

    assert_eq!(body, fixture("request")["request"]);
    assert_eq!(body["system"], "fixture system instruction");
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["max_tokens"], 37);
}

#[test]
fn recorded_anthropic_end_turn_maps_content_usage_and_completion() {
    let fixture = fixture("end_turn");
    let response = provider("https://api.example.test/v1/messages")
        .parse_response(fixture["status"].as_u64().unwrap() as u16, &fixture["body"])
        .unwrap();

    assert_eq!(response.content, "fixture completion");
    assert_eq!(response.usage.input_tokens, 12);
    assert_eq!(response.usage.output_tokens, 7);
    assert_eq!(response.finish, ModelFinishReason::Complete);
}

#[test]
fn anthropic_max_tokens_stop_reason_maps_to_max_output_tokens() {
    let fixture = fixture("max_tokens");
    let response = provider("https://api.example.test/v1/messages")
        .parse_response(fixture["status"].as_u64().unwrap() as u16, &fixture["body"])
        .unwrap();

    assert_eq!(response.finish, ModelFinishReason::MaxOutputTokens);
}

#[test]
fn recorded_anthropic_statuses_map_to_typed_provider_reasons() {
    for (name, reason) in [
        ("unauthorized", ProviderErrorReason::Unauthorized),
        ("not_found", ProviderErrorReason::ModelUnavailable),
        ("rate_limited", ProviderErrorReason::RateLimited),
        ("server_error", ProviderErrorReason::Unreachable),
    ] {
        let fixture = fixture(name);
        let error = provider("https://api.example.test/v1/messages")
            .parse_response(fixture["status"].as_u64().unwrap() as u16, &fixture["body"])
            .unwrap_err();
        assert_eq!(error.reason, reason, "fixture {name}");
    }
}

#[test]
fn missing_anthropic_usage_is_a_malformed_response_not_zero_usage() {
    let fixture = fixture("missing_usage");
    let error = provider("https://api.example.test/v1/messages")
        .parse_response(fixture["status"].as_u64().unwrap() as u16, &fixture["body"])
        .unwrap_err();

    assert_eq!(error.reason, ProviderErrorReason::MalformedResponse);
}

#[test]
fn anthropic_provider_errors_have_a_specific_public_error_type() {
    let store = InMemoryCredentialStore::default();
    store
        .set("provider/anthropic-fixture", "test-key-not-real")
        .unwrap();
    let error = AnthropicProvider::new(
        manifest(),
        serde_json::json!({"endpoint": "http://api.example.test/v1/messages"}),
        &store,
    )
    .unwrap_err();

    assert!(matches!(error, AnthropicProviderError::InvalidEndpoint(_)));
}
