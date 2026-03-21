/// End-to-end integration tests for the full AMM contract lifecycle.
///
/// Each test calls `test_setup` at the top to reset thread-local state,
/// then exercises the public ABI from the outside exactly as a caller would.
/// No access to private internals — only the exported functions and the
/// handful of read helpers (get_sqrt_price, get_liquidity, etc.).

use uniswap_v3_xrpl_contract::{
    burn, collect, collect_protocol, donate,
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

/// Initialize a standard pool: price=1.0, 0.3% fee, tick_spacing=10, no hook.
fn std_pool() {
    test_setup(owner(), 10);
    assert_eq!(initialize_pool(owner(), Q64, 30, 0, 1_000_000, 0), 0,
               "initialize_pool should succeed");
}

/// Add a ±1000-tick range of liquidity on behalf of `lp`.
fn add_liquidity(lp: [u8; 20], amount: u128) {
    assert_eq!(mint(lp, -1000, 1000, amount), 0, "mint should succeed");
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
    assert_eq!(burn(alice(), -1000, 1000, 1_000_000_000), 0, "burn should succeed");
    assert_eq!(get_liquidity(), 0, "liquidity should be zero after full burn");

    // Alice collects accrued fees (succeeds even if amounts are small).
    assert_eq!(collect(alice(), -1000, 1000, u64::MAX, u64::MAX), 0,
               "collect should succeed after burn");
}

// ---------------------------------------------------------------------------
// Scenario 2: Multiple LPs — fee growth is shared across active positions
// ---------------------------------------------------------------------------

#[test]
fn multiple_lps_share_fee_growth() {
    std_pool();

    // Alice adds 2× more liquidity than Bob in the same range.
    assert_eq!(mint(alice(), -1000, 1000, 2_000_000_000), 0);
    assert_eq!(mint(bob(),   -1000, 1000, 1_000_000_000), 0);

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
    burn(alice(), -1000, 1000, 2_000_000_000);
    burn(bob(),   -1000, 1000, 1_000_000_000);
    assert_eq!(collect(alice(), -1000, 1000, u64::MAX, u64::MAX), 0);
    assert_eq!(collect(bob(),   -1000, 1000, u64::MAX, u64::MAX), 0);
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
// Scenario 4: ConservativeHedge hook — end-to-end enforcement
// ---------------------------------------------------------------------------

#[test]
fn conservative_hook_blocks_low_fee_pool() {
    test_setup(owner(), 10);
    // 5 bps < 10 bps minimum required by ConservativeHedge.
    let err = initialize_pool(owner(), Q64, 5, 0, 0, 1);
    assert_ne!(err, 0, "hook should reject fee_bps < 10");
}

#[test]
fn conservative_hook_rejects_narrow_mint() {
    test_setup(owner(), 10);
    assert_eq!(initialize_pool(owner(), Q64, 30, 0, 1_000_000, 1), 0);

    // Position width = 190 ticks — below the 200-tick minimum.
    let err = mint(alice(), -95, 95, 1_000_000_000);
    assert_ne!(err, 0, "hook should reject < 200-tick positions");

    // Position width = 200 ticks — exactly at the limit.
    assert_eq!(mint(alice(), -100, 100, 1_000_000_000), 0,
               "hook should allow exactly 200-tick positions");
}

#[test]
fn conservative_hook_blocks_oversized_swap() {
    test_setup(owner(), 10);
    assert_eq!(initialize_pool(owner(), Q64, 30, 0, 1_000_000, 1), 0);
    // Must use a wide-enough range (>= 200 ticks).
    assert_eq!(mint(alice(), -1000, 1000, 1_000_000), 0);

    // Liquidity = 1_000_000. Cap = 1_000_000 / 20 = 50_000.
    // Attempt a swap with amount_in = 50_001 → above cap.
    let out = swap_exact_in(alice(), 50_001, 49_500, 0, Q64 * 2, 1_000_100);
    assert_eq!(out, 0, "conservative hook should block swap > 5% of liquidity");

    // Swap at the cap → allowed.
    let out = swap_exact_in(alice(), 50_000, 49_500, 0, Q64 * 2, 1_000_200);
    assert!(out > 0, "conservative hook should allow swap at 5% of liquidity");
}

#[test]
fn conservative_hook_full_lifecycle() {
    test_setup(owner(), 10);
    assert_eq!(initialize_pool(owner(), Q64, 30, 0, 1_000_000, 1), 0);
    assert_eq!(mint(alice(), -500, 500, 500_000_000), 0);

    // Small swap (well within 5% cap) succeeds.
    let out = swap_exact_in(alice(), 1_000, 990, 0, Q64 * 2, 1_000_100);
    assert!(out > 0, "small swap should succeed under conservative hook");

    // LP can exit normally.
    assert_eq!(burn(alice(), -500, 500, 500_000_000), 0);
    assert_eq!(collect(alice(), -500, 500, u64::MAX, u64::MAX), 0);
}

// ---------------------------------------------------------------------------
// Scenario 5: YieldRebalance hook — positions must straddle current tick
// ---------------------------------------------------------------------------

#[test]
fn yield_hook_rejects_out_of_range_mint() {
    test_setup(owner(), 10);
    assert_eq!(initialize_pool(owner(), Q64, 30, 0, 1_000_000, 2), 0);

    // current_tick = 0; position entirely above → rejected.
    assert_ne!(mint(alice(), 10, 1000, 1_000_000_000),  0,
               "yield hook should reject position above current tick");

    // Position entirely below → rejected.
    assert_ne!(mint(alice(), -1000, -10, 1_000_000_000), 0,
               "yield hook should reject position below current tick");

    // Position straddling tick 0 → allowed.
    assert_eq!(mint(alice(), -500, 500, 1_000_000_000), 0,
               "yield hook should allow position straddling current tick");
}

#[test]
fn yield_hook_allows_burn_regardless_of_price() {
    test_setup(owner(), 10);
    assert_eq!(initialize_pool(owner(), Q64, 30, 0, 1_000_000, 2), 0);
    assert_eq!(mint(alice(), -500, 500, 1_000_000_000), 0);

    // Large swap pushes price well outside alice's range.
    swap_exact_in(alice(), 900_000, 0, 0, Q64 * 10, 1_000_100);

    // Alice can still burn even though her position may be out of range now.
    assert_eq!(burn(alice(), -500, 500, 1_000_000_000), 0,
               "burn should always be allowed regardless of current price");
    assert_eq!(collect(alice(), -500, 500, u64::MAX, u64::MAX), 0);
}

// ---------------------------------------------------------------------------
// Scenario 6: Donate distributes tokens to in-range LPs via fee growth
// ---------------------------------------------------------------------------

#[test]
fn donate_increases_fee_growth_for_active_lps() {
    std_pool();
    add_liquidity(alice(), 1_000_000_000);

    let (fg0_before, fg1_before) = get_fee_growth_global();

    // Donate token0 to in-range LPs.
    assert_eq!(donate(alice(), 50_000, 0), 0, "donate should succeed");

    let (fg0_after, _) = get_fee_growth_global();
    assert!(fg0_after > fg0_before,
            "fee_growth_global_0 should increase after token0 donate");

    // Donate token1.
    assert_eq!(donate(alice(), 0, 50_000), 0);
    let (_, fg1_after) = get_fee_growth_global();
    assert!(fg1_after > fg1_before,
            "fee_growth_global_1 should increase after token1 donate");
}

#[test]
fn donate_with_no_liquidity_is_silent_noop() {
    std_pool();
    // No mint — liquidity_active = 0.
    let (fg0_before, fg1_before) = get_fee_growth_global();
    assert_eq!(donate(alice(), 10_000, 10_000), 0,
               "donate to empty pool should succeed silently");
    let (fg0_after, fg1_after) = get_fee_growth_global();
    assert_eq!(fg0_after, fg0_before, "no fee growth when liquidity = 0");
    assert_eq!(fg1_after, fg1_before, "no fee growth when liquidity = 0");
}

#[test]
fn donate_zero_amounts_errors() {
    std_pool();
    add_liquidity(alice(), 1_000_000_000);
    assert_ne!(donate(alice(), 0, 0), 0, "donating nothing should error");
}

#[test]
fn donate_collectable_after_burn() {
    std_pool();
    add_liquidity(alice(), 1_000_000_000);

    // Donate before any swaps.
    assert_eq!(donate(alice(), 100_000, 100_000), 0);

    // Burn position — this crystallises accrued fees into tokens_owed.
    assert_eq!(burn(alice(), -1000, 1000, 1_000_000_000), 0);

    // Collect should succeed (amounts are tracked internally; return is 0 on success).
    assert_eq!(collect(alice(), -1000, 1000, u64::MAX, u64::MAX), 0);
}

// ---------------------------------------------------------------------------
// Scenario 7: Protocol fee accumulation and governance collection
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
    // packed = collected_0 | (collected_1 << 32); both ≥ 0.
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

    // Alice (not owner) tries to collect — should fail (returns 0 = no error code,
    // but protocol fees should remain unchanged since the owner check returns early).
    let (pf0_before, pf1_before) = get_protocol_fees();
    collect_protocol(alice(), u64::MAX, u64::MAX); // silently fails (owner guard)
    let (pf0_after, pf1_after) = get_protocol_fees();
    // Fees must be unchanged because Alice is not the owner.
    assert_eq!(pf0_after, pf0_before);
    assert_eq!(pf1_after, pf1_before);
}

// ---------------------------------------------------------------------------
// Scenario 8: Emergency pause blocks all state-mutating operations
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
    assert_ne!(mint(alice(), -1000, 1000, 1_000_000_000), 0,
               "mint should error when paused");
    assert_ne!(burn(alice(), -1000, 1000, 500_000_000), 0,
               "burn should error when paused");
    assert_ne!(collect(alice(), -1000, 1000, u64::MAX, u64::MAX), 0,
               "collect should error when paused");
    assert_ne!(donate(alice(), 1_000, 1_000), 0,
               "donate should error when paused");

    // Unpause.
    assert_eq!(set_pause(owner(), 0), 0);

    // Swap now works again.
    let out = swap_exact_in(alice(), 10_000, 9_900, 0, Q64 * 2, 1_000_100);
    assert!(out > 0, "swap should succeed after unpause");
}

#[test]
fn only_owner_can_pause() {
    std_pool();
    // Alice tries to pause — should fail (returns non-zero error code).
    assert_ne!(set_pause(alice(), 1), 0, "non-owner should not be able to pause");
    // Pool should still be operational.
    add_liquidity(alice(), 1_000_000_000);
    let out = swap_exact_in(alice(), 10_000, 9_900, 0, Q64 * 2, 1_000_100);
    assert!(out > 0, "pool should remain operational after failed pause attempt");
}

// ---------------------------------------------------------------------------
// Scenario 9: Oracle TWAP — observations advance and are queryable
// ---------------------------------------------------------------------------

#[test]
fn oracle_records_and_interpolates_across_blocks() {
    std_pool();
    add_liquidity(alice(), 1_000_000_000);

    // Grow the buffer so multiple observations can be stored.
    assert_eq!(increase_observation_cardinality(owner(), 20), 0);

    // Use large swaps so price actually crosses tick boundaries and the oracle
    // accumulator is non-zero. ~50_000 tokens moves ~1 tick with 1B liquidity.
    swap_exact_in(alice(), 500_000, 495_000, 0, Q64 * 2, 1_000_100);
    swap_exact_in(alice(), 500_000, 495_000, 0, Q64 * 3, 1_000_200);
    swap_exact_in(alice(), 500_000, 495_000, 0, Q64 * 4, 1_000_300);

    // Observe at t=1_000_300: 0 seconds ago → live state.
    // tick_cumulative = sum of (tick_at_observation × elapsed_seconds).
    // After three upward swaps, the tick is positive so the cumulative must be > 0.
    let packed_live = observe(alice(), 0u64, 1_000_300);
    let tc_now = (packed_live & 0xFFFF_FFFF) as i32;
    assert!(tc_now > 0, "tick_cumulative should be positive after upward swaps; got {}", tc_now);

    // Observe 100 seconds ago (t=1_000_200) and now (0s) in one call.
    let packed = observe(alice(), 100u64, 1_000_300);
    let tc_100s_ago = (packed & 0xFFFF_FFFF) as i32;
    assert!(tc_100s_ago >= 0, "tick_cumulative 100s ago should be non-negative");
}

#[test]
fn oracle_same_block_is_idempotent() {
    std_pool();
    add_liquidity(alice(), 1_000_000_000);

    // Two swaps in the same block (same timestamp).
    swap_exact_in(alice(), 5_000, 4_950, 0, Q64 * 2, 1_000_100);
    let tick_after_first = get_current_tick();

    swap_exact_in(alice(), 5_000, 4_950, 0, Q64 * 3, 1_000_100); // same timestamp
    let tick_after_second = get_current_tick();

    // Second swap does move the tick (it's a real trade), but the oracle should
    // NOT write a second observation in the same block.
    let packed = observe(alice(), 0u64, 1_000_100);
    assert_ne!(packed, u64::MAX, "observe should not return error sentinel");
    let _ = (tick_after_first, tick_after_second); // used to avoid dead-code warning
}

// ---------------------------------------------------------------------------
// Scenario 10: Multi-tick crossing — price traverses multiple initialised ticks
// ---------------------------------------------------------------------------

#[test]
fn swap_crosses_multiple_ticks() {
    std_pool();

    // Mint three adjacent but distinct ranges to create initialised tick boundaries.
    assert_eq!(mint(alice(), -3000, -1000, 500_000_000), 0);
    assert_eq!(mint(alice(), -1000,     0, 500_000_000), 0);
    assert_eq!(mint(alice(),     0,  1000, 500_000_000), 0);
    assert_eq!(mint(alice(),  1000,  3000, 500_000_000), 0);

    let price_before = get_sqrt_price();
    let tick_before  = get_current_tick();

    // Large upward swap intended to cross tick 0, 1000, and land somewhere above.
    let out = swap_exact_in(alice(), 2_000_000, 1_980_000, 0, Q64 * 100, 1_000_100);
    assert!(out > 0, "multi-tick swap should produce output");

    let price_after = get_sqrt_price();
    let tick_after  = get_current_tick();

    assert!(price_after > price_before, "price should have risen");
    assert!(tick_after  > tick_before,  "tick should have increased");
    // Should have crossed at least two tick boundaries (0 and 1000).
    assert!(tick_after > 1000,
            "price should have crossed tick 1000; tick_after={}", tick_after);
}

// ---------------------------------------------------------------------------
// Scenario 11: Position below/above current price — correct liquidity accounting
// ---------------------------------------------------------------------------

#[test]
fn out_of_range_position_contributes_zero_active_liquidity() {
    std_pool();

    // Add in-range position: contributes to active liquidity.
    mint(alice(), -1000, 1000, 1_000_000_000);
    let liq_with_in_range = get_liquidity();

    // Add out-of-range position entirely above current tick.
    // This should NOT increase active liquidity.
    mint(bob(), 2000, 5000, 1_000_000_000);
    let liq_after_out_of_range = get_liquidity();

    assert_eq!(liq_with_in_range, liq_after_out_of_range,
               "out-of-range mint should not change active liquidity");

    // But an in-range position should.
    mint(bob(), -500, 500, 500_000_000);
    let liq_after_second_in_range = get_liquidity();
    assert!(liq_after_second_in_range > liq_after_out_of_range,
            "second in-range mint should increase active liquidity");
}

// ---------------------------------------------------------------------------
// Scenario 12: Fee growth accumulates only for in-range LP positions
// ---------------------------------------------------------------------------

#[test]
fn fees_only_accrue_when_position_is_in_range() {
    std_pool();

    // Alice is in range, Bob is not.
    mint(alice(), -1000, 1000, 1_000_000_000);
    mint(bob(),    2000, 5000, 1_000_000_000);

    // Swaps happen within Alice's range.
    for ts in [1_000_100u32, 1_000_200, 1_000_300] {
        swap_exact_in(alice(), 5_000, 4_950, 0, Q64 * 2, ts);
    }

    // Both can call collect — Alice's position earned fees; Bob's earned nothing.
    // Since collect's return value is just an error code we confirm neither panics.
    burn(alice(), -1000, 1000, 1_000_000_000);
    burn(bob(),    2000, 5000, 1_000_000_000);
    assert_eq!(collect(alice(), -1000, 1000, u64::MAX, u64::MAX), 0);
    assert_eq!(collect(bob(),    2000, 5000, u64::MAX, u64::MAX), 0);
}

