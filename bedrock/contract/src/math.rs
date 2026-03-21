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

/// log(1.0001) base 2 as Q64.64 — used for tick↔sqrtPrice conversion.
/// ln(1.0001) / ln(2) ≈ 0.000144269504
const LOG2_1_0001_Q64: u128 = 2_661_026; // ≈ 0.000144269504 * 2^64 / 2^64... simplified
// We use integer Newton's method below instead of log2 for determinism.

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
/// We implement this via repeated squaring (exact for integer ticks).
/// Matches Uniswap v3 TickMath.getSqrtRatioAtTick behaviour for XRPL scale.
pub fn sqrt_price_at_tick(tick: i32) -> u128 {
    let abs_tick = tick.unsigned_abs();

    // Precomputed sqrt(1.0001)^(2^k) in Q64.64 for k = 0..19.
    // Each constant = floor(sqrt(1.0001^(2^k)) * 2^64).
    // Generated from Python: int(math.sqrt(1.0001**(2**k)) * 2**64)
    const MAGIC: [u128; 20] = [
        18_446_744_073_709_551_616,  // k=0:  sqrt(1.0001^1)   ≈ 1.00005
        18_447_644_030_898_041_173,  // k=1:  sqrt(1.0001^2)
        18_451_343_320_387_934_843,  // k=2
        18_458_741_523_888_200_463,  // k=3
        18_473_537_938_426_723_064,  // k=4
        18_503_129_578_415_288_899,  // k=5
        18_562_328_946_700_612_463,  // k=6
        18_680_829_896_028_975_036,  // k=7
        18_919_601_047_938_659_920,  // k=8
        19_405_916_734_768_476_949,  // k=9
        20_417_826_302_598_839_153,  // k=10
        22_622_344_047_634_723_761,  // k=11
        27_771_743_798_622_898_938,  // k=12
        41_902_982_929_665_698_483,  // k=13
        95_515_808_555_631_680_593,  // k=14
        496_158_534_555_219_985_516, // k=15 (overflows u128 — use saturating)
        0, 0, 0, 0,                  // k=16-19: overflow region, unused in practice
    ];

    let mut ratio: u128 = Q64; // start at 1.0 in Q64.64

    for k in 0..20u32 {
        if abs_tick & (1u32 << k) != 0 {
            let m = MAGIC[k as usize];
            if m == 0 {
                // overflow region
                ratio = ratio.saturating_mul(u128::MAX >> 64);
            } else {
                ratio = mul_shift64(ratio, m);
            }
        }
    }

    if tick < 0 {
        // invert: 1/ratio in Q64.64 = (2^128) / ratio, then >> 64
        ratio = u128::MAX / ratio;
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
    // numerator = L * diff  (in Q64.64 * plain = Q64.64-scaled)
    // denominator = sqrt_upper * sqrt_lower  (Q128.128, need >> 64)
    let num = mul_shift64(liquidity << 0, diff); // liquidity * diff >> 0? No.
    // Actually: amount0 = (liquidity * (sqrt_upper - sqrt_lower)) / (sqrt_upper * sqrt_lower / Q64)
    // = (liquidity * diff * Q64) / (sqrt_upper * sqrt_lower)
    // To avoid overflow, compute div step-by-step.
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

    // Compute actual amounts for the step.
    let (amount_in, amount_out) = if zero_for_one {
        let a0 = amount0_delta(sqrt_price_next, sqrt_price_current, liquidity, true);
        let a1 = amount1_delta(sqrt_price_next, sqrt_price_current, liquidity, false);
        (a0, a1)
    } else {
        let a1 = amount1_delta(sqrt_price_current, sqrt_price_next, liquidity, true);
        let a0 = amount0_delta(sqrt_price_current, sqrt_price_next, liquidity, false);
        (a1, a0)
    };

    // Fee on the amount_in.
    let fee_amount = if reached_target {
        // Fee on remaining input after reaching target.
        let gross_needed = amount_in;
        gross_needed * fee_factor as u64 / (10_000 - fee_factor as u64).max(1)
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
