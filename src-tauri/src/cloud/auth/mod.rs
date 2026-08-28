mod aws;
mod azure;
mod gcp;

pub use aws::AwsCredentialProvider;
pub use azure::AzureCredentialProvider;
pub use gcp::GcpCredentialProvider;

#[derive(Debug, thiserror::Error)]
pub enum CloudAuthError {
    /// No credential could be resolved at all: no SSO cache, no az login
    /// session, no application default credentials.
    #[error("no credential available")]
    NoCredential { login_command: String },
    /// A credential was resolved but the provider refused it.
    #[error("credential rejected")]
    Rejected { login_command: String },
    /// Signing or token exchange failed for a reason that is not the
    /// operator's to fix.
    #[error("credential resolution failed")]
    Failed,
}

#[async_trait::async_trait]
pub trait CloudCredentialProvider: Send + Sync {
    async fn authorize(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::RequestBuilder, CloudAuthError>;
}

pub struct FakeCredentialProvider {
    outcome: Result<String, CloudAuthError>,
}

impl FakeCredentialProvider {
    pub fn authorized(header: &str) -> Self {
        Self {
            outcome: Ok(header.to_string()),
        }
    }

    pub fn no_credential() -> Self {
        Self {
            outcome: Err(CloudAuthError::NoCredential {
                login_command: "aws sso login --profile test".into(),
            }),
        }
    }
}

#[async_trait::async_trait]
impl CloudCredentialProvider for FakeCredentialProvider {
    async fn authorize(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::RequestBuilder, CloudAuthError> {
        match &self.outcome {
            Ok(header) => Ok(request.header("authorization", header)),
            Err(error) => Err(match error {
                CloudAuthError::NoCredential { login_command } => CloudAuthError::NoCredential {
                    login_command: login_command.clone(),
                },
                CloudAuthError::Rejected { login_command } => CloudAuthError::Rejected {
                    login_command: login_command.clone(),
                },
                CloudAuthError::Failed => CloudAuthError::Failed,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_provider_authorizes_and_can_report_a_missing_credential() {
        let client = reqwest::Client::new();

        let ok = FakeCredentialProvider::authorized("Bearer test-token");
        let request = ok
            .authorize(client.get("http://example.test/x"))
            .await
            .expect("authorized");
        let built = request.build().unwrap();
        assert_eq!(built.headers()["authorization"], "Bearer test-token");

        let missing = FakeCredentialProvider::no_credential();
        let error = missing
            .authorize(client.get("http://example.test/x"))
            .await
            .expect_err("must fail");
        assert!(matches!(error, CloudAuthError::NoCredential { .. }));
    }
}
