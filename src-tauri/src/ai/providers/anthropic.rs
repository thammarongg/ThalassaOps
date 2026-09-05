use std::net::IpAddr;
use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use reqwest::Url;
use serde::Deserialize;
use serde_json::{json, Map, Value};
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
const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnthropicConfig {
    pub endpoint: String,
}

#[derive(Debug, Error)]
pub enum AnthropicProviderError {
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
pub struct AnthropicProvider {
    client: Client,
    endpoint: Url,
    credential: String,
    manifest: ProviderManifest,
}

impl AnthropicProvider {
    pub fn new(
        manifest: ProviderManifest,
        config_metadata: Value,
        credential_store: &dyn CredentialStore,
    ) -> Result<Self, AnthropicProviderError> {
        let config: AnthropicConfig = serde_json::from_value(config_metadata)
            .map_err(|error| AnthropicProviderError::Configuration(error.to_string()))?;
        let endpoint = validate_endpoint(&config.endpoint)?;
        let credential = credential_store
            .get(&format!("provider/{}", manifest.id))
            .map_err(|error| AnthropicProviderError::Credential(error.to_string()))?
            .filter(|credential| !credential.trim().is_empty())
            .ok_or(AnthropicProviderError::MissingCredential)?;
        let client = Client::builder()
            .timeout(HTTP_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| AnthropicProviderError::HttpClient)?;

        Ok(Self {
            client,
            endpoint,
            credential,
            manifest,
        })
    }

    pub fn request_body(&self, request: &ProviderRequest) -> Result<Value, AnthropicProviderError> {
        let messages = request
            .messages
            .iter()
            .map(anthropic_message)
            .collect::<Result<Vec<_>, _>>()?;
        let mut body = Map::from_iter([
            ("model".into(), json!(request.model_id)),
            ("messages".into(), json!(messages)),
            ("max_tokens".into(), json!(request.max_output_tokens)),
        ]);
        if let Some(instruction) = &request.instruction {
            body.insert("system".into(), json!(instruction));
        }
        Ok(Value::Object(body))
    }

    pub fn parse_response(
        &self,
        status: u16,
        body: &Value,
    ) -> Result<ProviderResponse, ProviderError> {
        if !(200..=299).contains(&status) {
            return Err(ProviderError::new(
                reason_for_status(status),
                format!("Anthropic provider returned status {status}"),
            ));
        }

        let parsed: AnthropicResponse = serde_json::from_value(body.clone())
            .map_err(|_| malformed_response("Anthropic response does not match its schema"))?;
        let content = parsed
            .content
            .first()
            .map(|block| match block {
                AnthropicContent::Text { text } => text.clone(),
            })
            .ok_or_else(|| malformed_response("Anthropic response has no text content"))?;
        let usage = parsed
            .usage
            .ok_or_else(|| malformed_response("Anthropic response has no usage"))?;
        let model = parsed
            .model
            .as_deref()
            .and_then(|model_id| self.manifest.model(model_id))
            .or_else(|| self.manifest.models.first());

        Ok(ProviderResponse {
            content,
            usage: ModelUsage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cost_micros: model.and_then(|model| {
                    cost_for_usage(model, usage.input_tokens, usage.output_tokens)
                }),
            },
            finish: finish_reason(parsed.stop_reason.as_deref())?,
        })
    }

    fn request_error(error: AnthropicProviderError) -> ProviderError {
        ProviderError::new(ProviderErrorReason::InvalidRequest, error.to_string())
    }
}

impl ModelProvider for AnthropicProvider {
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
            .header("x-api-key", &self.credential)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&body)
            .send()
            .map_err(|_| {
                ProviderError::new(ProviderErrorReason::Unreachable, "provider request failed")
            })?;
        let status = response.status().as_u16();
        let body = response
            .json::<Value>()
            .map_err(|_| malformed_response("Anthropic provider returned invalid JSON"))?;
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
struct AnthropicResponse {
    model: Option<String>,
    content: Vec<AnthropicContent>,
    stop_reason: Option<String>,
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum AnthropicContent {
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u64,
    output_tokens: u64,
}

fn anthropic_message(message: &ModelMessage) -> Result<Value, AnthropicProviderError> {
    let role = match message.role {
        ModelRole::User => "user",
        ModelRole::Assistant => "assistant",
        ModelRole::System => {
            return Err(AnthropicProviderError::Configuration(
                "system messages must use the instruction field".into(),
            ));
        }
    };
    Ok(json!({
        "role": role,
        "content": message.content,
    }))
}

fn validate_endpoint(value: &str) -> Result<Url, AnthropicProviderError> {
    let endpoint = Url::parse(value)
        .map_err(|error| AnthropicProviderError::InvalidEndpoint(error.to_string()))?;
    let scheme_allowed =
        endpoint.scheme() == "https" || (endpoint.scheme() == "http" && is_loopback(&endpoint));
    if !scheme_allowed {
        return Err(AnthropicProviderError::InvalidEndpoint(
            "endpoint must use https, or http for a loopback host".into(),
        ));
    }
    if endpoint.host_str().is_none() {
        return Err(AnthropicProviderError::InvalidEndpoint(
            "endpoint must have a host".into(),
        ));
    }
    if !endpoint.username().is_empty() || endpoint.password().is_some() {
        return Err(AnthropicProviderError::InvalidEndpoint(
            "endpoint cannot contain embedded credentials".into(),
        ));
    }
    if endpoint.query().is_some() || endpoint.fragment().is_some() {
        return Err(AnthropicProviderError::InvalidEndpoint(
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
        Some("end_turn") => Ok(ModelFinishReason::Complete),
        Some("max_tokens") => Ok(ModelFinishReason::MaxOutputTokens),
        Some("stop_sequence") => Ok(ModelFinishReason::ProviderStop),
        _ => Err(malformed_response(
            "Anthropic response has an unknown stop reason",
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
