/// Full portfolio risk analysis pipeline.
///
/// Steps:
/// 1. Validate intent - `wallet_address` is required.
/// 2. Fetch all trust lines for the wallet.
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

/// Well-known XRPL AMM pool labels. In production these would be discovered
/// dynamically from the ledger; for the MVP we filter against this allowlist.
const KNOWN_POOLS: &[&str] = &["XRP/USD", "XRP/BTC", "XRP/USDC", "XRP/USDT"];

fn is_account_not_found_error(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("account not found")
        || normalized.contains("actnotfound")
        || normalized.contains("account_not_found")
}

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
    let mut analysis_warnings = Vec::new();

    tracing::info!(wallet, "step 1: fetching account lines");
    let lines = match xrpl.account_lines(wallet).await {
        Ok(lines) => {
            tracing::info!(wallet, count = lines.lines.len(), "account lines fetched");
            lines
        }
        Err(AnalysisError::XrplRpc(ref msg)) if is_account_not_found_error(msg) => {
            tracing::warn!(
                wallet,
                error = %msg,
                "account not found on current network - treating as empty portfolio"
            );
            tracing::info!(wallet, "wallet missing on current network");
            analysis_warnings.push(
                "Wallet was not found on the current XRPL network. This can mean the wallet is brand new, unfunded, or connected to a different network than the backend."
                    .to_string(),
            );
            crate::types::xrpl::AccountLinesResponse {
                lines: vec![],
                marker: None,
            }
        }
        Err(AnalysisError::XrplRpc(msg)) => {
            tracing::error!(
                wallet,
                error = %msg,
                "account_lines failed with non-recoverable XRPL RPC error"
            );
            return Err(AnalysisError::XrplRpc(msg));
        }
        Err(error) => {
            tracing::error!(wallet, error = %error, "account_lines failed");
            return Err(error);
        }
    };

    tracing::info!(wallet, "step 2: fetching XRP/USD price");
    let xrp_price = xrpl.xrp_usd_price().await?;
    tracing::info!(xrp_price, "XRP/USD price fetched");

    tracing::info!(wallet, "step 3: fetching 90-day price history");
    let price_history = xrpl.price_history(90).await.unwrap_or_default();
    tracing::info!(days = price_history.len(), "price history fetched");

    let pool_label_filter: Option<&str> = intent.parameters.pool.as_deref();
    let mut snapshots = Vec::new();

    let pools_to_check: Vec<&str> = if let Some(label) = pool_label_filter {
        vec![label]
    } else {
        KNOWN_POOLS.to_vec()
    };

    tracing::info!(wallet, pools = ?pools_to_check, "step 4: scanning pools for LP positions");

    for pool_label in pools_to_check {
        let should_check_pool = pool_label_filter.is_some() || !lines.lines.is_empty();
        if !should_check_pool {
            continue;
        }

        tracing::info!(pool = pool_label, wallet, "fetching pool snapshot");
        match xrpl.fetch_pool_snapshot(wallet, pool_label).await {
            Ok(mut snapshot) => {
                tracing::info!(
                    pool = pool_label,
                    positions = snapshot.positions.len(),
                    "pool snapshot fetched"
                );
                if !snapshot.positions.is_empty() {
                    snapshot.price_history = price_history.clone();
                    snapshots.push(snapshot);
                }
            }
            Err(error) => {
                tracing::warn!(pool = pool_label, error = %error, "pool snapshot failed - skipping");
            }
        }
    }

    tracing::info!(wallet, snapshots = snapshots.len(), "step 5: running quant model");

    if snapshots.is_empty() {
        tracing::info!(wallet, "no AMM positions found - returning empty portfolio summary");
        return Ok(PortfolioRiskSummary::empty_with_warnings(
            xrp_price,
            analysis_warnings,
        ));
    }

    let mut summary =
        quant.compute_portfolio_risk(&snapshots, &price_history, REPLAY_WINDOW_SECS)?;

    tracing::info!(wallet, "step 6: fetching XLS-66d lending context");

    let mut seen_currencies = std::collections::HashSet::new();
    let mut vault_assets: Vec<String> = Vec::new();
    for snapshot in &snapshots {
        for part in snapshot.pool_label.split('/') {
            if seen_currencies.insert(part.to_string()) {
                vault_assets.push(part.to_string());
            }
        }
    }

    let mut lending_vaults = Vec::new();
    for asset in &vault_assets {
        let vault = xrpl.lending_vault_info(asset).await.unwrap_or_default();
        if vault.total_supply_usd > 0.0 {
            lending_vaults.push(vault);
        }
    }

    let open_loans = xrpl.account_loans(wallet).await.unwrap_or_default();

    if !lending_vaults.is_empty() || !open_loans.is_empty() {
        tracing::info!(
            vaults = lending_vaults.len(),
            loans = open_loans.len(),
            "lending context attached"
        );
    }

    summary.lending_vaults = lending_vaults;
    summary.open_loans = open_loans.clone();

    let total_borrowed: f64 = open_loans.iter().map(|loan| loan.amount_borrowed_usd).sum();
    let weighted_borrow_apy = if total_borrowed > 0.0 {
        open_loans
            .iter()
            .map(|loan| loan.borrow_apy * loan.amount_borrowed_usd)
            .sum::<f64>()
            / total_borrowed
    } else {
        0.0
    };
    summary.net_carry =
        summary.fee_apr - weighted_borrow_apy - summary.impermanent_loss_pct.abs() / 100.0;
    summary.analysis_warnings.extend(analysis_warnings);

    Ok(summary)
}
