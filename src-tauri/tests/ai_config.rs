use std::sync::Arc;

use thalassa_ai::{ModelDescriptor, ProviderKind};
use thalassaops::ai::config::{ProviderConfigError, ProviderConfigStore, ProviderConfiguration};
use thalassaops::connectors::{CredentialStore, InMemoryCredentialStore, SharedCredentialStore};

fn configuration(id: &str, kind: ProviderKind) -> ProviderConfiguration {
    ProviderConfiguration {
        id: id.into(),
        kind,
        endpoint: format!("https://{id}.example.test/v1/chat/completions"),
        models: vec![ModelDescriptor::new(
            format!("{id}-model"),
            8_192,
            1_024,
            true,
        )],
    }
}

fn store() -> (Arc<InMemoryCredentialStore>, ProviderConfigStore) {
    let credentials = Arc::new(InMemoryCredentialStore::default());
    let shared: SharedCredentialStore = credentials.clone();
    (credentials, ProviderConfigStore::new(shared))
}

#[test]
fn configuring_a_provider_stores_a_secret_but_the_read_model_only_reports_its_presence() {
    let (credentials, mut providers) = store();

    let summary = providers
        .configure_provider(
            configuration("openai", ProviderKind::OpenAiCompatible),
            Some("test-key-not-real"),
        )
        .unwrap();

    assert!(credentials.has("provider/openai").unwrap());
    assert!(summary.credential_configured);
    let serialized = serde_json::to_string(&summary).unwrap();
    assert!(!serialized.contains("test-key-not-real"));
}

#[test]
fn reconfiguring_without_a_new_secret_keeps_the_existing_secret() {
    let (credentials, mut providers) = store();

    providers
        .configure_provider(
            configuration("openai", ProviderKind::OpenAiCompatible),
            Some("test-key-not-real"),
        )
        .unwrap();
    providers
        .configure_provider(
            configuration("openai", ProviderKind::OpenAiCompatible),
            None,
        )
        .unwrap();

    assert_eq!(
        credentials.get("provider/openai").unwrap().as_deref(),
        Some("test-key-not-real")
    );
}

#[test]
fn removing_a_provider_deletes_its_secret_and_configuration() {
    let (credentials, mut providers) = store();

    providers
        .configure_provider(
            configuration("openai", ProviderKind::OpenAiCompatible),
            Some("test-key-not-real"),
        )
        .unwrap();
    providers
        .set_provider_order(["openai"])
        .expect("configured provider can be ordered");

    providers.remove_provider("openai").unwrap();

    assert!(!credentials.has("provider/openai").unwrap());
    assert!(providers.providers().is_empty());
    assert!(providers.provider_order().is_empty());
}

#[test]
fn provider_order_is_empty_until_the_operator_chooses_it() {
    let (_, providers) = store();

    assert!(providers.provider_order().is_empty());
}

#[test]
fn provider_order_rejects_unknown_and_duplicate_ids_without_changing_the_order() {
    let (_, mut providers) = store();
    providers
        .configure_provider(
            configuration("openai", ProviderKind::OpenAiCompatible),
            Some("test-key-not-real"),
        )
        .unwrap();
    providers
        .configure_provider(configuration("ollama", ProviderKind::Ollama), None)
        .unwrap();

    assert!(matches!(
        providers.set_provider_order(["missing"]),
        Err(ProviderConfigError::UnknownProviderInOrder { provider_id })
            if provider_id == "missing"
    ));
    assert!(providers.provider_order().is_empty());

    assert!(matches!(
        providers.set_provider_order(["openai", "openai"]),
        Err(ProviderConfigError::DuplicateProviderInOrder { provider_id })
            if provider_id == "openai"
    ));
    assert!(providers.provider_order().is_empty());
}
