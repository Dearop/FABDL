/// Impermanent loss calculation.
///
/// Two modes:
/// * **Constant-product** (native XRPL AMM) — uses the closed-form formula.
/// * **V3 tick-based** (Bedrock contract positions) — simulates `burn()` at
///   the target price and compares to the HODL value.
use crate::types::pool::PositionSnapshot;

use super::math::{amount0_delta, amount1_delta, sqrt_price_at_tick, Q64};

// ---------------------------------------------------------------------------
// Constant-product IL (native XRPL AMM)
// ---------------------------------------------------------------------------

/// Impermanent loss ratio for a constant-product AMM given a price ratio
/// `r = price_now / price_entry`.
///
/// Returns a value ≤ 0 (negative = loss relative to HODL).
///
/// Formula: `IL = 2 * sqrt(r) / (1 + r) - 1`
pub fn cp_il_ratio(price_ratio: f64) -> f64 {
    if price_ratio <= 0.0 {
        return -1.0; // complete loss edge case
    }
    2.0 * price_ratio.sqrt() / (1.0 + price_ratio) - 1.0
}

/// Impermanent loss for a constant-product position valued at `position_value_usd`
/// given entry and current prices (both in USD).
///
/// Returns `(il_ratio, il_usd)` where `il_ratio ≤ 0`.
pub fn cp_il(
    position_value_usd: f64,
    entry_price_usd: f64,
    current_price_usd: f64,
) -> (f64, f64) {
    if entry_price_usd <= 0.0 {
        return (0.0, 0.0);
    }
    let r = current_price_usd / entry_price_usd;
    let ratio = cp_il_ratio(r);
    (ratio, position_value_usd * ratio)
}

// ---------------------------------------------------------------------------
// V3 tick-based IL (Bedrock contract positions)
// ---------------------------------------------------------------------------

/// Compute the IL ratio for a V3-style position at a given target price.
///
/// `position`  — the LP position snapshot (contains entry amounts and ticks).
/// `current_price_usd` — the price at which to evaluate IL (USD per XRP).
/// `price_per_drop`    — how to convert raw drops to USD.
///
/// Returns `(il_ratio, position_value_usd, hodl_value_usd)`.
/// `il_ratio` is `(position_value - hodl_value) / hodl_value`, ≤ 0 = loss.
pub fn v3_il(
    position: &PositionSnapshot,
    current_price_usd: f64,
) -> (f64, f64, f64) {
    // Convert price to Q64.64 sqrt_price (XRP-denominated, so price_ratio = 1
    // when XRP/USD equals the pool's reference).  For simplicity we treat the
    // price axis as XRP/token1 where token1 is USD-pegged.
    let sqrt_current = price_to_sqrt_q64(current_price_usd);
    let sqrt_lower = sqrt_price_at_tick(position.lower_tick);
    let sqrt_upper = sqrt_price_at_tick(position.upper_tick);

    // Clamp current price to the position's range (piecewise behaviour).
    let sqrt_eff = sqrt_current.clamp(sqrt_lower, sqrt_upper);

    let liquidity = position.liquidity;

    // Amounts at target price (raw drops).
    let amount0_raw = amount0_delta(sqrt_eff, sqrt_upper, liquidity, false);
    let amount1_raw = amount1_delta(sqrt_lower, sqrt_eff, liquidity, false);

    // Convert drops to whole units (1 XRP = 1_000_000 drops; tokens use 6 dp scale).
    let amount0_now = amount0_raw as f64 / 1_000_000.0;
    let amount1_now = amount1_raw as f64 / 1_000_000.0;

    // Position value at target price.
    let position_value = amount0_now * current_price_usd + amount1_now;

    // HODL value: what the entry amounts would be worth at the target price.
    let hodl_value = position.amount0_at_entry * current_price_usd + position.amount1_at_entry;

    if hodl_value <= 0.0 {
        return (0.0, position_value, hodl_value);
    }

    let il_ratio = (position_value - hodl_value) / hodl_value;
    (il_ratio, position_value, hodl_value)
}

/// Convert a USD/XRP price to a Q64.64 sqrt_price.
///
/// Assumes token1 = USD (1.0), token0 = XRP.
/// `sqrt_price_q64 = sqrt(price) * 2^64`
fn price_to_sqrt_q64(price_usd: f64) -> u128 {
    if price_usd <= 0.0 {
        return 0;
    }
    let sqrt_f = price_usd.sqrt();
    (sqrt_f * (Q64 as f64)) as u128
}

/// Dispatch: use V3 calculation when the position has tick data, otherwise
/// fall back to the constant-product formula.
pub fn position_il(
    position: &PositionSnapshot,
    current_price_usd: f64,
    position_value_usd: f64,
) -> (f64, f64) {
    // A V3 position has non-zero tick range.
    if position.lower_tick != 0 || position.upper_tick != 0 {
        let (il_ratio, _pos_val, _hodl_val) = v3_il(position, current_price_usd);
        let il_usd = position_value_usd * il_ratio;
        (il_ratio, il_usd)
    } else {
        cp_il(position_value_usd, position.entry_price_usd, current_price_usd)
    }
}
