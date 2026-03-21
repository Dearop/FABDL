/// Dispatches an `IntentRouterOutput` to the appropriate pipeline variant.
use crate::{
    error::AnalysisError,
    quant::QuantModel,
    types::{intent::{IntentAction, IntentRouterOutput}, quant::PortfolioRiskSummary},
    xrpl::XrplClient,
};

use super::{analyze_risk, check_position};

pub async fn dispatch(
    intent: &IntentRouterOutput,
    xrpl: &dyn XrplClient,
    quant: &dyn QuantModel,
) -> Result<PortfolioRiskSummary, AnalysisError> {
    match intent.action {
        IntentAction::AnalyzeRisk => analyze_risk::run(intent, xrpl, quant).await,
        IntentAction::CheckPosition => check_position::run(intent, xrpl, quant).await,
        IntentAction::GetPrice => {
            // Price-only query: return a skeleton summary with just the price filled in.
            let price = xrpl.xrp_usd_price().await?;
            Ok(PortfolioRiskSummary::empty(price))
        }
        IntentAction::ExecuteStrategy => {
            // Execution is handled by the contract/adapter layer, not the analysis backend.
            Err(AnalysisError::XrplRpc(
                "ExecuteStrategy must be submitted to the contract layer, not the analysis backend"
                    .to_string(),
            ))
        }
    }
}
