use crate::observability::client::{ObservabilityClient, ObservabilityClientError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

const TRACE_ENDPOINT_PREFIX: &str = "/api/traces/";
const HEALTH_ENDPOINT: &str = "/ready";

pub const ALLOWED_SPAN_ATTRIBUTES: [&str; 8] = [
    "http.status_code",
    "http.method",
    "http.route",
    "rpc.service",
    "rpc.method",
    "db.system",
    "exception.type",
    "otel.status_description",
];

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SpanSummary {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub service_name: String,
    pub start_time_unix_nano: String,
    pub duration_nano: String,
    pub status: String,
    pub attributes: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TraceSourceReference {
    pub connector_id: String,
    pub trace_id: String,
    pub endpoint: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TraceResult {
    pub trace_id: String,
    pub spans: Vec<SpanSummary>,
    pub source: TraceSourceReference,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TempoTraceRequest {
    pub connector_id: String,
    pub trace_id: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TempoError {
    #[error("client error: {0}")]
    Client(#[from] ObservabilityClientError),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("provider error: {0}")]
    Provider(String),
}

#[derive(Debug, Deserialize)]
struct TempoTraceResponse {
    #[serde(default)]
    batches: Vec<TempoBatch>,
    #[serde(rename = "resourceSpans", default)]
    resource_spans: Vec<TempoBatch>,
    #[serde(default)]
    trace: Option<TempoTracePayload>,
}

#[derive(Debug, Deserialize)]
struct TempoTracePayload {
    #[serde(default)]
    batches: Vec<TempoBatch>,
    #[serde(rename = "resourceSpans", default)]
    resource_spans: Vec<TempoBatch>,
}

#[derive(Debug, Default, Deserialize)]
struct TempoBatch {
    #[serde(default)]
    resource: TempoResource,
    #[serde(rename = "scopeSpans", default, alias = "instrumentationLibrarySpans")]
    scope_spans: Vec<TempoScopeSpans>,
}

#[derive(Debug, Default, Deserialize)]
struct TempoResource {
    #[serde(default)]
    attributes: Vec<TempoAttribute>,
}

#[derive(Debug, Deserialize)]
struct TempoScopeSpans {
    #[serde(default)]
    spans: Vec<TempoRawSpan>,
}

#[derive(Debug, Deserialize)]
struct TempoRawSpan {
    #[serde(rename = "traceId")]
    trace_id: String,
    #[serde(rename = "spanId")]
    span_id: String,
    #[serde(rename = "parentSpanId", default)]
    parent_span_id: Option<String>,
    name: String,
    #[serde(rename = "startTimeUnixNano")]
    start_time_unix_nano: Value,
    #[serde(rename = "endTimeUnixNano", default)]
    end_time_unix_nano: Option<Value>,
    #[serde(rename = "durationNano", default)]
    duration_nano: Option<Value>,
    #[serde(default)]
    status: Option<TempoStatus>,
    #[serde(default)]
    attributes: Vec<TempoAttribute>,
}

#[derive(Debug, Deserialize)]
struct TempoStatus {
    #[serde(default)]
    code: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct TempoAttribute {
    key: String,
    value: Value,
}

pub fn validate_trace_id(value: &str) -> Result<(), TempoError> {
    if value.len() != 32
        || !value
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
    {
        return Err(TempoError::Validation(
            "trace_id must be 32 lowercase hexadecimal characters".into(),
        ));
    }
    Ok(())
}

pub async fn trace(
    client: &ObservabilityClient,
    request: TempoTraceRequest,
) -> Result<TraceResult, TempoError> {
    validate_trace_id(&request.trace_id)?;
    let endpoint = format!("{TRACE_ENDPOINT_PREFIX}{}", request.trace_id);
    let url = client.build_url(&endpoint).map_err(TempoError::Client)?;
    let req = client.prepare_get(url).map_err(TempoError::Client)?;
    let response: TempoTraceResponse = client.execute_json(req).await?;

    let mut batches = if response.batches.is_empty() {
        response.resource_spans
    } else {
        response.batches
    };
    if batches.is_empty() {
        if let Some(trace) = response.trace {
            batches = if trace.batches.is_empty() {
                trace.resource_spans
            } else {
                trace.batches
            };
        }
    }
    if batches.is_empty() {
        return Err(TempoError::Provider(
            "trace response contained no batches".into(),
        ));
    }

    let mut spans = Vec::new();
    for batch in batches {
        let service_name = service_name(&batch.resource)?;
        for scope in batch.scope_spans {
            for span in scope.spans {
                spans.push(map_span(span, &service_name)?);
            }
        }
    }

    Ok(TraceResult {
        trace_id: request.trace_id.clone(),
        spans,
        source: TraceSourceReference {
            connector_id: request.connector_id,
            trace_id: request.trace_id,
            endpoint,
        },
    })
}

pub async fn health(client: &ObservabilityClient) -> Result<(), TempoError> {
    let url = client
        .build_url(HEALTH_ENDPOINT)
        .map_err(TempoError::Client)?;
    let req = client.prepare_get(url).map_err(TempoError::Client)?;
    client.execute_empty(req).await?;
    Ok(())
}

fn service_name(resource: &TempoResource) -> Result<String, TempoError> {
    resource
        .attributes
        .iter()
        .find(|attribute| attribute.key == "service.name")
        .and_then(|attribute| any_value(&attribute.value))
        .ok_or_else(|| TempoError::Provider("trace resource is missing service.name".into()))
}

fn map_span(span: TempoRawSpan, service_name: &str) -> Result<SpanSummary, TempoError> {
    let start_time_unix_nano = nanos_string(&span.start_time_unix_nano, "start_time_unix_nano")?;
    let duration_nano = match span.duration_nano {
        Some(value) => nanos_string(&value, "duration_nano")?,
        None => {
            let end = span.end_time_unix_nano.as_ref().ok_or_else(|| {
                TempoError::Provider("trace span is missing end_time_unix_nano".into())
            })?;
            let start = parse_nanos(&start_time_unix_nano, "start_time_unix_nano")?;
            let end = parse_nanos_value(end, "end_time_unix_nano")?;
            end.checked_sub(start)
                .ok_or_else(|| TempoError::Provider("trace span end precedes start".into()))?
                .to_string()
        }
    };

    let attributes = span
        .attributes
        .into_iter()
        .filter(|attribute| ALLOWED_SPAN_ATTRIBUTES.contains(&attribute.key.as_str()))
        .filter_map(|attribute| {
            attribute_value(&attribute.value).map(|value| (attribute.key, value))
        })
        .collect();
    let status = span
        .status
        .and_then(|status| status.code)
        .and_then(|value| any_value(&value))
        .unwrap_or_else(|| "STATUS_CODE_UNSET".into());

    Ok(SpanSummary {
        trace_id: span.trace_id,
        span_id: span.span_id,
        parent_span_id: span.parent_span_id.filter(|value| !value.is_empty()),
        name: span.name,
        service_name: service_name.into(),
        start_time_unix_nano,
        duration_nano,
        status,
        attributes,
    })
}

fn nanos_string(value: &Value, name: &str) -> Result<String, TempoError> {
    let value = scalar_value(value)
        .ok_or_else(|| TempoError::Provider(format!("trace span has invalid {name}")))?;
    parse_nanos(&value, name)?;
    Ok(value)
}

fn parse_nanos_value(value: &Value, name: &str) -> Result<u128, TempoError> {
    let value = nanos_string(value, name)?;
    parse_nanos(&value, name)
}

fn parse_nanos(value: &str, name: &str) -> Result<u128, TempoError> {
    value
        .parse::<u128>()
        .map_err(|_| TempoError::Provider(format!("trace span has invalid {name}")))
}

fn attribute_value(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    for key in [
        "stringValue",
        "intValue",
        "doubleValue",
        "boolValue",
        "bytesValue",
    ] {
        if let Some(value) = object.get(key) {
            return Some(match value {
                Value::String(value) => value.clone(),
                _ => value.to_string(),
            });
        }
    }
    None
}

fn any_value(value: &Value) -> Option<String> {
    attribute_value(value).or_else(|| scalar_value(value))
}

fn scalar_value(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectors::{ConnectorSummary, InMemoryCredentialStore};
    use crate::observability::client::ObservabilityClient;
    use httpmock::MockServer;
    use serde_json::json;

    const TRACE_ID: &str = "4bf92f3577b34da6a3ce929d0e0e4736";

    fn test_connector(base_url: &str) -> ConnectorSummary {
        ConnectorSummary {
            id: "test-tempo".into(),
            kind: "tempo".into(),
            display_name: "Tempo".into(),
            enabled: true,
            config_metadata: json!({
                "base_url": base_url,
                "auth_mode": "none",
                "tenant_id": "team-a"
            }),
            credential_configured: false,
            health_state: "healthy".into(),
            last_checked_at: None,
            last_successful_sync_at: None,
        }
    }

    #[test]
    fn trace_id_validation_accepts_only_lowercase_32_character_hex() {
        assert!(validate_trace_id(TRACE_ID).is_ok());
        let uppercase = TRACE_ID.to_ascii_uppercase();
        let short = &TRACE_ID[..16];
        let thirty_one = &TRACE_ID[..31];
        let long = format!("{TRACE_ID}0");

        for invalid in [
            uppercase.as_str(),
            short,
            thirty_one,
            long.as_str(),
            "a/b",
            "a.b",
            "a%b",
        ] {
            assert!(validate_trace_id(invalid).is_err(), "{invalid}");
        }
    }

    #[tokio::test]
    async fn trace_uses_the_fixed_get_path_and_allow_lists_span_attributes() {
        let server = MockServer::start();
        let response = json!({
            "trace": {
                "resourceSpans": [{
                "resource": {
                    "attributes": [
                        {"key": "service.name", "value": {"stringValue": "api"}},
                        {"key": "app.customer_email", "value": {"stringValue": "alice@example.test"}}
                    ]
                },
                "scopeSpans": [{
                    "spans": [{
                        "traceId": TRACE_ID,
                        "spanId": "0123456789abcdef",
                        "name": "GET /orders",
                        "startTimeUnixNano": "1735689600000000000",
                        "endTimeUnixNano": "1735689600000000123",
                        "attributes": [
                            {"key": "http.status_code", "value": {"intValue": "200"}},
                            {"key": "http.url", "value": {"stringValue": "https://api.test/orders?token=secret"}},
                            {"key": "db.statement", "value": {"stringValue": "select * from users"}},
                            {"key": "app.customer_email", "value": {"stringValue": "alice@example.test"}}
                        ],
                        "status": {"code": "STATUS_CODE_OK"}
                    }]
                }]
                }]
            }
        });
        let mock = server.mock(|when, then| {
            when.method("GET")
                .path("/api/traces/4bf92f3577b34da6a3ce929d0e0e4736")
                .header("X-Scope-OrgID", "team-a");
            then.status(200)
                .header("content-type", "application/json")
                .body(response.to_string());
        });

        let client = ObservabilityClient::new(
            &test_connector(&server.url("")),
            &InMemoryCredentialStore::default(),
        )
        .unwrap();
        let result = trace(
            &client,
            TempoTraceRequest {
                connector_id: "test-tempo".into(),
                trace_id: TRACE_ID.into(),
            },
        )
        .await
        .unwrap();

        mock.assert();
        assert_eq!(result.trace_id, TRACE_ID);
        assert_eq!(result.spans.len(), 1);
        assert_eq!(result.spans[0].service_name, "api");
        assert_eq!(result.spans[0].start_time_unix_nano, "1735689600000000000");
        assert_eq!(result.spans[0].duration_nano, "123");
        assert_eq!(result.spans[0].status, "STATUS_CODE_OK");
        assert_eq!(result.spans[0].attributes["http.status_code"], "200");
        let serialized = serde_json::to_string(&result).unwrap();
        assert!(serialized.contains("http.status_code"));
        assert!(!serialized.contains("http.url"));
        assert!(!serialized.contains("db.statement"));
        assert!(!serialized.contains("app.customer_email"));
        assert!(!serialized.contains("alice@example.test"));
        assert!(!serialized.contains("token=secret"));
    }

    #[tokio::test]
    async fn rejected_trace_id_never_reaches_the_provider() {
        let server = MockServer::start();
        let unexpected = server.mock(|when, then| {
            when.method("GET");
            then.status(500);
        });
        let client = ObservabilityClient::new(
            &test_connector(&server.url("")),
            &InMemoryCredentialStore::default(),
        )
        .unwrap();

        let result = trace(
            &client,
            TempoTraceRequest {
                connector_id: "test-tempo".into(),
                trace_id: "../../etc/passwd".into(),
            },
        )
        .await;

        assert!(matches!(result, Err(TempoError::Validation(_))));
        unexpected.assert_hits(0);
    }

    #[tokio::test]
    async fn health_uses_the_fixed_readiness_get_path() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("GET").path("/ready");
            then.status(200).body("ready");
        });
        let client = ObservabilityClient::new(
            &test_connector(&server.url("")),
            &InMemoryCredentialStore::default(),
        )
        .unwrap();

        health(&client).await.unwrap();
        mock.assert();
    }
}
