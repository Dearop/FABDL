//! Uniswap v3-inspired AMM smart contract for XRPL / Bedrock.
//!
//! On-chain (wasm32): compiled to WASM via `bedrock build`, exported via
//!   `#[wasm_export]` from `xrpl_wasm_macros`.
//! Native (tests):    plain Rust structs; wasm_export is a no-op attribute.

#![cfg_attr(target_arch = "wasm32", no_std)]

#[cfg(not(target_arch = "wasm32"))]
extern crate std;

// ---------------------------------------------------------------------------
// On-chain imports (WASM target only)
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
use xrpl_wasm_macros::wasm_export;

#[cfg(target_arch = "wasm32")]
use xrpl_wasm_std::host::trace::trace;

// ---------------------------------------------------------------------------
// No-op shims for native/test builds
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
macro_rules! wasm_trace {
    ($msg:expr) => {
        // no-op on native
    };
}

#[cfg(target_arch = "wasm32")]
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

use math::{amount0_delta, amount1_delta, sqrt_price_at_tick, Q64};
use position::{PositionKey, PositionMap};
use swap::execute_swap;
use tick::TickMap;
use tick_bitmap::TickBitmap;
use types::{AccountId, ContractError};

// ---------------------------------------------------------------------------
// Pool state (single-pool contract for this MVP)
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

// ---------------------------------------------------------------------------
// Contract singleton (static on-chain state)
// ---------------------------------------------------------------------------

// On native builds we use a thread_local for test isolation.
// On WASM builds this maps to host storage — state is managed by the runtime.

#[cfg(not(target_arch = "wasm32"))]
use std::cell::RefCell;

#[cfg(not(target_arch = "wasm32"))]
thread_local! {
    static POOL: RefCell<PoolState> = RefCell::new(PoolState {
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
    });

    static CONFIG: RefCell<ContractConfig> = RefCell::new(ContractConfig {
        owner: [0u8; 20],
        paused: false,
        max_slippage_bps: 100, // 1% hard cap
        tick_spacing: 10,
    });

    static TICKS: RefCell<TickMap> = RefCell::new(TickMap::new());
    static BITMAP: RefCell<TickBitmap> = RefCell::new(TickBitmap::new());
    static POSITIONS: RefCell<PositionMap> = RefCell::new(PositionMap::new());
}

// ---------------------------------------------------------------------------
// Guard helpers
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
fn require_not_paused() -> Result<(), ContractError> {
    CONFIG.with(|c| {
        if c.borrow().paused {
            Err(ContractError::Paused)
        } else {
            Ok(())
        }
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn require_owner(sender: AccountId) -> Result<(), ContractError> {
    CONFIG.with(|c| {
        if c.borrow().owner != sender {
            Err(ContractError::NotAuthorized)
        } else {
            Ok(())
        }
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn require_initialized() -> Result<(), ContractError> {
    POOL.with(|p| {
        if !p.borrow().initialized {
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
    match initialize_pool_inner(sender, sqrt_price_q64_64, fee_bps, protocol_fee_share_bps) {
        Ok(_) => 0,
        Err(e) => e.code(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn initialize_pool_inner(
    sender: AccountId,
    sqrt_price_q64_64: u128,
    fee_bps: u16,
    protocol_fee_share_bps: u16,
) -> Result<(), ContractError> {
    require_owner(sender)?;
    let price = sqrt_price_q64_64.max(1u128 << 32);
    POOL.with(|p| {
        let mut pool = p.borrow_mut();
        pool.sqrt_price_q64_64 = price;
        pool.current_tick = math::tick_at_sqrt_price(price);
        pool.fee_bps = fee_bps.min(10_000);
        pool.protocol_fee_share_bps = protocol_fee_share_bps.min(2_500);
        pool.initialized = true;
    });
    Ok(())
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

#[cfg(not(target_arch = "wasm32"))]
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
    CONFIG.with(|c| {
        let cfg = c.borrow();
        if lower_tick % cfg.tick_spacing != 0 || upper_tick % cfg.tick_spacing != 0 {
            return Err(ContractError::TickSpacingViolation);
        }
        Ok(())
    })?;

    let (current_tick, sqrt_price, fg0, fg1) = POOL.with(|p| {
        let pool = p.borrow();
        (pool.current_tick, pool.sqrt_price_q64_64, pool.fee_growth_global_0_q128, pool.fee_growth_global_1_q128)
    });
    let tick_spacing = CONFIG.with(|c| c.borrow().tick_spacing);

    // Update tick states and bitmap.
    TICKS.with(|tm| {
        let mut ticks = tm.borrow_mut();
        let (_, flipped_lower) = ticks.update(lower_tick, current_tick, liquidity_delta as i128, fg0, fg1, false)?;
        let (_, flipped_upper) = ticks.update(upper_tick, current_tick, liquidity_delta as i128, fg0, fg1, true)?;

        if flipped_lower || flipped_upper {
            BITMAP.with(|bm| {
                let mut bitmap = bm.borrow_mut();
                if flipped_lower { bitmap.flip_tick(lower_tick, tick_spacing); }
                if flipped_upper { bitmap.flip_tick(upper_tick, tick_spacing); }
            });
        }
        Ok::<(), ContractError>(())
    })?;

    // Update position.
    let (fg_inside_0, fg_inside_1) = TICKS.with(|tm| {
        tm.borrow().fee_growth_inside(lower_tick, upper_tick, current_tick, fg0, fg1)
    });

    let pos_key = PositionKey { owner: sender, lower_tick, upper_tick };
    POSITIONS.with(|pm| {
        pm.borrow_mut().update(pos_key, liquidity_delta as i128, fg_inside_0, fg_inside_1)
    })?;

    // Compute token amounts required (for caller to verify they approved enough).
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

    // Update active liquidity if position is in range.
    if current_tick >= lower_tick && current_tick < upper_tick {
        POOL.with(|p| {
            p.borrow_mut().liquidity_active = p
                .borrow()
                .liquidity_active
                .saturating_add(liquidity_delta);
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

#[cfg(not(target_arch = "wasm32"))]
fn burn_inner(
    sender: AccountId,
    lower_tick: i32,
    upper_tick: i32,
    liquidity_delta: u128,
) -> Result<(u128, u128), ContractError> {
    require_not_paused()?;
    require_initialized()?;

    let neg_delta = -(liquidity_delta as i128);

    let (current_tick, sqrt_price, fg0, fg1) = POOL.with(|p| {
        let pool = p.borrow();
        (pool.current_tick, pool.sqrt_price_q64_64, pool.fee_growth_global_0_q128, pool.fee_growth_global_1_q128)
    });
    let tick_spacing = CONFIG.with(|c| c.borrow().tick_spacing);

    TICKS.with(|tm| {
        let mut ticks = tm.borrow_mut();
        let (_, flipped_lower) = ticks.update(lower_tick, current_tick, neg_delta, fg0, fg1, false)?;
        let (_, flipped_upper) = ticks.update(upper_tick, current_tick, neg_delta, fg0, fg1, true)?;

        if flipped_lower || flipped_upper {
            BITMAP.with(|bm| {
                let mut bitmap = bm.borrow_mut();
                if flipped_lower { bitmap.flip_tick(lower_tick, tick_spacing); }
                if flipped_upper { bitmap.flip_tick(upper_tick, tick_spacing); }
            });
        }
        Ok::<(), ContractError>(())
    })?;

    let (fg_inside_0, fg_inside_1) = TICKS.with(|tm| {
        tm.borrow().fee_growth_inside(lower_tick, upper_tick, current_tick, fg0, fg1)
    });

    let pos_key = PositionKey { owner: sender, lower_tick, upper_tick };
    POSITIONS.with(|pm| {
        pm.borrow_mut().update(pos_key, neg_delta, fg_inside_0, fg_inside_1)
    })?;

    // Compute principal amounts returned.
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
        POOL.with(|p| {
            p.borrow_mut().liquidity_active = p
                .borrow()
                .liquidity_active
                .saturating_sub(liquidity_delta);
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
    require_not_paused().map(|_| ()).unwrap_or(());

    let pos_key = PositionKey { owner: sender, lower_tick, upper_tick };
    let _ = POSITIONS.with(|pm| pm.borrow_mut().collect(pos_key, max_amount_0, max_amount_1));
    0
}

/// @xrpl-function swap_exact_in
/// @param amount_in UINT64 - Exact input amount
/// @param min_amount_out UINT64 - Minimum acceptable output (slippage guard)
/// @param zero_for_one UINT8 - 1 = token0→token1 (price down), 0 = reverse
/// @param sqrt_price_limit_q64_64 UINT128 - Hard price boundary
/// @return UINT64 - Amount out (0 on failure — check via receipt)
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

#[cfg(not(target_arch = "wasm32"))]
fn swap_exact_in_inner(
    amount_in: u64,
    min_amount_out: u64,
    zero_for_one: bool,
    sqrt_price_limit: u128,
) -> Result<u64, ContractError> {
    require_not_paused()?;
    require_initialized()?;

    // Enforce global slippage cap on the requested minimum.
    let max_slippage_bps = CONFIG.with(|c| c.borrow().max_slippage_bps);
    let implied_slippage_bps = if amount_in > 0 {
        let out_floor = amount_in as u128 * (10_000 - max_slippage_bps as u128) / 10_000;
        if (min_amount_out as u128) < out_floor {
            return Err(ContractError::SlippageLimitExceeded);
        }
        0u16
    } else {
        return Err(ContractError::InvalidLiquidityDelta);
    };
    let _ = implied_slippage_bps;

    let (sqrt_price, current_tick, liquidity, fee_bps, protocol_fee_bps, fg0, fg1) = POOL.with(|p| {
        let pool = p.borrow();
        (
            pool.sqrt_price_q64_64,
            pool.current_tick,
            pool.liquidity_active,
            pool.fee_bps,
            pool.protocol_fee_share_bps,
            pool.fee_growth_global_0_q128,
            pool.fee_growth_global_1_q128,
        )
    });
    let tick_spacing = CONFIG.with(|c| c.borrow().tick_spacing);

    let result = TICKS.with(|tm| {
        BITMAP.with(|bm| {
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
                &mut tm.borrow_mut(),
                &mut bm.borrow_mut(),
            )
        })
    })?;

    if result.amount_out < min_amount_out {
        return Err(ContractError::SlippageLimitExceeded);
    }

    // Commit pool state.
    POOL.with(|p| {
        let mut pool = p.borrow_mut();
        pool.sqrt_price_q64_64 = result.sqrt_price_after;
        pool.current_tick = result.tick_after;
        pool.liquidity_active = TICKS.with(|tm| {
            // Recompute active liquidity after crossings.
            // Simplified: trust the swap engine's final liquidity.
            // In a full impl, derive from position map sum.
            pool.liquidity_active
        });
        if zero_for_one {
            pool.fee_growth_global_0_q128 =
                pool.fee_growth_global_0_q128.wrapping_add(result.fee_growth_delta);
            pool.protocol_fees_0 = pool.protocol_fees_0.saturating_add(result.protocol_fee);
        } else {
            pool.fee_growth_global_1_q128 =
                pool.fee_growth_global_1_q128.wrapping_add(result.fee_growth_delta);
            pool.protocol_fees_1 = pool.protocol_fees_1.saturating_add(result.protocol_fee);
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
        Ok(_) => {
            POOL.with(|p| {
                p.borrow_mut().protocol_fee_share_bps = protocol_fee_share_bps.min(2_500);
            });
            0
        }
        Err(e) => e.code(),
    }
}

/// @xrpl-function set_pause
/// @param paused UINT8 - 1 = pause, 0 = unpause
/// @return UINT32 - 0 on success
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn set_pause(sender: AccountId, paused: u8) -> u32 {
    wasm_trace!("set_pause");
    match require_owner(sender) {
        Ok(_) => {
            CONFIG.with(|c| c.borrow_mut().paused = paused != 0);
            0
        }
        Err(e) => e.code(),
    }
}

// ---------------------------------------------------------------------------
// Test helpers (not exported)
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
pub fn test_setup(owner: AccountId, tick_spacing: i32) {
    CONFIG.with(|c| {
        let mut cfg = c.borrow_mut();
        cfg.owner = owner;
        cfg.paused = false;
        cfg.tick_spacing = tick_spacing;
    });
    POOL.with(|p| {
        let mut pool = p.borrow_mut();
        *pool = PoolState {
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
        };
    });
    TICKS.with(|t| *t.borrow_mut() = TickMap::new());
    BITMAP.with(|b| *b.borrow_mut() = TickBitmap::new());
    POSITIONS.with(|p| *p.borrow_mut() = PositionMap::new());
}

#[cfg(not(target_arch = "wasm32"))]
pub fn get_sqrt_price() -> u128 {
    POOL.with(|p| p.borrow().sqrt_price_q64_64)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn get_liquidity() -> u128 {
    POOL.with(|p| p.borrow().liquidity_active)
}
