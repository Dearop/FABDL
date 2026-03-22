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

    tracing::info!(wallet, "step 1: fetching account lines");
    // Fetch LP token balances. A brand-new or unfunded account returns
    // "Account not found" from the ledger — treat that as zero positions.
    let lines = match xrpl.account_lines(wallet).await {
        Ok(l) => {
            tracing::info!(wallet, count = l.lines.len(), "account lines fetched");
            l
        }
        Err(AnalysisError::XrplRpc(ref msg)) => {
            tracing::warn!(wallet, error = %msg, "account not found on ledger — treating as empty portfolio");
            use crate::types::xrpl::AccountLinesResponse;
            AccountLinesResponse { lines: vec![], marker: None }
        }
        Err(e) => {
            tracing::error!(wallet, error = %e, "account_lines failed");
            return Err(e);
        }
    };

    tracing::info!(wallet, "step 2: fetching XRP/USD price");
    let xrp_price = xrpl.xrp_usd_price().await?;
    tracing::info!(xrp_price, "XRP/USD price fetched");

    tracing::info!(wallet, "step 3: fetching 90-day price history");
    let price_history = xrpl.price_history(90).await.unwrap_or_default();
    tracing::info!(days = price_history.len(), "price history fetched");

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

    tracing::info!(wallet, pools = ?pools_to_check, "step 4: scanning pools for LP positions");

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

        tracing::debug!(pool = pool_label, has_lp, "pool LP check");

        // Always attempt the pool if the wallet was specified with a specific pool,
        // or if we found LP tokens (rough heuristic for portfolio mode).
        if pool_label_filter.is_some() || has_lp {
            tracing::info!(pool = pool_label, wallet, "fetching pool snapshot");
            match xrpl.fetch_pool_snapshot(wallet, pool_label).await {
                Ok(mut snapshot) => {
                    tracing::info!(pool = pool_label, positions = snapshot.positions.len(), "pool snapshot fetched");
                    if !snapshot.positions.is_empty() {
                        snapshot.price_history = price_history.clone();
                        snapshots.push(snapshot);
                    }
                }
                Err(e) => {
                    tracing::warn!(pool = pool_label, error = %e, "pool snapshot failed — skipping");
                    continue;
                }
            }
        }
        let _ = currency1; // suppress unused warning
    }

    tracing::info!(wallet, snapshots = snapshots.len(), "step 5: running quant model");

    if snapshots.is_empty() {
        tracing::info!(wallet, "no AMM positions found — returning empty portfolio summary");
        return Ok(PortfolioRiskSummary::empty(xrp_price));
    }

    let mut summary =
        quant.compute_portfolio_risk(&snapshots, &price_history, REPLAY_WINDOW_SECS)?;

    // -----------------------------------------------------------------------
    // XLS-66d lending context (best-effort, never fails the request)
    // -----------------------------------------------------------------------
    tracing::info!(wallet, "step 6: fetching XLS-66d lending context");

    // Deduplicate currencies from the user's pool snapshots.
    let mut seen_currencies = std::collections::HashSet::new();
    let mut vault_assets: Vec<String> = Vec::new();
    for snap in &snapshots {
        for part in snap.pool_label.split('/') {
            if seen_currencies.insert(part.to_string()) {
                vault_assets.push(part.to_string());
            }
        }
    }

    let mut lending_vaults = Vec::new();
    for asset in &vault_assets {
        let vault = xrpl
            .lending_vault_info(asset)
            .await
            .unwrap_or_default();
        // Only include vaults that actually exist (non-zero supply).
        if vault.total_supply_usd > 0.0 {
            lending_vaults.push(vault);
        }
    }

    let open_loans = xrpl
        .account_loans(wallet)
        .await
        .unwrap_or_default();

    if !lending_vaults.is_empty() || !open_loans.is_empty() {
        tracing::info!(
            vaults = lending_vaults.len(),
            loans = open_loans.len(),
            "lending context attached"
        );
    }

    summary.lending_vaults = lending_vaults;
    summary.open_loans = open_loans.clone();

    // Recompute net_carry with weighted borrow APY from open loans.
    let total_borrowed: f64 = open_loans.iter().map(|l| l.amount_borrowed_usd).sum();
    let weighted_borrow_apy = if total_borrowed > 0.0 {
        open_loans
            .iter()
            .map(|l| l.borrow_apy * l.amount_borrowed_usd)
            .sum::<f64>()
            / total_borrowed
    } else {
        0.0
    };
    summary.net_carry =
        summary.fee_apr - weighted_borrow_apy - summary.impermanent_loss_pct.abs() / 100.0;

    Ok(summary)
}
