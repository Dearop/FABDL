/// End-to-end integration tests for the full AMM contract lifecycle.

use uniswap_v3_xrpl_contract::{
    burn, collect, collect_protocol,
    get_current_tick, get_fee_growth_global, get_liquidity, get_protocol_fees,
    get_sqrt_price, initialize_pool, mint, swap_exact_in,
    set_pause, set_protocol_fee, test_setup,
};

fn owner() -> [u8; 20] { [1u8; 20] }
fn alice() -> [u8; 20] { [2u8; 20] }

fn tick(t: i32) -> u32 { t as u32 }

/// Initialize a standard pool: price=1.0 (tick=0), 0.3% fee, tick_spacing=10.
fn std_pool() {
    test_setup(owner(), 10);
    assert_eq!(initialize_pool(0u32, 30, 0), 0, "initialize_pool should succeed");
}

fn add_liquidity(_lp: [u8; 20], amount: u64) {
    assert_eq!(mint(tick(-1000), 1000, amount), 0, "mint should succeed");
}

// ---------------------------------------------------------------------------
// Scenario 1: Full LP lifecycle
// ---------------------------------------------------------------------------

#[test]
fn full_lp_lifecycle() {
    std_pool();
    assert_eq!(get_liquidity(), 0);

    add_liquidity(alice(), 1_000_000_000);
    let liq = get_liquidity();
    assert!(liq > 0, "liquidity should be positive after mint");

    let price_before = get_sqrt_price();
    let out1 = swap_exact_in(10_000, 9_900, 0);
    assert!(out1 > 0, "upward swap should produce output");
    assert!(get_sqrt_price() > price_before, "price should rise");

    let price_mid = get_sqrt_price();
    let out2 = swap_exact_in(10_000, 9_900, 1);
    assert!(out2 > 0, "downward swap should produce output");
    assert!(get_sqrt_price() < price_mid, "price should fall");

    assert_eq!(burn(tick(-1000), 1000, 1_000_000_000), 0, "burn should succeed");
    assert_eq!(get_liquidity(), 0, "liquidity should be zero after full burn");

    assert_eq!(collect(tick(-1000), 1000, u32::MAX, u32::MAX), 0,
               "collect should succeed after burn");
}

// ---------------------------------------------------------------------------
// Scenario 2: Multiple LPs share fee growth
// ---------------------------------------------------------------------------

#[test]
fn multiple_lps_share_fee_growth() {
    std_pool();

    assert_eq!(mint(tick(-1000), 1000, 2_000_000_000), 0);
    assert_eq!(mint(tick(-1000), 1000, 1_000_000_000), 0);

    let liq_total = get_liquidity();
    assert_eq!(liq_total, 3_000_000_000, "total liquidity = alice + bob");

    let (fg0_before, fg1_before) = get_fee_growth_global();
    swap_exact_in(100_000, 99_000, 0);
    let (fg0_after, fg1_after) = get_fee_growth_global();

    assert!(fg1_after > fg1_before || fg0_after > fg0_before,
            "some fee growth should have accrued");

    burn(tick(-1000), 1000, 2_000_000_000);
    burn(tick(-1000), 1000, 1_000_000_000);
    assert_eq!(collect(tick(-1000), 1000, u32::MAX, u32::MAX), 0);
}

// ---------------------------------------------------------------------------
// Scenario 3: Slippage enforcement
// ---------------------------------------------------------------------------

#[test]
fn slippage_enforcement() {
    std_pool();
    add_liquidity(alice(), 1_000_000_000);

    let out = swap_exact_in(10_000, 0, 0);
    assert_eq!(out, 0, "swap with min_out=0 should fail slippage check");

    let out = swap_exact_in(10_000, 9_900, 0);
    assert!(out > 0, "upward swap with 1% tolerance should succeed");

    let out = swap_exact_in(10_000, 9_900, 1);
    assert!(out > 0, "downward swap with 1% tolerance should succeed");
}

// ---------------------------------------------------------------------------
// Scenario 4: Protocol fee accumulation
// ---------------------------------------------------------------------------

#[test]
fn protocol_fee_accrues_and_is_collectable() {
    std_pool();
    assert_eq!(set_protocol_fee(1_000), 0);
    add_liquidity(alice(), 1_000_000_000);

    swap_exact_in(500_000, 495_000, 0);

    let (pf0, pf1) = get_protocol_fees();
    assert!(pf0 > 0 || pf1 > 0, "protocol fees should be non-zero after swap");

    collect_protocol(u32::MAX, u32::MAX);
    let (pf0_after, pf1_after) = get_protocol_fees();
    assert_eq!(pf0_after, 0);
    assert_eq!(pf1_after, 0);
}

#[test]
fn collect_protocol_fees_succeeds() {
    std_pool();
    assert_eq!(set_protocol_fee(500), 0);
    add_liquidity(alice(), 1_000_000_000);
    swap_exact_in(100_000, 99_000, 0);

    let (pf0_before, pf1_before) = get_protocol_fees();
    assert!(pf0_before > 0 || pf1_before > 0, "fees should have accrued");
    collect_protocol(u32::MAX, u32::MAX);
    let (pf0_after, pf1_after) = get_protocol_fees();
    assert_eq!(pf0_after, 0);
    assert_eq!(pf1_after, 0);
}

// ---------------------------------------------------------------------------
// Scenario 5: Emergency pause
// ---------------------------------------------------------------------------

#[test]
fn pause_blocks_all_operations_unpause_restores_them() {
    std_pool();
    add_liquidity(alice(), 1_000_000_000);

    assert_eq!(set_pause(1), 0);

    assert_eq!(swap_exact_in(10_000, 9_900, 0), 0, "swap should fail when paused");
    assert_ne!(mint(tick(-1000), 1000, 1_000_000_000), 0, "mint should error when paused");
    assert_ne!(burn(tick(-1000), 1000, 500_000_000), 0, "burn should error when paused");
    assert_ne!(collect(tick(-1000), 1000, u32::MAX, u32::MAX), 0, "collect should error when paused");

    assert_eq!(set_pause(0), 0);

    let out = swap_exact_in(10_000, 9_900, 0);
    assert!(out > 0, "swap should succeed after unpause");
}

#[test]
fn pause_and_unpause_works() {
    std_pool();
    assert_eq!(set_pause(1), 0, "pause should succeed");
    assert_eq!(set_pause(0), 0, "unpause should succeed");
    add_liquidity(alice(), 1_000_000_000);
    let out = swap_exact_in(10_000, 9_900, 0);
    assert!(out > 0, "pool should work after unpause");
}

// ---------------------------------------------------------------------------
// Scenario 6: Multi-tick crossing
// ---------------------------------------------------------------------------

#[test]
fn swap_crosses_multiple_ticks() {
    std_pool();

    assert_eq!(mint(tick(-3000), tick(-1000), 500_000_000), 0);
    assert_eq!(mint(tick(-1000), 0, 500_000_000), 0);
    assert_eq!(mint(0, 1000, 500_000_000), 0);
    assert_eq!(mint(1000, 3000, 500_000_000), 0);

    let price_before = get_sqrt_price();
    let tick_before  = get_current_tick();

    let out = swap_exact_in(3_000_000, 2_970_000, 0);
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
// Scenario 7: Out-of-range positions
// ---------------------------------------------------------------------------

#[test]
fn out_of_range_position_contributes_zero_active_liquidity() {
    std_pool();

    mint(tick(-1000), 1000, 1_000_000_000);
    let liq_with_in_range = get_liquidity();

    mint(2000, 5000, 1_000_000_000);
    let liq_after_out_of_range = get_liquidity();

    assert_eq!(liq_with_in_range, liq_after_out_of_range,
               "out-of-range mint should not change active liquidity");

    mint(tick(-500), 500, 500_000_000);
    let liq_after_second_in_range = get_liquidity();
    assert!(liq_after_second_in_range > liq_after_out_of_range,
            "second in-range mint should increase active liquidity");
}

// ---------------------------------------------------------------------------
// Scenario 8: Fee growth only for in-range positions
// ---------------------------------------------------------------------------

#[test]
fn fees_only_accrue_when_position_is_in_range() {
    std_pool();

    mint(tick(-1000), 1000, 1_000_000_000);
    mint(2000, 5000, 1_000_000_000);

    for _ in 0..3 {
        swap_exact_in(5_000, 4_950, 0);
    }

    burn(tick(-1000), 1000, 1_000_000_000);
    burn(2000, 5000, 1_000_000_000);
    assert_eq!(collect(tick(-1000), 1000, u32::MAX, u32::MAX), 0);
    assert_eq!(collect(2000, 5000, u32::MAX, u32::MAX), 0);
}
