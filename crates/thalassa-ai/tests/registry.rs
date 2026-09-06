use std::time::Instant;

use thalassa_ai::{
    ModelProvider, ProviderError, ProviderManifest, ProviderRegistry, ProviderRequest,
    ProviderResponse, RegistryError,
};
use thalassa_domain::{
    ModelCapabilityRequirement, ModelDescriptor, ProviderErrorReason, ProviderHealth, ProviderKind,
};

struct FakeProvider {
    manifest: ProviderManifest,
}

impl FakeProvider {
    fn new(manifest: ProviderManifest) -> Self {
        Self { manifest }
    }
}

impl ModelProvider for FakeProvider {
    fn manifest(&self) -> &ProviderManifest {
        &self.manifest
    }

    fn complete(
        &self,
        _request: &ProviderRequest,
        _deadline: Instant,
    ) -> Result<ProviderResponse, ProviderError> {
        Err(ProviderError::new(
            ProviderErrorReason::Unreachable,
            "fake provider",
        ))
    }

    fn probe(&self, _deadline: Instant) -> Result<ProviderHealth, ProviderError> {
        Ok(ProviderHealth::Healthy)
    }
}

fn manifest(id: &str, model_id: &str, system_instruction: bool) -> ProviderManifest {
    ProviderManifest::new(id, ProviderKind::OpenAiCompatible).with_model(ModelDescriptor::new(
        model_id,
        8_192,
        1_024,
        system_instruction,
    ))
}

#[test]
fn registry_selects_an_explicit_provider_and_reports_a_missing_model() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(FakeProvider::new(manifest("alpha", "alpha-model", true)))
        .unwrap();

    let selection = registry
        .select(&thalassa_domain::ModelSelector::Explicit {
            provider_id: "alpha".into(),
            model_id: "alpha-model".into(),
        })
        .unwrap();
    assert_eq!(selection.provider_id, "alpha");
    assert_eq!(selection.model.id, "alpha-model");

    let error = registry
        .select(&thalassa_domain::ModelSelector::Explicit {
            provider_id: "alpha".into(),
            model_id: "not-configured".into(),
        })
        .unwrap_err();
    assert!(matches!(
        error,
        RegistryError::ModelNotFound {
            provider_id,
            model_id
        } if provider_id == "alpha" && model_id == "not-configured"
    ));
}

#[test]
fn capability_selection_preserves_declared_registration_order() {
    let mut registry = ProviderRegistry::new();
    for id in ["first", "second", "third"] {
        registry
            .register(FakeProvider::new(manifest(
                id,
                &format!("{id}-model"),
                true,
            )))
            .unwrap();
    }

    let requirement = ModelCapabilityRequirement::default();
    let selections = registry.providers_for_capability(&requirement);
    let provider_ids: Vec<_> = selections
        .iter()
        .map(|selection| selection.provider_id.as_str())
        .collect();
    assert_eq!(provider_ids, ["first", "second", "third"]);
}

#[test]
fn provider_without_a_required_capability_is_not_selected() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(FakeProvider::new(manifest("no-system", "model", false)))
        .unwrap();

    let requirement = ModelCapabilityRequirement {
        requires_system_instruction: true,
        ..Default::default()
    };
    assert!(registry.providers_for_capability(&requirement).is_empty());
    assert!(matches!(
        registry.select(&thalassa_domain::ModelSelector::Capability(requirement)),
        Err(RegistryError::NoMatchingProvider)
    ));
}

#[test]
fn registry_rejects_duplicate_provider_ids() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(FakeProvider::new(manifest("duplicate", "one", true)))
        .unwrap();

    let error = registry
        .register(FakeProvider::new(manifest("duplicate", "two", true)))
        .unwrap_err();
    assert!(
        matches!(error, RegistryError::DuplicateProvider { provider_id } if provider_id == "duplicate")
    );
}
