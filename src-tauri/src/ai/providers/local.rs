use std::net::IpAddr;
use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use reqwest::Url;
use serde::Deserialize;
use serde_json::{json, Value};
use thalassa_ai::{
    ModelProvider, ProviderError, ProviderManifest, ProviderRequest, ProviderResponse,
};
use thalassa_domain::{
    ModelFinishReason, ModelRole, ModelUsage, ProviderErrorReason, ProviderHealth, ProviderKind,
};
use thiserror::Error;
use uuid::Uuid;

use super::openai_compatible::{openai_request_body, parse_openai_response};

const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

/// The local adapter covers both local runtimes deliberately: vLLM reuses the
/// OpenAI-compatible wire helpers, while Ollama uses its native `/api/chat`
/// shape and mapping below. They are not treated as interchangeable formats.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalProviderConfig {
    pub endpoint: String,
}

#[derive(Debug, Error)]
pub enum LocalProviderError {
    #[error("invalid local provider configuration: {0}")]
    Configuration(String),
    #[error("invalid local provider endpoint: {0}")]
    InvalidEndpoint(String),
    #[error("could not build local provider HTTP client")]
    HttpClient,
}

#[derive(Debug)]
pub struct LocalProvider {
    client: Client,
    endpoint: Url,
    manifest: ProviderManifest,
}

impl LocalProvider {
    pub fn new(
        manifest: ProviderManifest,
        config_metadata: Value,
    ) -> Result<Self, LocalProviderError> {
        if !manifest.kind.is_local() {
            return Err(LocalProviderError::Configuration(
                "local provider kind must be ollama or vllm".into(),
            ));
        }
        let config: LocalProviderConfig = serde_json::from_value(config_metadata)
            .map_err(|error| LocalProviderError::Configuration(error.to_string()))?;
        let endpoint = validate_endpoint(&config.endpoint)?;
        let client = Client::builder()
            .timeout(HTTP_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| LocalProviderError::HttpClient)?;
        Ok(Self {
            client,
            endpoint,
            manifest,
        })
    }

    pub fn request_body(&self, request: &ProviderRequest) -> Result<Value, LocalProviderError> {
        match self.manifest.kind {
            ProviderKind::Vllm => openai_request_body(request)
                .map_err(|error| LocalProviderError::Configuration(error.to_string())),
            ProviderKind::Ollama => ollama_request_body(request),
            _ => Err(LocalProviderError::Configuration(
                "local provider kind must be ollama or vllm".into(),
            )),
        }
    }

    pub fn parse_response(
        &self,
        status: u16,
        body: &Value,
    ) -> Result<ProviderResponse, ProviderError> {
        match self.manifest.kind {
            ProviderKind::Vllm => {
                let mut response = parse_openai_response(&self.manifest, status, body)?;
                response.usage.cost_micros = None;
                Ok(response)
            }
            ProviderKind::Ollama => parse_ollama_response(status, body),
            _ => Err(ProviderError::new(
                ProviderErrorReason::InvalidRequest,
                "local provider kind must be ollama or vllm",
            )),
        }
    }
}

impl ModelProvider for LocalProvider {
    fn manifest(&self) -> &ProviderManifest {
        &self.manifest
    }

    fn complete(
        &self,
        request: &ProviderRequest,
        deadline: Instant,
    ) -> Result<ProviderResponse, ProviderError> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ProviderError::new(
                ProviderErrorReason::DeadlineExceeded,
                "provider deadline elapsed before request",
            ));
        }
        let body = self.request_body(request).map_err(|error| {
            ProviderError::new(ProviderErrorReason::InvalidRequest, error.to_string())
        })?;
        let response = self
            .client
            .post(self.endpoint.clone())
            .timeout(remaining)
            .json(&body)
            .send()
            .map_err(|_| {
                ProviderError::new(ProviderErrorReason::Unreachable, "provider request failed")
            })?;
        let status = response.status().as_u16();
        let body = response
            .json::<Value>()
            .map_err(|_| malformed_response("local provider returned invalid JSON"))?;
        self.parse_response(status, &body)
    }

    fn probe(&self, deadline: Instant) -> Result<ProviderHealth, ProviderError> {
        let model_id = self
            .manifest
            .models
            .first()
            .map(|model| model.id.clone())
            .ok_or_else(|| {
                ProviderError::new(
                    ProviderErrorReason::ModelUnavailable,
                    "provider has no configured model",
                )
            })?;
        let request = ProviderRequest {
            request_id: Uuid::nil(),
            model_id,
            instruction: None,
            messages: Vec::new(),
            max_output_tokens: 1,
        };
        self.complete(&request, deadline)
            .map(|_| ProviderHealth::Healthy)
    }
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    message: OllamaMessage,
    done: bool,
    done_reason: Option<String>,
    prompt_eval_count: Option<u64>,
    eval_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    content: String,
}

fn ollama_request_body(request: &ProviderRequest) -> Result<Value, LocalProviderError> {
    let mut messages = Vec::with_capacity(request.messages.len() + 1);
    if let Some(instruction) = &request.instruction {
        messages.push(json!({
            "role": "system",
            "content": instruction,
        }));
    }
    messages.extend(request.messages.iter().map(|message| {
        json!({
            "role": role_wire(message.role),
            "content": message.content,
        })
    }));
    Ok(json!({
        "model": request.model_id,
        "messages": messages,
        "stream": false,
        "options": {"num_predict": request.max_output_tokens},
    }))
}

fn parse_ollama_response(status: u16, body: &Value) -> Result<ProviderResponse, ProviderError> {
    if !(200..=299).contains(&status) {
        return Err(ProviderError::new(
            reason_for_status(status),
            format!("Ollama provider returned status {status}"),
        ));
    }
    let parsed: OllamaResponse = serde_json::from_value(body.clone())
        .map_err(|_| malformed_response("Ollama response does not match its schema"))?;
    if !parsed.done {
        return Err(malformed_response("Ollama response is not complete"));
    }
    let input_tokens = parsed
        .prompt_eval_count
        .ok_or_else(|| malformed_response("Ollama response has no input usage"))?;
    let output_tokens = parsed
        .eval_count
        .ok_or_else(|| malformed_response("Ollama response has no output usage"))?;
    Ok(ProviderResponse {
        content: parsed.message.content,
        usage: ModelUsage {
            input_tokens,
            output_tokens,
            cost_micros: None,
        },
        finish: finish_reason(parsed.done_reason.as_deref())?,
    })
}

fn role_wire(role: ModelRole) -> &'static str {
    match role {
        ModelRole::System => "system",
        ModelRole::User => "user",
        ModelRole::Assistant => "assistant",
    }
}

fn validate_endpoint(value: &str) -> Result<Url, LocalProviderError> {
    let endpoint = Url::parse(value)
        .map_err(|error| LocalProviderError::InvalidEndpoint(error.to_string()))?;
    let scheme_allowed =
        endpoint.scheme() == "https" || (endpoint.scheme() == "http" && is_loopback(&endpoint));
    if !scheme_allowed {
        return Err(LocalProviderError::InvalidEndpoint(
            "endpoint must use https, or http for a loopback host".into(),
        ));
    }
    if endpoint.host_str().is_none() {
        return Err(LocalProviderError::InvalidEndpoint(
            "endpoint must have a host".into(),
        ));
    }
    if !endpoint.username().is_empty() || endpoint.password().is_some() {
        return Err(LocalProviderError::InvalidEndpoint(
            "endpoint cannot contain embedded credentials".into(),
        ));
    }
    if endpoint.query().is_some() || endpoint.fragment().is_some() {
        return Err(LocalProviderError::InvalidEndpoint(
            "endpoint cannot contain a query or fragment".into(),
        ));
    }
    Ok(endpoint)
}

fn is_loopback(endpoint: &Url) -> bool {
    let Some(host) = endpoint.host_str() else {
        return false;
    };
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn reason_for_status(status: u16) -> ProviderErrorReason {
    match status {
        401 | 403 => ProviderErrorReason::Unauthorized,
        404 => ProviderErrorReason::ModelUnavailable,
        429 => ProviderErrorReason::RateLimited,
        400..=499 => ProviderErrorReason::InvalidRequest,
        _ => ProviderErrorReason::Unreachable,
    }
}

fn finish_reason(reason: Option<&str>) -> Result<ModelFinishReason, ProviderError> {
    match reason {
        Some("stop") => Ok(ModelFinishReason::Complete),
        Some("length") => Ok(ModelFinishReason::MaxOutputTokens),
        Some("unload") => Ok(ModelFinishReason::ProviderStop),
        _ => Err(malformed_response(
            "Ollama response has an unknown finish reason",
        )),
    }
}

fn malformed_response(message: &str) -> ProviderError {
    ProviderError::new(ProviderErrorReason::MalformedResponse, message)
}
