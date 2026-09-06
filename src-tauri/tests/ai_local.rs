use serde_json::Value;
use thalassa_ai::{ProviderManifest, ProviderRequest};
use thalassa_domain::{
    ModelDescriptor, ModelFinishReason, ModelMessage, ModelRole, ProviderErrorReason, ProviderKind,
};
use thalassaops::ai::providers::local::{LocalProvider, LocalProviderError};
use uuid::Uuid;

fn manifest(kind: ProviderKind, id: &str) -> ProviderManifest {
    ProviderManifest::new(id, kind).with_model(ModelDescriptor::new(
        if kind == ProviderKind::Ollama {
            "ollama-fixture"
        } else {
            "vllm-fixture"
        },
        16_384,
        4_096,
        true,
    ))
}

fn request(model_id: &str) -> ProviderRequest {
    ProviderRequest {
        request_id: Uuid::from_u128(0x8001),
        model_id: model_id.into(),
        instruction: Some("fixture system instruction".into()),
        messages: vec![ModelMessage {
            role: ModelRole::User,
            content: "fixture question".into(),
        }],
        max_output_tokens: 37,
    }
}

fn vllm_provider(endpoint: &str) -> LocalProvider {
    LocalProvider::new(
        manifest(ProviderKind::Vllm, "vllm-fixture"),
        serde_json::json!({"endpoint": endpoint}),
    )
    .unwrap()
}

fn ollama_provider(endpoint: &str) -> LocalProvider {
    LocalProvider::new(
        manifest(ProviderKind::Ollama, "ollama-fixture"),
        serde_json::json!({"endpoint": endpoint}),
    )
    .unwrap()
}

fn fixture(name: &str) -> Value {
    match name {
        "vllm_request" => {
            serde_json::from_str(include_str!("../src/ai/fixtures/local/vllm_request.json"))
                .unwrap()
        }
        "vllm_success" => {
            serde_json::from_str(include_str!("../src/ai/fixtures/local/vllm_success.json"))
                .unwrap()
        }
        "ollama_request" => {
            serde_json::from_str(include_str!("../src/ai/fixtures/local/ollama_request.json"))
                .unwrap()
        }
        "ollama_success" => {
            serde_json::from_str(include_str!("../src/ai/fixtures/local/ollama_success.json"))
                .unwrap()
        }
        "unauthorized" => {
            serde_json::from_str(include_str!("../src/ai/fixtures/local/unauthorized.json"))
                .unwrap()
        }
        "not_found" => {
            serde_json::from_str(include_str!("../src/ai/fixtures/local/not_found.json")).unwrap()
        }
        "rate_limited" => {
            serde_json::from_str(include_str!("../src/ai/fixtures/local/rate_limited.json"))
                .unwrap()
        }
        "server_error" => {
            serde_json::from_str(include_str!("../src/ai/fixtures/local/server_error.json"))
                .unwrap()
        }
        _ => panic!("unknown fixture"),
    }
}

#[test]
fn vllm_uses_the_openai_compatible_request_shape() {
    let body = vllm_provider("http://127.0.0.1:8000/v1/chat/completions")
        .request_body(&request("vllm-fixture"))
        .unwrap();

    assert_eq!(body, fixture("vllm_request")["request"]);
    assert_eq!(body["max_tokens"], 37);
}

#[test]
fn ollama_uses_native_chat_shape_and_num_predict_for_the_output_limit() {
    let body = ollama_provider("http://127.0.0.1:11434/api/chat")
        .request_body(&request("ollama-fixture"))
        .unwrap();

    assert_eq!(body, fixture("ollama_request")["request"]);
    assert_eq!(body["stream"], false);
    assert_eq!(body["options"]["num_predict"], 37);
}

#[test]
fn local_vllm_response_reports_usage_without_claiming_a_cost() {
    let fixture = fixture("vllm_success");
    let response = vllm_provider("http://127.0.0.1:8000/v1/chat/completions")
        .parse_response(fixture["status"].as_u64().unwrap() as u16, &fixture["body"])
        .unwrap();

    assert_eq!(response.content, "vllm fixture completion");
    assert_eq!(response.usage.input_tokens, 12);
    assert_eq!(response.usage.output_tokens, 7);
    assert_eq!(response.usage.cost_micros, None);
    assert_eq!(response.finish, ModelFinishReason::Complete);
}

#[test]
fn local_ollama_response_reports_usage_without_claiming_a_cost() {
    let fixture = fixture("ollama_success");
    let response = ollama_provider("http://127.0.0.1:11434/api/chat")
        .parse_response(fixture["status"].as_u64().unwrap() as u16, &fixture["body"])
        .unwrap();

    assert_eq!(response.content, "ollama fixture completion");
    assert_eq!(response.usage.input_tokens, 12);
    assert_eq!(response.usage.output_tokens, 7);
    assert_eq!(response.usage.cost_micros, None);
}

#[test]
fn local_statuses_map_to_typed_provider_reasons() {
    for (name, reason) in [
        ("unauthorized", ProviderErrorReason::Unauthorized),
        ("not_found", ProviderErrorReason::ModelUnavailable),
        ("rate_limited", ProviderErrorReason::RateLimited),
        ("server_error", ProviderErrorReason::Unreachable),
    ] {
        let fixture = fixture(name);
        let error = ollama_provider("http://127.0.0.1:11434/api/chat")
            .parse_response(fixture["status"].as_u64().unwrap() as u16, &fixture["body"])
            .unwrap_err();
        assert_eq!(error.reason, reason, "fixture {name}");
    }
}

#[test]
fn non_loopback_http_local_endpoint_is_rejected() {
    let error = LocalProvider::new(
        manifest(ProviderKind::Ollama, "ollama-fixture"),
        serde_json::json!({"endpoint": "http://model.example.test/api/chat"}),
    )
    .unwrap_err();

    assert!(matches!(error, LocalProviderError::InvalidEndpoint(_)));
}
