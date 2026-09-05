use thalassa_domain::{ModelDescriptor, ModelRequest, ModelUsage};

use crate::ProviderRequest;

const MICROS_PER_MILLION_TOKENS: u128 = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetBound {
    MaxInputTokens,
    MaxOutputTokens,
    MaxCostMicros,
    WindowInputTokens,
    WindowOutputTokens,
    WindowCostMicros,
    UnpricedCost,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("budget refused at {bound:?}: requested {requested}, limit {limit:?}")]
pub struct BudgetRefusal {
    pub bound: BudgetBound,
    pub requested: u64,
    pub limit: Option<u64>,
}

impl BudgetRefusal {
    fn new(bound: BudgetBound, requested: u64, limit: Option<u64>) -> Self {
        Self {
            bound,
            requested,
            limit,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WindowBudget {
    pub max_input_tokens: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub max_cost_micros: Option<u64>,
}

impl WindowBudget {
    pub fn new(
        max_input_tokens: Option<u64>,
        max_output_tokens: Option<u64>,
        max_cost_micros: Option<u64>,
    ) -> Self {
        Self {
            max_input_tokens,
            max_output_tokens,
            max_cost_micros,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct BudgetLedger {
    window: Option<WindowBudget>,
    usage: ModelUsage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedRequest {
    pub provider_request: ProviderRequest,
    pub estimated_input_tokens: u64,
    pub estimated_cost_micros: Option<u64>,
}

impl BudgetLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_window(window: WindowBudget) -> Self {
        Self {
            window: Some(window),
            usage: ModelUsage::default(),
        }
    }

    pub fn prepare(
        &self,
        request: &ModelRequest,
        model: &ModelDescriptor,
    ) -> Result<PreparedRequest, BudgetRefusal> {
        let estimated_input_tokens = estimate_input_tokens(request);
        if let Some(limit) = request.budget.max_input_tokens {
            if estimated_input_tokens > limit {
                return Err(BudgetRefusal::new(
                    BudgetBound::MaxInputTokens,
                    estimated_input_tokens,
                    Some(limit),
                ));
            }
        }

        if request.budget.max_output_tokens == 0
            || request.budget.max_output_tokens > model.max_output_tokens
        {
            return Err(BudgetRefusal::new(
                BudgetBound::MaxOutputTokens,
                request.budget.max_output_tokens,
                Some(model.max_output_tokens),
            ));
        }

        let estimated_cost_micros = estimate_cost_micros(
            model,
            estimated_input_tokens,
            request.budget.max_output_tokens,
        );
        if let Some(limit) = request.budget.max_cost_micros {
            let estimated_cost = estimated_cost_micros
                .ok_or_else(|| BudgetRefusal::new(BudgetBound::UnpricedCost, 0, Some(limit)))?;
            if estimated_cost > limit {
                return Err(BudgetRefusal::new(
                    BudgetBound::MaxCostMicros,
                    estimated_cost,
                    Some(limit),
                ));
            }
        }

        if let Some(window) = &self.window {
            let input_total = self
                .usage
                .input_tokens
                .saturating_add(estimated_input_tokens);
            if let Some(limit) = window.max_input_tokens {
                if input_total > limit {
                    return Err(BudgetRefusal::new(
                        BudgetBound::WindowInputTokens,
                        input_total,
                        Some(limit),
                    ));
                }
            }

            let output_total = self
                .usage
                .output_tokens
                .saturating_add(request.budget.max_output_tokens);
            if let Some(limit) = window.max_output_tokens {
                if output_total > limit {
                    return Err(BudgetRefusal::new(
                        BudgetBound::WindowOutputTokens,
                        output_total,
                        Some(limit),
                    ));
                }
            }

            if let Some(limit) = window.max_cost_micros {
                let estimated_cost = estimated_cost_micros
                    .ok_or_else(|| BudgetRefusal::new(BudgetBound::UnpricedCost, 0, Some(limit)))?;
                let current_cost = self
                    .usage
                    .cost_micros
                    .ok_or_else(|| BudgetRefusal::new(BudgetBound::UnpricedCost, 0, Some(limit)))?;
                let cost_total = current_cost.saturating_add(estimated_cost);
                if cost_total > limit {
                    return Err(BudgetRefusal::new(
                        BudgetBound::WindowCostMicros,
                        cost_total,
                        Some(limit),
                    ));
                }
            }
        }

        Ok(PreparedRequest {
            provider_request: ProviderRequest {
                request_id: request.request_id,
                model_id: model.id.clone(),
                instruction: request.instruction.clone(),
                messages: request.messages.clone(),
                max_output_tokens: request.budget.max_output_tokens,
            },
            estimated_input_tokens,
            estimated_cost_micros,
        })
    }

    pub fn record_usage(&mut self, usage: ModelUsage) {
        self.usage.input_tokens = self.usage.input_tokens.saturating_add(usage.input_tokens);
        self.usage.output_tokens = self.usage.output_tokens.saturating_add(usage.output_tokens);
        self.usage.cost_micros = add_optional(self.usage.cost_micros, usage.cost_micros);
    }

    pub fn recorded_usage(&self) -> ModelUsage {
        self.usage
    }

    pub fn window_usage(&self) -> ModelUsage {
        self.usage
    }
}

/// Estimates input tokens conservatively from Unicode scalar values. Provider
/// tokenizers are deliberately outside this provider-neutral crate.
pub fn estimate_input_tokens(request: &ModelRequest) -> u64 {
    let instruction_chars: usize = request
        .instruction
        .as_deref()
        .map_or(0, |instruction| instruction.chars().count());
    let message_chars: usize = request
        .messages
        .iter()
        .map(|message| message.content.chars().count())
        .sum();
    let characters = instruction_chars.saturating_add(message_chars);
    let content_tokens = characters.saturating_add(3) / 4;
    content_tokens.saturating_add(request.messages.len()).max(1) as u64
}

fn estimate_cost_micros(
    model: &ModelDescriptor,
    input_tokens: u64,
    output_tokens: u64,
) -> Option<u64> {
    let input_price = model.input_cost_micros_per_million_tokens?;
    let output_price = model.output_cost_micros_per_million_tokens?;
    Some(
        cost_for_tokens(input_tokens, input_price)
            .saturating_add(cost_for_tokens(output_tokens, output_price)),
    )
}

fn cost_for_tokens(tokens: u64, price_micros_per_million: u64) -> u64 {
    let amount = (u128::from(tokens) * u128::from(price_micros_per_million))
        .saturating_add(MICROS_PER_MILLION_TOKENS - 1)
        / MICROS_PER_MILLION_TOKENS;
    amount.min(u128::from(u64::MAX)) as u64
}

fn add_optional(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.saturating_add(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}
