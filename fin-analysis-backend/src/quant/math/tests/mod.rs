use super::super::*;

#[test]
fn mul_shift64_identity() {
    // Q64 * Q64 >> 64 == Q64 (1.0 * 1.0 = 1.0 in Q64.64)
    assert_eq!(mul_shift64(Q64, Q64), Q64);
}

#[test]
fn mul_shift128_identity() {
    // 2^64 * 2^64 = 2^128, >> 128 = 1
    let result = mul_shift128(1u128 << 64, 1u128 << 64);
    assert_eq!(result, 1);
}

#[test]
fn mul_shift128_zero() {
    assert_eq!(mul_shift128(0, u128::MAX), 0);
    assert_eq!(mul_shift128(u128::MAX, 0), 0);
}

#[test]
fn sqrt_price_tick_zero_is_q64() {
    // tick 0 → sqrt(1) = 1.0 → Q64
    assert_eq!(sqrt_price_at_tick(0), Q64);
}

#[test]
fn sqrt_price_monotone() {
    let p_neg = sqrt_price_at_tick(-1);
    let p_zero = sqrt_price_at_tick(0);
    let p_pos = sqrt_price_at_tick(1);
    assert!(p_neg < p_zero, "negative tick → smaller price");
    assert!(p_pos > p_zero, "positive tick → larger price");
}

#[test]
fn tick_round_trip() {
    for t in [-1000i32, -100, -1, 0, 1, 100, 1000] {
        let p = sqrt_price_at_tick(t);
        let recovered = tick_at_sqrt_price(p);
        assert!(
            (recovered - t).abs() <= 1,
            "round-trip failed: tick={t}, recovered={recovered}"
        );
    }
}

#[test]
fn amount0_delta_zero_range() {
    // sqrt_lower == sqrt_upper → 0
    let result = amount0_delta(Q64, Q64, 1_000_000, false);
    assert_eq!(result, 0);
}

#[test]
fn amount1_delta_zero_range() {
    let result = amount1_delta(Q64, Q64, 1_000_000, false);
    assert_eq!(result, 0);
}

#[test]
fn amount0_increases_with_liquidity() {
    let sqrt_lower = sqrt_price_at_tick(-100);
    let sqrt_upper = sqrt_price_at_tick(100);
    let a = amount0_delta(sqrt_lower, sqrt_upper, 1_000, false);
    let b = amount0_delta(sqrt_lower, sqrt_upper, 2_000, false);
    assert!(b > a, "more liquidity → more token0");
}

#[test]
fn fee_growth_per_unit_zero_liquidity() {
    assert_eq!(fee_growth_per_unit_q128(1_000, 0), 0);
}

#[test]
fn fee_growth_per_unit_basic() {
    // fee_amount = active_liquidity → one full Q128 unit per liquidity unit,
    // i.e. the result should be non-zero.
    let result = fee_growth_per_unit_q128(1_000_000, 1_000_000);
    assert!(result > 0);
}
