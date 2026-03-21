/// Sharpe ratio computation from historical price data.
use crate::{error::AnalysisError, types::pool::PricePoint};

/// Minimum observations required (need at least 2 for one return).
const MIN_POINTS: usize = 2;

/// Compute the annualised Sharpe ratio from a price history.
///
/// Uses daily log-returns.  Mean and variance are computed in a single pass
/// with Welford's online algorithm to avoid cancellation.
///
/// `risk_free_rate_annual` — annualised risk-free rate (e.g. 0.05 for 5 %).
///
/// Returns `f64::INFINITY` when volatility is zero (perfectly flat price).
pub fn sharpe_ratio(
    price_history: &[PricePoint],
    risk_free_rate_annual: f64,
) -> Result<f64, AnalysisError> {
    if price_history.len() < MIN_POINTS {
        return Err(AnalysisError::InsufficientHistory {
            need: MIN_POINTS,
            got: price_history.len(),
        });
    }

    // Compute log-returns.
    let returns: Vec<f64> = price_history
        .windows(2)
        .filter_map(|w| {
            if w[0].xrp_usd > 0.0 && w[1].xrp_usd > 0.0 {
                Some((w[1].xrp_usd / w[0].xrp_usd).ln())
            } else {
                None
            }
        })
        .collect();

    if returns.is_empty() {
        return Err(AnalysisError::InsufficientHistory {
            need: MIN_POINTS,
            got: 0,
        });
    }

    // Welford's online mean + variance.
    let mut count = 0u64;
    let mut mean = 0.0f64;
    let mut m2 = 0.0f64;

    for &r in &returns {
        count += 1;
        let delta = r - mean;
        mean += delta / count as f64;
        let delta2 = r - mean;
        m2 += delta * delta2;
    }

    let variance = if count > 1 { m2 / (count - 1) as f64 } else { 0.0 };
    let std_dev = variance.sqrt();

    // Annualise (assume daily returns, 365 trading days).
    let annual_return = mean * 365.0;
    let annual_vol = std_dev * 365.0_f64.sqrt();

    if annual_vol == 0.0 {
        return Ok(f64::INFINITY);
    }

    Ok((annual_return - risk_free_rate_annual) / annual_vol)
}
