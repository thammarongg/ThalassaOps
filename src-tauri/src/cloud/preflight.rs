use super::{CloudAccessState, CloudAuthError, CloudClientError};

pub fn classify_access(result: &Result<(), CloudClientError>) -> (CloudAccessState, String) {
    match result {
        Ok(()) => (CloudAccessState::Confirmed, String::new()),
        Err(CloudClientError::Auth(CloudAuthError::NoCredential { login_command })) => {
            (CloudAccessState::NoCredential, login_command.clone())
        }
        Err(CloudClientError::Auth(CloudAuthError::Rejected { login_command })) => {
            (CloudAccessState::SessionExpired, login_command.clone())
        }
        Err(CloudClientError::ProviderError(401)) => {
            (CloudAccessState::SessionExpired, String::new())
        }
        Err(CloudClientError::ProviderError(403)) => {
            (CloudAccessState::PermissionDenied, String::new())
        }
        _ => (CloudAccessState::Unavailable, String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::{CloudAccessState, CloudAuthError, CloudClientError};

    #[test]
    fn access_classification_maps_each_failure_to_its_own_remedy() {
        let (state, remedy) =
            classify_access(&Err(CloudClientError::Auth(CloudAuthError::NoCredential {
                login_command: "aws sso login --profile prod".into(),
            })));
        assert_eq!(state, CloudAccessState::NoCredential);
        assert_eq!(remedy, "aws sso login --profile prod");

        let (state, _) = classify_access(&Err(CloudClientError::ProviderError(401)));
        assert_eq!(state, CloudAccessState::SessionExpired);

        let (state, _) = classify_access(&Err(CloudClientError::ProviderError(403)));
        assert_eq!(state, CloudAccessState::PermissionDenied);

        let (state, _) = classify_access(&Err(CloudClientError::ProviderError(500)));
        assert_eq!(state, CloudAccessState::Unavailable);

        let (state, remedy) = classify_access(&Ok(()));
        assert_eq!(state, CloudAccessState::Confirmed);
        assert!(remedy.is_empty());
    }
}
