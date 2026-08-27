use super::auth::{CloudAuthError, CloudCredentialProvider};
use reqwest::{Client, Url};
use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum CloudClientError {
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error(transparent)]
    Auth(#[from] CloudAuthError),
    #[error("request failed")]
    RequestFailed,
    #[error("provider error: {0}")]
    ProviderError(u16),
    #[error("response format error")]
    MalformedResponse,
}

pub struct CloudClient {
    client: Client,
    provider: Arc<dyn CloudCredentialProvider>,
}

impl CloudClient {
    pub fn new(provider: Arc<dyn CloudCredentialProvider>) -> Result<Self, CloudClientError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| CloudClientError::Configuration("failed to build http client".into()))?;

        Ok(Self { client, provider })
    }

    pub async fn get_json<T: DeserializeOwned>(&self, url: Url) -> Result<T, CloudClientError> {
        let request = self.provider.authorize(self.client.get(url)).await?;
        let response = request
            .send()
            .await
            .map_err(|_| CloudClientError::RequestFailed)?;

        let status = response.status();
        if !status.is_success() {
            return Err(CloudClientError::ProviderError(status.as_u16()));
        }

        response
            .json::<T>()
            .await
            .map_err(|_| CloudClientError::MalformedResponse)
    }

    pub async fn get_paginated<T, F>(&self, first: Url, next: F) -> Result<Vec<T>, CloudClientError>
    where
        F: Fn(&serde_json::Value) -> Option<(Vec<T>, Option<Url>)>,
    {
        const MAX_PAGES: usize = 50;
        let mut url = Some(first);
        let mut pages = 0;
        let mut resources = Vec::new();

        while let Some(current) = url.take() {
            if pages == MAX_PAGES {
                break;
            }
            pages += 1;

            let body: serde_json::Value = self.get_json(current).await?;
            let (mut page_resources, next_url) =
                next(&body).ok_or(CloudClientError::MalformedResponse)?;
            resources.append(&mut page_resources);
            url = next_url;
        }

        Ok(resources)
    }
}

#[cfg(test)]
mod tests {
    use crate::cloud::{CloudAuthError, CloudClient, CloudClientError, FakeCredentialProvider};
    use httpmock::MockServer;
    use reqwest::Url;
    use serde_json::json;
    use std::sync::Arc;

    #[tokio::test]
    async fn get_json_sends_the_authorization_from_the_provider() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("GET")
                .path("/things")
                .header("authorization", "Bearer t");
            then.status(200).json_body(json!({ "value": 1 }));
        });
        let client =
            CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
        let body: serde_json::Value = client
            .get_json(Url::parse(&server.url("/things")).unwrap())
            .await
            .unwrap();
        assert_eq!(body["value"], json!(1));
        mock.assert();
    }

    #[tokio::test]
    async fn a_missing_credential_surfaces_before_any_request_is_sent() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("GET").path("/things");
            then.status(200).json_body(json!({}));
        });
        let client = CloudClient::new(Arc::new(FakeCredentialProvider::no_credential())).unwrap();
        let error = client
            .get_json::<serde_json::Value>(Url::parse(&server.url("/things")).unwrap())
            .await
            .expect_err("must fail");
        assert!(matches!(
            error,
            CloudClientError::Auth(CloudAuthError::NoCredential { .. })
        ));
        assert_eq!(
            mock.hits(),
            0,
            "no request may be sent without a credential"
        );
    }

    #[tokio::test]
    async fn provider_errors_carry_only_a_status_code_and_never_the_body() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method("GET").path("/things");
            then.status(403)
                .body("AccessDenied: user arn:aws:iam::123:user/secret is not authorized");
        });
        let client =
            CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
        let error = client
            .get_json::<serde_json::Value>(Url::parse(&server.url("/things")).unwrap())
            .await
            .expect_err("must fail");
        assert!(matches!(error, CloudClientError::ProviderError(403)));
        let rendered = format!("{error}");
        assert!(
            !rendered.contains("arn:aws:iam"),
            "response body must not leak: {rendered}"
        );
        assert!(
            !rendered.contains("secret"),
            "response body must not leak: {rendered}"
        );
    }

    #[tokio::test]
    async fn redirects_are_not_followed() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method("GET").path("/start");
            then.status(302).header("location", "/elsewhere");
        });
        let followed = server.mock(|when, then| {
            when.method("GET").path("/elsewhere");
            then.status(200).json_body(json!({}));
        });
        let client =
            CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
        let _ = client
            .get_json::<serde_json::Value>(Url::parse(&server.url("/start")).unwrap())
            .await;
        assert_eq!(followed.hits(), 0);
    }

    #[tokio::test]
    async fn get_paginated_follows_pages_until_the_next_link_is_absent() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method("GET").path("/items").query_param("page", "1");
            then.status(200)
                .json_body(json!({ "items": [1, 2], "next": "2" }));
        });
        server.mock(|when, then| {
            when.method("GET").path("/items").query_param("page", "2");
            then.status(200)
                .json_body(json!({ "items": [3], "next": null }));
        });
        let client =
            CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
        let first = Url::parse(&server.url("/items?page=1")).unwrap();
        let base = server.url("/items");
        let all: Vec<i64> = client
            .get_paginated(first, |body: &serde_json::Value| {
                let items = body["items"]
                    .as_array()?
                    .iter()
                    .filter_map(|v| v.as_i64())
                    .collect();
                let next = body["next"]
                    .as_str()
                    .and_then(|token| Url::parse(&format!("{base}?page={token}")).ok());
                Some((items, next))
            })
            .await
            .unwrap();
        assert_eq!(all, vec![1, 2, 3]);
    }
}
