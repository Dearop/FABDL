/// Single-position deep-dive pipeline.
///
/// Requires both `wallet_address` and `pool` to be set in the intent parameters.
use crate::{
    error::AnalysisError,
    quant::QuantModel,
    types::{intent::IntentRouterOutput, quant::PortfolioRiskSummary},
    xrpl::XrplClient,
};

const REPLAY_WINDOW_SECS: u64 = 7 * 24 * 3600;

pub async fn run(
    intent: &IntentRouterOutput,
    xrpl: &dyn XrplClient,
    quant: &dyn QuantModel,
) -> Result<PortfolioRiskSummary, AnalysisError> {
    let wallet = intent
        .parameters
        .wallet_address
        .as_deref()
        .ok_or(AnalysisError::MissingParameter("wallet_address"))?;

    let pool_label = intent
        .parameters
        .pool
        .as_deref()
        .ok_or(AnalysisError::MissingParameter("pool"))?;

    let price_history = xrpl.price_history(90).await.unwrap_or_default();

    let mut snapshot = xrpl.fetch_pool_snapshot(wallet, pool_label).await?;
    snapshot.price_history = price_history.clone();

    quant.compute_portfolio_risk(&[snapshot], &price_history, REPLAY_WINDOW_SECS)
}
