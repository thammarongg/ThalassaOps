use super::{CloudAuthError, CloudCredentialProvider};
use aws_credential_types::{provider::ProvideCredentials, Credentials};
use aws_sigv4::http_request::{sign, SignableBody, SignableRequest, SigningSettings};
use std::time::SystemTime;

#[allow(deprecated)]
use aws_config::profile::profile_file::ProfileFiles;

pub struct AwsCredentialProvider {
    profile: String,
    region: String,
}

impl AwsCredentialProvider {
    pub fn new(profile: impl Into<String>, region: impl Into<String>) -> Self {
        Self {
            profile: profile.into(),
            region: region.into(),
        }
    }

    fn login_command(&self) -> String {
        format!("aws sso login --profile {}", self.profile)
    }
}

#[async_trait::async_trait]
impl CloudCredentialProvider for AwsCredentialProvider {
    async fn authorize(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::RequestBuilder, CloudAuthError> {
        // AWS CLI profiles can contain both static keys in the shared credentials file and a
        // `login_session` in the config file. The SDK gives login_session precedence, which can
        // hide valid static keys behind an expired console session. Resolve the credentials file
        // first through aws-config (never by reading it here), then fall back to the complete
        // SSO-enabled chain so other configured sources continue to work.
        #[allow(deprecated)]
        let credentials_files = ProfileFiles::builder()
            .include_default_credentials_file(true)
            .build();
        let static_provider = aws_config::profile::ProfileFileCredentialsProvider::builder()
            .profile_files(credentials_files)
            .profile_name(&self.profile)
            .build();
        let credentials = match static_provider.provide_credentials().await {
            Ok(credentials) => credentials,
            Err(_) => {
                #[allow(deprecated)]
                let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::v2023_11_09())
                    .profile_name(&self.profile)
                    .region(aws_config::Region::new(self.region.clone()))
                    .load()
                    .await;
                let provider = sdk_config.credentials_provider().ok_or_else(|| {
                    CloudAuthError::NoCredential {
                        login_command: self.login_command(),
                    }
                })?;
                provider
                    .provide_credentials()
                    .await
                    .map_err(|_| CloudAuthError::NoCredential {
                        login_command: self.login_command(),
                    })?
            }
        };

        let identity = Credentials::new(
            credentials.access_key_id(),
            credentials.secret_access_key(),
            credentials.session_token().map(str::to_owned),
            None,
            "thalassaops",
        )
        .into();
        let request_for_signing = request
            .try_clone()
            .ok_or(CloudAuthError::Failed)?
            .build()
            .map_err(|_| CloudAuthError::Failed)?;
        let headers = request_for_signing
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_owned(), value.to_owned()))
            })
            .collect::<Vec<_>>();
        let signable_request = SignableRequest::new(
            request_for_signing.method().as_str(),
            request_for_signing.url().as_str(),
            headers
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str())),
            SignableBody::Bytes(&[]),
        )
        .map_err(|_| CloudAuthError::Failed)?;
        let service = request_for_signing
            .url()
            .host_str()
            .and_then(|host| host.split('.').next())
            .filter(|service| matches!(*service, "eks" | "ec2"))
            .unwrap_or("eks");
        let signing_params = aws_sigv4::sign::v4::SigningParams::builder()
            .identity(&identity)
            .region(&self.region)
            .name(service)
            .time(SystemTime::now())
            .settings(SigningSettings::default())
            .build()
            .map_err(|_| CloudAuthError::Failed)?
            .into();
        let (instructions, _) = sign(signable_request, &signing_params)
            .map_err(|_| CloudAuthError::Failed)?
            .into_parts();
        let (signed_headers, signed_params) = instructions.into_parts();
        let mut authorized = request;
        for header in signed_headers {
            authorized = authorized.header(header.name(), header.value());
        }
        for (name, value) in signed_params {
            authorized = authorized.query(&[(name, value.as_ref())]);
        }
        Ok(authorized)
    }
}
