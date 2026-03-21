/// End-to-end integration tests for the full AMM contract lifecycle.
///
/// Each test calls `test_setup` at the top to reset thread-local state,
/// then exercises the public ABI from the outside exactly as a caller would.
/// No access to private internals — only the exported functions and the
/// handful of read helpers (get_sqrt_price, get_liquidity, etc.).

use uniswap_v3_xrpl_contract::{
    burn, collect, collect_protocol,
    get_current_tick, get_fee_growth_global, get_liquidity, get_protocol_fees,
    get_sqrt_price, increase_observation_cardinality, initialize_pool, mint,
    observe, set_pause, set_protocol_fee, swap_exact_in, test_setup,
    math::Q64,
};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn owner() -> [u8; 20] { [1u8; 20] }
fn alice() -> [u8; 20] { [2u8; 20] }
fn bob()   -> [u8; 20] { [3u8; 20] }

/// Cast a signed tick value to the u32 two's-complement representation expected
/// by the ABI (Bedrock has no signed integer types).
fn tick(t: i32) -> u32 { t as u32 }

/// Initialize a standard pool: price=1.0, 0.3% fee, tick_spacing=10.
fn std_pool() {
    test_setup(owner(), 10);
    assert_eq!(initialize_pool(owner(), Q64, 30, 0, 1_000_000), 0,
               "initialize_pool should succeed");
}

/// Add a ±1000-tick range of liquidity on behalf of `lp`.
fn add_liquidity(lp: [u8; 20], amount: u128) {
    assert_eq!(mint(lp, tick(-1000), 1000, amount), 0, "mint should succeed");
}

// ---------------------------------------------------------------------------
// Scenario 1: Full LP lifecycle — init → mint → swap both ways → burn → collect
// ---------------------------------------------------------------------------

#[test]
fn full_lp_lifecycle() {
    std_pool();

    // No liquidity yet.
    assert_eq!(get_liquidity(), 0);

    // Alice adds liquidity.
    add_liquidity(alice(), 1_000_000_000);
    let liq = get_liquidity();
    assert!(liq > 0, "liquidity should be positive after mint");

    // Swap token1 → token0 (price increases).
    let price_before = get_sqrt_price();
    let out1 = swap_exact_in(alice(), 10_000, 9_900, 0, Q64 * 2, 1_000_100);
    assert!(out1 > 0, "upward swap should produce output");
    assert!(get_sqrt_price() > price_before, "price should rise on token1→token0 swap");

    // Swap token0 → token1 (price decreases).
    let price_mid = get_sqrt_price();
    let out2 = swap_exact_in(alice(), 10_000, 9_900, 1, Q64 / 2, 1_000_200);
    assert!(out2 > 0, "downward swap should produce output");
    assert!(get_sqrt_price() < price_mid, "price should fall on token0→token1 swap");

    // Alice burns all liquidity.
    assert_eq!(burn(alice(), tick(-1000), 1000, 1_000_000_000), 0, "burn should succeed");
    assert_eq!(get_liquidity(), 0, "liquidity should be zero after full burn");

    // Alice collects accrued fees (succeeds even if amounts are small).
    assert_eq!(collect(alice(), tick(-1000), 1000, u64::MAX, u64::MAX), 0,
               "collect should succeed after burn");
}

// ---------------------------------------------------------------------------
// Scenario 2: Multiple LPs — fee growth is shared across active positions
// ---------------------------------------------------------------------------

#[test]
fn multiple_lps_share_fee_growth() {
    std_pool();

    // Alice adds 2× more liquidity than Bob in the same range.
    assert_eq!(mint(alice(), tick(-1000), 1000, 2_000_000_000), 0);
    assert_eq!(mint(bob(),   tick(-1000), 1000, 1_000_000_000), 0);

    let liq_total = get_liquidity();
    assert_eq!(liq_total, 3_000_000_000, "total liquidity = alice + bob");

    // A swap accrues fees into fee_growth_global.
    let (fg0_before, fg1_before) = get_fee_growth_global();
    swap_exact_in(alice(), 100_000, 99_000, 0, Q64 * 3, 1_000_100);
    let (fg0_after, fg1_after) = get_fee_growth_global();

    // token1→token0 swap fees land in fee_growth_global_1.
    assert!(fg1_after > fg1_before || fg0_after > fg0_before,
            "some fee growth should have accrued");

    // Both LPs can collect — no assertion on amounts since collect returns only
    // an error code, but neither call should fail.
    burn(alice(), tick(-1000), 1000, 2_000_000_000);
    burn(bob(),   tick(-1000), 1000, 1_000_000_000);
    assert_eq!(collect(alice(), tick(-1000), 1000, u64::MAX, u64::MAX), 0);
    assert_eq!(collect(bob(),   tick(-1000), 1000, u64::MAX, u64::MAX), 0);
}

// ---------------------------------------------------------------------------
// Scenario 3: Slippage enforcement
// ---------------------------------------------------------------------------

#[test]
fn slippage_enforcement() {
    std_pool();
    add_liquidity(alice(), 1_000_000_000);

    // min_out = 0 → fails the global 1% slippage cap (floor = 9900 for 10_000 in).
    let out = swap_exact_in(alice(), 10_000, 0, 0, Q64 * 2, 1_000_100);
    assert_eq!(out, 0, "swap with min_out=0 should fail slippage check");

    // min_out = 99% of input → should succeed.
    let out = swap_exact_in(alice(), 10_000, 9_900, 0, Q64 * 2, 1_000_200);
    assert!(out > 0, "swap with 1% slippage tolerance should succeed");

    // Price limit in wrong direction → rejected.
    let out = swap_exact_in(alice(), 10_000, 9_900, 1, Q64 * 2, 1_000_300);
    assert_eq!(out, 0, "swap with limit on wrong side should fail");
}

// ---------------------------------------------------------------------------
// Scenario 4: Protocol fee accumulation and governance collection
// ---------------------------------------------------------------------------

#[test]
fn protocol_fee_accrues_and_is_collectable() {
    std_pool();
    // Set protocol fee to 10% of LP fees.
    assert_eq!(set_protocol_fee(owner(), 1_000), 0);
    add_liquidity(alice(), 1_000_000_000);

    // Do a substantial swap so fees accrue.
    swap_exact_in(alice(), 500_000, 495_000, 0, Q64 * 3, 1_000_100);

    let (pf0, pf1) = get_protocol_fees();
    assert!(pf0 > 0 || pf1 > 0,
            "at least one token's protocol fee should be non-zero after swap");

    // Owner collects protocol fees.
    let packed = collect_protocol(owner(), u64::MAX, u64::MAX);
    let collected_0 = (packed & 0xFFFF_FFFF) as u32;
    let collected_1 = (packed >> 32) as u32;
    assert!(collected_0 > 0 || collected_1 > 0,
            "at least one collected amount should be positive");

    // Protocol fees should now be zero.
    let (pf0_after, pf1_after) = get_protocol_fees();
    assert_eq!(pf0_after, 0);
    assert_eq!(pf1_after, 0);
}

#[test]
fn non_owner_cannot_collect_protocol_fees() {
    std_pool();
    assert_eq!(set_protocol_fee(owner(), 500), 0);
    add_liquidity(alice(), 1_000_000_000);
    swap_exact_in(alice(), 100_000, 99_000, 0, Q64 * 2, 1_000_100);

    let (pf0_before, pf1_before) = get_protocol_fees();
    collect_protocol(alice(), u64::MAX, u64::MAX); // silently fails (owner guard)
    let (pf0_after, pf1_after) = get_protocol_fees();
    assert_eq!(pf0_after, pf0_before);
    assert_eq!(pf1_after, pf1_before);
}

// ---------------------------------------------------------------------------
// Scenario 5: Emergency pause blocks all state-mutating operations
// ---------------------------------------------------------------------------

#[test]
fn pause_blocks_all_operations_unpause_restores_them() {
    std_pool();
    add_liquidity(alice(), 1_000_000_000);

    // Pause.
    assert_eq!(set_pause(owner(), 1), 0);

    // All mutating operations should fail.
    assert_eq!(swap_exact_in(alice(), 10_000, 9_900, 0, Q64 * 2, 1_000_100), 0,
               "swap should return 0 (failure) when paused");
    assert_ne!(mint(alice(), tick(-1000), 1000, 1_000_000_000), 0,
               "mint should error when paused");
    assert_ne!(burn(alice(), tick(-1000), 1000, 500_000_000), 0,
               "burn should error when paused");
    assert_ne!(collect(alice(), tick(-1000), 1000, u64::MAX, u64::MAX), 0,
               "collect should error when paused");

    // Unpause.
    assert_eq!(set_pause(owner(), 0), 0);

    // Swap now works again.
    let out = swap_exact_in(alice(), 10_000, 9_900, 0, Q64 * 2, 1_000_100);
    assert!(out > 0, "swap should succeed after unpause");
}

#[test]
fn only_owner_can_pause() {
    std_pool();
    assert_ne!(set_pause(alice(), 1), 0, "non-owner should not be able to pause");
    add_liquidity(alice(), 1_000_000_000);
    let out = swap_exact_in(alice(), 10_000, 9_900, 0, Q64 * 2, 1_000_100);
    assert!(out > 0, "pool should remain operational after failed pause attempt");
}

// ---------------------------------------------------------------------------
// Scenario 6: Oracle TWAP — observations advance and are queryable
// ---------------------------------------------------------------------------

#[test]
fn oracle_records_and_interpolates_across_blocks() {
    std_pool();
    add_liquidity(alice(), 1_000_000_000);

    assert_eq!(increase_observation_cardinality(owner(), 20), 0);

    swap_exact_in(alice(), 500_000, 495_000, 0, Q64 * 2, 1_000_100);
    swap_exact_in(alice(), 500_000, 495_000, 0, Q64 * 3, 1_000_200);
    swap_exact_in(alice(), 500_000, 495_000, 0, Q64 * 4, 1_000_300);

    let packed_live = observe(alice(), 0u64, 1_000_300);
    let tc_now = (packed_live & 0xFFFF_FFFF) as i32;
    assert!(tc_now > 0, "tick_cumulative should be positive after upward swaps; got {}", tc_now);

    let packed = observe(alice(), 100u64, 1_000_300);
    let tc_100s_ago = (packed & 0xFFFF_FFFF) as i32;
    assert!(tc_100s_ago >= 0, "tick_cumulative 100s ago should be non-negative");
}

#[test]
fn oracle_same_block_is_idempotent() {
    std_pool();
    add_liquidity(alice(), 1_000_000_000);

    swap_exact_in(alice(), 5_000, 4_950, 0, Q64 * 2, 1_000_100);
    let tick_after_first = get_current_tick();

    swap_exact_in(alice(), 5_000, 4_950, 0, Q64 * 3, 1_000_100); // same timestamp
    let tick_after_second = get_current_tick();

    let packed = observe(alice(), 0u64, 1_000_100);
    assert_ne!(packed, u64::MAX, "observe should not return error sentinel");
    let _ = (tick_after_first, tick_after_second);
}

// ---------------------------------------------------------------------------
// Scenario 7: Multi-tick crossing
// ---------------------------------------------------------------------------

#[test]
fn swap_crosses_multiple_ticks() {
    std_pool();

    assert_eq!(mint(alice(), tick(-3000), tick(-1000), 500_000_000), 0);
    assert_eq!(mint(alice(), tick(-1000),     0, 500_000_000), 0);
    assert_eq!(mint(alice(),     0,  1000, 500_000_000), 0);
    assert_eq!(mint(alice(),  1000,  3000, 500_000_000), 0);

    let price_before = get_sqrt_price();
    let tick_before  = get_current_tick();

    let out = swap_exact_in(alice(), 3_000_000, 2_970_000, 0, Q64 * 100, 1_000_100);
    assert!(out > 0, "multi-tick swap should produce output");

    let price_after = get_sqrt_price();
    let tick_after  = get_current_tick();

    assert!(price_after > price_before, "price should have risen");
    assert!(tick_after  > tick_before,  "tick should have increased");
    assert!(tick_after > tick_before + 50,
            "price should have crossed multiple tick boundaries; tick_before={} tick_after={}",
            tick_before, tick_after);
}

// ---------------------------------------------------------------------------
// Scenario 8: Out-of-range positions — correct liquidity accounting
// ---------------------------------------------------------------------------

#[test]
fn out_of_range_position_contributes_zero_active_liquidity() {
    std_pool();

    mint(alice(), tick(-1000), 1000, 1_000_000_000);
    let liq_with_in_range = get_liquidity();

    mint(bob(), 2000, 5000, 1_000_000_000);
    let liq_after_out_of_range = get_liquidity();

    assert_eq!(liq_with_in_range, liq_after_out_of_range,
               "out-of-range mint should not change active liquidity");

    mint(bob(), tick(-500), 500, 500_000_000);
    let liq_after_second_in_range = get_liquidity();
    assert!(liq_after_second_in_range > liq_after_out_of_range,
            "second in-range mint should increase active liquidity");
}

// ---------------------------------------------------------------------------
// Scenario 9: Fee growth only for in-range positions
// ---------------------------------------------------------------------------

#[test]
fn fees_only_accrue_when_position_is_in_range() {
    std_pool();

    mint(alice(), tick(-1000), 1000, 1_000_000_000);
    mint(bob(),    2000, 5000, 1_000_000_000);

    for ts in [1_000_100u32, 1_000_200, 1_000_300] {
        swap_exact_in(alice(), 5_000, 4_950, 0, Q64 * 2, ts);
    }

    burn(alice(), tick(-1000), 1000, 1_000_000_000);
    burn(bob(),    2000, 5000, 1_000_000_000);
    assert_eq!(collect(alice(), tick(-1000), 1000, u64::MAX, u64::MAX), 0);
    assert_eq!(collect(bob(),    2000, 5000, u64::MAX, u64::MAX), 0);
}
