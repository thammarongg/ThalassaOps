use crate::observability::client::{ObservabilityClient, ObservabilityClientError};
use crate::observability::masking::mask_json_object;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub const MAX_LOG_LINES: u32 = 200;
const QUERY_RANGE_ENDPOINT: &str = "/loki/api/v1/query_range";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LogEntry {
    pub timestamp_ns: String,
    pub line: String,
    pub parsed: bool,
    pub masked: bool,
    pub fields: Option<BTreeMap<String, String>>,
    pub trace_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LogStream {
    pub labels: BTreeMap<String, String>,
    pub entries: Vec<LogEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LogSourceReference {
    pub connector_id: String,
    pub query: String,
    pub endpoint: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LokiQueryResult {
    pub streams: Vec<LogStream>,
    pub source: LogSourceReference,
    pub unparsed_count: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LokiQueryRangeRequest {
    pub connector_id: String,
    pub query: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub limit: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum LokiError {
    #[error("client error: {0}")]
    Client(#[from] ObservabilityClientError),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("provider error: {0}")]
    Provider(String),
}

#[derive(Debug, Deserialize)]
struct LokiResponse {
    status: String,
    data: Option<LokiData>,
}

#[derive(Debug, Deserialize)]
struct LokiData {
    #[serde(rename = "resultType")]
    result_type: String,
    result: Vec<LokiStreamResponse>,
}

#[derive(Debug, Deserialize)]
struct LokiStreamResponse {
    stream: BTreeMap<String, String>,
    values: Vec<LokiValueTuple>,
}

#[derive(Debug, Deserialize)]
struct LokiValueTuple(String, String);

pub async fn query_range(
    client: &ObservabilityClient,
    request: LokiQueryRangeRequest,
) -> Result<LokiQueryResult, LokiError> {
    if request.query.trim().is_empty() {
        return Err(LokiError::Validation("query cannot be empty".into()));
    }
    if request.start > request.end {
        return Err(LokiError::Validation(
            "start time must not be after end time".into(),
        ));
    }
    if request.limit == 0 {
        return Err(LokiError::Validation("limit must be positive".into()));
    }
    if request.limit > MAX_LOG_LINES {
        return Err(LokiError::Validation(format!(
            "limit must not exceed {MAX_LOG_LINES}"
        )));
    }

    let url = client
        .build_url(QUERY_RANGE_ENDPOINT)
        .map_err(LokiError::Client)?;
    let start = request.start.to_rfc3339();
    let end = request.end.to_rfc3339();
    let limit = request.limit.to_string();
    let req = client.prepare_get(url).map_err(LokiError::Client)?.query(&[
        ("query", request.query.as_str()),
        ("start", start.as_str()),
        ("end", end.as_str()),
        ("limit", limit.as_str()),
        ("direction", "backward"),
    ]);
    let response: LokiResponse = client.execute_json(req).await?;

    if response.status != "success" {
        return Err(LokiError::Provider("loki returned an error status".into()));
    }
    let data = response
        .data
        .ok_or_else(|| LokiError::Provider("missing data field".into()))?;
    if data.result_type != "streams" {
        return Err(LokiError::Provider(
            "expected streams result from Loki".into(),
        ));
    }

    let mut unparsed_count = 0;
    let mut streams = Vec::with_capacity(data.result.len());
    for stream in data.result {
        let mut entries = Vec::with_capacity(stream.values.len());
        for LokiValueTuple(timestamp_ns, line) in stream.values {
            let entry = map_entry(timestamp_ns, line, &mut unparsed_count)?;
            entries.push(entry);
        }
        streams.push(LogStream {
            labels: mask_stream_labels(stream.stream),
            entries,
        });
    }

    Ok(LokiQueryResult {
        streams,
        source: LogSourceReference {
            connector_id: request.connector_id,
            query: request.query,
            endpoint: QUERY_RANGE_ENDPOINT.into(),
        },
        unparsed_count,
    })
}

fn map_entry(
    timestamp_ns: String,
    line: String,
    unparsed_count: &mut usize,
) -> Result<LogEntry, LokiError> {
    let mut object: Map<String, Value> = match serde_json::from_str(&line) {
        Ok(object) => object,
        Err(_) => {
            *unparsed_count += 1;
            return Ok(LogEntry {
                timestamp_ns,
                line,
                parsed: false,
                masked: false,
                fields: None,
                trace_id: None,
            });
        }
    };

    let trace_id = trace_id_from_object(&object);
    let masked = mask_json_object(&mut object);
    let fields = object
        .iter()
        .map(|(key, value)| (key.clone(), field_value(value)))
        .collect();
    let line = serde_json::to_string(&Value::Object(object))
        .map_err(|_| LokiError::Provider("failed to serialize parsed log".into()))?;

    Ok(LogEntry {
        timestamp_ns,
        line,
        parsed: true,
        masked,
        fields: Some(fields),
        trace_id,
    })
}

fn field_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

fn mask_stream_labels(labels: BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut object = labels
        .into_iter()
        .map(|(key, value)| (key, Value::String(value)))
        .collect();
    mask_json_object(&mut object);
    object
        .into_iter()
        .map(|(key, value)| (key, field_value(&value)))
        .collect()
}

fn trace_id_from_object(object: &Map<String, Value>) -> Option<String> {
    for key in ["trace_id", "traceID", "traceparent"] {
        let Some(value) = object.get(key).and_then(Value::as_str) else {
            continue;
        };
        let trace_id = if key == "traceparent" {
            parse_traceparent(value)
        } else if is_lower_hex(value, 32) {
            Some(value.to_owned())
        } else {
            None
        };
        if trace_id.is_some() {
            return trace_id;
        }
    }
    None
}

fn parse_traceparent(value: &str) -> Option<String> {
    let mut parts = value.split('-');
    let version = parts.next()?;
    let trace_id = parts.next()?;
    let parent_id = parts.next()?;
    let flags = parts.next()?;
    if parts.next().is_some()
        || version == "ff"
        || !is_lower_hex(version, 2)
        || !is_lower_hex(trace_id, 32)
        || is_all_zero(trace_id)
        || !is_lower_hex(parent_id, 16)
        || is_all_zero(parent_id)
        || !is_lower_hex(flags, 2)
    {
        return None;
    }
    Some(trace_id.to_owned())
}

fn is_lower_hex(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn is_all_zero(value: &str) -> bool {
    value.bytes().all(|byte| byte == b'0')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectors::{ConnectorSummary, InMemoryCredentialStore};
    use crate::observability::client::ObservabilityClient;
    use crate::observability::masking::REDACTED;
    use chrono::{TimeZone, Utc};
    use httpmock::MockServer;
    use serde_json::json;

    const TRACE_ID: &str = "4bf92f3577b34da6a3ce929d0e0e4736";

    fn test_connector(base_url: &str) -> ConnectorSummary {
        ConnectorSummary {
            id: "test-loki".into(),
            kind: "loki".into(),
            display_name: "Loki".into(),
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

    fn request(start: chrono::DateTime<Utc>, end: chrono::DateTime<Utc>) -> LokiQueryRangeRequest {
        LokiQueryRangeRequest {
            connector_id: "test-loki".into(),
            query: "{namespace=\"prod\", pod=\"api-0\"}".into(),
            start,
            end,
            limit: 20,
        }
    }

    #[tokio::test]
    async fn query_range_maps_streams_masks_json_and_counts_unparsed_lines() {
        let server = MockServer::start();
        let start = Utc.timestamp_opt(1735689600, 0).single().unwrap();
        let end = Utc.timestamp_opt(1735689660, 0).single().unwrap();
        let start_param = start.to_rfc3339();
        let end_param = end.to_rfc3339();
        let mock = server.mock(|when, then| {
            when.method("GET")
                .path("/loki/api/v1/query_range")
                .query_param("query", "{namespace=\"prod\", pod=\"api-0\"}")
                .query_param("start", &start_param)
                .query_param("end", &end_param)
                .query_param("limit", "20")
                .query_param("direction", "backward")
                .header("X-Scope-OrgID", "team-a");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    json!({
                        "status": "success",
                        "data": {
                            "resultType": "streams",
                            "result": [{
                                "stream": {"namespace": "prod", "pod": "api-0", "api_token": "stream-secret"},
                                "values": [
                                    ["1735689600000000001", format!("{{\"msg\":\"boom\",\"api_key\":\"sk-live-1\",\"trace_id\":\"{TRACE_ID}\"}}")],
                                    ["1735689600000000002", "plain text line with api_key=sk-live-2"]
                                ]
                            }]
                        }
                    })
                    .to_string(),
                );
        });

        let client = ObservabilityClient::new(
            &test_connector(&server.url("")),
            &InMemoryCredentialStore::default(),
        )
        .unwrap();
        let result = query_range(&client, request(start, end)).await.unwrap();

        mock.assert();
        assert_eq!(result.streams.len(), 1);
        assert_eq!(result.streams[0].labels["namespace"], "prod");
        assert_eq!(result.streams[0].labels["api_token"], REDACTED);
        let serialized = serde_json::to_string(&result).unwrap();
        assert!(!serialized.contains("stream-secret"));
        assert_eq!(result.streams[0].entries.len(), 2);
        let parsed = &result.streams[0].entries[0];
        assert!(parsed.parsed);
        assert!(parsed.masked);
        assert_eq!(parsed.fields.as_ref().unwrap()["api_key"], REDACTED);
        assert_eq!(parsed.trace_id.as_deref(), Some(TRACE_ID));
        assert_eq!(parsed.timestamp_ns, "1735689600000000001");
        assert!(!parsed.line.contains("sk-live-1"));

        let unparsed = &result.streams[0].entries[1];
        assert!(!unparsed.parsed);
        assert!(!unparsed.masked);
        assert_eq!(unparsed.line, "plain text line with api_key=sk-live-2");
        assert_eq!(unparsed.trace_id, None);
        assert_eq!(result.unparsed_count, 1);
    }

    #[tokio::test]
    async fn query_range_recursively_masks_nested_objects_and_arrays_before_serialization() {
        let server = MockServer::start();
        let nested_line = json!({
            "message": "request failed",
            "context": {
                "client_secret": "nested-secret",
                "safe": "keep"
            },
            "items": [
                {"password": "array-secret"},
                {"nested": {"access_token": "deep-token", "value": "keep"}}
            ]
        })
        .to_string();
        let mock = server.mock(|when, then| {
            when.method("GET").path("/loki/api/v1/query_range");
            then.status(200).body(
                json!({
                    "status": "success",
                    "data": {
                        "resultType": "streams",
                        "result": [{
                            "stream": {
                                "namespace": "prod",
                                "api_key": "label-secret"
                            },
                            "values": [["1", nested_line]]
                        }]
                    }
                })
                .to_string(),
            );
        });
        let client = ObservabilityClient::new(
            &test_connector(&server.url("")),
            &InMemoryCredentialStore::default(),
        )
        .unwrap();

        let result = query_range(
            &client,
            request(
                Utc.timestamp_opt(1735689600, 0).single().unwrap(),
                Utc.timestamp_opt(1735689660, 0).single().unwrap(),
            ),
        )
        .await
        .unwrap();

        mock.assert();
        let entry = &result.streams[0].entries[0];
        assert!(entry.parsed);
        assert!(entry.masked);
        assert_eq!(result.streams[0].labels["api_key"], REDACTED);

        let parsed_line: Value = serde_json::from_str(&entry.line).unwrap();
        assert_eq!(parsed_line["context"]["client_secret"], json!(REDACTED));
        assert_eq!(parsed_line["items"][0]["password"], json!(REDACTED));
        assert_eq!(
            parsed_line["items"][1]["nested"]["access_token"],
            json!(REDACTED)
        );
        assert_eq!(parsed_line["context"]["safe"], json!("keep"));
        assert_eq!(parsed_line["items"][1]["nested"]["value"], json!("keep"));

        let serialized = serde_json::to_string(&result).unwrap();
        for leaked in [
            "nested-secret",
            "array-secret",
            "deep-token",
            "label-secret",
        ] {
            assert!(
                !serialized.contains(leaked),
                "sensitive value leaked: {leaked}"
            );
        }
    }

    #[tokio::test]
    async fn unparsed_lines_do_not_extract_traceparent_shaped_substrings() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("GET").path("/loki/api/v1/query_range");
            then.status(200).body(
                json!({
                    "status": "success",
                    "data": {
                        "resultType": "streams",
                        "result": [{
                            "stream": {},
                            "values": [["1", format!("line contains 00-{TRACE_ID}-0123456789abcdef-01")]]
                        }]
                    }
                })
                .to_string(),
            );
        });
        let client = ObservabilityClient::new(
            &test_connector(&server.url("")),
            &InMemoryCredentialStore::default(),
        )
        .unwrap();

        let result = query_range(
            &client,
            request(
                Utc.timestamp_opt(1735689600, 0).single().unwrap(),
                Utc.timestamp_opt(1735689660, 0).single().unwrap(),
            ),
        )
        .await
        .unwrap();

        mock.assert();
        assert_eq!(result.streams[0].entries[0].trace_id, None);
    }

    #[tokio::test]
    async fn structured_traceparent_extracts_only_its_valid_trace_id() {
        let server = MockServer::start();
        let valid_traceparent = format!("00-{TRACE_ID}-00f067aa0ba902b7-01");
        let malformed_traceparent = format!("00-{TRACE_ID}-not-a-span-01");
        let mock = server.mock(|when, then| {
            when.method("GET").path("/loki/api/v1/query_range");
            then.status(200).body(
                json!({
                    "status": "success",
                    "data": {
                        "resultType": "streams",
                        "result": [{
                            "stream": {},
                            "values": [
                                ["1", json!({"traceparent": valid_traceparent}).to_string()],
                                ["2", json!({"traceparent": malformed_traceparent}).to_string()]
                            ]
                        }]
                    }
                })
                .to_string(),
            );
        });
        let client = ObservabilityClient::new(
            &test_connector(&server.url("")),
            &InMemoryCredentialStore::default(),
        )
        .unwrap();

        let result = query_range(
            &client,
            request(
                Utc.timestamp_opt(1735689600, 0).single().unwrap(),
                Utc.timestamp_opt(1735689660, 0).single().unwrap(),
            ),
        )
        .await
        .unwrap();

        mock.assert();
        assert_eq!(
            result.streams[0].entries[0].trace_id.as_deref(),
            Some(TRACE_ID)
        );
        assert_eq!(result.streams[0].entries[1].trace_id, None);
    }

    #[tokio::test]
    async fn query_range_rejects_invalid_request_bounds_before_http() {
        let server = MockServer::start();
        let client = ObservabilityClient::new(
            &test_connector(&server.url("")),
            &InMemoryCredentialStore::default(),
        )
        .unwrap();
        let start = Utc.timestamp_opt(1735689660, 0).single().unwrap();
        let end = Utc.timestamp_opt(1735689600, 0).single().unwrap();

        for invalid in [
            LokiQueryRangeRequest {
                query: "  ".into(),
                ..request(start, start)
            },
            LokiQueryRangeRequest {
                start,
                end,
                ..request(start, end)
            },
            LokiQueryRangeRequest {
                limit: 0,
                ..request(start, start)
            },
            LokiQueryRangeRequest {
                limit: MAX_LOG_LINES + 1,
                ..request(start, start)
            },
        ] {
            assert!(matches!(
                query_range(&client, invalid).await,
                Err(LokiError::Validation(_))
            ));
        }
    }
}
