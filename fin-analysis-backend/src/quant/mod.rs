/// Quantitative models for LP position analysis.
///
/// Module layout:
/// * `math`      — fixed-point arithmetic (Q64/Q128), copied from bedrock.
/// * `il`        — impermanent loss (constant-product + V3 tick-based).
/// * `fees`      — fee growth accounting, fees earned, fee APR.
/// * `breakeven` — binary-search break-even prices.
/// * `var`       — 95 % Value at Risk (historical simulation).
/// * `sharpe`    — Sharpe ratio.
/// * `delta`     — XRP delta exposure.
/// * `replay`    — swap volume replay → `fee_growth_global` accumulation.
pub mod breakeven;
pub mod delta;
pub mod fees;
pub mod il;
pub mod math;
pub mod replay;
pub mod sharpe;
pub mod var;

use crate::{
    error::AnalysisError,
    types::{
        pool::{PoolSnapshot, PositionSnapshot, PricePoint},
        quant::{PortfolioRiskSummary, PositionRisk},
    },
};

// ---------------------------------------------------------------------------
// QuantModel trait
// ---------------------------------------------------------------------------

/// Computes risk metrics for LP positions.
pub trait QuantModel: Send + Sync {
    /// Compute risk metrics for a single position.
    fn compute_position_risk(
        &self,
        position: &PositionSnapshot,
        pool: &PoolSnapshot,
        price_history: &[PricePoint],
        replay_window_secs: u64,
    ) -> Result<PositionRisk, AnalysisError>;

    /// Compute aggregated risk summary for an entire portfolio.
    fn compute_portfolio_risk(
        &self,
        snapshots: &[PoolSnapshot],
        price_history: &[PricePoint],
        replay_window_secs: u64,
    ) -> Result<PortfolioRiskSummary, AnalysisError>;
}

// ---------------------------------------------------------------------------
// DefaultQuantModel
// ---------------------------------------------------------------------------

/// Default implementation that wires together all sub-modules.
pub struct DefaultQuantModel {
    /// Annualised risk-free rate for Sharpe calculation (default 0.05 = 5 %).
    pub risk_free_rate: f64,
}

impl Default for DefaultQuantModel {
    fn default() -> Self {
        Self { risk_free_rate: 0.05 }
    }
}

impl QuantModel for DefaultQuantModel {
    fn compute_position_risk(
        &self,
        position: &PositionSnapshot,
        pool: &PoolSnapshot,
        price_history: &[PricePoint],
        replay_window_secs: u64,
    ) -> Result<PositionRisk, AnalysisError> {
        let price = pool.current_xrp_price_usd;

        // ---- Position value ----
        // For a 50/50 CP pool: value = 2 * xrp_held * price
        let xrp_held = pool.reserve_xrp() * (position.lp_tokens_held / pool.lp_token_supply.max(1.0));
        let token_held = pool.reserve_token() * (position.lp_tokens_held / pool.lp_token_supply.max(1.0));
        let position_value_usd = xrp_held * price + token_held;

        let lp_share = if pool.lp_token_supply > 0.0 {
            position.lp_tokens_held / pool.lp_token_supply
        } else {
            0.0
        };

        // ---- IL ----
        let (il_pct, il_usd) =
            il::position_il(position, price, position_value_usd);
        let il_pct_pct = il_pct * 100.0; // convert fraction → percentage

        // ---- Fees ----
        let (fee_apr_val, fees_7d_usd) = if pool.is_v3() {
            compute_v3_fees(position, pool, price, position_value_usd, replay_window_secs)
        } else {
            // Native AMM: use simplified fee APR.
            // Estimate 7-day volume from 24h volume * 7 (rough).
            let total_pool_value = pool.reserve_xrp() * price * 2.0;
            let vol_24h = estimate_volume_24h(pool);
            let apr = fees::native_amm_fee_apr(vol_24h, pool.trading_fee_bps, position_value_usd, total_pool_value);
            let fees_7d = position_value_usd * apr / 52.0; // ~7 days = 1/52 year
            (apr, fees_7d)
        };

        // ---- Break-even ----
        let (be_lower, be_upper) =
            breakeven::break_even_prices(position, price, fee_apr_val);

        // ---- Delta ----
        let delta_xrp_val = delta::delta_xrp(position_value_usd, price);
        let _delta_usd = delta::delta_usd_if_down_10(delta_xrp_val, price);

        // ---- VaR ----
        let var_95 = var::historical_var_95(position_value_usd, price_history)
            .unwrap_or(0.0);

        // ---- Sharpe ----
        let sharpe = sharpe::sharpe_ratio(price_history, self.risk_free_rate)
            .unwrap_or(0.0);

        // ---- Gamma (constant-product AMM) ----
        // For CP AMM: gamma ≈ -0.5 * position_value / price²
        let gamma_usd = if price > 0.0 {
            -0.5 * position_value_usd / (price * price)
        } else {
            0.0
        };

        Ok(PositionRisk {
            pool_label: pool.pool_label.clone(),
            position_value_usd,
            il_pct: il_pct_pct,
            il_usd,
            fee_apr: fee_apr_val,
            fees_earned_7d_usd: fees_7d_usd,
            break_even_lower: be_lower,
            break_even_upper: be_upper,
            delta_xrp: delta_xrp_val,
            sharpe,
            var_95_usd: var_95,
            lp_share_pct: lp_share,
            gamma_usd,
        })
    }

    fn compute_portfolio_risk(
        &self,
        snapshots: &[PoolSnapshot],
        price_history: &[PricePoint],
        replay_window_secs: u64,
    ) -> Result<PortfolioRiskSummary, AnalysisError> {
        let xrp_price = snapshots
            .first()
            .map(|s| s.current_xrp_price_usd)
            .unwrap_or(0.0);

        let mut position_risks: Vec<PositionRisk> = Vec::new();

        for pool in snapshots {
            for pos in &pool.positions {
                let risk = self.compute_position_risk(pos, pool, price_history, replay_window_secs)?;
                position_risks.push(risk);
            }
        }

        if position_risks.is_empty() {
            return Ok(PortfolioRiskSummary::empty(xrp_price));
        }

        // Aggregate.
        let total_value: f64 = position_risks.iter().map(|r| r.position_value_usd).sum();
        let il_usd: f64 = position_risks.iter().map(|r| r.il_usd).sum();
        let il_pct = if total_value > 0.0 { il_usd / total_value * 100.0 } else { 0.0 };
        let fee_income_7d: f64 = position_risks.iter().map(|r| r.fees_earned_7d_usd).sum();
        let delta_xrp: f64 = position_risks.iter().map(|r| r.delta_xrp).sum();
        let delta_usd_down10 = delta::delta_usd_if_down_10(delta_xrp, xrp_price);

        // Weighted average fee APR.
        let fee_apr = if total_value > 0.0 {
            position_risks
                .iter()
                .map(|r| r.fee_apr * r.position_value_usd)
                .sum::<f64>()
                / total_value
        } else {
            0.0
        };

        // Portfolio-level VaR and Sharpe use shared price history.
        let var_95 = var::historical_var_95(total_value, price_history).unwrap_or(0.0);
        let sharpe = sharpe::sharpe_ratio(price_history, self.risk_free_rate).unwrap_or(0.0);

        // Weighted average break-even (simple mean).
        let be_lower = position_risks.iter().map(|r| r.break_even_lower).sum::<f64>()
            / position_risks.len() as f64;
        let be_upper = position_risks.iter().map(|r| r.break_even_upper).sum::<f64>()
            / position_risks.len() as f64;

        // Portfolio-level gamma (sum across positions).
        let gamma_usd: f64 = position_risks.iter().map(|r| r.gamma_usd).sum();

        // CVaR (expected shortfall).
        let cvar_95_usd = var::historical_cvar_95(total_value, price_history).unwrap_or(0.0);

        // Net carry: fee_apr - weighted_borrow_apy - |IL_pct|/100.
        // weighted_borrow_apy is 0.0 until loans are attached in the pipeline.
        let net_carry = fee_apr - il_pct.abs() / 100.0;

        Ok(PortfolioRiskSummary {
            total_value_usd: total_value,
            impermanent_loss_pct: il_pct,
            impermanent_loss_usd: il_usd,
            delta_exposure_xrp: delta_xrp,
            delta_exposure_usd_if_down_10: delta_usd_down10,
            fee_income_7d,
            current_xrp_price: xrp_price,
            sharpe_ratio: sharpe,
            var_95_usd: var_95,
            break_even_lower: be_lower,
            break_even_upper: be_upper,
            fee_apr,
            positions: position_risks,
            lending_vaults: Vec::new(),
            open_loans: Vec::new(),
            cvar_95_usd,
            gamma_usd,
            net_carry,
        })
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Compute V3 fee APR and 7-day fees for a Bedrock position.
fn compute_v3_fees(
    position: &PositionSnapshot,
    pool: &PoolSnapshot,
    price: f64,
    position_value_usd: f64,
    replay_window_secs: u64,
) -> (f64, f64) {
    let (fg_inside_0, fg_inside_1) = match (
        pool.current_tick,
        pool.fee_growth_global_0_q128,
        pool.fee_growth_global_1_q128,
    ) {
        (Some(ct), Some(fg0), Some(fg1)) => {
            // Find the tick snapshots for the position's boundaries.
            let lower_ts = pool.ticks.iter().find(|t| t.tick == position.lower_tick);
            let upper_ts = pool.ticks.iter().find(|t| t.tick == position.upper_tick);
            let (lo_out0, lo_out1) = lower_ts
                .map(|t| (t.fee_growth_outside_0_q128, t.fee_growth_outside_1_q128))
                .unwrap_or((0, 0));
            let (up_out0, up_out1) = upper_ts
                .map(|t| (t.fee_growth_outside_0_q128, t.fee_growth_outside_1_q128))
                .unwrap_or((0, 0));

            fees::fee_growth_inside(
                position.lower_tick,
                position.upper_tick,
                ct,
                fg0,
                fg1,
                lo_out0,
                lo_out1,
                up_out0,
                up_out1,
            )
        }
        _ => (0, 0),
    };

    let (earned_0, earned_1) = fees::fees_earned(
        position.liquidity,
        fg_inside_0,
        fg_inside_1,
        position.fee_growth_inside_0_last_q128,
        position.fee_growth_inside_1_last_q128,
    );

    let window = if replay_window_secs == 0 { 7 * 86_400 } else { replay_window_secs };
    let apr = fees::fee_apr(earned_0, earned_1, price, 1.0, position_value_usd, window);
    let fees_usd = fees::fees_earned_usd(earned_0, earned_1, price, 1.0);

    (apr, fees_usd)
}

/// Rough 24-hour volume estimate from pool reserve size.
/// Used as a fallback when no transaction history is available.
fn estimate_volume_24h(pool: &PoolSnapshot) -> f64 {
    // Assume daily turnover ≈ 5 % of TVL (conservative default).
    let tvl = pool.reserve_xrp() * pool.current_xrp_price_usd * 2.0;
    tvl * 0.05
}

#[cfg(test)]
mod tests;
