/// Fixed-point arithmetic for Uniswap v3-style concentrated liquidity math.
///
/// These functions are adapted from `bedrock/contract/src/math.rs` and
/// `bedrock/contract/src/position.rs`.  The bedrock crate targets
/// `wasm32-unknown-unknown` and cannot be linked into a native tokio binary,
/// so the pure-arithmetic functions are duplicated here (no logic changes).
///
/// Representations
/// ---------------
/// * `sqrt_price` — Q64.64 (`u128`): upper 64 bits = integer, lower 64 = fraction.
/// * `fee_growth`  — Q128  (`u128`): entire value is fractional (scaled by 2^128).
/// * `liquidity`   — plain `u128` (drop-scale integer).

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// One in Q64.64 representation (2^64).
pub const Q64: u128 = 1u128 << 64;

pub const MIN_TICK: i32 = -887_272;
pub const MAX_TICK: i32 = 887_272;

// ---------------------------------------------------------------------------
// Multiply-shift helpers
// ---------------------------------------------------------------------------

/// Multiply two u128 values and shift right by 64 bits (Q64.64 multiply).
///
/// Uses 256-bit emulation via 128-bit half-words.  Saturates on overflow.
pub fn mul_shift64(a: u128, b: u128) -> u128 {
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

/// Multiply two u128 values and shift right by 128 bits (Q128 multiply).
///
/// Returns the high 128 bits of the full 256-bit product, using wrapping
/// arithmetic on the intermediate carries (same as V3 reference).
pub fn mul_shift128(a: u128, b: u128) -> u128 {
    let a_lo = a & u64::MAX as u128;
    let a_hi = a >> 64;
    let b_lo = b & u64::MAX as u128;
    let b_hi = b >> 64;

    let ll = a_lo * b_lo;
    let lh = a_lo * b_hi;
    let hl = a_hi * b_lo;
    let hh = a_hi * b_hi;

    let mid = (ll >> 64)
        .wrapping_add(lh & u64::MAX as u128)
        .wrapping_add(hl & u64::MAX as u128);
    let carry = mid >> 64;

    hh.wrapping_add(lh >> 64)
        .wrapping_add(hl >> 64)
        .wrapping_add(carry)
}

/// Divide `a` by `b` in Q64.64 space: `(a << 64) / b`, saturating.
pub fn div_q64(a: u128, b: u128) -> u128 {
    if b == 0 {
        return u128::MAX;
    }
    let a_shifted_hi = a >> 64;
    let a_shifted_lo = a << 64;
    if a_shifted_hi == 0 {
        return a_shifted_lo / b;
    }
    u128::MAX
}

// ---------------------------------------------------------------------------
// Tick ↔ sqrt_price conversion
// ---------------------------------------------------------------------------

/// Returns `sqrt_price_q64_64` for a given tick:
///   `sqrt(1.0001^tick)` stored in Q64.64.
pub fn sqrt_price_at_tick(tick: i32) -> u128 {
    let abs_tick = tick.unsigned_abs();

    // MAGIC[0] = floor(sqrt(1.0001) * 2^64).
    const MAGIC_0: u128 = 18_447_666_411_007_353_954;

    let mut magic = [0u128; 20];
    magic[0] = MAGIC_0;
    for k in 1..20usize {
        magic[k] = mul_shift64(magic[k - 1], magic[k - 1]);
        if magic[k] == 0 {
            for j in k..20 {
                magic[j] = u128::MAX;
            }
            break;
        }
    }

    let mut ratio: u128 = Q64;
    for k in 0..20u32 {
        if abs_tick & (1u32 << k) != 0 {
            ratio = mul_shift64(ratio, magic[k as usize]);
        }
    }

    if tick < 0 {
        ratio = if ratio == 0 { u128::MAX } else { u128::MAX / ratio };
    }

    ratio
}

/// Returns the floor tick for a given `sqrt_price_q64_64` (binary search).
pub fn tick_at_sqrt_price(sqrt_price: u128) -> i32 {
    let mut lo = MIN_TICK;
    let mut hi = MAX_TICK;
    while lo < hi {
        let mid = lo + (hi - lo + 1) / 2;
        if sqrt_price_at_tick(mid) <= sqrt_price {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

// ---------------------------------------------------------------------------
// Liquidity delta → token amounts (V3 piecewise formulas)
// ---------------------------------------------------------------------------

/// `amount0 = L * (sqrt_upper - sqrt_lower) / (sqrt_upper * sqrt_lower)`
pub fn amount0_delta(
    sqrt_lower: u128,
    sqrt_upper: u128,
    liquidity: u128,
    round_up: bool,
) -> u128 {
    if sqrt_lower >= sqrt_upper {
        return 0;
    }
    let diff = sqrt_upper.saturating_sub(sqrt_lower);
    let denom = mul_shift64(sqrt_upper, sqrt_lower);
    if denom == 0 {
        return 0;
    }
    let numer_scaled = liquidity.saturating_mul(diff);
    let denom_int = (denom >> 64).max(1);
    let result = numer_scaled / denom_int;
    if round_up && numer_scaled % denom_int != 0 {
        result.saturating_add(1)
    } else {
        result
    }
}

/// `amount1 = L * (sqrt_upper - sqrt_lower) / Q64`
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
    let result = mul_shift64(liquidity, diff);
    if round_up && (liquidity.wrapping_mul(diff) & (u64::MAX as u128)) != 0 {
        result.saturating_add(1)
    } else {
        result
    }
}

// ---------------------------------------------------------------------------
// Fee growth helpers
// ---------------------------------------------------------------------------

/// Compute the Q128 fee growth per unit of liquidity for one swap step.
///
/// `fee_amount * 2^128 / active_liquidity`
///
/// Returns 0 if `active_liquidity` is 0 (no liquidity → no fee distribution).
pub fn fee_growth_per_unit_q128(fee_amount: u128, active_liquidity: u128) -> u128 {
    if active_liquidity == 0 {
        return 0;
    }
    // (fee_amount << 128) / active_liquidity, approximated via two 64-bit shifts
    // to avoid actual 256-bit division.  Sufficient precision for the accumulator.
    let hi = fee_amount / active_liquidity;
    let lo_num = (fee_amount % active_liquidity) << 64;
    let lo = (lo_num / active_liquidity) << 64;
    hi.wrapping_shl(128).wrapping_add(lo)
}

