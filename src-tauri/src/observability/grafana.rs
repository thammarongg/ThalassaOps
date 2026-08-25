use serde::{Deserialize, Serialize};
use reqwest::Url;
use chrono::{DateTime, Utc};
use crate::observability::client::{ObservabilityClient, ObservabilityClientError};

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

pub async fn health(
    client: &ObservabilityClient,
) -> Result<GrafanaHealth, GrafanaError> {
    let url = client.build_url("/api/health").map_err(GrafanaError::Client)?;
    let req = client.prepare_get(url).map_err(GrafanaError::Client)?;

    let response: GrafanaHealthResponse = client.execute_json(req).await?;

    Ok(GrafanaHealth {
        database: response.database,
        version: response.version,
    })
}

pub fn link(
    request: GrafanaLinkRequest,
    base_url: &str,
    datasource_uid: Option<&str>,
    default_dashboard_uid: Option<&str>,
) -> Result<GrafanaLinkResult, GrafanaError> {
    if request.target != "dashboard" && request.target != "explore" {
        return Err(GrafanaError::Validation("target must be dashboard or explore".into()));
    }

    let base = Url::parse(base_url).map_err(|e| GrafanaError::Validation(e.to_string()))?;
    
    let url = if request.target == "dashboard" {
        let dash_uid = default_dashboard_uid.ok_or_else(|| GrafanaError::Configuration("missing default_dashboard_uid".into()))?;
        let mut u = base.join(&format!("/d/{}", dash_uid)).map_err(|e| GrafanaError::Validation(e.to_string()))?;
        u.query_pairs_mut()
            .append_pair("from", &request.start.timestamp_millis().to_string())
            .append_pair("to", &request.end.timestamp_millis().to_string())
            .append_pair("var-query", &request.query); // Adjust variable name if needed
        u
    } else {
        // Explore
        let ds_uid = datasource_uid.ok_or_else(|| GrafanaError::Configuration("missing datasource_uid".into()))?;
        let mut u = base.join("/explore").map_err(|e| GrafanaError::Validation(e.to_string()))?;
        
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
        
        u.query_pairs_mut().append_pair("left", &left_pane.to_string());
        u
    };

    Ok(GrafanaLinkResult {
        url: url.to_string(),
    })
}
