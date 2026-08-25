use crate::observability::client::{ObservabilityClient, ObservabilityClientError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
struct PrometheusResponse {
    status: String,
    data: Option<PrometheusData>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "resultType", content = "result")]
enum PrometheusData {
    #[serde(rename = "vector")]
    Vector(Vec<PrometheusVectorResult>),
    #[serde(rename = "matrix")]
    Matrix(Vec<PrometheusMatrixResult>),
}

#[derive(Debug, Deserialize)]
struct PrometheusVectorResult {
    metric: BTreeMap<String, String>,
    value: PrometheusSampleTuple,
}

#[derive(Debug, Deserialize)]
struct PrometheusMatrixResult {
    metric: BTreeMap<String, String>,
    values: Vec<PrometheusSampleTuple>,
}

#[derive(Debug, Deserialize)]
struct PrometheusSampleTuple(f64, String);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MetricSample {
    pub timestamp: f64,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MetricSeries {
    pub labels: BTreeMap<String, String>,
    pub samples: Vec<MetricSample>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MetricSourceReference {
    pub connector_id: String,
    pub query: String,
    pub endpoint: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PrometheusQueryResult {
    pub series: Vec<MetricSeries>,
    pub source: MetricSourceReference,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PrometheusQueryRequest {
    pub connector_id: String,
    pub query: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PrometheusQueryRangeRequest {
    pub connector_id: String,
    pub query: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub step_seconds: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum PrometheusError {
    #[error("client error: {0}")]
    Client(#[from] ObservabilityClientError),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("provider error: {0}")]
    Provider(String),
}

pub async fn query(
    client: &ObservabilityClient,
    request: PrometheusQueryRequest,
) -> Result<PrometheusQueryResult, PrometheusError> {
    if request.query.trim().is_empty() {
        return Err(PrometheusError::Validation("query cannot be empty".into()));
    }

    let url = client
        .build_url("/api/v1/query")
        .map_err(PrometheusError::Client)?;
    let req = client
        .prepare_get(url.clone())
        .map_err(PrometheusError::Client)?
        .query(&[("query", &request.query)]);

    let response: PrometheusResponse = client.execute_json(req).await?;

    if response.status != "success" {
        return Err(PrometheusError::Provider(
            "prometheus returned error status".into(),
        ));
    }

    let data = response
        .data
        .ok_or_else(|| PrometheusError::Provider("missing data field".into()))?;

    let series = match data {
        PrometheusData::Vector(results) => results
            .into_iter()
            .map(|res| MetricSeries {
                labels: res.metric,
                samples: vec![MetricSample {
                    timestamp: res.value.0,
                    value: res.value.1,
                }],
            })
            .collect(),
        PrometheusData::Matrix(_) => {
            return Err(PrometheusError::Provider(
                "expected vector result for instant query".into(),
            ));
        }
    };

    Ok(PrometheusQueryResult {
        series,
        source: MetricSourceReference {
            connector_id: request.connector_id,
            query: request.query,
            endpoint: "/api/v1/query".into(),
        },
    })
}

pub async fn query_range(
    client: &ObservabilityClient,
    request: PrometheusQueryRangeRequest,
) -> Result<PrometheusQueryResult, PrometheusError> {
    if request.query.trim().is_empty() {
        return Err(PrometheusError::Validation("query cannot be empty".into()));
    }
    if request.start > request.end {
        return Err(PrometheusError::Validation(
            "start time must be before end time".into(),
        ));
    }
    if request.step_seconds == 0 {
        return Err(PrometheusError::Validation(
            "step_seconds must be positive".into(),
        ));
    }

    let url = client
        .build_url("/api/v1/query_range")
        .map_err(PrometheusError::Client)?;
    let req = client
        .prepare_get(url.clone())
        .map_err(PrometheusError::Client)?
        .query(&[
            ("query", &request.query),
            ("start", &request.start.to_rfc3339()),
            ("end", &request.end.to_rfc3339()),
            ("step", &request.step_seconds.to_string()),
        ]);

    let response: PrometheusResponse = client.execute_json(req).await?;

    if response.status != "success" {
        return Err(PrometheusError::Provider(
            "prometheus returned error status".into(),
        ));
    }

    let data = response
        .data
        .ok_or_else(|| PrometheusError::Provider("missing data field".into()))?;

    let series = match data {
        PrometheusData::Matrix(results) => results
            .into_iter()
            .map(|res| MetricSeries {
                labels: res.metric,
                samples: res
                    .values
                    .into_iter()
                    .map(|t| MetricSample {
                        timestamp: t.0,
                        value: t.1,
                    })
                    .collect(),
            })
            .collect(),
        PrometheusData::Vector(_) => {
            return Err(PrometheusError::Provider(
                "expected matrix result for range query".into(),
            ));
        }
    };

    Ok(PrometheusQueryResult {
        series,
        source: MetricSourceReference {
            connector_id: request.connector_id,
            query: request.query,
            endpoint: "/api/v1/query_range".into(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectors::{ConnectorSummary, InMemoryCredentialStore};
    use httpmock::MockServer;
    use serde_json::json;

    fn test_connector(base_url: &str) -> ConnectorSummary {
        ConnectorSummary {
            id: "test-prom".into(),
            kind: "prometheus".into(),
            display_name: "Prometheus".into(),
            enabled: true,
            config_metadata: json!({
                "base_url": base_url,
                "auth_mode": "none"
            }),
            credential_configured: false,
            health_state: "healthy".into(),
            last_checked_at: None,
            last_successful_sync_at: None,
        }
    }

    #[tokio::test]
    async fn test_query_instant_success() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("GET")
                .path("/api/v1/query")
                .query_param("query", "up");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    json!({
                        "status": "success",
                        "data": {
                            "resultType": "vector",
                            "result": [
                                {
                                    "metric": { "__name__": "up" },
                                    "value": [123.45, "1"]
                                }
                            ]
                        }
                    })
                    .to_string(),
                );
        });

        let connector = test_connector(&server.url(""));
        let store = InMemoryCredentialStore::default();
        let client = ObservabilityClient::new(&connector, &store).unwrap();

        let res = query(
            &client,
            PrometheusQueryRequest {
                connector_id: "test-prom".into(),
                query: "up".into(),
            },
        )
        .await
        .unwrap();

        mock.assert();
        assert_eq!(res.series.len(), 1);
        assert_eq!(res.series[0].samples[0].value, "1");
        assert_eq!(res.source.connector_id, "test-prom");
    }

    #[tokio::test]
    async fn test_query_range_success() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("GET")
                .path("/api/v1/query_range")
                .query_param("query", "up")
                .query_param_exists("start")
                .query_param_exists("end")
                .query_param("step", "60");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    json!({
                        "status": "success",
                        "data": {
                            "resultType": "matrix",
                            "result": [
                                {
                                    "metric": { "__name__": "up" },
                                    "values": [[123.45, "1"], [183.45, "1"]]
                                }
                            ]
                        }
                    })
                    .to_string(),
                );
        });

        let connector = test_connector(&server.url(""));
        let store = InMemoryCredentialStore::default();
        let client = ObservabilityClient::new(&connector, &store).unwrap();

        use chrono::Utc;
        let start = Utc::now();
        let end = start + std::time::Duration::from_secs(60);
        let res = query_range(
            &client,
            PrometheusQueryRangeRequest {
                connector_id: "test-prom".into(),
                query: "up".into(),
                start,
                end,
                step_seconds: 60,
            },
        )
        .await
        .unwrap();

        mock.assert();
        assert_eq!(res.series.len(), 1);
        assert_eq!(res.series[0].samples.len(), 2);
    }

    #[tokio::test]
    async fn test_query_range_invalid() {
        let connector = test_connector("http://localhost");
        let store = InMemoryCredentialStore::default();
        let client = ObservabilityClient::new(&connector, &store).unwrap();
        use chrono::Utc;

        let err1 = query_range(
            &client,
            PrometheusQueryRangeRequest {
                connector_id: "test-prom".into(),
                query: "".into(),
                start: Utc::now(),
                end: Utc::now(),
                step_seconds: 60,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err1, PrometheusError::Validation(_)));

        let err2 = query_range(
            &client,
            PrometheusQueryRangeRequest {
                connector_id: "test-prom".into(),
                query: "up".into(),
                start: Utc::now() + std::time::Duration::from_secs(60),
                end: Utc::now(),
                step_seconds: 60,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err2, PrometheusError::Validation(_)));

        let err3 = query_range(
            &client,
            PrometheusQueryRangeRequest {
                connector_id: "test-prom".into(),
                query: "up".into(),
                start: Utc::now(),
                end: Utc::now() + std::time::Duration::from_secs(60),
                step_seconds: 0,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err3, PrometheusError::Validation(_)));
    }
}
