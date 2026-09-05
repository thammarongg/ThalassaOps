use serde_json::Value;
use thalassa_ai::{ProviderManifest, ProviderRequest};
use thalassa_domain::{
    ModelDescriptor, ModelFinishReason, ModelMessage, ModelRole, ProviderErrorReason, ProviderKind,
};
use thalassaops::ai::providers::openai_compatible::OpenAiCompatibleProvider;
use thalassaops::connectors::{CredentialStore, InMemoryCredentialStore};
use uuid::Uuid;

fn manifest() -> ProviderManifest {
    ProviderManifest::new("openai-fixture", ProviderKind::OpenAiCompatible).with_model(
        ModelDescriptor::new("gpt-fixture", 16_384, 4_096, true).with_pricing(2_000, 4_000),
    )
}

fn provider(endpoint: &str) -> OpenAiCompatibleProvider {
    let store = InMemoryCredentialStore::default();
    store
        .set("provider/openai-fixture", "test-key-not-real")
        .unwrap();
    OpenAiCompatibleProvider::new(
        manifest(),
        serde_json::json!({"endpoint": endpoint}),
        &store,
    )
    .unwrap()
}

fn request() -> ProviderRequest {
    ProviderRequest {
        request_id: Uuid::from_u128(0x6001),
        model_id: "gpt-fixture".into(),
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
            serde_json::from_str(include_str!("../src/ai/fixtures/openai/request.json")).unwrap()
        }
        "success" => {
            serde_json::from_str(include_str!("../src/ai/fixtures/openai/success.json")).unwrap()
        }
        "unauthorized" => {
            serde_json::from_str(include_str!("../src/ai/fixtures/openai/unauthorized.json"))
                .unwrap()
        }
        "not_found" => {
            serde_json::from_str(include_str!("../src/ai/fixtures/openai/not_found.json")).unwrap()
        }
        "rate_limited" => {
            serde_json::from_str(include_str!("../src/ai/fixtures/openai/rate_limited.json"))
                .unwrap()
        }
        "server_error" => {
            serde_json::from_str(include_str!("../src/ai/fixtures/openai/server_error.json"))
                .unwrap()
        }
        "missing_usage" => {
            serde_json::from_str(include_str!("../src/ai/fixtures/openai/missing_usage.json"))
                .unwrap()
        }
        _ => panic!("unknown fixture"),
    }
}

#[test]
fn openai_request_body_matches_the_recorded_fixture_and_carries_output_limit() {
    let body = provider("https://api.example.test/v1/chat/completions")
        .request_body(&request())
        .unwrap();

    assert_eq!(body, fixture("request")["request"]);
    assert_eq!(body["max_tokens"], 37);
}

#[test]
fn recorded_openai_success_maps_content_usage_and_finish_reason() {
    let fixture = fixture("success");
    let response = provider("https://api.example.test/v1/chat/completions")
        .parse_response(fixture["status"].as_u64().unwrap() as u16, &fixture["body"])
        .unwrap();

    assert_eq!(response.content, "fixture completion");
    assert_eq!(response.usage.input_tokens, 12);
    assert_eq!(response.usage.output_tokens, 7);
    assert_eq!(response.finish, ModelFinishReason::Complete);
}

#[test]
fn recorded_openai_statuses_map_to_typed_provider_reasons() {
    for (name, reason) in [
        ("unauthorized", ProviderErrorReason::Unauthorized),
        ("not_found", ProviderErrorReason::ModelUnavailable),
        ("rate_limited", ProviderErrorReason::RateLimited),
        ("server_error", ProviderErrorReason::Unreachable),
    ] {
        let fixture = fixture(name);
        let error = provider("https://api.example.test/v1/chat/completions")
            .parse_response(fixture["status"].as_u64().unwrap() as u16, &fixture["body"])
            .unwrap_err();
        assert_eq!(error.reason, reason, "fixture {name}");
    }
}

#[test]
fn missing_openai_usage_is_a_malformed_response_not_zero_usage() {
    let fixture = fixture("missing_usage");
    let error = provider("https://api.example.test/v1/chat/completions")
        .parse_response(fixture["status"].as_u64().unwrap() as u16, &fixture["body"])
        .unwrap_err();

    assert_eq!(error.reason, ProviderErrorReason::MalformedResponse);
}

#[test]
fn non_loopback_http_endpoint_is_rejected_during_construction() {
    let store = InMemoryCredentialStore::default();
    store
        .set("provider/openai-fixture", "test-key-not-real")
        .unwrap();
    let error = OpenAiCompatibleProvider::new(
        manifest(),
        serde_json::json!({"endpoint": "http://api.example.test/v1/chat/completions"}),
        &store,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        thalassaops::ai::providers::openai_compatible::OpenAiCompatibleError::InvalidEndpoint(_)
    ));
}
