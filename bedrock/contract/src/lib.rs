//! Uniswap v3-inspired AMM smart contract for XRPL / Bedrock.
//!
//! On-chain (wasm32): compiled to WASM via `bedrock build`. State lives in a
//!   static mut (WASM is single-threaded; each invocation is isolated).
//! Native (tests): state lives in a thread_local for test isolation.

#![cfg_attr(target_arch = "wasm32", no_std)]

#[cfg(not(target_arch = "wasm32"))]
extern crate std;

#[cfg(target_arch = "wasm32")]
extern crate alloc;

// ---------------------------------------------------------------------------
// Bedrock host imports
// On native: stubs from xrpl-wasm-macros-stub / xrpl-wasm-std-stub.
// On wasm32: `bedrock build` links the real xrpl-commons crates.
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// State structs
// ---------------------------------------------------------------------------

struct PoolState {
    sqrt_price_q64_64: u128,
    current_tick: i32,
    liquidity_active: u128,
    fee_bps: u16,
    protocol_fee_share_bps: u16,
    fee_growth_global_0_q128: u128,
    fee_growth_global_1_q128: u128,
    protocol_fees_0: u128,
    protocol_fees_1: u128,
    initialized: bool,
}

struct ContractConfig {
    owner: AccountId,
    paused: bool,
    max_slippage_bps: u16,
    tick_spacing: i32,
}

struct ContractState {
    pool: PoolState,
    config: ContractConfig,
    ticks: TickMap,
    bitmap: TickBitmap,
    positions: PositionMap,
}

impl ContractState {
    fn new() -> Self {
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

// Native builds: thread_local for test isolation.
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

// WASM builds: static mut (safe — WASM is single-threaded per-invocation).
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
// Guards
// ---------------------------------------------------------------------------

fn require_not_paused() -> Result<(), ContractError> {
    with_state(|s| {
        if s.config.paused {
            Err(ContractError::Paused)
        } else {
            Ok(())
        }
    })
}

fn require_owner(sender: AccountId) -> Result<(), ContractError> {
    with_state(|s| {
        if s.config.owner != sender {
            Err(ContractError::NotAuthorized)
        } else {
            Ok(())
        }
    })
}

fn require_initialized() -> Result<(), ContractError> {
    with_state(|s| {
        if !s.pool.initialized {
            Err(ContractError::PoolNotInitialized)
        } else {
            Ok(())
        }
    })
}

// ---------------------------------------------------------------------------
// ABI-exported functions
// ---------------------------------------------------------------------------

/// @xrpl-function initialize_pool
/// @param sqrt_price_q64_64 UINT128 - Initial sqrt price in Q64.64
/// @param fee_bps UINT16 - LP fee in basis points (e.g. 30 = 0.3%)
/// @param protocol_fee_share_bps UINT16 - Protocol's share of fee in bps
/// @return UINT32 - 0 on success, error code otherwise
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn initialize_pool(
    sender: AccountId,
    sqrt_price_q64_64: u128,
    fee_bps: u16,
    protocol_fee_share_bps: u16,
) -> u32 {
    wasm_trace!("initialize_pool");
    match require_owner(sender) {
        Err(e) => return e.code(),
        Ok(_) => {}
    }
    let price = sqrt_price_q64_64.max(1u128 << 32);
    with_state(|s| {
        s.pool.sqrt_price_q64_64 = price;
        s.pool.current_tick = math::tick_at_sqrt_price(price);
        s.pool.fee_bps = fee_bps.min(10_000);
        s.pool.protocol_fee_share_bps = protocol_fee_share_bps.min(2_500);
        s.pool.initialized = true;
    });
    0
}

/// @xrpl-function mint
/// @param lower_tick INT32 - Lower tick boundary
/// @param upper_tick INT32 - Upper tick boundary
/// @param liquidity_delta UINT128 - Positive liquidity amount to add
/// @return UINT32 - 0 on success
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn mint(
    sender: AccountId,
    lower_tick: i32,
    upper_tick: i32,
    liquidity_delta: u128,
) -> u32 {
    wasm_trace!("mint");
    match mint_inner(sender, lower_tick, upper_tick, liquidity_delta) {
        Ok(_) => 0,
        Err(e) => e.code(),
    }
}

fn mint_inner(
    sender: AccountId,
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

    // Update tick states and flip bitmap if tick initialized/uninitialized.
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

    let pos_key = PositionKey { owner: sender, lower_tick, upper_tick };
    with_state(|s| s.positions.update(pos_key, liquidity_delta as i128, fg_inside_0, fg_inside_1))?;

    // Compute required token amounts.
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

    // Update active liquidity if range contains current tick.
    if current_tick >= lower_tick && current_tick < upper_tick {
        with_state(|s| {
            s.pool.liquidity_active = s.pool.liquidity_active.saturating_add(liquidity_delta);
        });
    }

    Ok((amount0, amount1))
}

/// @xrpl-function burn
/// @param lower_tick INT32 - Lower tick boundary
/// @param upper_tick INT32 - Upper tick boundary
/// @param liquidity_delta UINT128 - Positive liquidity amount to remove
/// @return UINT32 - 0 on success
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn burn(
    sender: AccountId,
    lower_tick: i32,
    upper_tick: i32,
    liquidity_delta: u128,
) -> u32 {
    wasm_trace!("burn");
    match burn_inner(sender, lower_tick, upper_tick, liquidity_delta) {
        Ok(_) => 0,
        Err(e) => e.code(),
    }
}

fn burn_inner(
    sender: AccountId,
    lower_tick: i32,
    upper_tick: i32,
    liquidity_delta: u128,
) -> Result<(u128, u128), ContractError> {
    require_not_paused()?;
    require_initialized()?;

    let neg_delta = -(liquidity_delta as i128);
    let tick_spacing = with_state(|s| s.config.tick_spacing);
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

    let pos_key = PositionKey { owner: sender, lower_tick, upper_tick };
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
/// @param lower_tick INT32 - Lower tick boundary
/// @param upper_tick INT32 - Upper tick boundary
/// @param max_amount_0 UINT64 - Max token0 to collect
/// @param max_amount_1 UINT64 - Max token1 to collect
/// @return UINT32 - 0 on success
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn collect(
    sender: AccountId,
    lower_tick: i32,
    upper_tick: i32,
    max_amount_0: u64,
    max_amount_1: u64,
) -> u32 {
    wasm_trace!("collect");
    if require_not_paused().is_err() {
        return ContractError::Paused.code();
    }
    let pos_key = PositionKey { owner: sender, lower_tick, upper_tick };
    with_state(|s| s.positions.collect(pos_key, max_amount_0, max_amount_1));
    0
}

/// @xrpl-function swap_exact_in
/// @param amount_in UINT64 - Exact input amount
/// @param min_amount_out UINT64 - Minimum acceptable output (slippage guard)
/// @param zero_for_one UINT8 - 1 = token0→token1 (price down), 0 = reverse
/// @param sqrt_price_limit_q64_64 UINT128 - Hard price boundary
/// @return UINT64 - Amount out (0 on failure)
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn swap_exact_in(
    _sender: AccountId,
    amount_in: u64,
    min_amount_out: u64,
    zero_for_one: u8,
    sqrt_price_limit_q64_64: u128,
) -> u64 {
    wasm_trace!("swap_exact_in");
    match swap_exact_in_inner(amount_in, min_amount_out, zero_for_one != 0, sqrt_price_limit_q64_64) {
        Ok(out) => out,
        Err(_) => 0,
    }
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

    // Enforce 1% hard slippage cap.
    let max_slippage_bps = with_state(|s| s.config.max_slippage_bps);
    let floor = amount_in as u128 * (10_000 - max_slippage_bps as u128) / 10_000;
    if (min_amount_out as u128) < floor {
        return Err(ContractError::SlippageLimitExceeded);
    }

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

/// @xrpl-function set_protocol_fee
/// @param protocol_fee_share_bps UINT16 - Protocol fee share (max 2500 = 25%)
/// @return UINT32 - 0 on success
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn set_protocol_fee(sender: AccountId, protocol_fee_share_bps: u16) -> u32 {
    wasm_trace!("set_protocol_fee");
    match require_owner(sender) {
        Err(e) => e.code(),
        Ok(_) => {
            with_state(|s| {
                s.pool.protocol_fee_share_bps = protocol_fee_share_bps.min(2_500);
            });
            0
        }
    }
}

/// @xrpl-function set_pause
/// @param paused UINT8 - 1 = pause, 0 = unpause
/// @return UINT32 - 0 on success
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn set_pause(sender: AccountId, paused: u8) -> u32 {
    wasm_trace!("set_pause");
    match require_owner(sender) {
        Err(e) => e.code(),
        Ok(_) => {
            with_state(|s| s.config.paused = paused != 0);
            0
        }
    }
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

pub fn get_sqrt_price() -> u128 {
    with_state(|s| s.pool.sqrt_price_q64_64)
}

pub fn get_liquidity() -> u128 {
    with_state(|s| s.pool.liquidity_active)
}
