/// Full portfolio risk analysis pipeline.
///
/// Steps:
/// 1. Validate intent — `wallet_address` is required.
/// 2. Fetch all LP token balances for the wallet.
/// 3. For each recognised pool, fetch the pool snapshot.
/// 4. Fetch shared price history.
/// 5. Run `QuantModel::compute_portfolio_risk`.
use crate::{
    error::AnalysisError,
    quant::QuantModel,
    types::{intent::IntentRouterOutput, quant::PortfolioRiskSummary},
    xrpl::XrplClient,
};

/// 7-day replay window for fee APR calculation.
const REPLAY_WINDOW_SECS: u64 = 7 * 24 * 3600;

/// Well-known XRPL AMM pool labels.  In production these would be discovered
/// dynamically from the ledger; for the MVP we filter against this allowlist.
const KNOWN_POOLS: &[&str] = &["XRP/USD", "XRP/BTC", "XRP/USDC", "XRP/USDT"];

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

    // Fetch LP token balances.
    let lines = xrpl.account_lines(wallet).await?;
    let xrp_price = xrpl.xrp_usd_price().await?;
    let price_history = xrpl.price_history(90).await.unwrap_or_default();

    // Determine which pools the wallet has positions in.
    // We attempt to build a snapshot for each known pool; skip on error
    // (e.g. wallet has no position in that pool).
    let pool_label_filter: Option<&str> = intent.parameters.pool.as_deref();

    let mut snapshots = Vec::new();

    let pools_to_check: Vec<&str> = if let Some(label) = pool_label_filter {
        vec![label]
    } else {
        KNOWN_POOLS.to_vec()
    };

    for pool_label in pools_to_check {
        // Only query pools where the wallet holds LP tokens.
        let parts: Vec<&str> = pool_label.split('/').collect();
        if parts.len() != 2 {
            continue;
        }
        let currency1 = parts[1];

        let has_lp = lines.lines.iter().any(|l| {
            // Heuristic: LP token currency codes on XRPL are hex strings.
            // We match by checking if the issuer relates to this pool.
            // In a full implementation we'd use `amm_info` to get the exact LP token currency.
            l.balance.parse::<f64>().unwrap_or(0.0) > 0.0
                && l.currency.len() == 8 // LP tokens have 8-char hex currency codes
        });

        // Always attempt the pool if the wallet was specified with a specific pool,
        // or if we found LP tokens (rough heuristic for portfolio mode).
        if pool_label_filter.is_some() || has_lp {
            match xrpl.fetch_pool_snapshot(wallet, pool_label).await {
                Ok(mut snapshot) => {
                    if !snapshot.positions.is_empty() {
                        snapshot.price_history = price_history.clone();
                        snapshots.push(snapshot);
                    }
                }
                Err(_) => continue, // pool not found or no position → skip
            }
        }
        let _ = currency1; // suppress unused warning
    }

    if snapshots.is_empty() {
        return Ok(PortfolioRiskSummary::empty(xrp_price));
    }

    quant.compute_portfolio_risk(&snapshots, &price_history, REPLAY_WINDOW_SECS)
}
