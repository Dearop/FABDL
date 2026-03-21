/// Break-even price calculation.
///
/// Finds the two prices (one above, one below the current price) at which
/// cumulative fee income exactly offsets impermanent loss.
///
/// Uses binary search (60 iterations, convergence < 0.0001 USD).
use crate::types::pool::PositionSnapshot;

use super::il::{cp_il_ratio, v3_il};

const MAX_ITER: usize = 60;
const CONVERGENCE: f64 = 0.0001;

/// Compute break-even prices for an LP position.
///
/// `position`          — the LP position (used for V3 tick-based IL if ticks are set).
/// `current_price_usd` — current XRP/USD price.
/// `fee_apr_fraction`  — annualised fee APR as a fraction (e.g. 0.15 for 15 %).
///
/// Returns `(break_even_lower, break_even_upper)`.
///
/// At these prices: `abs(il_ratio) == fee_apr_fraction`.
/// Below `break_even_lower` or above `break_even_upper` the fees no longer
/// offset the IL and the position is net-negative vs HODL.
pub fn break_even_prices(
    position: &PositionSnapshot,
    current_price_usd: f64,
    fee_apr_fraction: f64,
) -> (f64, f64) {
    let is_v3 = position.lower_tick != 0 || position.upper_tick != 0;

    // f(P) = il_ratio(P) + fee_apr_fraction
    // Break-even is where f(P) = 0, i.e. il_ratio = -fee_apr_fraction.
    let f = |price: f64| -> f64 {
        let il = if is_v3 {
            v3_il(position, price).0
        } else {
            if position.entry_price_usd > 0.0 {
                cp_il_ratio(price / position.entry_price_usd)
            } else {
                cp_il_ratio(price / current_price_usd)
            }
        };
        il + fee_apr_fraction
    };

    // If fees are zero both break-evens collapse to current price.
    if fee_apr_fraction == 0.0 {
        return (current_price_usd, current_price_usd);
    }

    let lower = search_lower(current_price_usd, &f);
    let upper = search_upper(current_price_usd, &f);
    (lower, upper)
}

/// Binary search for the largest price P < current where f(P) = 0.
fn search_lower(current_price: f64, f: &impl Fn(f64) -> f64) -> f64 {
    let mut lo = current_price * 0.01;
    let mut hi = current_price;

    // At current price IL ≈ 0, so f(current) ≈ fee_apr > 0.
    // As price drops, IL becomes more negative, so f decreases.
    // We want the largest P where f(P) ≈ 0.

    for _ in 0..MAX_ITER {
        if hi - lo < CONVERGENCE {
            break;
        }
        let mid = (lo + hi) / 2.0;
        if f(mid) > 0.0 {
            hi = mid; // f is still positive, price is too high → move down
        } else {
            lo = mid; // f went negative, price is too low → move up
        }
    }

    (lo + hi) / 2.0
}

/// Binary search for the smallest price P > current where f(P) = 0.
fn search_upper(current_price: f64, f: &impl Fn(f64) -> f64) -> f64 {
    let mut lo = current_price;
    let mut hi = current_price * 100.0;

    for _ in 0..MAX_ITER {
        if hi - lo < CONVERGENCE {
            break;
        }
        let mid = (lo + hi) / 2.0;
        if f(mid) > 0.0 {
            lo = mid; // f still positive → need higher price
        } else {
            hi = mid; // f went negative → mid is above break-even
        }
    }

    (lo + hi) / 2.0
}
