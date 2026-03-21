/// AMM-specific XRPL queries and normalisation.
use serde_json::json;

use crate::{
    error::AnalysisError,
    types::{
        pool::{PoolSnapshot, PositionSnapshot, TickSnapshot},
        xrpl::{AmmInfoResponse, AmountField},
    },
    xrpl::XrplClient,
};

use super::client::HttpXrplClient;

// ---------------------------------------------------------------------------
// amm_info query
// ---------------------------------------------------------------------------

pub(super) async fn fetch_amm_info(
    client: &HttpXrplClient,
    asset_currency: &str,
    asset2_currency: &str,
    asset2_issuer: Option<&str>,
) -> Result<AmmInfoResponse, AnalysisError> {
    let asset = if asset_currency == "XRP" {
        json!({ "currency": "XRP" })
    } else {
        json!({ "currency": asset_currency })
    };

    let asset2 = if let Some(issuer) = asset2_issuer {
        json!({ "currency": asset2_currency, "issuer": issuer })
    } else {
        json!({ "currency": asset2_currency })
    };

    let result = client.rpc("amm_info", json!({ "asset": asset, "asset2": asset2 })).await?;

    if result.get("amm").is_none() {
        return Err(AnalysisError::PoolNotFound(format!(
            "{asset_currency}/{asset2_currency}"
        )));
    }

    serde_json::from_value(result).map_err(AnalysisError::Json)
}

// ---------------------------------------------------------------------------
// Build normalised PoolSnapshot
// ---------------------------------------------------------------------------

pub(super) async fn build_pool_snapshot(
    client: &HttpXrplClient,
    wallet: &str,
    pool_label: &str,
) -> Result<PoolSnapshot, AnalysisError> {
    let parts: Vec<&str> = pool_label.split('/').collect();
    if parts.len() != 2 {
        return Err(AnalysisError::XrplRpc(format!(
            "invalid pool label '{pool_label}', expected 'TOKEN0/TOKEN1'"
        )));
    }
    let (currency0, currency1) = (parts[0], parts[1]);

    // Fetch pool state.
    let amm_resp = client.amm_info(currency0, currency1, None).await?;
    let xrp_price = client.xrp_usd_price().await?;
    let history = client.price_history(90).await.unwrap_or_default();

    // Parse reserves.
    let (reserve_xrp_drops, reserve_token_raw, token_currency, token_issuer) =
        parse_reserves(&amm_resp.amm.amount, &amm_resp.amm.amount2);

    let lp_supply: f64 = amm_resp.amm.lp_token.value.parse().unwrap_or(0.0);

    // Find positions for this wallet.
    let lines = client.account_lines(wallet).await?;
    let lp_currency = &amm_resp.amm.lp_token.currency;
    let lp_issuer = &amm_resp.amm.lp_token.issuer;

    let positions = lines
        .lines
        .iter()
        .filter(|l| &l.currency == lp_currency && &l.issuer == lp_issuer)
        .map(|l| {
            let lp_held: f64 = l.balance.parse().unwrap_or(0.0);
            let share = if lp_supply > 0.0 { lp_held / lp_supply } else { 0.0 };
            let xrp_held = (reserve_xrp_drops as f64 / 1_000_000.0) * share;
            let tok_held = (reserve_token_raw as f64 / 1_000_000.0) * share;
            PositionSnapshot {
                owner: wallet.to_string(),
                lower_tick: 0,
                upper_tick: 0,
                liquidity: 0,
                fee_growth_inside_0_last_q128: 0,
                fee_growth_inside_1_last_q128: 0,
                amount0_at_entry: xrp_held,
                amount1_at_entry: tok_held,
                entry_price_usd: xrp_price,
                lp_tokens_held: lp_held,
            }
        })
        .collect();

    Ok(PoolSnapshot {
        pool_label: pool_label.to_string(),
        amm_account: amm_resp.amm.account,
        reserve_xrp_drops,
        reserve_token_raw,
        token_currency,
        token_issuer,
        lp_token_supply: lp_supply,
        trading_fee_bps: amm_resp.amm.trading_fee,
        current_xrp_price_usd: xrp_price,
        sqrt_price_q64: None,
        current_tick: None,
        liquidity_active: None,
        fee_growth_global_0_q128: None,
        fee_growth_global_1_q128: None,
        ticks: Vec::<TickSnapshot>::new(),
        positions,
        price_history: history,
    })
}

// ---------------------------------------------------------------------------
// Extract swap events from account_tx
// ---------------------------------------------------------------------------

/// A minimal swap event parsed from a raw XRPL transaction.
#[derive(Debug, Clone)]
pub struct SwapEvent {
    pub timestamp_secs: u64,
    pub amount_in: u128,
    pub fee_bps: u16,
    pub zero_for_one: bool,
}

/// Extract AMM swap events from a raw `account_tx` response.
///
/// Recognises `AMMDeposit`, `AMMWithdraw`, and Payment transactions that
/// pass through an AMM pool.  For simplicity we treat each matched transaction
/// as one swap event.
pub fn extract_swap_events(
    txs: &[crate::types::xrpl::TxEntry],
    pool_account: &str,
    fee_bps: u16,
) -> Vec<crate::quant::replay::SwapEvent> {
    txs.iter()
        .filter_map(|entry| {
            let tx = &entry.tx;

            // Only process transactions that interact with this AMM.
            let destination = tx.get("Destination").and_then(|v| v.as_str());
            let account = tx.get("Account").and_then(|v| v.as_str());
            if destination != Some(pool_account) && account != Some(pool_account) {
                return None;
            }

            // Parse amount_in from the Amount field (XRP drops as string).
            let amount_in: u128 = tx
                .get("Amount")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            if amount_in == 0 {
                return None;
            }

            // Infer direction from transaction type.
            let tx_type = tx.get("TransactionType").and_then(|v| v.as_str());
            let zero_for_one = matches!(tx_type, Some("Payment") | Some("AMMDeposit"));

            let date: u64 = tx.get("date").and_then(|v| v.as_u64()).unwrap_or(0);
            // XRPL epoch starts 2000-01-01; convert to Unix by adding 946_684_800.
            let timestamp_secs = date + 946_684_800;

            Some(crate::quant::replay::SwapEvent {
                timestamp_secs,
                amount_in,
                fee_bps,
                zero_for_one,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_reserves(
    amount: &AmountField,
    amount2: &AmountField,
) -> (u128, u128, String, String) {
    match (amount, amount2) {
        (AmountField::Xrp(drops), AmountField::Token { value, currency, issuer }) => {
            let xrp_drops: u128 = drops.parse().unwrap_or(0);
            let tok_raw = (value.parse::<f64>().unwrap_or(0.0) * 1_000_000.0) as u128;
            (xrp_drops, tok_raw, currency.clone(), issuer.clone())
        }
        (AmountField::Token { value, currency, issuer }, AmountField::Xrp(drops)) => {
            let xrp_drops: u128 = drops.parse().unwrap_or(0);
            let tok_raw = (value.parse::<f64>().unwrap_or(0.0) * 1_000_000.0) as u128;
            (xrp_drops, tok_raw, currency.clone(), issuer.clone())
        }
        // Both tokens (non-XRP pair) — treat amount as token0.
        (AmountField::Token { value: v0, .. }, AmountField::Token { value: v1, currency, issuer }) => {
            let tok0_raw = (v0.parse::<f64>().unwrap_or(0.0) * 1_000_000.0) as u128;
            let tok1_raw = (v1.parse::<f64>().unwrap_or(0.0) * 1_000_000.0) as u128;
            (tok0_raw, tok1_raw, currency.clone(), issuer.clone())
        }
        _ => (0, 0, String::new(), String::new()),
    }
}
