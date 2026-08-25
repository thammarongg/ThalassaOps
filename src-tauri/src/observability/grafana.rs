use crate::observability::client::{ObservabilityClient, ObservabilityClientError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct GrafanaHealthResponse {
    pub database: String,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GrafanaHealth {
    pub database: String,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GrafanaLinkRequest {
    pub connector_id: String,
    pub target: String, // "dashboard" or "explore"
    pub query: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GrafanaLinkResult {
    pub url: String,
}

#[derive(Debug, thiserror::Error)]
pub enum GrafanaError {
    #[error("client error: {0}")]
    Client(#[from] ObservabilityClientError),
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("validation error: {0}")]
    Validation(String),
}

pub async fn health(client: &ObservabilityClient) -> Result<GrafanaHealth, GrafanaError> {
    let url = client
        .build_url("/api/health")
        .map_err(GrafanaError::Client)?;
    let req = client.prepare_get(url).map_err(GrafanaError::Client)?;

    let response: GrafanaHealthResponse = client.execute_json(req).await?;

    Ok(GrafanaHealth {
        database: response.database,
        version: response.version,
    })
}

pub fn link(
    request: GrafanaLinkRequest,
    client: &ObservabilityClient,
    datasource_uid: Option<&str>,
    default_dashboard_uid: Option<&str>,
) -> Result<GrafanaLinkResult, GrafanaError> {
    if request.target != "dashboard" && request.target != "explore" {
        return Err(GrafanaError::Validation(
            "target must be dashboard or explore".into(),
        ));
    }
    if request.query.trim().is_empty() {
        return Err(GrafanaError::Validation("query cannot be empty".into()));
    }
    if request.start >= request.end {
        return Err(GrafanaError::Validation(
            "start time must be before end time".into(),
        ));
    }

    let url = if request.target == "dashboard" {
        let dash_uid = default_dashboard_uid
            .ok_or_else(|| GrafanaError::Configuration("missing default_dashboard_uid".into()))?;
        let mut u = client
            .build_url(&format!("/d/{}", dash_uid))
            .map_err(|e| GrafanaError::Validation(e.to_string()))?;
        u.query_pairs_mut()
            .append_pair("from", &request.start.timestamp_millis().to_string())
            .append_pair("to", &request.end.timestamp_millis().to_string())
            .append_pair("var-query", &request.query); // Adjust variable name if needed
        u
    } else {
        // Explore
        let ds_uid = datasource_uid
            .ok_or_else(|| GrafanaError::Configuration("missing datasource_uid".into()))?;
        let mut u = client
            .build_url("/explore")
            .map_err(|e| GrafanaError::Validation(e.to_string()))?;

        let left_pane = serde_json::json!({
            "datasource": ds_uid,
            "queries": [
                {
                    "refId": "A",
                    "expr": request.query,
                    "datasource": { "uid": ds_uid }
                }
            ],
            "range": {
                "from": request.start.timestamp_millis().to_string(),
                "to": request.end.timestamp_millis().to_string()
            }
        });

        u.query_pairs_mut()
            .append_pair("left", &left_pane.to_string());
        u
    };

    Ok(GrafanaLinkResult {
        url: url.to_string(),
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
            id: "test-grafana".into(),
            kind: "grafana".into(),
            display_name: "Grafana".into(),
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
    async fn test_health_success() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("GET").path("/api/health");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    json!({
                        "database": "ok",
                        "version": "10.0.0",
                        "commit": "123"
                    })
                    .to_string(),
                );
        });

        let connector = test_connector(&server.url(""));
        let store = InMemoryCredentialStore::default();
        let client = ObservabilityClient::new(&connector, &store).unwrap();

        let res = health(&client).await.unwrap();

        mock.assert();
        assert_eq!(res.database, "ok");
        assert_eq!(res.version, "10.0.0");
    }

    #[test]
    fn test_link_dashboard_and_explore() {
        let store = InMemoryCredentialStore::default();
        let connector = test_connector("http://localhost/subpath");
        let client = ObservabilityClient::new(&connector, &store).unwrap();

        use chrono::TimeZone;
        let start = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2024, 1, 1, 1, 0, 0).unwrap();

        let req = GrafanaLinkRequest {
            connector_id: "test".into(),
            target: "dashboard".into(),
            query: "up".into(),
            start,
            end,
        };

        let res = link(req.clone(), &client, Some("ds1"), Some("dash1")).unwrap();
        assert!(res.url.starts_with("http://localhost/subpath/d/dash1"));
        assert!(res.url.contains("from=1704067200000"));
        assert!(res.url.contains("to=1704070800000"));
        assert!(res.url.contains("var-query=up"));

        let mut req_explore = req.clone();
        req_explore.target = "explore".into();
        let res2 = link(req_explore, &client, Some("ds1"), Some("dash1")).unwrap();
        assert!(res2.url.starts_with("http://localhost/subpath/explore"));
        assert!(res2.url.contains("left="));
    }
}
