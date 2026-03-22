//! Uniswap v3-inspired AMM smart contract for XRPL / Bedrock.
//!
//! On-chain (wasm32): compiled to WASM via `bedrock build`. State is persisted
//!   to Bedrock host storage at the start/end of each exported function call.
//! Native (tests): state lives in a thread_local for test isolation.

#![cfg_attr(target_arch = "wasm32", no_std)]

#[cfg(not(target_arch = "wasm32"))]
extern crate std;

#[cfg(target_arch = "wasm32")]
extern crate alloc;

use xrpl_wasm_macros::wasm_export;
use xrpl_wasm_std::host::trace::trace;

macro_rules! wasm_trace {
    ($msg:expr) => {
        let _ = trace($msg);
    };
}

// ---------------------------------------------------------------------------
// Modules
// ---------------------------------------------------------------------------

pub mod codec;
pub mod math;
pub mod position;
pub mod swap;
pub mod tick;
pub mod tick_bitmap;
pub mod types;

use math::{amount0_delta, amount1_delta, sqrt_price_at_tick};
pub use math::Q64;
use position::{PositionKey, PositionMap};
use swap::execute_swap;
use tick::TickMap;
use tick_bitmap::TickBitmap;
use types::{AccountId, ContractError};

/// Placeholder owner — used for position tracking since we have no sender.
const ZERO_ADDR: AccountId = [0u8; 20];

// ---------------------------------------------------------------------------
// State structs (pub(crate) so codec.rs can reference field types)
// ---------------------------------------------------------------------------

pub(crate) struct PoolState {
    pub sqrt_price_q64_64: u128,
    pub current_tick: i32,
    pub liquidity_active: u128,
    pub fee_bps: u16,
    pub protocol_fee_share_bps: u16,
    pub fee_growth_global_0_q128: u128,
    pub fee_growth_global_1_q128: u128,
    pub protocol_fees_0: u128,
    pub protocol_fees_1: u128,
    pub initialized: bool,
}

pub(crate) struct ContractConfig {
    pub owner: AccountId,
    /// Address of the manager contract that may call admin functions.
    /// All-zeros means no manager set yet (manager not deployed).
    pub manager: AccountId,
    pub paused: bool,
    pub max_slippage_bps: u16,
    pub tick_spacing: i32,
}

pub(crate) struct ContractState {
    pub pool: PoolState,
    pub config: ContractConfig,
    pub ticks: TickMap,
    pub bitmap: TickBitmap,
    pub positions: PositionMap,
}

impl ContractState {
    pub fn new() -> Self {
        ContractState {
            pool: PoolState {
                sqrt_price_q64_64: 0,
                current_tick: 0,
                liquidity_active: 0,
                fee_bps: 30,
                protocol_fee_share_bps: 0,
                fee_growth_global_0_q128: 0,
                fee_growth_global_1_q128: 0,
                protocol_fees_0: 0,
                protocol_fees_1: 0,
                initialized: false,
            },
            config: ContractConfig {
                owner: [0u8; 20],
                manager: [0u8; 20],
                paused: false,
                max_slippage_bps: 100, // 1% hard cap
                tick_spacing: 10,
            },
            ticks: TickMap::new(),
            bitmap: TickBitmap::new(),
            positions: PositionMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// State accessor — platform-specific
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
use std::cell::RefCell;

#[cfg(not(target_arch = "wasm32"))]
thread_local! {
    static STATE: RefCell<ContractState> = RefCell::new(ContractState::new());
}

#[cfg(not(target_arch = "wasm32"))]
fn with_state<F, T>(f: F) -> T
where
    F: FnOnce(&mut ContractState) -> T,
{
    STATE.with(|s| f(&mut s.borrow_mut()))
}

#[cfg(target_arch = "wasm32")]
static mut STATE: Option<ContractState> = None;

#[cfg(target_arch = "wasm32")]
fn with_state<F, T>(f: F) -> T
where
    F: FnOnce(&mut ContractState) -> T,
{
    #[allow(static_mut_refs)]
    unsafe {
        if STATE.is_none() {
            STATE = Some(ContractState::new());
        }
        f(STATE.as_mut().unwrap())
    }
}

// ---------------------------------------------------------------------------
// Host storage persistence (WASM only)
//
// On each exported function entry we deserialize the full state blob from host
// storage into `STATE` via get_data_object_field (key = "state").
// On exit we serialize it back via set_data_object_field.
// Native builds keep state in the thread_local and don't touch host storage.
// ---------------------------------------------------------------------------

/// Key used for the contract state field in ContractData.
const STATE_KEY: &str = "state";

#[cfg(target_arch = "wasm32")]
#[allow(static_mut_refs)]
fn load_state_from_host() {
    wasm_trace!("load_state: calling get_data");
    if let Some(bytes) = xrpl_wasm_std::host_get_data() {
        wasm_trace!("load_state: got data from host");
        if let Some(state) = codec::decode_state(&bytes) {
            wasm_trace!("load_state: decoded state ok");
            unsafe { STATE = Some(state); }
        } else {
            wasm_trace!("load_state: decode_state FAILED");
        }
    } else {
        wasm_trace!("load_state: get_data returned None");
    }
}

#[cfg(target_arch = "wasm32")]
#[allow(static_mut_refs)]
fn save_state_to_host() {
    unsafe {
        if let Some(state) = STATE.as_ref() {
            let bytes = codec::encode_state(state);
            let ret = xrpl_wasm_std::host_set_data(&bytes);
            if ret < 0 {
                wasm_trace!("save_state: set_data FAILED");
            } else {
                wasm_trace!("save_state: set_data ok");
            }
        }
    }
}

/// Wrap a WASM exported function with load/save bookends.
/// On native this is a no-op.
macro_rules! with_storage {
    ($body:expr) => {{
        #[cfg(target_arch = "wasm32")]
        load_state_from_host();
        let __result = $body;
        #[cfg(target_arch = "wasm32")]
        save_state_to_host();
        __result
    }};
}

// ---------------------------------------------------------------------------
// Guards
// ---------------------------------------------------------------------------

fn require_not_paused() -> Result<(), ContractError> {
    with_state(|s| {
        if s.config.paused { Err(ContractError::Paused) } else { Ok(()) }
    })
}

fn require_initialized() -> Result<(), ContractError> {
    with_state(|s| {
        if !s.pool.initialized { Err(ContractError::PoolNotInitialized) } else { Ok(()) }
    })
}

// ---------------------------------------------------------------------------
// ABI-exported functions
// ---------------------------------------------------------------------------

/// @xrpl-function initialize_pool
/// @param initial_tick UINT32 - Initial tick (two's-complement; pass e.g. 0 for price=1.0)
/// @param fee_bps UINT16 - LP fee in basis points (e.g. 30 = 0.3%)
/// @param protocol_fee_share_bps UINT16 - Protocol's share of fee in bps
/// @return UINT32 - 0 on success, error code otherwise
#[wasm_export]
pub fn initialize_pool(
    initial_tick: u32,
    fee_bps: u16,
    protocol_fee_share_bps: u16,
) -> u32 {
    with_storage!({
        wasm_trace!("initialize_pool");
        let tick = initial_tick as i32;
        let price = math::sqrt_price_at_tick(tick).max(1u128 << 32);
        let fee_clamped = fee_bps.min(10_000);
        with_state(|s| {
            s.pool.sqrt_price_q64_64 = price;
            s.pool.current_tick = tick;
            s.pool.fee_bps = fee_clamped;
            s.pool.protocol_fee_share_bps = protocol_fee_share_bps.min(2_500);
            s.pool.initialized = true;
        });
        0
    })
}

/// @xrpl-function mint
/// @param lower_tick UINT32 - Lower tick boundary (two's-complement; pass e.g. 0xFFFFFC18 for -1000)
/// @param upper_tick UINT32 - Upper tick boundary (two's-complement)
/// @param liquidity_delta UINT64 - Positive liquidity amount to add (split to lo/hi u32 in WASM)
/// @return UINT32 - 0 on success
#[wasm_export]
pub fn mint(
    lower_tick: u32,
    upper_tick: u32,
    liquidity_delta: u64,
) -> u32 {
    with_storage!({
        wasm_trace!("mint");
        match mint_inner(lower_tick as i32, upper_tick as i32, liquidity_delta as u128) {
            Ok(_) => 0,
            Err(e) => e.code(),
        }
    })
}

fn mint_inner(
    lower_tick: i32,
    upper_tick: i32,
    liquidity_delta: u128,
) -> Result<(u128, u128), ContractError> {
    require_not_paused()?;
    require_initialized()?;

    if lower_tick >= upper_tick {
        return Err(ContractError::InvalidTickRange);
    }

    let tick_spacing = with_state(|s| s.config.tick_spacing);
    if lower_tick % tick_spacing != 0 || upper_tick % tick_spacing != 0 {
        return Err(ContractError::TickSpacingViolation);
    }

    let (current_tick, sqrt_price, fg0, fg1) = with_state(|s| {
        (s.pool.current_tick, s.pool.sqrt_price_q64_64,
         s.pool.fee_growth_global_0_q128, s.pool.fee_growth_global_1_q128)
    });

    with_state(|s| {
        let (_, flipped_lower) = s.ticks.update(
            lower_tick, current_tick, liquidity_delta as i128, fg0, fg1, false)?;
        let (_, flipped_upper) = s.ticks.update(
            upper_tick, current_tick, liquidity_delta as i128, fg0, fg1, true)?;
        if flipped_lower { s.bitmap.flip_tick(lower_tick, tick_spacing); }
        if flipped_upper { s.bitmap.flip_tick(upper_tick, tick_spacing); }
        Ok::<(), ContractError>(())
    })?;

    let (fg_inside_0, fg_inside_1) = with_state(|s| {
        s.ticks.fee_growth_inside(lower_tick, upper_tick, current_tick, fg0, fg1)
    });

    let pos_key = PositionKey { owner: ZERO_ADDR, lower_tick, upper_tick };
    with_state(|s| s.positions.update(pos_key, liquidity_delta as i128, fg_inside_0, fg_inside_1))?;

    let sqrt_lower = sqrt_price_at_tick(lower_tick);
    let sqrt_upper = sqrt_price_at_tick(upper_tick);
    let sqrt_current = sqrt_price;

    let amount0 = if sqrt_current < sqrt_lower {
        amount0_delta(sqrt_lower, sqrt_upper, liquidity_delta, true)
    } else if sqrt_current < sqrt_upper {
        amount0_delta(sqrt_current, sqrt_upper, liquidity_delta, true)
    } else {
        0
    };

    let amount1 = if sqrt_current < sqrt_lower {
        0
    } else if sqrt_current < sqrt_upper {
        amount1_delta(sqrt_lower, sqrt_current, liquidity_delta, true)
    } else {
        amount1_delta(sqrt_lower, sqrt_upper, liquidity_delta, true)
    };

    if current_tick >= lower_tick && current_tick < upper_tick {
        with_state(|s| {
            s.pool.liquidity_active = s.pool.liquidity_active.saturating_add(liquidity_delta);
        });
    }

    Ok((amount0, amount1))
}

/// @xrpl-function burn
/// @param lower_tick UINT32 - Lower tick boundary (two's-complement; pass e.g. 0xFFFFFC18 for -1000)
/// @param upper_tick UINT32 - Upper tick boundary (two's-complement)
/// @param liquidity_delta UINT64 - Positive liquidity amount to remove (split to lo/hi u32 in WASM)
/// @return UINT32 - 0 on success
#[wasm_export]
pub fn burn(
    lower_tick: u32,
    upper_tick: u32,
    liquidity_delta: u64,
) -> u32 {
    with_storage!({
        wasm_trace!("burn");
        match burn_inner(lower_tick as i32, upper_tick as i32, liquidity_delta as u128) {
            Ok(_) => 0,
            Err(e) => e.code(),
        }
    })
}

fn burn_inner(
    lower_tick: i32,
    upper_tick: i32,
    liquidity_delta: u128,
) -> Result<(u128, u128), ContractError> {
    require_not_paused()?;
    require_initialized()?;

    let tick_spacing = with_state(|s| s.config.tick_spacing);
    let neg_delta = -(liquidity_delta as i128);
    let (current_tick, sqrt_price, fg0, fg1) = with_state(|s| {
        (s.pool.current_tick, s.pool.sqrt_price_q64_64,
         s.pool.fee_growth_global_0_q128, s.pool.fee_growth_global_1_q128)
    });

    with_state(|s| {
        let (_, flipped_lower) = s.ticks.update(
            lower_tick, current_tick, neg_delta, fg0, fg1, false)?;
        let (_, flipped_upper) = s.ticks.update(
            upper_tick, current_tick, neg_delta, fg0, fg1, true)?;
        if flipped_lower { s.bitmap.flip_tick(lower_tick, tick_spacing); }
        if flipped_upper { s.bitmap.flip_tick(upper_tick, tick_spacing); }
        Ok::<(), ContractError>(())
    })?;

    let (fg_inside_0, fg_inside_1) = with_state(|s| {
        s.ticks.fee_growth_inside(lower_tick, upper_tick, current_tick, fg0, fg1)
    });

    let pos_key = PositionKey { owner: ZERO_ADDR, lower_tick, upper_tick };
    with_state(|s| s.positions.update(pos_key, neg_delta, fg_inside_0, fg_inside_1))?;

    let sqrt_lower = sqrt_price_at_tick(lower_tick);
    let sqrt_upper = sqrt_price_at_tick(upper_tick);
    let sqrt_current = sqrt_price;

    let amount0 = if sqrt_current < sqrt_lower {
        amount0_delta(sqrt_lower, sqrt_upper, liquidity_delta, false)
    } else if sqrt_current < sqrt_upper {
        amount0_delta(sqrt_current, sqrt_upper, liquidity_delta, false)
    } else {
        0
    };

    let amount1 = if sqrt_current < sqrt_lower {
        0
    } else if sqrt_current < sqrt_upper {
        amount1_delta(sqrt_lower, sqrt_current, liquidity_delta, false)
    } else {
        amount1_delta(sqrt_lower, sqrt_upper, liquidity_delta, false)
    };

    if current_tick >= lower_tick && current_tick < upper_tick {
        with_state(|s| {
            s.pool.liquidity_active = s.pool.liquidity_active.saturating_sub(liquidity_delta);
        });
    }

    Ok((amount0, amount1))
}

/// @xrpl-function collect
/// @param lower_tick UINT32 - Lower tick boundary (two's-complement)
/// @param upper_tick UINT32 - Upper tick boundary
/// @param max_amount_0 UINT32 - Max token0 to collect
/// @param max_amount_1 UINT32 - Max token1 to collect
/// @return UINT32 - 0 on success
#[wasm_export]
pub fn collect(
    lower_tick: u32,
    upper_tick: u32,
    max_amount_0: u32,
    max_amount_1: u32,
) -> u32 {
    with_storage!({
        wasm_trace!("collect");
        if require_not_paused().is_err() {
            return ContractError::Paused.code();
        }
        let pos_key = PositionKey { owner: ZERO_ADDR, lower_tick: lower_tick as i32, upper_tick: upper_tick as i32 };
        with_state(|s| s.positions.collect(pos_key, max_amount_0 as u64, max_amount_1 as u64));
        0
    })
}

/// @xrpl-function swap_exact_in
/// @param amount_in UINT32 - Exact input amount
/// @param min_amount_out UINT32 - Minimum acceptable output (slippage guard)
/// @param zero_for_one UINT8 - 1 = token0→token1 (price down), 0 = reverse
/// @return UINT32 - Amount out (0 on failure)
#[wasm_export]
pub fn swap_exact_in(
    amount_in: u32,
    min_amount_out: u32,
    zero_for_one: u8,
) -> u32 {
    with_storage!({
        wasm_trace!("swap_exact_in");
        let direction = zero_for_one != 0;
        let sqrt_price_limit = if direction { 1u128 } else { u128::MAX };
        match swap_exact_in_inner(amount_in as u64, min_amount_out as u64, direction, sqrt_price_limit) {
            Ok(out) => out as u32,
            Err(_) => 0,
        }
    })
}

fn swap_exact_in_inner(
    amount_in: u64,
    min_amount_out: u64,
    zero_for_one: bool,
    sqrt_price_limit: u128,
) -> Result<u64, ContractError> {
    require_not_paused()?;
    require_initialized()?;

    if amount_in == 0 {
        return Err(ContractError::InvalidLiquidityDelta);
    }

    let max_slippage_bps = with_state(|s| s.config.max_slippage_bps);
    let floor = amount_in as u128 * (10_000 - max_slippage_bps as u128) / 10_000;
    if (min_amount_out as u128) < floor {
        return Err(ContractError::SlippageLimitExceeded);
    }

    let (tick_cumulative, spl_q128) = (0i64, 0u128);

    let (sqrt_price, current_tick, liquidity, fee_bps, protocol_fee_bps, fg0, fg1, tick_spacing) =
        with_state(|s| {
            (
                s.pool.sqrt_price_q64_64,
                s.pool.current_tick,
                s.pool.liquidity_active,
                s.pool.fee_bps,
                s.pool.protocol_fee_share_bps,
                s.pool.fee_growth_global_0_q128,
                s.pool.fee_growth_global_1_q128,
                s.config.tick_spacing,
            )
        });

    let result = with_state(|s| {
        execute_swap(
            sqrt_price,
            current_tick,
            liquidity,
            fee_bps,
            protocol_fee_bps,
            if zero_for_one { fg0 } else { fg1 },
            amount_in,
            zero_for_one,
            sqrt_price_limit,
            tick_spacing,
            tick_cumulative,
            spl_q128,
            &mut s.ticks,
            &mut s.bitmap,
        )
    })?;

    if result.amount_out < min_amount_out {
        return Err(ContractError::SlippageLimitExceeded);
    }

    // Commit updated pool state.
    with_state(|s| {
        s.pool.sqrt_price_q64_64 = result.sqrt_price_after;
        s.pool.current_tick = result.tick_after;
        s.pool.liquidity_active = result.liquidity_after;
        if zero_for_one {
            s.pool.fee_growth_global_0_q128 =
                s.pool.fee_growth_global_0_q128.wrapping_add(result.fee_growth_delta);
            s.pool.protocol_fees_0 = s.pool.protocol_fees_0.saturating_add(result.protocol_fee);
        } else {
            s.pool.fee_growth_global_1_q128 =
                s.pool.fee_growth_global_1_q128.wrapping_add(result.fee_growth_delta);
            s.pool.protocol_fees_1 = s.pool.protocol_fees_1.saturating_add(result.protocol_fee);
        }
    });

    Ok(result.amount_out)
}

/// collect_protocol — NOT exported to the on-chain ABI (7-function limit per spec).
/// Callable from WASM code via cross-contract invoke or from native tests directly.
/// @param max_amount_0 UINT32 - Max token0 protocol fees to collect
/// @param max_amount_1 UINT32 - Max token1 protocol fees to collect
/// @return UINT32 - Amount of token0 collected (token1 also drained internally)
pub fn collect_protocol(
    max_amount_0: u32,
    max_amount_1: u32,
) -> u32 {
    with_storage!({
        wasm_trace!("collect_protocol");
        with_state(|s| {
            let c0 = s.pool.protocol_fees_0.min(max_amount_0 as u128) as u32;
            let c1 = s.pool.protocol_fees_1.min(max_amount_1 as u128) as u32;
            s.pool.protocol_fees_0 -= c0 as u128;
            s.pool.protocol_fees_1 -= c1 as u128;
            c0
        })
    })
}

/// @xrpl-function set_protocol_fee
/// @param protocol_fee_share_bps UINT16 - Protocol fee share (max 2500 = 25%)
/// @return UINT32 - 0 on success
#[wasm_export]
pub fn set_protocol_fee(protocol_fee_share_bps: u16) -> u32 {
    with_storage!({
        wasm_trace!("set_protocol_fee");
        with_state(|s| {
            s.pool.protocol_fee_share_bps = protocol_fee_share_bps.min(2_500);
        });
        0
    })
}

/// @xrpl-function deposit
/// @param amount AMOUNT - XRP amount to deposit into contract (flag=2 triggers transfer)
/// @return UINT32 - 0 on success
#[wasm_export]
pub fn deposit(_amount: u64) -> u32 {
    wasm_trace!("deposit");
    0
}

/// @xrpl-function set_pause
/// @param paused UINT8 - 1 = pause, 0 = unpause
/// @return UINT32 - 0 on success
#[wasm_export]
pub fn set_pause(paused: u8) -> u32 {
    with_storage!({
        wasm_trace!("set_pause");
        with_state(|s| s.config.paused = paused != 0);
        0
    })
}

// set_manager is NOT exported to the on-chain ABI (kept below the 7-function limit per spec).
// Used only in native tests and off-chain tooling.
pub fn set_manager(mgr_lo: u64, mgr_mid: u64, mgr_hi: u32) -> u32 {
    let mut manager = [0u8; 20];
    manager[0..8].copy_from_slice(&mgr_lo.to_le_bytes());
    manager[8..16].copy_from_slice(&mgr_mid.to_le_bytes());
    manager[16..20].copy_from_slice(&mgr_hi.to_le_bytes());
    with_state(|s| s.config.manager = manager);
    0
}

// ---------------------------------------------------------------------------
// Test helpers (not exported to ABI)
// ---------------------------------------------------------------------------

/// Reset all contract state. Call at the top of each test.
pub fn test_setup(owner: AccountId, tick_spacing: i32) {
    with_state(|s| {
        *s = ContractState::new();
        s.config.owner = owner;
        s.config.tick_spacing = tick_spacing;
    });
}

/// Reset state with an explicit manager address (used by manager contract tests).
pub fn test_setup_with_manager(owner: AccountId, manager: AccountId, tick_spacing: i32) {
    with_state(|s| {
        *s = ContractState::new();
        s.config.owner = owner;
        s.config.manager = manager;
        s.config.tick_spacing = tick_spacing;
    });
}

pub fn get_sqrt_price() -> u128 {
    with_state(|s| s.pool.sqrt_price_q64_64)
}

pub fn get_liquidity() -> u128 {
    with_state(|s| s.pool.liquidity_active)
}

pub fn get_protocol_fees() -> (u128, u128) {
    with_state(|s| (s.pool.protocol_fees_0, s.pool.protocol_fees_1))
}

pub fn get_current_tick() -> i32 {
    with_state(|s| s.pool.current_tick)
}

pub fn get_manager() -> AccountId {
    with_state(|s| s.config.manager)
}

pub fn get_fee_growth_global() -> (u128, u128) {
    with_state(|s| (s.pool.fee_growth_global_0_q128, s.pool.fee_growth_global_1_q128))
}

// ---------------------------------------------------------------------------
// Integration tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn init_pool() {
        test_setup([0u8; 20], 10);
        // initial_tick = 0 → price = 1.0 in Q64.64
        assert_eq!(initialize_pool(0u32, 30, 0), 0);
    }

    fn tick(t: i32) -> u32 { t as u32 }

    fn add_liquidity() {
        assert_eq!(mint(tick(-1000), 1000, 1_000_000_000), 0);
    }

    #[test]
    fn full_lifecycle() {
        init_pool();
        add_liquidity();
        let sp_before = get_sqrt_price();
        let liq_before = get_liquidity();
        assert!(liq_before > 0);

        let out = swap_exact_in(10_000, 9_900, 0);
        assert!(out > 0, "swap should produce output");
        let sp_after = get_sqrt_price();
        assert!(sp_after > sp_before, "price should have increased");
    }

    #[test]
    fn swap_decreases_price() {
        init_pool();
        add_liquidity();
        let sp_before = get_sqrt_price();
        let out = swap_exact_in(10_000, 9_900, 1);
        assert!(out > 0);
        assert!(get_sqrt_price() < sp_before);
    }

    #[test]
    fn mint_increases_liquidity() {
        init_pool();
        assert_eq!(get_liquidity(), 0);
        add_liquidity();
        assert!(get_liquidity() > 0);
    }

    #[test]
    fn burn_decreases_liquidity() {
        init_pool();
        add_liquidity();
        let liq = get_liquidity();
        assert_eq!(burn(tick(-1000), 1000, 500_000_000), 0);
        assert!(get_liquidity() < liq);
    }

    #[test]
    fn collect_works_after_burn() {
        init_pool();
        add_liquidity();
        burn(tick(-1000), 1000, 1_000_000_000);
        assert_eq!(collect(tick(-1000), 1000, u32::MAX, u32::MAX), 0);
    }

    #[test]
    fn pause_blocks_swap() {
        init_pool();
        add_liquidity();
        assert_eq!(set_pause(1), 0);
        let out = swap_exact_in(10_000, 0, 0);
        assert_eq!(out, 0); // paused → returns 0
    }

    #[test]
    fn set_protocol_fee_and_collect() {
        init_pool();
        add_liquidity();
        assert_eq!(set_protocol_fee(1_000), 0); // 10% of fees

        // Do a swap so fees accumulate.
        swap_exact_in(100_000, 99_000, 0);

        let (pf0, pf1) = get_protocol_fees();
        assert!(pf0 > 0 || pf1 > 0, "protocol fees should have accrued");

        let _ = collect_protocol(u32::MAX, u32::MAX);
        let (pf0_after, pf1_after) = get_protocol_fees();
        assert_eq!(pf0_after, 0);
        assert_eq!(pf1_after, 0);
    }

    #[test]
    fn slippage_cap_enforced() {
        init_pool();
        add_liquidity();
        let out = swap_exact_in(10_000, 0, 0);
        assert_eq!(out, 0);
    }
}
