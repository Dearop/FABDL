/// End-to-end pipeline tests using mock XRPL and quant implementations.
use crate::{
    pipeline::{AnalysisPipeline, DefaultPipeline},
    types::intent::{IntentAction, IntentParameters, IntentRouterOutput, IntentScope},
    xrpl::tests::MockXrplClient,
};

fn make_intent(action: IntentAction, wallet: Option<&str>, pool: Option<&str>) -> IntentRouterOutput {
    IntentRouterOutput {
        action,
        scope: IntentScope::Portfolio,
        parameters: IntentParameters {
            wallet_address: wallet.map(str::to_string),
            pool: pool.map(str::to_string),
            focus: None,
        },
        confidence: Some(0.95),
    }
}

#[tokio::test]
async fn analyze_risk_missing_wallet_returns_error() {
    let pipeline = DefaultPipeline::new(MockXrplClient::default());
    let intent = make_intent(IntentAction::AnalyzeRisk, None, None);
    let result = pipeline.run(intent).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("wallet_address"),
        "expected MissingParameter error, got: {err}"
    );
}

#[tokio::test]
async fn get_price_returns_summary_with_price() {
    let pipeline = DefaultPipeline::new(MockXrplClient { xrp_price: 0.75, ..Default::default() });
    let intent = make_intent(IntentAction::GetPrice, None, None);
    let summary = pipeline.run(intent).await.unwrap();
    assert!((summary.current_xrp_price - 0.75).abs() < 1e-6);
    assert!(summary.positions.is_empty(), "get_price should return empty positions");
}

#[tokio::test]
async fn execute_strategy_returns_error() {
    let pipeline = DefaultPipeline::new(MockXrplClient::default());
    let intent = make_intent(IntentAction::ExecuteStrategy, Some("rWallet"), None);
    let result = pipeline.run(intent).await;
    assert!(result.is_err(), "ExecuteStrategy should be rejected");
}

#[tokio::test]
async fn check_position_missing_pool_returns_error() {
    let pipeline = DefaultPipeline::new(MockXrplClient::default());
    let intent = make_intent(IntentAction::CheckPosition, Some("rWallet"), None);
    let result = pipeline.run(intent).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("pool"), "expected MissingParameter(pool) error, got: {err}");
}

#[tokio::test]
async fn check_position_happy_path() {
    let pipeline = DefaultPipeline::new(MockXrplClient::default());
    let intent = make_intent(IntentAction::CheckPosition, Some("rWallet"), Some("XRP/USD"));
    // MockXrplClient returns a flat price history which gives VaR=0 and
    // Sharpe=∞; the pipeline should still succeed and return a summary.
    let result = pipeline.run(intent).await;
    // The mock positions may or may not be populated depending on LP token
    // filtering logic; we only assert no crash / no unexpected error.
    assert!(result.is_ok(), "check_position happy path should succeed: {:?}", result);
}

#[tokio::test]
async fn analyze_risk_account_not_found_returns_empty_summary_with_warning() {
    let pipeline = DefaultPipeline::new(MockXrplClient {
        account_lines_error: Some("Account not found.".to_string()),
        ..Default::default()
    });
    let intent = make_intent(IntentAction::AnalyzeRisk, Some("rMissingWallet"), None);

    let summary = pipeline.run(intent).await.unwrap();

    assert!(summary.positions.is_empty(), "expected empty positions");
    assert_eq!(summary.total_value_usd, 0.0);
    assert_eq!(summary.analysis_warnings.len(), 1);
    assert!(
        summary.analysis_warnings[0].contains("current XRPL network"),
        "unexpected warning: {:?}",
        summary.analysis_warnings
    );
}

#[tokio::test]
async fn analyze_risk_non_account_rpc_error_propagates() {
    let pipeline = DefaultPipeline::new(MockXrplClient {
        account_lines_error: Some("internalError: upstream timeout".to_string()),
        ..Default::default()
    });
    let intent = make_intent(IntentAction::AnalyzeRisk, Some("rWallet"), None);

    let err = pipeline.run(intent).await.unwrap_err().to_string();
    assert!(
        err.contains("internalError: upstream timeout"),
        "expected original XRPL RPC error, got: {err}"
    );
}
