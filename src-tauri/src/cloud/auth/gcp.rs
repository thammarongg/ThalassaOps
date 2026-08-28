use super::{CloudAuthError, CloudCredentialProvider};
use gcp_auth::{ConfigDefaultCredentials, MetadataServiceAccount, TokenProvider};
use std::env;
use std::sync::Arc;
use tokio::sync::OnceCell;

pub struct GcpCredentialProvider {
    provider: OnceCell<Arc<dyn TokenProvider>>,
}

impl GcpCredentialProvider {
    pub fn new() -> Self {
        Self {
            provider: OnceCell::const_new(),
        }
    }

    async fn provider(&self) -> Result<Arc<dyn TokenProvider>, CloudAuthError> {
        self.provider
            .get_or_try_init(|| async { resolve_adc_provider().await })
            .await
            .cloned()
    }
}

impl Default for GcpCredentialProvider {
    fn default() -> Self {
        Self::new()
    }
}

async fn resolve_adc_provider() -> Result<Arc<dyn TokenProvider>, CloudAuthError> {
    if env::var_os("GOOGLE_APPLICATION_CREDENTIALS").is_some_and(|path| !path.is_empty()) {
        return match gcp_auth::CustomServiceAccount::from_env() {
            Ok(Some(provider)) => Ok(Arc::new(provider)),
            Ok(None) | Err(_) => Err(no_credential()),
        };
    }

    if let Ok(provider) = ConfigDefaultCredentials::new().await {
        return Ok(Arc::new(provider));
    }
    if let Ok(provider) = MetadataServiceAccount::new().await {
        return Ok(Arc::new(provider));
    }
    Err(no_credential())
}

fn no_credential() -> CloudAuthError {
    CloudAuthError::NoCredential {
        login_command: "gcloud auth application-default login".into(),
    }
}

#[async_trait::async_trait]
impl CloudCredentialProvider for GcpCredentialProvider {
    async fn authorize(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::RequestBuilder, CloudAuthError> {
        let provider = self.provider().await?;
        let token = provider
            .token(&["https://www.googleapis.com/auth/cloud-platform"])
            .await
            .map_err(|_| no_credential())?;
        Ok(request.bearer_auth(token.as_str()))
    }
}
