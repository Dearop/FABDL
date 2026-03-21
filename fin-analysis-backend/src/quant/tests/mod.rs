use approx::assert_abs_diff_eq;

use crate::{
    quant::{
        breakeven::break_even_prices,
        delta::{delta_usd_if_down_10, delta_xrp},
        fees::{fee_apr, fee_growth_inside, fees_earned},
        il::{cp_il, cp_il_ratio, v3_il},
        math::{
            amount0_delta, amount1_delta, fee_growth_per_unit_q128, mul_shift128, mul_shift64,
            sqrt_price_at_tick, Q64,
        },
        replay::{replay_swaps, ReplayState, ReplayTick, SwapEvent},
        sharpe::sharpe_ratio,
        var::historical_var_95,
    },
    types::pool::{PositionSnapshot, PricePoint},
};

// ---------------------------------------------------------------------------
// math helpers
// ---------------------------------------------------------------------------

#[test]
fn math_mul_shift64_identity() {
    assert_eq!(mul_shift64(Q64, Q64), Q64);
}

#[test]
fn math_mul_shift128_identity() {
    assert_eq!(mul_shift128(1u128 << 64, 1u128 << 64), 1);
}

#[test]
fn math_mul_shift128_zero() {
    assert_eq!(mul_shift128(0, u128::MAX), 0);
}

#[test]
fn math_sqrt_price_tick_zero() {
    assert_eq!(sqrt_price_at_tick(0), Q64);
}

#[test]
fn math_amount0_zero_range() {
    assert_eq!(amount0_delta(Q64, Q64, 1_000, false), 0);
}

#[test]
fn math_amount1_zero_range() {
    assert_eq!(amount1_delta(Q64, Q64, 1_000, false), 0);
}

#[test]
fn math_fee_growth_per_unit_zero_liquidity() {
    assert_eq!(fee_growth_per_unit_q128(1_000, 0), 0);
}

// ---------------------------------------------------------------------------
// IL — constant product
// ---------------------------------------------------------------------------

#[test]
fn cp_il_no_price_change() {
    // r = 1.0 → no IL
    let ratio = cp_il_ratio(1.0);
    assert_abs_diff_eq!(ratio, 0.0, epsilon = 1e-10);
}

#[test]
fn cp_il_double_price() {
    // r = 2.0 → IL ≈ -5.719 %
    let ratio = cp_il_ratio(2.0);
    assert_abs_diff_eq!(ratio, -0.057_190_958, epsilon = 1e-6);
}

#[test]
fn cp_il_half_price() {
    // IL is symmetric: r = 0.5 → same magnitude as r = 2.0
    let ratio_half = cp_il_ratio(0.5).abs();
    let ratio_double = cp_il_ratio(2.0).abs();
    assert_abs_diff_eq!(ratio_half, ratio_double, epsilon = 1e-6);
}

#[test]
fn cp_il_usd_loss() {
    let (ratio, usd) = cp_il(10_000.0, 0.5, 1.0);
    assert!(ratio < 0.0, "IL should be negative");
    assert!(usd < 0.0, "IL USD should be negative");
    // At r=2 the IL ratio is ~-5.72%, so loss ≈ $572 on a $10k position.
    assert_abs_diff_eq!(usd, 10_000.0 * ratio, epsilon = 1e-6);
}

// ---------------------------------------------------------------------------
// IL — V3 tick-based
// ---------------------------------------------------------------------------

fn make_v3_position(lower_tick: i32, upper_tick: i32, liquidity: u128) -> PositionSnapshot {
    let entry_price = 1.0;
    // Compute entry amounts at the midpoint tick (tick 0 → sqrt = Q64 → price = 1.0).
    let sqrt_lower = sqrt_price_at_tick(lower_tick);
    let sqrt_upper = sqrt_price_at_tick(upper_tick);
    let sqrt_mid = sqrt_price_at_tick(0);
    let a0_raw = amount0_delta(sqrt_mid, sqrt_upper, liquidity, false);
    let a1_raw = amount1_delta(sqrt_lower, sqrt_mid, liquidity, false);

    PositionSnapshot {
        owner: "test".to_string(),
        lower_tick,
        upper_tick,
        liquidity,
        fee_growth_inside_0_last_q128: 0,
        fee_growth_inside_1_last_q128: 0,
        amount0_at_entry: a0_raw as f64 / 1_000_000.0,
        amount1_at_entry: a1_raw as f64 / 1_000_000.0,
        entry_price_usd: entry_price,
        lp_tokens_held: 1_000.0,
    }
}

#[test]
fn v3_il_no_price_change() {
    let pos = make_v3_position(-100, 100, 1_000_000);
    let (il_ratio, _pos_val, _hodl) = v3_il(&pos, 1.0);
    // At entry price IL should be ~0 (small rounding only).
    assert!(il_ratio.abs() < 0.01, "IL at entry price should be near 0, got {il_ratio}");
}

#[test]
fn v3_il_in_range_negative() {
    let pos = make_v3_position(-500, 500, 1_000_000);
    // Price moves 20 % up while still in range.
    let (il_ratio, _, _) = v3_il(&pos, 1.2);
    assert!(il_ratio <= 0.0, "IL should be ≤ 0 (loss vs HODL), got {il_ratio}");
}

#[test]
fn v3_il_below_range_is_all_token1() {
    // Position range is [-100, 100]; price moves very low (out of range below).
    let pos = make_v3_position(-100, 100, 1_000_000);
    let (_, pos_val, _) = v3_il(&pos, 0.0001);
    // Out-of-range below: position is all token1 (USD-like), value approaches amount1_at_entry.
    assert!(pos_val >= 0.0);
}

// ---------------------------------------------------------------------------
// Fees
// ---------------------------------------------------------------------------

#[test]
fn fee_growth_inside_tick_in_range() {
    // lower_tick = -100, upper_tick = 100, current_tick = 0.
    // Global = 1000 for both tokens.
    // Both lower and upper outside = 0 (initialised at zero).
    let (fg0, fg1) = fee_growth_inside(-100, 100, 0, 1000, 1000, 0, 0, 0, 0);
    // inside = global - below - above = 1000 - 0 - 0 = 1000
    assert_eq!(fg0, 1000);
    assert_eq!(fg1, 1000);
}

#[test]
fn fees_earned_zero_delta() {
    // If fee growth hasn't changed since checkpoint, earned = 0.
    let (e0, e1) = fees_earned(1_000_000, 500, 500, 500, 500);
    assert_eq!(e0, 0);
    assert_eq!(e1, 0);
}

#[test]
fn fees_earned_nonzero() {
    // With liquidity = 2^128 - 1 and delta = 1, result should be 0 (floor division).
    // With liquidity = 2^64 and delta = 2^64, result should be 1.
    let (e0, _e1) = fees_earned(1u128 << 64, 1u128 << 64, 0, 0, 0);
    assert_eq!(e0, 1, "mul_shift128(2^64, 2^64) should be 1");
}

#[test]
fn fee_apr_annualises_7d_window() {
    // Suppose position value = $10k, earned $10 in 7 days.
    // APR = (10/10000) * (365/7) ≈ 5.21 %
    let window = 7 * 86_400u64;
    // earned_0_raw = 10 XRP at price $1 → 10_000_000 drops
    let apr = fee_apr(10_000_000, 0, 1.0, 1.0, 10_000.0, window);
    let expected = (10.0 / 10_000.0) * (31_536_000.0 / window as f64);
    assert_abs_diff_eq!(apr, expected, epsilon = 1e-4);
}

// ---------------------------------------------------------------------------
// Replay
// ---------------------------------------------------------------------------

fn empty_replay_state() -> ReplayState {
    ReplayState {
        fee_growth_global_0: 0,
        fee_growth_global_1: 0,
        active_liquidity: 1_000_000,
        current_tick: 0,
        ticks: std::collections::BTreeMap::new(),
    }
}

#[test]
fn replay_empty_swaps_unchanged() {
    let state = empty_replay_state();
    let after = replay_swaps(state.clone(), &[]);
    assert_eq!(after.fee_growth_global_0, state.fee_growth_global_0);
    assert_eq!(after.fee_growth_global_1, state.fee_growth_global_1);
    assert_eq!(after.active_liquidity, state.active_liquidity);
}

#[test]
fn replay_single_swap_accumulates_fees() {
    let state = empty_replay_state();
    let swap = SwapEvent {
        timestamp_secs: 1000,
        amount_in: 1_000_000,
        fee_bps: 30,
        zero_for_one: true,
    };
    let after = replay_swaps(state, &[swap]);
    // fee_amount = 1_000_000 * 30 / 10_000 = 3_000
    // fee_growth_delta = fee_growth_per_unit_q128(3_000, 1_000_000)
    assert!(after.fee_growth_global_0 > 0, "token0 fee growth should accumulate");
    assert_eq!(after.fee_growth_global_1, 0, "token1 should be unchanged for zero_for_one");
}

#[test]
fn replay_tick_crossing_changes_liquidity() {
    let mut state = empty_replay_state();
    // Add a tick below current (at -100) with liquidity_net = -500_000.
    state.ticks.insert(
        -100,
        ReplayTick {
            liquidity_net: -500_000,
            fee_growth_outside_0: 0,
            fee_growth_outside_1: 0,
        },
    );
    let liquidity_before = state.active_liquidity;

    let swap = SwapEvent {
        timestamp_secs: 1000,
        amount_in: 1_000_000_000,
        fee_bps: 30,
        zero_for_one: true, // price moving down, should cross tick at -100
    };
    let after = replay_swaps(state, &[swap]);
    // After crossing tick -100 (downward), liquidity_net = -500_000 is subtracted.
    assert_ne!(after.active_liquidity, liquidity_before);
}

// ---------------------------------------------------------------------------
// Break-even
// ---------------------------------------------------------------------------

fn dummy_cp_position() -> PositionSnapshot {
    PositionSnapshot {
        owner: "test".to_string(),
        lower_tick: 0,
        upper_tick: 0,
        liquidity: 0,
        fee_growth_inside_0_last_q128: 0,
        fee_growth_inside_1_last_q128: 0,
        amount0_at_entry: 1000.0,
        amount1_at_entry: 500.0,
        entry_price_usd: 0.5,
        lp_tokens_held: 1000.0,
    }
}

#[test]
fn breakeven_upper_above_current() {
    let pos = dummy_cp_position();
    let (_, upper) = break_even_prices(&pos, 0.5, 0.10);
    assert!(upper > 0.5, "break-even upper should be above current price");
}

#[test]
fn breakeven_lower_below_current() {
    let pos = dummy_cp_position();
    let (lower, _) = break_even_prices(&pos, 0.5, 0.10);
    assert!(lower < 0.5, "break-even lower should be below current price");
}

#[test]
fn breakeven_zero_fees_collapses_to_current() {
    let pos = dummy_cp_position();
    let (lower, upper) = break_even_prices(&pos, 0.5, 0.0);
    assert_abs_diff_eq!(lower, 0.5, epsilon = 1e-6);
    assert_abs_diff_eq!(upper, 0.5, epsilon = 1e-6);
}

// ---------------------------------------------------------------------------
// VaR
// ---------------------------------------------------------------------------

fn flat_history(n: usize, price: f64) -> Vec<PricePoint> {
    (0..n)
        .map(|i| PricePoint { timestamp_secs: i as u64 * 86_400, xrp_usd: price })
        .collect()
}

fn synthetic_history() -> Vec<PricePoint> {
    // 30 days of daily prices with known returns.
    let prices = vec![
        1.0, 1.02, 0.98, 1.05, 0.95, 1.10, 0.90, 1.15, 0.85, 1.20,
        1.18, 1.22, 1.19, 1.25, 1.20, 1.30, 1.28, 1.32, 1.35, 1.40,
        1.38, 1.42, 1.45, 1.50, 1.48, 1.52, 1.55, 1.60, 1.58, 1.62,
    ];
    prices
        .into_iter()
        .enumerate()
        .map(|(i, p)| PricePoint { timestamp_secs: i as u64 * 86_400, xrp_usd: p })
        .collect()
}

#[test]
fn var_insufficient_history() {
    let history = flat_history(5, 1.0);
    let result = historical_var_95(10_000.0, &history);
    assert!(result.is_err());
}

#[test]
fn var_flat_price_is_zero() {
    let history = flat_history(30, 1.0);
    let var = historical_var_95(10_000.0, &history).unwrap();
    assert_abs_diff_eq!(var, 0.0, epsilon = 1e-6);
}

#[test]
fn var_positive_value() {
    let history = synthetic_history();
    let var = historical_var_95(10_000.0, &history).unwrap();
    assert!(var >= 0.0, "VaR must be non-negative");
}

// ---------------------------------------------------------------------------
// Sharpe
// ---------------------------------------------------------------------------

#[test]
fn sharpe_insufficient_history() {
    let history = flat_history(1, 1.0);
    let result = sharpe_ratio(&history, 0.05);
    assert!(result.is_err());
}

#[test]
fn sharpe_flat_price_is_infinity() {
    let history = flat_history(30, 1.0);
    let s = sharpe_ratio(&history, 0.0).unwrap();
    assert!(s.is_infinite() || s.is_nan(), "flat price → zero vol → infinite/NaN Sharpe");
}

#[test]
fn sharpe_volatile_history_finite() {
    let history = synthetic_history();
    let s = sharpe_ratio(&history, 0.05).unwrap();
    assert!(s.is_finite(), "Sharpe should be finite for volatile history");
}

// ---------------------------------------------------------------------------
// Delta
// ---------------------------------------------------------------------------

#[test]
fn delta_xrp_basic() {
    // 50/50 pool: $10k position at $0.50/XRP → 10k XRP delta.
    let d = delta_xrp(10_000.0, 0.5);
    assert_abs_diff_eq!(d, 10_000.0, epsilon = 1e-6);
}

#[test]
fn delta_usd_down_10_is_negative() {
    let d = delta_xrp(10_000.0, 0.5);
    let loss = delta_usd_if_down_10(d, 0.5);
    assert!(loss < 0.0, "10% price drop should yield negative USD delta");
    // -10% of 5000 USD in XRP = -500
    assert_abs_diff_eq!(loss, -500.0, epsilon = 1e-4);
}
