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
    ModelDescriptor, ModelFinishReason, ModelMessage, ModelRole, ModelUsage, ProviderErrorReason,
    ProviderHealth,
};
use thiserror::Error;
use uuid::Uuid;

use crate::connectors::CredentialStore;

const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenAiCompatibleConfig {
    pub endpoint: String,
}

#[derive(Debug, Error)]
pub enum OpenAiCompatibleError {
    #[error("invalid provider configuration: {0}")]
    Configuration(String),
    #[error("invalid provider endpoint: {0}")]
    InvalidEndpoint(String),
    #[error("provider credential error: {0}")]
    Credential(String),
    #[error("provider credential is not configured")]
    MissingCredential,
    #[error("could not build provider HTTP client")]
    HttpClient,
}

#[derive(Debug)]
pub struct OpenAiCompatibleProvider {
    client: Client,
    endpoint: Url,
    credential: String,
    manifest: ProviderManifest,
}

impl OpenAiCompatibleProvider {
    pub fn new(
        manifest: ProviderManifest,
        config_metadata: Value,
        credential_store: &dyn CredentialStore,
    ) -> Result<Self, OpenAiCompatibleError> {
        let config: OpenAiCompatibleConfig = serde_json::from_value(config_metadata)
            .map_err(|error| OpenAiCompatibleError::Configuration(error.to_string()))?;
        let endpoint = validate_endpoint(&config.endpoint)?;
        let credential = credential_store
            .get(&format!("provider/{}", manifest.id))
            .map_err(|error| OpenAiCompatibleError::Credential(error.to_string()))?
            .filter(|credential| !credential.trim().is_empty())
            .ok_or(OpenAiCompatibleError::MissingCredential)?;
        let client = Client::builder()
            .timeout(HTTP_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| OpenAiCompatibleError::HttpClient)?;

        Ok(Self {
            client,
            endpoint,
            credential,
            manifest,
        })
    }

    pub fn request_body(&self, request: &ProviderRequest) -> Result<Value, OpenAiCompatibleError> {
        openai_request_body(request)
    }

    pub fn parse_response(
        &self,
        status: u16,
        body: &Value,
    ) -> Result<ProviderResponse, ProviderError> {
        parse_openai_response(&self.manifest, status, body)
    }

    fn request_error(error: OpenAiCompatibleError) -> ProviderError {
        ProviderError::new(ProviderErrorReason::InvalidRequest, error.to_string())
    }
}

impl ModelProvider for OpenAiCompatibleProvider {
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
        let body = self.request_body(request).map_err(Self::request_error)?;
        let response = self
            .client
            .post(self.endpoint.clone())
            .timeout(remaining)
            .bearer_auth(&self.credential)
            .json(&body)
            .send()
            .map_err(|_| {
                ProviderError::new(ProviderErrorReason::Unreachable, "provider request failed")
            })?;
        let status = response.status().as_u16();
        let body = response
            .json::<Value>()
            .map_err(|_| malformed_response("OpenAI-compatible provider returned invalid JSON"))?;
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
struct OpenAiResponse {
    model: Option<String>,
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
}

fn message_value(message: &ModelMessage) -> Value {
    json!({
        "role": role_wire(message.role),
        "content": message.content,
    })
}

pub(crate) fn openai_request_body(
    request: &ProviderRequest,
) -> Result<Value, OpenAiCompatibleError> {
    let mut messages = Vec::with_capacity(request.messages.len() + 1);
    if let Some(instruction) = &request.instruction {
        messages.push(json!({
            "role": "system",
            "content": instruction,
        }));
    }
    messages.extend(request.messages.iter().map(message_value));
    Ok(json!({
        "model": request.model_id,
        "messages": messages,
        "max_tokens": request.max_output_tokens,
    }))
}

pub(crate) fn parse_openai_response(
    manifest: &ProviderManifest,
    status: u16,
    body: &Value,
) -> Result<ProviderResponse, ProviderError> {
    if !(200..=299).contains(&status) {
        return Err(ProviderError::new(
            reason_for_status(status),
            format!("OpenAI-compatible provider returned status {status}"),
        ));
    }

    let parsed: OpenAiResponse = serde_json::from_value(body.clone())
        .map_err(|_| malformed_response("OpenAI-compatible response does not match its schema"))?;
    let choice = parsed
        .choices
        .first()
        .ok_or_else(|| malformed_response("OpenAI-compatible response has no choices"))?;
    let usage = parsed
        .usage
        .ok_or_else(|| malformed_response("OpenAI-compatible response has no usage"))?;
    let model = parsed
        .model
        .as_deref()
        .and_then(|model_id| manifest.model(model_id))
        .or_else(|| manifest.models.first());

    Ok(ProviderResponse {
        content: choice.message.content.clone(),
        usage: ModelUsage {
            input_tokens: usage.prompt_tokens,
            output_tokens: usage.completion_tokens,
            cost_micros: model.and_then(|model| {
                cost_for_usage(model, usage.prompt_tokens, usage.completion_tokens)
            }),
        },
        finish: finish_reason(choice.finish_reason.as_deref())?,
    })
}

fn role_wire(role: ModelRole) -> &'static str {
    match role {
        ModelRole::System => "system",
        ModelRole::User => "user",
        ModelRole::Assistant => "assistant",
    }
}

fn validate_endpoint(value: &str) -> Result<Url, OpenAiCompatibleError> {
    let endpoint = Url::parse(value)
        .map_err(|error| OpenAiCompatibleError::InvalidEndpoint(error.to_string()))?;
    let scheme_allowed =
        endpoint.scheme() == "https" || (endpoint.scheme() == "http" && is_loopback(&endpoint));
    if !scheme_allowed {
        return Err(OpenAiCompatibleError::InvalidEndpoint(
            "endpoint must use https, or http for a loopback host".into(),
        ));
    }
    if endpoint.host_str().is_none() {
        return Err(OpenAiCompatibleError::InvalidEndpoint(
            "endpoint must have a host".into(),
        ));
    }
    if !endpoint.username().is_empty() || endpoint.password().is_some() {
        return Err(OpenAiCompatibleError::InvalidEndpoint(
            "endpoint cannot contain embedded credentials".into(),
        ));
    }
    if endpoint.query().is_some() || endpoint.fragment().is_some() {
        return Err(OpenAiCompatibleError::InvalidEndpoint(
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
        Some("content_filter") => Ok(ModelFinishReason::ProviderStop),
        _ => Err(malformed_response(
            "OpenAI-compatible response has an unknown finish reason",
        )),
    }
}

fn malformed_response(message: &str) -> ProviderError {
    ProviderError::new(ProviderErrorReason::MalformedResponse, message)
}

fn cost_for_usage(model: &ModelDescriptor, input_tokens: u64, output_tokens: u64) -> Option<u64> {
    let input_price = model.input_cost_micros_per_million_tokens?;
    let output_price = model.output_cost_micros_per_million_tokens?;
    Some(
        cost_for_tokens(input_tokens, input_price)
            .saturating_add(cost_for_tokens(output_tokens, output_price)),
    )
}

fn cost_for_tokens(tokens: u64, price_micros_per_million: u64) -> u64 {
    let amount = (u128::from(tokens) * u128::from(price_micros_per_million))
        .saturating_add(999_999)
        / 1_000_000;
    amount.min(u128::from(u64::MAX)) as u64
}
