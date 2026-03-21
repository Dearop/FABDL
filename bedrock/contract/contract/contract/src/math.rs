/// Fixed-point math for Uniswap v3-style AMM on XRPL.
///
/// Representations:
///   sqrt_price  — Q64.64  (u128):  upper 64 bits = integer, lower 64 = fraction
///   fee_growth  — Q128    (u128):  entire value is fractional (scaled by 2^128)
///   liquidity   — u128 plain integer (drops-scale)

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// One in Q64.64 representation.
pub const Q64: u128 = 1u128 << 64;

/// Minimum and maximum allowed tick values (mirrors Uniswap v3).
pub const MIN_TICK: i32 = -887_272;
pub const MAX_TICK: i32 = 887_272;

// ---------------------------------------------------------------------------
// Overflow-safe multiply-shift helpers
// ---------------------------------------------------------------------------

/// Multiply two u128 values and shift right by 64 bits (Q64.64 multiply).
/// Panics on overflow — callers must validate inputs are in range.
pub fn mul_shift64(a: u128, b: u128) -> u128 {
    // Use u256-emulation via 128-bit halves.
    let a_hi = a >> 64;
    let a_lo = a & (u64::MAX as u128);
    let b_hi = b >> 64;
    let b_lo = b & (u64::MAX as u128);

    let lo_lo = (a_lo * b_lo) >> 64;
    let lo_hi = a_lo * b_hi;
    let hi_lo = a_hi * b_lo;
    let hi_hi = a_hi * b_hi;

    hi_hi
        .checked_shl(64)
        .unwrap_or(u128::MAX)
        .saturating_add(hi_lo)
        .saturating_add(lo_hi)
        .saturating_add(lo_lo)
}

/// Divide a by b in Q64.64 space: (a << 64) / b, saturating.
pub fn div_q64(a: u128, b: u128) -> u128 {
    if b == 0 {
        return u128::MAX;
    }
    // Shift a left by 64 using u256 emulation.
    let a_shifted_hi = a >> 64;
    let a_shifted_lo = a << 64; // lower 128 bits of (a * 2^64)

    // Simple: if a_shifted_hi == 0, just do (a << 64) / b
    if a_shifted_hi == 0 {
        return a_shifted_lo / b;
    }
    // Otherwise saturate — in practice tick ranges keep values bounded.
    u128::MAX
}

// ---------------------------------------------------------------------------
// Tick ↔ sqrt_price conversion
// ---------------------------------------------------------------------------

/// Returns sqrt_price_q64_64 for a given tick using the identity:
///   sqrt(1.0001^tick) stored in Q64.64.
///
/// Algorithm (same as Uniswap v3 TickMath, adapted to Q64.64):
///   1. Compute MAGIC[0] = floor(sqrt(1.0001) * 2^64).
///   2. MAGIC[k] = floor(sqrt(1.0001^(2^k)) * 2^64) = mul_shift64(MAGIC[k-1], MAGIC[k-1]).
///   3. ratio = product of MAGIC[k] for bits k set in |tick|, starting from 1.0 (Q64).
///   4. If tick < 0: invert via 2^128 / ratio.
pub fn sqrt_price_at_tick(tick: i32) -> u128 {
    let abs_tick = tick.unsigned_abs();

    // MAGIC[0] = floor(sqrt(1.0001) * 2^64).
    // Derived from Uniswap v3 inverse constant: floor(2^128 / MAGIC_INV_Q128[0]) >> 64
    // where MAGIC_INV_Q128[0] = 0xfffcb933bd6fad37aa2d162d1a594001.
    // Equivalently: floor(1.0000499987500625 * 2^64).
    const MAGIC_0: u128 = 18_447_666_411_007_353_954;

    // Build the rest by squaring (MAGIC[k] = MAGIC[k-1]^2 / 2^64).
    let mut magic = [0u128; 20];
    magic[0] = MAGIC_0;
    for k in 1..20usize {
        magic[k] = mul_shift64(magic[k - 1], magic[k - 1]);
        if magic[k] == 0 {
            // Overflow region — price is astronomical, cap and stop.
            for j in k..20 { magic[j] = u128::MAX; }
            break;
        }
    }

    let mut ratio: u128 = Q64; // 1.0 in Q64.64
    for k in 0..20u32 {
        if abs_tick & (1u32 << k) != 0 {
            ratio = mul_shift64(ratio, magic[k as usize]);
        }
    }

    if tick < 0 {
        // Invert: 1/ratio in Q64.64 = 2^128 / ratio.
        // Approximated as u128::MAX / ratio (error < 1 ULP, negligible).
        ratio = if ratio == 0 { u128::MAX } else { u128::MAX / ratio };
    }

    ratio
}

/// Returns the tick corresponding to a given sqrt_price_q64_64.
/// Returns the floor tick: largest tick such that sqrt_price_at_tick(t) <= price.
pub fn tick_at_sqrt_price(sqrt_price: u128) -> i32 {
    // Binary search over [MIN_TICK, MAX_TICK].
    let mut lo = MIN_TICK;
    let mut hi = MAX_TICK;

    while lo < hi {
        let mid = lo + (hi - lo + 1) / 2;
        let p = sqrt_price_at_tick(mid);
        if p <= sqrt_price {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }

    lo
}

// ---------------------------------------------------------------------------
// Liquidity delta → token delta (v3 piecewise formulas)
// ---------------------------------------------------------------------------

/// Compute amount0 needed to add `liquidity` in range [lower_sqrt, upper_sqrt]
/// given current price `current_sqrt`, all in Q64.64.
///
/// amount0 = L * (sqrt_upper - sqrt_current) / (sqrt_upper * sqrt_current)
pub fn amount0_delta(
    sqrt_lower: u128,
    sqrt_upper: u128,
    liquidity: u128,
    round_up: bool,
) -> u128 {
    if sqrt_lower >= sqrt_upper {
        return 0;
    }
    let diff = sqrt_upper.saturating_sub(sqrt_lower); // Q64.64
    // amount0 = liquidity * (sqrt_upper - sqrt_lower) / (sqrt_upper * sqrt_lower / Q64)
    let denom = mul_shift64(sqrt_upper, sqrt_lower); // Q64.64 * Q64.64 >> 64 = Q64.64
    if denom == 0 {
        return 0;
    }
    let numer_scaled = liquidity.saturating_mul(diff); // could overflow for huge L
    let result = numer_scaled / (denom >> 64).max(1);
    if round_up && numer_scaled % (denom >> 64).max(1) != 0 {
        result.saturating_add(1)
    } else {
        result
    }
}

/// Compute amount1 needed to add `liquidity` in range [lower_sqrt, upper_sqrt].
///
/// amount1 = L * (sqrt_current - sqrt_lower)
pub fn amount1_delta(
    sqrt_lower: u128,
    sqrt_upper: u128,
    liquidity: u128,
    round_up: bool,
) -> u128 {
    if sqrt_lower >= sqrt_upper {
        return 0;
    }
    let diff = sqrt_upper.saturating_sub(sqrt_lower);
    // amount1 = L * (sqrt_upper - sqrt_lower) / Q64
    let result = mul_shift64(liquidity, diff);
    if round_up && (liquidity.wrapping_mul(diff) & (u64::MAX as u128)) != 0 {
        result.saturating_add(1)
    } else {
        result
    }
}

// ---------------------------------------------------------------------------
// Within-tick swap step math
// ---------------------------------------------------------------------------

/// Given current sqrt_price, active liquidity, fee_bps, amount_remaining,
/// and a target sqrt_price boundary, compute one tick step.
///
/// Returns (next_sqrt_price, amount_in_used, amount_out, fee_amount).
pub fn compute_swap_step(
    sqrt_price_current: u128,
    sqrt_price_target: u128,
    liquidity: u128,
    amount_remaining: u64,
    fee_bps: u16,
    zero_for_one: bool,
) -> (u128, u64, u64, u64) {
    if liquidity == 0 || amount_remaining == 0 {
        return (sqrt_price_current, 0, 0, 0);
    }

    let fee_factor = fee_bps as u128; // bps out of 10_000

    // Amount after fee deduction.
    let amount_in_max_net = amount_remaining as u128 * (10_000 - fee_factor) / 10_000;

    // Compute how far we can move price with amount_in_max_net.
    // For zero_for_one (token0 in, token1 out):
    //   Δsqrt = -amount0 * sqrt_lower * sqrt_upper / (L * Q64 + amount0 * sqrt_upper)
    // For one_for_zero (token1 in, token0 out):
    //   Δsqrt = amount1 / L
    let sqrt_price_next = if zero_for_one {
        // Moving down: sqrt_price decreases.
        // sqrt_next = (L * sqrt_current) / (L + amount_in * sqrt_current / Q64)
        let denom = liquidity.saturating_add(
            amount_in_max_net
                .saturating_mul(sqrt_price_current)
                .checked_shr(64)
                .unwrap_or(u128::MAX),
        );
        mul_shift64(liquidity, sqrt_price_current) / denom.max(1)
    } else {
        // Moving up: sqrt_price increases.
        // sqrt_next = sqrt_current + amount_in / L
        let delta = (amount_in_max_net << 64) / liquidity.max(1);
        sqrt_price_current.saturating_add(delta)
    };

    // Clamp to target.
    let sqrt_price_next = if zero_for_one {
        sqrt_price_next.max(sqrt_price_target)
    } else {
        sqrt_price_next.min(sqrt_price_target)
    };

    let reached_target = sqrt_price_next == sqrt_price_target;

    // Compute actual amounts for the step (clamp to u64 max).
    let (amount_in, amount_out): (u64, u64) = if zero_for_one {
        let a0 = amount0_delta(sqrt_price_next, sqrt_price_current, liquidity, true).min(u64::MAX as u128) as u64;
        let a1 = amount1_delta(sqrt_price_next, sqrt_price_current, liquidity, false).min(u64::MAX as u128) as u64;
        (a0, a1)
    } else {
        let a1 = amount1_delta(sqrt_price_current, sqrt_price_next, liquidity, true).min(u64::MAX as u128) as u64;
        let a0 = amount0_delta(sqrt_price_current, sqrt_price_next, liquidity, false).min(u64::MAX as u128) as u64;
        (a1, a0)
    };

    // Fee on the amount_in.
    let fee_amount: u64 = if reached_target {
        (amount_in as u128 * fee_factor / (10_000 - fee_factor).max(1)) as u64
    } else {
        amount_remaining.saturating_sub(amount_in)
    };

    (sqrt_price_next, amount_in, amount_out, fee_amount)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqrt_price_at_zero_tick_is_one() {
        let p = sqrt_price_at_tick(0);
        // Should be exactly Q64 = 2^64.
        assert_eq!(p, Q64);
    }

    #[test]
    fn sqrt_price_monotone() {
        let p0 = sqrt_price_at_tick(0);
        let p1 = sqrt_price_at_tick(1);
        let pm1 = sqrt_price_at_tick(-1);
        assert!(p1 > p0, "price increases with tick");
        assert!(pm1 < p0, "price decreases for negative tick");
    }

    #[test]
    fn tick_round_trip() {
        for t in [-1000, -100, -1, 0, 1, 100, 1000] {
            let p = sqrt_price_at_tick(t);
            let recovered = tick_at_sqrt_price(p);
            // Allow ±1 due to floor rounding.
            assert!(
                (recovered - t).abs() <= 1,
                "round-trip tick={} recovered={}",
                t,
                recovered
            );
        }
    }

    #[test]
    fn amount0_delta_zero_range() {
        let result = amount0_delta(Q64, Q64, 1000, false);
        assert_eq!(result, 0);
    }

    #[test]
    fn compute_swap_step_zero_liquidity() {
        let (p, ai, ao, fee) = compute_swap_step(Q64, Q64 * 2, 0, 1000, 30, false);
        assert_eq!(p, Q64);
        assert_eq!(ai, 0);
        assert_eq!(ao, 0);
        assert_eq!(fee, 0);
    }

    #[test]
    fn compute_swap_step_moves_price_up() {
        let (p_next, _, _, _) = compute_swap_step(Q64, Q64 * 2, 1_000_000, 1_000, 30, false);
        assert!(p_next > Q64, "price should increase for one_for_zero");
    }

    #[test]
    fn compute_swap_step_moves_price_down() {
        let (p_next, _, _, _) = compute_swap_step(Q64 * 2, Q64, 1_000_000, 1_000, 30, true);
        assert!(p_next < Q64 * 2, "price should decrease for zero_for_one");
    }
}
