use std::collections::{BTreeMap, HashSet};
use std::{fmt, sync::Arc};

use thalassa_domain::{ModelCapabilityRequirement, ModelDescriptor, ModelSelector};

use crate::{ModelProvider, ProviderManifest};

#[derive(Clone)]
pub struct ProviderSelection {
    pub provider_id: String,
    pub model: ModelDescriptor,
    pub provider: Arc<dyn ModelProvider>,
}

impl fmt::Debug for ProviderSelection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderSelection")
            .field("provider_id", &self.provider_id)
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum RegistryError {
    #[error("provider is already registered: {provider_id}")]
    DuplicateProvider { provider_id: String },
    #[error("provider is not registered: {provider_id}")]
    ProviderNotFound { provider_id: String },
    #[error("model {model_id} is not on provider {provider_id}")]
    ModelNotFound {
        provider_id: String,
        model_id: String,
    },
    #[error("no provider satisfies the requested model capability")]
    NoMatchingProvider,
    #[error("provider order contains an unknown provider: {provider_id}")]
    UnknownProviderInOrder { provider_id: String },
}

pub struct ProviderRegistry {
    providers: BTreeMap<String, Arc<dyn ModelProvider>>,
    declared_order: Vec<String>,
    provider_order: Vec<String>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: BTreeMap::new(),
            declared_order: Vec::new(),
            provider_order: Vec::new(),
        }
    }

    pub fn register<P>(&mut self, provider: P) -> Result<(), RegistryError>
    where
        P: ModelProvider + 'static,
    {
        let provider = Arc::new(provider);
        let manifest = provider.manifest();
        let provider_id = manifest.id.clone();
        if self.providers.contains_key(&provider_id) {
            return Err(RegistryError::DuplicateProvider { provider_id });
        }
        self.declared_order.push(provider_id.clone());
        self.providers.insert(provider_id, provider);
        Ok(())
    }

    pub fn register_arc(&mut self, provider: Arc<dyn ModelProvider>) -> Result<(), RegistryError> {
        let manifest = provider.manifest();
        let provider_id = manifest.id.clone();
        if self.providers.contains_key(&provider_id) {
            return Err(RegistryError::DuplicateProvider { provider_id });
        }
        self.declared_order.push(provider_id.clone());
        self.providers.insert(provider_id, provider);
        Ok(())
    }

    pub fn select(&self, selector: &ModelSelector) -> Result<ProviderSelection, RegistryError> {
        match selector {
            ModelSelector::Explicit {
                provider_id,
                model_id,
            } => {
                let provider = self.providers.get(provider_id).cloned().ok_or_else(|| {
                    RegistryError::ProviderNotFound {
                        provider_id: provider_id.clone(),
                    }
                })?;
                let model = provider
                    .manifest()
                    .model(model_id)
                    .cloned()
                    .ok_or_else(|| RegistryError::ModelNotFound {
                        provider_id: provider_id.clone(),
                        model_id: model_id.clone(),
                    })?;
                Ok(ProviderSelection {
                    provider_id: provider_id.clone(),
                    model,
                    provider,
                })
            }
            ModelSelector::Capability(requirement) => self
                .providers_for_capability(requirement)
                .into_iter()
                .next()
                .ok_or(RegistryError::NoMatchingProvider),
        }
    }

    pub fn providers_for_capability(
        &self,
        requirement: &ModelCapabilityRequirement,
    ) -> Vec<ProviderSelection> {
        self.declared_order
            .iter()
            .filter_map(|provider_id| {
                let provider = self.providers.get(provider_id)?.clone();
                let model = provider
                    .manifest()
                    .models
                    .iter()
                    .find(|model| model.satisfies(requirement))?
                    .clone();
                Some(ProviderSelection {
                    provider_id: provider_id.clone(),
                    model,
                    provider,
                })
            })
            .collect()
    }

    pub fn provider(&self, provider_id: &str) -> Option<Arc<dyn ModelProvider>> {
        self.providers.get(provider_id).cloned()
    }

    pub fn declared_provider_ids(&self) -> &[String] {
        &self.declared_order
    }

    pub fn set_provider_order<I, S>(&mut self, provider_ids: I) -> Result<(), RegistryError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let provider_order: Vec<String> = provider_ids.into_iter().map(Into::into).collect();
        let mut seen = HashSet::new();
        for provider_id in &provider_order {
            if !self.providers.contains_key(provider_id) {
                return Err(RegistryError::UnknownProviderInOrder {
                    provider_id: provider_id.clone(),
                });
            }
            if !seen.insert(provider_id) {
                return Err(RegistryError::DuplicateProvider {
                    provider_id: provider_id.clone(),
                });
            }
        }
        self.provider_order = provider_order;
        Ok(())
    }

    pub fn provider_order(&self) -> &[String] {
        &self.provider_order
    }

    pub fn fallback_selections(
        &self,
        requirement: &ModelCapabilityRequirement,
    ) -> Vec<ProviderSelection> {
        self.provider_order
            .iter()
            .filter_map(|provider_id| {
                let provider = self.providers.get(provider_id)?.clone();
                let model = provider
                    .manifest()
                    .models
                    .iter()
                    .find(|model| model.satisfies(requirement))?
                    .clone();
                Some(ProviderSelection {
                    provider_id: provider_id.clone(),
                    model,
                    provider,
                })
            })
            .collect()
    }

    pub fn manifest(&self, provider_id: &str) -> Option<&ProviderManifest> {
        self.providers
            .get(provider_id)
            .map(|provider| provider.manifest())
    }
}
