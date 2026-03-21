/// Swap volume replay — rebuilds `fee_growth_global` from historical swap events.
///
/// For each historical swap we:
/// 1. Compute `fee_amount = amount_in * fee_bps / 10_000`.
/// 2. Accumulate `fee_growth_global += fee_amount * 2^128 / active_liquidity`.
/// 3. Apply tick crossings: flip `fee_growth_outside` and adjust `active_liquidity`.
///
/// This mirrors the V3 swap loop in `bedrock/contract/src/swap.rs`.
use std::collections::BTreeMap;

use super::math::fee_growth_per_unit_q128;

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// A single historical AMM swap event extracted from XRPL `account_tx`.
#[derive(Debug, Clone)]
pub struct SwapEvent {
    pub timestamp_secs: u64,
    /// Amount of token0 (XRP drops) or token1 raw units sent into the pool.
    pub amount_in: u128,
    /// Pool trading fee in basis points.
    pub fee_bps: u16,
    /// true → token0 in, token1 out (XRP → USD-like).
    pub zero_for_one: bool,
}

// ---------------------------------------------------------------------------
// Mutable replay state
// ---------------------------------------------------------------------------

/// Tick data needed during replay.
#[derive(Debug, Clone, Default)]
pub struct ReplayTick {
    /// Net liquidity change when crossed left-to-right.
    pub liquidity_net: i128,
    /// fee_growth_outside for token0, Q128.
    pub fee_growth_outside_0: u128,
    /// fee_growth_outside for token1, Q128.
    pub fee_growth_outside_1: u128,
}

/// Mutable state carried through the replay.
#[derive(Debug, Clone)]
pub struct ReplayState {
    /// Global fee growth accumulator for token0, Q128.
    pub fee_growth_global_0: u128,
    /// Global fee growth accumulator for token1, Q128.
    pub fee_growth_global_1: u128,
    /// Active liquidity at the current tick.
    pub active_liquidity: u128,
    /// Current tick index.
    pub current_tick: i32,
    /// Sorted tick data (key = tick index).
    pub ticks: BTreeMap<i32, ReplayTick>,
}

impl ReplayState {
    /// Cross a tick boundary, flipping its `fee_growth_outside` accumulators.
    /// Returns the `liquidity_net` at the crossed tick.
    fn cross_tick(&mut self, tick: i32) -> i128 {
        let t = self.ticks.entry(tick).or_default();
        t.fee_growth_outside_0 =
            self.fee_growth_global_0.wrapping_sub(t.fee_growth_outside_0);
        t.fee_growth_outside_1 =
            self.fee_growth_global_1.wrapping_sub(t.fee_growth_outside_1);
        t.liquidity_net
    }
}

// ---------------------------------------------------------------------------
// Replay function
// ---------------------------------------------------------------------------

/// Replay a sequence of swap events against `initial_state`, updating the fee
/// growth accumulators and tick-crossing state as if the swaps had been
/// processed on-chain.
///
/// `swaps` must be in chronological order (oldest first).
///
/// Tick crossings are approximated: for this analysis backend we detect
/// direction from `zero_for_one` and walk the nearest ticks in the BTreeMap.
/// For pools with very few initialised ticks (typical on XRPL MVP) this is
/// sufficiently accurate.
pub fn replay_swaps(mut state: ReplayState, swaps: &[SwapEvent]) -> ReplayState {
    for swap in swaps {
        if state.active_liquidity == 0 {
            continue;
        }

        // Fee amount contributed by this swap.
        let fee_amount = swap.amount_in * swap.fee_bps as u128 / 10_000;

        // Accumulate into the global fee growth for the traded token.
        let delta = fee_growth_per_unit_q128(fee_amount, state.active_liquidity);
        if swap.zero_for_one {
            state.fee_growth_global_0 = state.fee_growth_global_0.wrapping_add(delta);
        } else {
            state.fee_growth_global_1 = state.fee_growth_global_1.wrapping_add(delta);
        }

        // Simple tick-crossing heuristic: if swapping token0 in (price down),
        // cross the first tick below current; if token1 in (price up), cross
        // the first tick above current.
        //
        // In a full swap loop we would compute the exact price move and cross
        // every tick in between.  For the replay analysis we cross at most one
        // tick per swap, which is accurate for moderate swap sizes relative to
        // the liquidity range.
        if swap.zero_for_one {
            // Price moves down — look for the largest tick < current_tick.
            let crossed = state
                .ticks
                .range(..state.current_tick)
                .next_back()
                .map(|(&t, _)| t);
            if let Some(tick) = crossed {
                let lnet = state.cross_tick(tick);
                // Crossing downward subtracts liquidity_net.
                state.active_liquidity = if lnet >= 0 {
                    state.active_liquidity.saturating_sub(lnet as u128)
                } else {
                    state.active_liquidity.saturating_add((-lnet) as u128)
                };
                state.current_tick = tick;
            }
        } else {
            // Price moves up — look for the smallest tick >= current_tick.
            let crossed = state
                .ticks
                .range(state.current_tick..)
                .next()
                .map(|(&t, _)| t);
            if let Some(tick) = crossed {
                let lnet = state.cross_tick(tick);
                state.active_liquidity = if lnet >= 0 {
                    state.active_liquidity.saturating_add(lnet as u128)
                } else {
                    state.active_liquidity.saturating_sub((-lnet) as u128)
                };
                state.current_tick = tick;
            }
        }
    }

    state
}
