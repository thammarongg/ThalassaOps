use crate::connectors::{ConnectorError, ConnectorSummary, CredentialStore};
use crate::observability::{ObservabilityAuthMode, ObservabilityConnectorConfig};
use reqwest::{Client, Method, RequestBuilder, Url};
use serde::de::DeserializeOwned;
use std::time::Duration;

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
        let config: ObservabilityConnectorConfig =
            serde_json::from_value(connector.config_metadata.clone())
                .map_err(|e| ObservabilityClientError::Configuration(e.to_string()))?;

        let credential =
            if config.auth_mode != ObservabilityAuthMode::None && connector.credential_configured {
                store
                    .get(&format!("connector/{}", connector.id))
                    .map_err(ObservabilityClientError::Connector)?
            } else {
                None
            };

        config
            .validate(credential.is_some())
            .map_err(ObservabilityClientError::Configuration)?;

        let base_url = Url::parse(&config.base_url)
            .map_err(|e| ObservabilityClientError::InvalidUrl(e.to_string()))?;

        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| {
                ObservabilityClientError::Configuration("failed to build http client".into())
            })?;

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
                    return Err(ObservabilityClientError::Configuration(
                        "missing bearer token".into(),
                    ));
                }
            }
            ObservabilityAuthMode::Basic => {
                if let (Some(username), Some(password)) = (&self.username, &self.credential) {
                    builder = builder.basic_auth(username, Some(password));
                } else {
                    return Err(ObservabilityClientError::Configuration(
                        "missing basic auth credentials".into(),
                    ));
                }
            }
        }
        Ok(builder)
    }

    pub async fn execute_json<T: DeserializeOwned>(
        &self,
        request: RequestBuilder,
    ) -> Result<T, ObservabilityClientError> {
        let response = request
            .send()
            .await
            .map_err(|_| ObservabilityClientError::RequestFailed)?;

        let status = response.status();
        if !status.is_success() {
            return Err(ObservabilityClientError::ProviderError(status.as_u16()));
        }

        response
            .json::<T>()
            .await
            .map_err(|_| ObservabilityClientError::MalformedResponse)
    }

    pub async fn execute_empty(
        &self,
        request: RequestBuilder,
    ) -> Result<(), ObservabilityClientError> {
        let response = request
            .send()
            .await
            .map_err(|_| ObservabilityClientError::RequestFailed)?;

        let status = response.status();
        if !status.is_success() {
            return Err(ObservabilityClientError::ProviderError(status.as_u16()));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectors::InMemoryCredentialStore;
    use httpmock::MockServer;
    use serde_json::json;

    fn test_connector(base_url: &str, auth_mode: &str) -> ConnectorSummary {
        ConnectorSummary {
            id: "test".into(),
            kind: "prometheus".into(),
            display_name: "Test".into(),
            enabled: true,
            config_metadata: json!({
                "base_url": base_url,
                "auth_mode": auth_mode,
                "username": if auth_mode == "basic" { Some("user") } else { None }
            }),
            credential_configured: auth_mode != "none",
            health_state: "healthy".into(),
            last_checked_at: None,
            last_successful_sync_at: None,
        }
    }

    #[tokio::test]
    async fn test_none_auth_and_get_only() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("GET").path("/test");
            then.status(200).body("{}");
        });

        let connector = test_connector(&server.url(""), "none");
        let store = InMemoryCredentialStore::default();
        let client = ObservabilityClient::new(&connector, &store).unwrap();

        let req = client
            .prepare_get(client.build_url("/test").unwrap())
            .unwrap();
        let _: serde_json::Value = client.execute_json(req).await.unwrap();

        mock.assert();
    }

    #[tokio::test]
    async fn test_bearer_auth() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("GET")
                .path("/test")
                .header("Authorization", "Bearer my-token");
            then.status(200).body("{}");
        });

        let connector = test_connector(&server.url(""), "bearer");
        let store = InMemoryCredentialStore::default();
        store.set("connector/test", "my-token").unwrap();
        let client = ObservabilityClient::new(&connector, &store).unwrap();

        let req = client
            .prepare_get(client.build_url("/test").unwrap())
            .unwrap();
        let _: serde_json::Value = client.execute_json(req).await.unwrap();

        mock.assert();
    }

    #[tokio::test]
    async fn test_basic_auth() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("GET")
                .path("/test")
                .header("Authorization", "Basic dXNlcjpteS1wYXNz"); // user:my-pass
            then.status(200).body("{}");
        });

        let connector = test_connector(&server.url(""), "basic");
        let store = InMemoryCredentialStore::default();
        store.set("connector/test", "my-pass").unwrap();
        let client = ObservabilityClient::new(&connector, &store).unwrap();

        let req = client
            .prepare_get(client.build_url("/test").unwrap())
            .unwrap();
        let _: serde_json::Value = client.execute_json(req).await.unwrap();

        mock.assert();
    }

    #[tokio::test]
    async fn test_no_redirects() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("GET").path("/redirect");
            then.status(301).header("Location", "/test");
        });

        let connector = test_connector(&server.url(""), "none");
        let store = InMemoryCredentialStore::default();
        let client = ObservabilityClient::new(&connector, &store).unwrap();

        let req = client
            .prepare_get(client.build_url("/redirect").unwrap())
            .unwrap();
        let err = client
            .execute_json::<serde_json::Value>(req)
            .await
            .unwrap_err();

        match err {
            ObservabilityClientError::ProviderError(301) => {}
            _ => panic!(
                "Expected ProviderError(301) due to disabled redirects, got {:?}",
                err
            ),
        }

        mock.assert();
    }
}
