/// Multi-tick swap loop engine.
///
/// Implements the Uniswap v3 swap traversal algorithm:
///   1. Find next initialized tick in direction (via bitmap).
///   2. Compute how far price moves with remaining input.
///   3. If price doesn't reach the next tick → finish within current tick.
///   4. Else → consume to boundary, cross the tick (flip outside accumulators,
///      apply liquidity_net), continue loop.

use crate::math::{compute_swap_step, tick_at_sqrt_price};
use crate::tick::TickMap;
use crate::tick_bitmap::TickBitmap;
use crate::types::ContractError;

/// Maximum number of ticks that can be crossed in a single swap call.
/// Prevents runaway gas / infinite loops.
const MAX_TICK_CROSSINGS: u32 = 64;

/// State threaded through the swap loop.
struct SwapState {
    /// Remaining input amount (gross, before fee).
    amount_remaining: u64,
    /// Accumulated output so far.
    amount_out: u64,
    /// Current sqrt price (Q64.64).
    sqrt_price: u128,
    /// Current tick (floor of sqrt_price).
    tick: i32,
    /// Active liquidity in current range.
    liquidity: u128,
    /// Accumulated fee growth (token0 or token1 depending on direction), Q128.
    fee_growth_global: u128,
    /// Protocol fee accumulator.
    protocol_fees: u128,
}

pub struct SwapResult {
    pub amount_in: u64,
    pub amount_out: u64,
    /// Final sqrt price after swap.
    pub sqrt_price_after: u128,
    /// Final tick after swap.
    pub tick_after: i32,
    /// Active liquidity after all tick crossings have been applied.
    pub liquidity_after: u128,
    /// Total fee growth delta to apply to the relevant global accumulator.
    pub fee_growth_delta: u128,
    /// Protocol fee collected.
    pub protocol_fee: u128,
    /// Number of ticks crossed.
    pub ticks_crossed: u32,
}

/// Execute a swap, mutating tick state in place and returning the result.
///
/// Parameters:
/// - `sqrt_price_current`       — pool's current sqrt price (Q64.64)
/// - `current_tick`             — pool's current tick
/// - `liquidity`                — pool's active liquidity
/// - `fee_bps`                  — pool fee in basis points
/// - `protocol_fee_bps`         — protocol's share of fee in bps (of fee, not of trade)
/// - `fee_growth_global`        — current global fee growth for the input token (Q128)
/// - `amount_in`                — exact input amount requested
/// - `zero_for_one`             — swap direction (true = token0 → token1, price down)
/// - `sqrt_price_limit`         — price boundary (hard stop)
/// - `tick_spacing`             — pool tick spacing
/// - `tick_cumulative`          — oracle tick cumulative at swap start (for tick crossing)
/// - `seconds_per_liquidity_q128` — oracle spl accumulator at swap start
/// - `timestamp`                — current block timestamp (for tick crossing oracle)
/// - `ticks`                    — mutable tick map
/// - `bitmap`                   — mutable tick bitmap
#[allow(clippy::too_many_arguments)]
pub fn execute_swap(
    sqrt_price_current: u128,
    current_tick: i32,
    liquidity: u128,
    fee_bps: u16,
    protocol_fee_share_bps: u16,
    fee_growth_global: u128,
    amount_in: u64,
    zero_for_one: bool,
    sqrt_price_limit: u128,
    tick_spacing: i32,
    tick_cumulative: i64,
    seconds_per_liquidity_q128: u128,
    timestamp: u32,
    ticks: &mut TickMap,
    bitmap: &mut TickBitmap,
) -> Result<SwapResult, ContractError> {
    // Validate limit price direction.
    if zero_for_one {
        if sqrt_price_limit >= sqrt_price_current {
            return Err(ContractError::SlippageLimitExceeded);
        }
    } else if sqrt_price_limit <= sqrt_price_current {
        return Err(ContractError::SlippageLimitExceeded);
    }

    let amount_in_total = amount_in;

    let mut state = SwapState {
        amount_remaining: amount_in,
        amount_out: 0,
        sqrt_price: sqrt_price_current,
        tick: current_tick,
        liquidity,
        fee_growth_global,
        protocol_fees: 0,
    };

    let mut ticks_crossed: u32 = 0;

    while state.amount_remaining > 0 && state.sqrt_price != sqrt_price_limit {
        if ticks_crossed >= MAX_TICK_CROSSINGS {
            // Hard cap — return what we have so far.
            break;
        }

        // 1. Find next initialized tick in swap direction.
        let (next_tick, initialized) = bitmap.next_initialized_tick_within_one_word(
            state.tick,
            tick_spacing,
            zero_for_one,
        );

        // Clamp next_tick to valid price boundary.
        let next_tick_clamped = if zero_for_one {
            next_tick.max(crate::math::MIN_TICK)
        } else {
            next_tick.min(crate::math::MAX_TICK)
        };

        let sqrt_price_next_tick = crate::math::sqrt_price_at_tick(next_tick_clamped);

        // Clamp target price to user's limit.
        let sqrt_price_target = if zero_for_one {
            sqrt_price_next_tick.max(sqrt_price_limit)
        } else {
            sqrt_price_next_tick.min(sqrt_price_limit)
        };

        // If the bitmap returned an uninitialized word boundary that equals the
        // current price (e.g. we're at the leftmost bit of a word with no tick
        // below), advance the tick position one spacing into the next word so
        // that the next iteration searches the adjacent word.
        if !initialized && sqrt_price_target == state.sqrt_price {
            state.tick = if zero_for_one {
                next_tick_clamped - tick_spacing
            } else {
                next_tick_clamped + tick_spacing
            };
            ticks_crossed += 1; // count this as a step to bound iterations
            if ticks_crossed >= MAX_TICK_CROSSINGS {
                break;
            }
            continue;
        }

        // Hard stop: if price genuinely can't move (same target, initialized tick).
        if sqrt_price_target == state.sqrt_price {
            break;
        }

        // 2. Compute one step.
        let (sqrt_price_after, amount_in_step, amount_out_step, fee_amount) =
            compute_swap_step(
                state.sqrt_price,
                sqrt_price_target,
                state.liquidity,
                state.amount_remaining,
                fee_bps,
                zero_for_one,
            );

        // If nothing was consumed (e.g. zero liquidity in this range), stop.
        if amount_in_step == 0 && fee_amount == 0 {
            break;
        }

        // Deduct used input.
        state.amount_remaining = state
            .amount_remaining
            .saturating_sub(amount_in_step.saturating_add(fee_amount));
        state.amount_out = state.amount_out.saturating_add(amount_out_step);
        state.sqrt_price = sqrt_price_after;

        // 3. Update fee growth (Q128 fixed-point accumulation).
        // fee_growth_delta = fee_amount / liquidity  (Q128-scaled)
        if state.liquidity > 0 && fee_amount > 0 {
            let protocol_cut = (fee_amount as u128 * protocol_fee_share_bps as u128) / 10_000;
            let lp_fee = fee_amount as u128 - protocol_cut;
            // lp_fee / liquidity in Q128: (lp_fee << 128) / liquidity
            let growth_per_unit = fee_growth_per_unit_q128(lp_fee, state.liquidity);
            state.fee_growth_global = state.fee_growth_global.wrapping_add(growth_per_unit);
            state.protocol_fees = state.protocol_fees.saturating_add(protocol_cut);
        }

        // 4. If we reached the next tick boundary, cross it.
        if sqrt_price_after == sqrt_price_next_tick && initialized {
            let liquidity_net = ticks.cross(
                next_tick_clamped,
                if zero_for_one { state.fee_growth_global } else { fee_growth_global },
                if zero_for_one { fee_growth_global } else { state.fee_growth_global },
                tick_cumulative,
                seconds_per_liquidity_q128,
                timestamp,
            );

            // Adjust active liquidity: moving right adds net, moving left subtracts.
            state.liquidity = if zero_for_one {
                (state.liquidity as i128 - liquidity_net) as u128
            } else {
                (state.liquidity as i128 + liquidity_net) as u128
            };

            state.tick = if zero_for_one {
                next_tick_clamped - 1
            } else {
                next_tick_clamped
            };

            ticks_crossed += 1;
        } else {
            // Re-derive tick from price if we didn't reach a boundary.
            state.tick = tick_at_sqrt_price(state.sqrt_price);
        }
    }

    let amount_in_used = amount_in_total - state.amount_remaining;

    // fee_growth_delta = how much the global accumulator moved.
    let fee_growth_delta = state.fee_growth_global.wrapping_sub(fee_growth_global);

    Ok(SwapResult {
        amount_in: amount_in_used,
        amount_out: state.amount_out,
        sqrt_price_after: state.sqrt_price,
        tick_after: state.tick,
        liquidity_after: state.liquidity,
        fee_growth_delta,
        protocol_fee: state.protocol_fees,
        ticks_crossed,
    })
}

/// Compute fee growth per unit of liquidity in Q128 format.
/// = (fee_amount << 128) / liquidity,  saturating.
/// Exposed as pub(crate) so `donate` in lib.rs can reuse the same formula.
pub(crate) fn fee_growth_per_unit_q128(fee_amount: u128, liquidity: u128) -> u128 {
    if liquidity == 0 {
        return 0;
    }
    // (fee_amount * 2^128) / liquidity using high-precision division.
    // For fee_amount < 2^64, (fee_amount << 64) fits in u128.
    if fee_amount < (1u128 << 64) {
        ((fee_amount << 64) / liquidity) << 64
    } else {
        // fee_amount is large — compute high half, then low half.
        let hi = (fee_amount >> 64) * (u128::MAX / liquidity.max(1));
        let lo = ((fee_amount & u64::MAX as u128) << 64) / liquidity.max(1);
        hi.saturating_add(lo)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tick::TickMap;
    use crate::tick_bitmap::TickBitmap;

    /// Build a simple pool state for swap tests.
    fn simple_pool() -> (u128, i32, u128, TickMap, TickBitmap) {
        let sqrt_price = Q64; // price = 1.0
        let tick = 0;
        let liquidity = 1_000_000_000u128;
        let mut ticks = TickMap::new();
        let mut bitmap = TickBitmap::new();

        // Initialize tick range -1000..1000 with liquidity.
        ticks.update(-1000, tick, 1_000_000_000, 0, 0, false).unwrap();
        ticks.update(1000, tick, 1_000_000_000, 0, 0, true).unwrap();
        bitmap.flip_tick(-1000, 10);
        bitmap.flip_tick(1000, 10);

        (sqrt_price, tick, liquidity, ticks, bitmap)
    }

    // Helper: call execute_swap with zeroed oracle params (tests don't care about oracle).
    fn swap(
        sp: u128, tick: i32, liq: u128,
        fee_bps: u16, amount_in: u64, zero_for_one: bool, limit: u128,
        ticks: &mut TickMap, bitmap: &mut TickBitmap,
    ) -> Result<SwapResult, crate::types::ContractError> {
        execute_swap(sp, tick, liq, fee_bps, 0, 0, amount_in, zero_for_one, limit, 10,
                     0, 0, 0, ticks, bitmap)
    }

    #[test]
    fn swap_one_for_zero_increases_price() {
        let (sp, tick, liq, mut ticks, mut bitmap) = simple_pool();
        let result = swap(sp, tick, liq, 30, 10_000, false, sp * 2, &mut ticks, &mut bitmap).unwrap();
        assert!(result.sqrt_price_after > sp, "price should increase");
        assert!(result.amount_out > 0, "should produce output");
        assert!(result.amount_in <= 10_000, "used ≤ requested input");
    }

    #[test]
    fn swap_zero_for_one_decreases_price() {
        let (sp, tick, liq, mut ticks, mut bitmap) = simple_pool();
        let result = swap(sp, tick, liq, 30, 10_000, true, sp / 2, &mut ticks, &mut bitmap).unwrap();
        assert!(result.sqrt_price_after < sp, "price should decrease");
        assert!(result.amount_out > 0);
    }

    #[test]
    fn swap_fee_accumulates() {
        let (sp, tick, liq, mut ticks, mut bitmap) = simple_pool();
        let result = swap(sp, tick, liq, 100, 100_000, false, sp * 3, &mut ticks, &mut bitmap).unwrap();
        assert!(result.fee_growth_delta > 0, "fee growth should be positive");
    }

    #[test]
    fn swap_respects_price_limit() {
        let (sp, tick, liq, mut ticks, mut bitmap) = simple_pool();
        let limit = sp + (sp / 100); // +1%
        let result = swap(sp, tick, liq, 30, 1_000_000, false, limit, &mut ticks, &mut bitmap).unwrap();
        assert!(result.sqrt_price_after <= limit, "price must not exceed limit");
    }

    #[test]
    fn invalid_limit_direction_errors() {
        let (sp, tick, liq, mut ticks, mut bitmap) = simple_pool();
        let err = swap(sp, tick, liq, 30, 1000, true, sp * 2, &mut ticks, &mut bitmap);
        assert!(err.is_err());
    }

    #[test]
    fn fee_growth_per_unit_sanity() {
        // 1_000 fee / 1_000_000 liquidity = 0.001 per unit
        let g = fee_growth_per_unit_q128(1_000, 1_000_000);
        assert!(g > 0);
    }
}
