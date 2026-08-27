use super::{CloudAuthError, CloudCredentialProvider};
use azure_core::credentials::TokenCredential;
use azure_identity::{AzureCliCredential, AzureCliCredentialOptions};
use std::sync::Arc;

pub struct AzureCredentialProvider {
    credential: Arc<AzureCliCredential>,
    tenant_id: String,
}

impl AzureCredentialProvider {
    pub fn new(tenant_id: impl Into<String>) -> Result<Self, CloudAuthError> {
        let tenant_id = tenant_id.into();
        let credential = AzureCliCredential::new(Some(AzureCliCredentialOptions {
            tenant_id: Some(tenant_id.clone()),
            ..Default::default()
        }))
        .map_err(|_| CloudAuthError::Failed)?;
        Ok(Self {
            credential,
            tenant_id,
        })
    }

    fn login_command(&self) -> String {
        format!("az login --tenant {}", self.tenant_id)
    }
}

#[async_trait::async_trait]
impl CloudCredentialProvider for AzureCredentialProvider {
    async fn authorize(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::RequestBuilder, CloudAuthError> {
        let token = self
            .credential
            .get_token(&["https://management.azure.com/.default"], None)
            .await
            .map_err(|_| CloudAuthError::NoCredential {
                login_command: self.login_command(),
            })?;
        Ok(request.bearer_auth(token.token.secret()))
    }
}
