use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use thalassa_ai::{ModelDescriptor, ProviderHealth, ProviderKind};
use thiserror::Error;

use crate::connectors::{ConnectorError, SharedCredentialStore};

const PROVIDER_CREDENTIAL_PREFIX: &str = "provider/";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfiguration {
    pub id: String,
    pub kind: ProviderKind,
    pub endpoint: String,
    pub models: Vec<ModelDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProviderSummary {
    pub id: String,
    pub kind: ProviderKind,
    pub endpoint: String,
    pub models: Vec<ModelDescriptor>,
    pub health: Option<ProviderHealth>,
    pub credential_configured: bool,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ProviderConfigError {
    #[error("invalid provider configuration: {reason}")]
    InvalidConfiguration { reason: String },
    #[error("provider is not configured: {provider_id}")]
    ProviderNotFound { provider_id: String },
    #[error("provider order contains an unknown provider: {provider_id}")]
    UnknownProviderInOrder { provider_id: String },
    #[error("provider order contains a duplicate provider: {provider_id}")]
    DuplicateProviderInOrder { provider_id: String },
    #[error("credential store error: {message}")]
    CredentialStore { message: String },
}

impl From<ConnectorError> for ProviderConfigError {
    fn from(error: ConnectorError) -> Self {
        Self::CredentialStore {
            message: error.to_string(),
        }
    }
}

struct ConfiguredProvider {
    configuration: ProviderConfiguration,
    health: Option<ProviderHealth>,
}

pub struct ProviderConfigStore {
    providers: BTreeMap<String, ConfiguredProvider>,
    provider_order: Vec<String>,
    credential_store: SharedCredentialStore,
}

impl ProviderConfigStore {
    pub fn new(credential_store: SharedCredentialStore) -> Self {
        Self {
            providers: BTreeMap::new(),
            provider_order: Vec::new(),
            credential_store,
        }
    }

    pub fn configure_provider(
        &mut self,
        configuration: ProviderConfiguration,
        credential: Option<&str>,
    ) -> Result<ProviderSummary, ProviderConfigError> {
        validate_configuration(&configuration)?;
        if let Some(credential) = credential {
            if credential.trim().is_empty() {
                return Err(ProviderConfigError::InvalidConfiguration {
                    reason: "credential cannot be blank".into(),
                });
            }
            self.credential_store
                .set(&credential_reference(&configuration.id), credential)?;
        }

        let id = configuration.id.clone();
        self.providers.insert(
            id.clone(),
            ConfiguredProvider {
                configuration,
                // A configuration change invalidates the previous probe result.
                health: None,
            },
        );
        self.summary(&id)
    }

    pub fn remove_provider(&mut self, provider_id: &str) -> Result<(), ProviderConfigError> {
        if !self.providers.contains_key(provider_id) {
            return Err(ProviderConfigError::ProviderNotFound {
                provider_id: provider_id.into(),
            });
        }
        self.credential_store
            .delete(&credential_reference(provider_id))?;
        self.providers.remove(provider_id);
        self.provider_order.retain(|id| id != provider_id);
        Ok(())
    }

    pub fn providers(&self) -> Vec<ProviderSummary> {
        self.providers
            .keys()
            .filter_map(|provider_id| self.summary(provider_id).ok())
            .collect()
    }

    pub fn provider(&self, provider_id: &str) -> Option<ProviderSummary> {
        self.summary(provider_id).ok()
    }

    pub fn configuration(&self, provider_id: &str) -> Option<ProviderConfiguration> {
        self.providers
            .get(provider_id)
            .map(|provider| provider.configuration.clone())
    }

    pub fn configurations(&self) -> Vec<ProviderConfiguration> {
        self.providers
            .values()
            .map(|provider| provider.configuration.clone())
            .collect()
    }

    pub fn set_provider_order<I, S>(
        &mut self,
        provider_ids: I,
    ) -> Result<Vec<String>, ProviderConfigError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let provider_order: Vec<String> = provider_ids.into_iter().map(Into::into).collect();
        let mut seen = HashSet::new();
        for provider_id in &provider_order {
            if !self.providers.contains_key(provider_id) {
                return Err(ProviderConfigError::UnknownProviderInOrder {
                    provider_id: provider_id.clone(),
                });
            }
            if !seen.insert(provider_id) {
                return Err(ProviderConfigError::DuplicateProviderInOrder {
                    provider_id: provider_id.clone(),
                });
            }
        }
        self.provider_order = provider_order;
        Ok(self.provider_order.clone())
    }

    pub fn provider_order(&self) -> &[String] {
        &self.provider_order
    }

    pub fn set_health(
        &mut self,
        provider_id: &str,
        health: ProviderHealth,
    ) -> Result<ProviderSummary, ProviderConfigError> {
        let provider = self.providers.get_mut(provider_id).ok_or_else(|| {
            ProviderConfigError::ProviderNotFound {
                provider_id: provider_id.into(),
            }
        })?;
        provider.health = Some(health);
        self.summary(provider_id)
    }

    pub fn credential_store(&self) -> SharedCredentialStore {
        self.credential_store.clone()
    }

    fn summary(&self, provider_id: &str) -> Result<ProviderSummary, ProviderConfigError> {
        let provider = self.providers.get(provider_id).ok_or_else(|| {
            ProviderConfigError::ProviderNotFound {
                provider_id: provider_id.into(),
            }
        })?;
        Ok(ProviderSummary {
            id: provider.configuration.id.clone(),
            kind: provider.configuration.kind,
            endpoint: provider.configuration.endpoint.clone(),
            models: provider.configuration.models.clone(),
            health: provider.health,
            credential_configured: self
                .credential_store
                .has(&credential_reference(provider_id))?,
        })
    }
}

fn credential_reference(provider_id: &str) -> String {
    format!("{PROVIDER_CREDENTIAL_PREFIX}{provider_id}")
}

fn validate_configuration(
    configuration: &ProviderConfiguration,
) -> Result<(), ProviderConfigError> {
    if configuration.id.trim().is_empty() {
        return Err(ProviderConfigError::InvalidConfiguration {
            reason: "provider id is required".into(),
        });
    }
    if configuration.endpoint.trim().is_empty() {
        return Err(ProviderConfigError::InvalidConfiguration {
            reason: "provider endpoint is required".into(),
        });
    }
    if configuration.models.is_empty() {
        return Err(ProviderConfigError::InvalidConfiguration {
            reason: "at least one model is required".into(),
        });
    }
    let mut model_ids = HashSet::new();
    for model in &configuration.models {
        if model.id.trim().is_empty() {
            return Err(ProviderConfigError::InvalidConfiguration {
                reason: "model id is required".into(),
            });
        }
        if !model_ids.insert(&model.id) {
            return Err(ProviderConfigError::InvalidConfiguration {
                reason: format!("duplicate model id: {}", model.id),
            });
        }
    }
    Ok(())
}
