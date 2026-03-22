/// Value at Risk (VaR) — historical simulation method.
///
/// Uses the 95th percentile of the empirical loss distribution derived from
/// historical daily log-returns applied to the current position value.
use crate::{error::AnalysisError, types::pool::PricePoint};

/// Minimum number of price observations required to compute VaR.
pub const MIN_HISTORY_POINTS: usize = 20;

/// Compute 95 % 1-day VaR for a position valued at `position_value_usd`.
///
/// `price_history` should be in chronological order (oldest first).  The
/// function computes daily log-returns, simulates a 1-day P&L scenario for
/// each observation, and returns the 5th-percentile loss (a positive number).
///
/// Returns `AnalysisError::InsufficientHistory` when fewer than
/// `MIN_HISTORY_POINTS` observations are provided.
pub fn historical_var_95(
    position_value_usd: f64,
    price_history: &[PricePoint],
) -> Result<f64, AnalysisError> {
    if price_history.len() < MIN_HISTORY_POINTS {
        return Err(AnalysisError::InsufficientHistory {
            need: MIN_HISTORY_POINTS,
            got: price_history.len(),
        });
    }

    let mut pnl_scenarios: Vec<f64> = price_history
        .windows(2)
        .map(|w| {
            let ret = (w[1].xrp_usd / w[0].xrp_usd).ln();
            position_value_usd * (ret.exp() - 1.0)
        })
        .collect();

    // Sort ascending: most negative (worst loss) first.
    pnl_scenarios.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let idx = (0.05 * pnl_scenarios.len() as f64).floor() as usize;
    let idx = idx.min(pnl_scenarios.len().saturating_sub(1));

    // VaR is the absolute value of the 5th-percentile loss.
    Ok(pnl_scenarios[idx].min(0.0).abs())
}

/// Compute 95 % 1-day CVaR (Expected Shortfall) for a position.
///
/// CVaR is the mean of all losses that exceed the VaR threshold, i.e. the
/// mean of `sorted_losses[0..floor(0.05 * n)]` (the worst 5 % of scenarios).
pub fn historical_cvar_95(
    position_value_usd: f64,
    price_history: &[PricePoint],
) -> Option<f64> {
    if price_history.len() < MIN_HISTORY_POINTS {
        return None;
    }

    let mut pnl_scenarios: Vec<f64> = price_history
        .windows(2)
        .map(|w| {
            let ret = (w[1].xrp_usd / w[0].xrp_usd).ln();
            position_value_usd * (ret.exp() - 1.0)
        })
        .collect();

    // Sort ascending: most negative (worst loss) first.
    pnl_scenarios.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let tail_count = (0.05 * pnl_scenarios.len() as f64).floor() as usize;
    if tail_count == 0 {
        return None;
    }

    let tail_mean = pnl_scenarios[..tail_count]
        .iter()
        .sum::<f64>()
        / tail_count as f64;

    // CVaR is the absolute value of the mean tail loss.
    Some(tail_mean.min(0.0).abs())
}
