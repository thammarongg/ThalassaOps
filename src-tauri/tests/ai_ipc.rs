use std::sync::Arc;

use serde_json::{json, Value};
use tempfile::tempdir;
use thalassa_ai::ModelDescriptor;
use thalassa_domain::{
    ContentDeclaration, FailoverPermission, ModelBudget, ModelMessage, ModelRequest, ModelRole,
    ModelSelector,
};
use thalassa_ipc::{Capability, CommandEnvelope, CommandName, IpcErrorCode};
use thalassaops::app::{AppState, IpcResult};
use thalassaops::connectors::{InMemoryCredentialStore, SharedCredentialStore};
use uuid::Uuid;

fn state() -> (tempfile::TempDir, AppState) {
    let directory = tempdir().unwrap();
    let credentials: SharedCredentialStore = Arc::new(InMemoryCredentialStore::default());
    let state = AppState::open_with_credential_store(
        directory.path().join("thalassaops.sqlite"),
        credentials,
    )
    .unwrap();
    (directory, state)
}

fn envelope(
    state: &AppState,
    verb: &str,
    capability: Capability,
    payload: Value,
) -> CommandEnvelope<Value> {
    CommandEnvelope {
        request_id: Uuid::new_v4(),
        command: CommandName::new("ai", verb).unwrap(),
        capability,
        scope: state.bootstrap.scope.clone(),
        payload,
    }
}

fn model_request(data_class: &str, max_input_tokens: Option<u64>) -> Value {
    serde_json::to_value(ModelRequest {
        request_id: Uuid::new_v4(),
        instruction: None,
        messages: vec![ModelMessage {
            role: ModelRole::User,
            content: "public test question".into(),
        }],
        data_class: data_class.into(),
        declaration: ContentDeclaration::OperatorDeclared,
        budget: ModelBudget {
            max_input_tokens,
            max_output_tokens: 1,
            max_cost_micros: None,
        },
        timeout_ms: 1_000,
        model: ModelSelector::Explicit {
            provider_id: "openai".into(),
            model_id: "model".into(),
        },
        failover: FailoverPermission::Forbidden,
    })
    .unwrap()
}

fn provider_configuration() -> Value {
    json!({
        "id": "openai",
        "kind": "open_ai_compatible",
        "endpoint": "https://provider.example.test/v1/chat/completions",
        "models": [serde_json::to_value(ModelDescriptor::new("model", 8_192, 1_024, true)).unwrap()],
        "credential": "test-key-not-real"
    })
}

fn assert_invalid(result: IpcResult<Value>) {
    let IpcResult::Err { error, .. } = result else {
        panic!("expected invalid request");
    };
    assert_eq!(error.code, IpcErrorCode::InvalidRequest);
    assert_eq!(error.details["reason"], "ai_invalid_payload");
}

#[test]
fn every_ai_command_rejects_unknown_payload_keys() {
    let (_directory, state) = state();
    assert_invalid(
        state
            .ai_providers(envelope(
                &state,
                "providers",
                Capability::ConnectorRead,
                json!({"unexpected": true}),
            ))
            .map_value(),
    );
    assert_invalid(
        state
            .ai_configure_provider(envelope(
                &state,
                "configure_provider",
                Capability::ConnectorAct,
                json!({"unexpected": true}),
            ))
            .map_value(),
    );
    assert_invalid(
        state
            .ai_set_provider_order(envelope(
                &state,
                "set_provider_order",
                Capability::ConnectorAct,
                json!({"unexpected": true}),
            ))
            .map_value(),
    );
    assert_invalid(
        state
            .ai_probe(envelope(
                &state,
                "probe",
                Capability::ConnectorRead,
                json!({"unexpected": true}),
            ))
            .map_value(),
    );
    assert_invalid(
        state
            .ai_complete(envelope(&state, "complete", Capability::AiInvoke, {
                let mut request = model_request("public", None);
                request
                    .as_object_mut()
                    .unwrap()
                    .insert("unexpected".into(), json!(true));
                request
            }))
            .map_value(),
    );
    assert_invalid(
        state
            .ai_cancel(envelope(
                &state,
                "cancel",
                Capability::AiInvoke,
                json!({"request_id": Uuid::new_v4(), "unexpected": true}),
            ))
            .map_value(),
    );
}

#[test]
fn complete_with_the_wrong_capability_names_only_the_required_command() {
    let (_directory, state) = state();
    let result = state.ai_complete(envelope(
        &state,
        "complete",
        Capability::ConnectorRead,
        model_request("public", None),
    ));

    let IpcResult::Err { error, .. } = result else {
        panic!("expected permission denial");
    };
    assert_eq!(error.code, IpcErrorCode::PermissionDenied);
    assert_eq!(error.details["required_command"], "ai.complete");
    assert!(error.details.get("payload").is_none());
}

#[test]
fn policy_denial_exposes_its_typed_reason() {
    let (_directory, state) = state();
    let configured = state.ai_configure_provider(envelope(
        &state,
        "configure_provider",
        Capability::ConnectorAct,
        provider_configuration(),
    ));
    assert!(matches!(configured, IpcResult::Ok { .. }));

    let result = state.ai_complete(envelope(
        &state,
        "complete",
        Capability::AiInvoke,
        model_request("restricted", None),
    ));
    let IpcResult::Err { error, .. } = result else {
        panic!("expected policy denial");
    };
    assert_eq!(error.code, IpcErrorCode::PolicyDenied);
    assert_eq!(error.details["reason"], "ImmutableRestrictedData");
}

#[test]
fn budget_refusal_has_a_distinct_typed_reason() {
    let (_directory, state) = state();
    let configured = state.ai_configure_provider(envelope(
        &state,
        "configure_provider",
        Capability::ConnectorAct,
        provider_configuration(),
    ));
    assert!(matches!(configured, IpcResult::Ok { .. }));

    let result = state.ai_complete(envelope(
        &state,
        "complete",
        Capability::AiInvoke,
        model_request("public", Some(1)),
    ));
    let IpcResult::Err { error, .. } = result else {
        panic!("expected budget refusal");
    };
    assert_eq!(error.code, IpcErrorCode::InvalidRequest);
    assert_eq!(error.details["reason"], "budget_refused");
    assert_eq!(error.details["bound"], "max_input_tokens");
}

#[test]
fn cancelling_an_unknown_request_is_a_typed_not_found_error() {
    let (_directory, state) = state();
    let result = state.ai_cancel(envelope(
        &state,
        "cancel",
        Capability::AiInvoke,
        json!({"request_id": Uuid::new_v4()}),
    ));

    let IpcResult::Err { error, .. } = result else {
        panic!("expected not found error");
    };
    assert_eq!(error.code, IpcErrorCode::NotFound);
    assert_eq!(error.details["reason"], "ai_request_not_found");
}

#[test]
fn ai_command_names_use_snake_case_for_tauri_and_dotted_names_for_envelopes() {
    assert_eq!(thalassaops::app::AI_COMPLETE_TAURI_COMMAND, "ai_complete");
    assert_eq!(
        thalassaops::app::AI_COMPLETE_ENVELOPE_COMMAND,
        "ai.complete"
    );
    assert_eq!(
        CommandName::new("ai", "complete").unwrap().to_string(),
        "ai.complete"
    );
}

trait MapValue {
    fn map_value(self) -> IpcResult<Value>;
}

impl<T: serde::Serialize> MapValue for IpcResult<T> {
    fn map_value(self) -> IpcResult<Value> {
        match self {
            IpcResult::Ok { ok, value } => IpcResult::Ok {
                ok,
                value: serde_json::to_value(value).unwrap(),
            },
            IpcResult::Err { ok, error } => IpcResult::Err { ok, error },
        }
    }
}
