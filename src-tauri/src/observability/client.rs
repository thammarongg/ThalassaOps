use crate::connectors::{ConnectorError, ConnectorSummary, CredentialStore};
use crate::observability::{
    ObservabilityAuthMode, ObservabilityConnectorConfig, LOKI_CONNECTOR_KIND, TEMPO_CONNECTOR_KIND,
};
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
    tenant_id: Option<String>,
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
            .validate(credential.as_deref())
            .map_err(ObservabilityClientError::Configuration)?;

        let base_url = Url::parse(&config.base_url)
            .map_err(|e| ObservabilityClientError::InvalidUrl(e.to_string()))?;

        let tenant_id = match connector.kind.as_str() {
            LOKI_CONNECTOR_KIND | TEMPO_CONNECTOR_KIND => config.tenant_id,
            _ => None,
        };

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
            tenant_id,
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
        if let Some(tenant_id) = &self.tenant_id {
            builder = builder.header("X-Scope-OrgID", tenant_id);
        }
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

    fn tenant_connector(base_url: &str, kind: &str, tenant_id: Option<&str>) -> ConnectorSummary {
        ConnectorSummary {
            id: "tenant-test".into(),
            kind: kind.into(),
            display_name: "Tenant test".into(),
            enabled: true,
            config_metadata: json!({
                "base_url": base_url,
                "auth_mode": "none",
                "tenant_id": tenant_id,
            }),
            credential_configured: false,
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

    #[tokio::test]
    async fn sends_scope_org_id_when_tenant_is_configured() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("GET")
                .path("/ready")
                .header("X-Scope-OrgID", "team-a");
            then.status(200).body("ready");
        });
        let connector = tenant_connector(&server.url(""), "loki", Some("team-a"));
        let client =
            ObservabilityClient::new(&connector, &InMemoryCredentialStore::default()).unwrap();
        let request = client
            .prepare_get(client.build_url("/ready").unwrap())
            .unwrap();
        client.execute_empty(request).await.unwrap();
        mock.assert();
    }

    #[tokio::test]
    async fn omits_scope_org_id_for_non_tenant_kinds_and_when_absent() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("GET").path("/ready").matches(|req| {
                !req.headers
                    .iter()
                    .flatten()
                    .any(|(name, _)| name.eq_ignore_ascii_case("x-scope-orgid"))
            });
            then.status(200).body("ready");
        });
        let connector = tenant_connector(&server.url(""), "prometheus", Some("team-a"));
        let client =
            ObservabilityClient::new(&connector, &InMemoryCredentialStore::default()).unwrap();
        let request = client
            .prepare_get(client.build_url("/ready").unwrap())
            .unwrap();
        client.execute_empty(request).await.unwrap();
        mock.assert();
    }

    #[tokio::test]
    async fn sends_scope_org_id_for_tempo_when_tenant_is_configured() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("GET")
                .path("/ready")
                .header("X-Scope-OrgID", "team-a");
            then.status(200).body("ready");
        });
        let connector = tenant_connector(&server.url(""), "tempo", Some("team-a"));
        let client =
            ObservabilityClient::new(&connector, &InMemoryCredentialStore::default()).unwrap();
        let request = client
            .prepare_get(client.build_url("/ready").unwrap())
            .unwrap();
        client.execute_empty(request).await.unwrap();
        mock.assert();
    }

    #[tokio::test]
    async fn omits_scope_org_id_for_tempo_when_tenant_is_absent() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("GET").path("/ready").matches(|req| {
                !req.headers
                    .iter()
                    .flatten()
                    .any(|(name, _)| name.eq_ignore_ascii_case("x-scope-orgid"))
            });
            then.status(200).body("ready");
        });
        let connector = tenant_connector(&server.url(""), "tempo", None);
        let client =
            ObservabilityClient::new(&connector, &InMemoryCredentialStore::default()).unwrap();
        let request = client
            .prepare_get(client.build_url("/ready").unwrap())
            .unwrap();
        client.execute_empty(request).await.unwrap();
        mock.assert();
    }
}
