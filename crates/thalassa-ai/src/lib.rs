// SPDX-License-Identifier: Apache-2.0

//! Provider-neutral model contracts and runtime selection.

use serde::{Deserialize, Serialize};
use std::time::Instant;
use thiserror::Error;
use uuid::Uuid;

pub mod budget;
pub mod gateway;
pub mod registry;

pub use budget::{
    estimate_input_tokens, BudgetBound, BudgetLedger, BudgetRefusal, PreparedRequest, WindowBudget,
};
pub use gateway::{CancellationToken, Gateway, GatewayError};
pub use registry::{ProviderRegistry, ProviderSelection, RegistryError};
pub use thalassa_domain::{
    ContentDeclaration, FailoverPermission, ModelAttempt, ModelAttemptOutcome, ModelBudget,
    ModelCapabilityRequirement, ModelDataClass, ModelDescriptor, ModelFinishReason, ModelMessage,
    ModelRequest, ModelRequestError, ModelResponse, ModelRole, ModelSelector, ModelUsage,
    ProviderErrorReason, ProviderHealth, ProviderKind,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProviderManifest {
    pub id: String,
    pub kind: ProviderKind,
    pub models: Vec<ModelDescriptor>,
}

impl ProviderManifest {
    pub fn new(id: impl Into<String>, kind: ProviderKind) -> Self {
        Self {
            id: id.into(),
            kind,
            models: Vec::new(),
        }
    }

    pub fn with_model(mut self, model: ModelDescriptor) -> Self {
        self.models.push(model);
        self
    }

    pub fn model(&self, model_id: &str) -> Option<&ModelDescriptor> {
        self.models.iter().find(|model| model.id == model_id)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProviderRequest {
    pub request_id: Uuid,
    pub model_id: String,
    pub instruction: Option<String>,
    pub messages: Vec<ModelMessage>,
    pub max_output_tokens: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProviderResponse {
    pub content: String,
    pub usage: ModelUsage,
    pub finish: ModelFinishReason,
}

#[derive(Clone, Debug, Eq, PartialEq, Error)]
#[error("provider error ({reason:?}): {message}")]
pub struct ProviderError {
    pub reason: ProviderErrorReason,
    pub message: String,
}

impl ProviderError {
    pub fn new(reason: ProviderErrorReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }
}

pub trait ModelProvider: Send + Sync {
    fn manifest(&self) -> &ProviderManifest;
    fn complete(
        &self,
        request: &ProviderRequest,
        deadline: Instant,
    ) -> Result<ProviderResponse, ProviderError>;
    fn probe(&self, deadline: Instant) -> Result<ProviderHealth, ProviderError>;
}
