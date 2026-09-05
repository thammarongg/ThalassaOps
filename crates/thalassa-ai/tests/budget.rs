use thalassa_ai::budget::{BudgetBound, BudgetLedger, WindowBudget};
use thalassa_domain::{
    FailoverPermission, ModelBudget, ModelDescriptor, ModelMessage, ModelRequest, ModelRole,
    ModelSelector, ModelUsage,
};
use uuid::Uuid;

fn request(content: &str) -> ModelRequest {
    ModelRequest {
        request_id: Uuid::from_u128(2),
        instruction: None,
        messages: vec![ModelMessage {
            role: ModelRole::User,
            content: content.into(),
        }],
        data_class: "public".into(),
        budget: ModelBudget {
            max_input_tokens: None,
            max_output_tokens: 32,
            max_cost_micros: None,
        },
        timeout_ms: 1_000,
        model: ModelSelector::Capability(Default::default()),
        failover: FailoverPermission::Forbidden,
    }
}

fn model() -> ModelDescriptor {
    ModelDescriptor::new("model", 8_192, 1_024, true)
}

fn priced_model() -> ModelDescriptor {
    model().with_pricing(2_000, 4_000)
}

#[test]
fn input_estimate_refuses_before_a_provider_request_is_prepared() {
    let mut request = request(&"long input ".repeat(32));
    request.budget.max_input_tokens = Some(1);

    let refusal = BudgetLedger::new().prepare(&request, &model()).unwrap_err();
    assert_eq!(refusal.bound, BudgetBound::MaxInputTokens);
}

#[test]
fn output_budget_is_passed_to_the_provider_request() {
    let mut request = request("short input");
    request.budget.max_output_tokens = 77;

    let prepared = BudgetLedger::new().prepare(&request, &model()).unwrap();
    assert_eq!(prepared.provider_request.max_output_tokens, 77);
}

#[test]
fn cost_budget_refuses_a_model_without_published_pricing() {
    let mut request = request("priced input");
    request.budget.max_cost_micros = Some(100);

    let refusal = BudgetLedger::new().prepare(&request, &model()).unwrap_err();
    assert_eq!(refusal.bound, BudgetBound::UnpricedCost);
}

#[test]
fn reported_usage_is_recorded_and_window_usage_uses_the_reported_amount() {
    let mut ledger = BudgetLedger::with_window(WindowBudget::new(Some(10_000), None, None));
    let usage = ModelUsage {
        input_tokens: 900,
        output_tokens: 40,
        cost_micros: Some(12),
    };

    ledger.record_usage(usage);

    assert_eq!(ledger.recorded_usage(), usage);
    assert_eq!(ledger.window_usage(), usage);
}

#[test]
fn window_budget_admits_under_limit_and_refuses_a_request_that_would_cross_it() {
    let mut ledger = BudgetLedger::with_window(WindowBudget::new(Some(100), None, None));
    ledger.record_usage(ModelUsage {
        input_tokens: 80,
        output_tokens: 0,
        cost_micros: None,
    });

    assert!(ledger.prepare(&request("ok"), &model()).is_ok());

    let refusal = ledger
        .prepare(&request(&"x".repeat(400)), &model())
        .unwrap_err();
    assert_eq!(refusal.bound, BudgetBound::WindowInputTokens);
}

#[test]
fn priced_model_can_be_checked_against_a_cost_budget() {
    let mut request = request("priced input");
    request.budget.max_cost_micros = Some(1);

    let refusal = BudgetLedger::new()
        .prepare(&request, &priced_model())
        .unwrap_err();
    assert_eq!(refusal.bound, BudgetBound::MaxCostMicros);
}

#[test]
fn zero_published_price_is_distinct_from_missing_pricing() {
    let mut request = request("free input");
    request.budget.max_cost_micros = Some(0);

    let prepared = BudgetLedger::new()
        .prepare(&request, &model().with_pricing(0, 0))
        .unwrap();
    assert_eq!(prepared.estimated_cost_micros, Some(0));
}
