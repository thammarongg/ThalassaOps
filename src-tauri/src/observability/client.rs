use reqwest::{Client, Method, RequestBuilder, Url};
use serde::de::DeserializeOwned;
use std::time::Duration;
use crate::connectors::{ConnectorError, ConnectorSummary, CredentialStore};
use crate::observability::{ObservabilityAuthMode, ObservabilityConnectorConfig};

#[derive(Debug, thiserror::Error)]
pub enum ObservabilityClientError {
    #[error("invalid base URL: {0}")]
    InvalidUrl(String),
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("connector error: {0}")]
    Connector(#[from] ConnectorError),
    #[error("request failed")]
    RequestFailed,
    #[error("provider error: {0}")]
    ProviderError(u16),
    #[error("response format error")]
    MalformedResponse,
}

pub struct ObservabilityClient {
    client: Client,
    base_url: Url,
    auth_mode: ObservabilityAuthMode,
    username: Option<String>,
    credential: Option<String>,
}

impl ObservabilityClient {
    pub fn new(
        connector: &ConnectorSummary,
        store: &dyn CredentialStore,
    ) -> Result<Self, ObservabilityClientError> {
        let config: ObservabilityConnectorConfig = serde_json::from_value(connector.config_metadata.clone())
            .map_err(|e| ObservabilityClientError::Configuration(e.to_string()))?;
        
        let base_url = Url::parse(&config.base_url)
            .map_err(|e| ObservabilityClientError::InvalidUrl(e.to_string()))?;
            
        let credential = if config.auth_mode != ObservabilityAuthMode::None && connector.credential_configured {
            store.get(&format!("connector/{}", connector.id)).map_err(ObservabilityClientError::Connector)?
        } else {
            None
        };

        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| ObservabilityClientError::Configuration("failed to build http client".into()))?;

        Ok(Self {
            client,
            base_url,
            auth_mode: config.auth_mode,
            username: config.username,
            credential,
        })
    }

    pub fn build_url(&self, path: &str) -> Result<Url, ObservabilityClientError> {
        let path = path.trim_start_matches('/');
        let mut url = self.base_url.clone();
        url.set_path(&format!("{}/{}", url.path().trim_end_matches('/'), path));
        Ok(url)
    }

    pub fn prepare_get(&self, url: Url) -> Result<RequestBuilder, ObservabilityClientError> {
        let mut builder = self.client.request(Method::GET, url);
        match self.auth_mode {
            ObservabilityAuthMode::None => {}
            ObservabilityAuthMode::Bearer => {
                if let Some(token) = &self.credential {
                    builder = builder.bearer_auth(token);
                } else {
                    return Err(ObservabilityClientError::Configuration("missing bearer token".into()));
                }
            }
            ObservabilityAuthMode::Basic => {
                if let (Some(username), Some(password)) = (&self.username, &self.credential) {
                    builder = builder.basic_auth(username, Some(password));
                } else {
                    return Err(ObservabilityClientError::Configuration("missing basic auth credentials".into()));
                }
            }
        }
        Ok(builder)
    }

    pub async fn execute_json<T: DeserializeOwned>(&self, request: RequestBuilder) -> Result<T, ObservabilityClientError> {
        let response = request.send().await.map_err(|_| ObservabilityClientError::RequestFailed)?;
        
        let status = response.status();
        if !status.is_success() {
            return Err(ObservabilityClientError::ProviderError(status.as_u16()));
        }

        response.json::<T>().await.map_err(|_| ObservabilityClientError::MalformedResponse)
    }
    
    pub async fn execute_empty(&self, request: RequestBuilder) -> Result<(), ObservabilityClientError> {
        let response = request.send().await.map_err(|_| ObservabilityClientError::RequestFailed)?;
        
        let status = response.status();
        if !status.is_success() {
            return Err(ObservabilityClientError::ProviderError(status.as_u16()));
        }

        Ok(())
    }
}
