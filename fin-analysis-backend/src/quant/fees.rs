/// Fee growth accounting (Uniswap v3 per-position model).
///
/// Formulas from `fin-analysis-backend/LP_FORMULAS.md`.
use super::math::mul_shift128;

// ---------------------------------------------------------------------------
// Fee growth inside a tick range
// ---------------------------------------------------------------------------

/// Compute `fee_growth_inside` for a position range `[lower_tick, upper_tick]`.
///
/// All accumulator values are Q128.  Returns `(inside_0, inside_1)`.
///
/// Implements the standard V3 formula:
/// ```text
/// fee_below = if current >= lower { outside_lower } else { global - outside_lower }
/// fee_above = if current <  upper { outside_upper } else { global - outside_upper }
/// fee_inside = global - fee_below - fee_above   (all wrapping)
/// ```
pub fn fee_growth_inside(
    lower_tick: i32,
    upper_tick: i32,
    current_tick: i32,
    fee_growth_global_0: u128,
    fee_growth_global_1: u128,
    tick_lower_outside_0: u128,
    tick_lower_outside_1: u128,
    tick_upper_outside_0: u128,
    tick_upper_outside_1: u128,
) -> (u128, u128) {
    // fee growth below lower tick
    let (fgb0, fgb1) = if current_tick >= lower_tick {
        (tick_lower_outside_0, tick_lower_outside_1)
    } else {
        (
            fee_growth_global_0.wrapping_sub(tick_lower_outside_0),
            fee_growth_global_1.wrapping_sub(tick_lower_outside_1),
        )
    };

    // fee growth above upper tick
    let (fga0, fga1) = if current_tick < upper_tick {
        (tick_upper_outside_0, tick_upper_outside_1)
    } else {
        (
            fee_growth_global_0.wrapping_sub(tick_upper_outside_0),
            fee_growth_global_1.wrapping_sub(tick_upper_outside_1),
        )
    };

    (
        fee_growth_global_0.wrapping_sub(fgb0).wrapping_sub(fga0),
        fee_growth_global_1.wrapping_sub(fgb1).wrapping_sub(fga1),
    )
}

// ---------------------------------------------------------------------------
// Fees earned since last checkpoint
// ---------------------------------------------------------------------------

/// Compute fees earned by a position since its last fee growth checkpoint.
///
/// `liquidity`        — position liquidity (u128).
/// `fg_inside_0_now`  — current `fee_growth_inside` for token0, Q128.
/// `fg_inside_1_now`  — current `fee_growth_inside` for token1, Q128.
/// `fg_inside_0_last` — last-checkpointed value for token0, Q128.
/// `fg_inside_1_last` — last-checkpointed value for token1, Q128.
///
/// Returns `(earned_0_raw, earned_1_raw)` in raw drop units.
pub fn fees_earned(
    liquidity: u128,
    fg_inside_0_now: u128,
    fg_inside_1_now: u128,
    fg_inside_0_last: u128,
    fg_inside_1_last: u128,
) -> (u128, u128) {
    let delta0 = fg_inside_0_now.wrapping_sub(fg_inside_0_last);
    let delta1 = fg_inside_1_now.wrapping_sub(fg_inside_1_last);
    (mul_shift128(liquidity, delta0), mul_shift128(liquidity, delta1))
}

// ---------------------------------------------------------------------------
// Fee APR (annualised)
// ---------------------------------------------------------------------------

/// Compute annualised fee APR as a fraction (e.g. 0.15 = 15 %).
///
/// `earned_0_raw`        — fees earned in token0 raw units over `replay_window_secs`.
/// `earned_1_raw`        — fees earned in token1 raw units.
/// `price_token0_usd`    — USD price of token0 (XRP).
/// `price_token1_usd`    — USD price of token1 (issued token).
/// `position_value_usd`  — current USD value of the position.
/// `replay_window_secs`  — length of the replay window in seconds.
pub fn fee_apr(
    earned_0_raw: u128,
    earned_1_raw: u128,
    price_token0_usd: f64,
    price_token1_usd: f64,
    position_value_usd: f64,
    replay_window_secs: u64,
) -> f64 {
    if position_value_usd <= 0.0 || replay_window_secs == 0 {
        return 0.0;
    }
    // Convert raw drop units to whole units (6 decimal places).
    let earned0_usd = (earned_0_raw as f64 / 1_000_000.0) * price_token0_usd;
    let earned1_usd = (earned_1_raw as f64 / 1_000_000.0) * price_token1_usd;
    let fees_earned_usd = earned0_usd + earned1_usd;

    const SECONDS_PER_YEAR: f64 = 31_536_000.0;
    (fees_earned_usd / position_value_usd) * (SECONDS_PER_YEAR / replay_window_secs as f64)
}

/// Compute fees earned in USD over the replay window (no annualisation).
pub fn fees_earned_usd(
    earned_0_raw: u128,
    earned_1_raw: u128,
    price_token0_usd: f64,
    price_token1_usd: f64,
) -> f64 {
    let e0 = (earned_0_raw as f64 / 1_000_000.0) * price_token0_usd;
    let e1 = (earned_1_raw as f64 / 1_000_000.0) * price_token1_usd;
    e0 + e1
}

// ---------------------------------------------------------------------------
// Native XRPL AMM simplified fee APR
// ---------------------------------------------------------------------------

/// Simplified fee APR for native XRPL constant-product pools where
/// tick-level fee growth accounting is unavailable.
///
/// `volume_24h_usd`   — estimated 24-hour trading volume in USD.
/// `fee_bps`          — pool trading fee in basis points.
/// `position_value_usd` — LP position value in USD.
/// `total_pool_value_usd` — total pool TVL in USD.
pub fn native_amm_fee_apr(
    volume_24h_usd: f64,
    fee_bps: u16,
    position_value_usd: f64,
    total_pool_value_usd: f64,
) -> f64 {
    if total_pool_value_usd <= 0.0 || position_value_usd <= 0.0 {
        return 0.0;
    }
    let lp_share = position_value_usd / total_pool_value_usd;
    let daily_fees = volume_24h_usd * (fee_bps as f64 / 10_000.0) * lp_share;
    let annual_fees = daily_fees * 365.0;
    annual_fees / position_value_usd
}

/// Fee growth per unit of liquidity for a swap step (Q128).
///
/// Thin re-export so callers don't need to import from `math` directly.
pub use super::math::fee_growth_per_unit_q128;
