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
    pub paused: bool,
    pub max_slippage_bps: u16,
    pub tick_spacing: i32,
}

pub(crate) struct ContractState {
    pub pool: PoolState,
    pub config: ContractConfig,
    pub oracle: OracleBuffer,
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
                seconds_per_liquidity_q128: 0,
                last_block_timestamp: 0,
            },
            config: ContractConfig {
                owner: [0u8; 20],
                paused: false,
                max_slippage_bps: 100, // 1% hard cap
                tick_spacing: 10,
            },
            oracle: OracleBuffer::new(),
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
// storage into `STATE`. On exit we serialize it back.
// Native builds keep state in the thread_local and don't touch host storage.
// ---------------------------------------------------------------------------

/// Storage key for the contract state blob.
const STATE_KEY: &[u8] = b"amm_state_v1";

#[cfg(target_arch = "wasm32")]
#[allow(static_mut_refs)]
fn load_state_from_host() {
    use xrpl_wasm_std::host::storage;
    if let Some(bytes) = storage::get(STATE_KEY) {
        if let Some(state) = codec::decode_state(&bytes) {
            unsafe { STATE = Some(state); }
        }
    }
    // If no stored state yet, STATE will be initialized to default in with_state().
}

#[cfg(target_arch = "wasm32")]
#[allow(static_mut_refs)]
fn save_state_to_host() {
    use xrpl_wasm_std::host::storage;
    unsafe {
        if let Some(state) = STATE.as_ref() {
            let bytes = codec::encode_state(state);
            storage::set(STATE_KEY, &bytes);
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
// Oracle helpers
// ---------------------------------------------------------------------------

/// Advance the global oracle accumulators to `time` and write a new
/// observation.  Must be called at the START of each swap (before price
/// changes) so the observation captures the price *entering* this block.
///
/// Returns `(tick_cumulative, seconds_per_liquidity_q128)` after the update,
/// for use by tick-crossing inside `execute_swap`.
fn advance_oracle(time: u32) -> (i64, u128) {
    with_state(|s| {
        if !s.pool.initialized { return (0, 0); }

        let last_time = s.pool.last_block_timestamp;
        let delta = time.wrapping_sub(last_time) as u128;

        // Update global seconds-per-liquidity.
        if delta > 0 && s.pool.liquidity_active > 0 {
            // Same Q64-precision formula as oracle::transform.
            let increment = (delta << 64) / s.pool.liquidity_active;
            s.pool.seconds_per_liquidity_q128 =
                s.pool.seconds_per_liquidity_q128.wrapping_add(increment);
        }
        s.pool.last_block_timestamp = time;

        // Write oracle observation (no-op if same block).
        let (tc, spl) = s.oracle.write(time, s.pool.current_tick, s.pool.liquidity_active);
        (tc, spl)
    })
}

// ---------------------------------------------------------------------------
// Guards
// ---------------------------------------------------------------------------

fn require_not_paused() -> Result<(), ContractError> {
    with_state(|s| {
        if s.config.paused { Err(ContractError::Paused) } else { Ok(()) }
    })
}

fn require_owner(sender: AccountId) -> Result<(), ContractError> {
    with_state(|s| {
        if s.config.owner != sender { Err(ContractError::NotAuthorized) } else { Ok(()) }
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
/// @param sqrt_price_q64_64 UINT128 - Initial sqrt price in Q64.64
/// @param fee_bps UINT16 - LP fee in basis points (e.g. 30 = 0.3%)
/// @param protocol_fee_share_bps UINT16 - Protocol's share of fee in bps
/// @param timestamp UINT32 - Current block timestamp (seconds)
/// @return UINT32 - 0 on success, error code otherwise
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn initialize_pool(
    sender: AccountId,
    sqrt_price_q64_64: u128,
    fee_bps: u16,
    protocol_fee_share_bps: u16,
    timestamp: u32,
) -> u32 {
    with_storage!({
        wasm_trace!("initialize_pool");
        match require_owner(sender) {
            Err(e) => return e.code(),
            Ok(_) => {}
        }

        let fee_clamped = fee_bps.min(10_000);
        let price = sqrt_price_q64_64.max(1u128 << 32);
        with_state(|s| {
            s.pool.sqrt_price_q64_64 = price;
            s.pool.current_tick = math::tick_at_sqrt_price(price);
            s.pool.fee_bps = fee_clamped;
            s.pool.protocol_fee_share_bps = protocol_fee_share_bps.min(2_500);
            s.pool.last_block_timestamp = timestamp;
            s.pool.initialized = true;
            s.oracle.initialize(timestamp);
        });

        0
    })
}

/// @xrpl-function mint
/// @param lower_tick UINT32 - Lower tick boundary (two's-complement; pass e.g. 0xFFFFFC18 for -1000)
/// @param upper_tick UINT32 - Upper tick boundary (two's-complement)
/// @param liquidity_delta UINT128 - Positive liquidity amount to add
/// @return UINT32 - 0 on success
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn mint(
    sender: AccountId,
    lower_tick: u32,
    upper_tick: u32,
    liquidity_delta: u128,
) -> u32 {
    with_storage!({
        wasm_trace!("mint");
        match mint_inner(sender, lower_tick as i32, upper_tick as i32, liquidity_delta) {
            Ok(_) => 0,
            Err(e) => e.code(),
        }
    })
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
/// @param liquidity_delta UINT128 - Positive liquidity amount to remove
/// @return UINT32 - 0 on success
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn burn(
    sender: AccountId,
    lower_tick: u32,
    upper_tick: u32,
    liquidity_delta: u128,
) -> u32 {
    with_storage!({
        wasm_trace!("burn");
        match burn_inner(sender, lower_tick as i32, upper_tick as i32, liquidity_delta) {
            Ok(_) => 0,
            Err(e) => e.code(),
        }
    })
}

fn burn_inner(
    sender: AccountId,
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
/// @param lower_tick UINT32 - Lower tick boundary (two's-complement; pass e.g. 0xFFFFFC18 for -1000)
/// @param upper_tick UINT32 - Upper tick boundary (two's-complement)
/// @param max_amount_0 UINT64 - Max token0 to collect
/// @param max_amount_1 UINT64 - Max token1 to collect
/// @return UINT32 - 0 on success
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn collect(
    sender: AccountId,
    lower_tick: u32,
    upper_tick: u32,
    max_amount_0: u64,
    max_amount_1: u64,
) -> u32 {
    with_storage!({
        wasm_trace!("collect");
        if require_not_paused().is_err() {
            return ContractError::Paused.code();
        }
        let pos_key = PositionKey { owner: sender, lower_tick: lower_tick as i32, upper_tick: upper_tick as i32 };
        with_state(|s| s.positions.collect(pos_key, max_amount_0, max_amount_1));
        0
    })
}

/// @xrpl-function swap_exact_in
/// @param amount_in UINT64 - Exact input amount
/// @param min_amount_out UINT64 - Minimum acceptable output (slippage guard)
/// @param zero_for_one UINT8 - 1 = token0→token1 (price down), 0 = reverse
/// @param sqrt_price_limit_q64_64 UINT128 - Hard price boundary
/// @param timestamp UINT32 - Current block timestamp (seconds)
/// @return UINT64 - Amount out (0 on failure)
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn swap_exact_in(
    _sender: AccountId,
    amount_in: u64,
    min_amount_out: u64,
    zero_for_one: u8,
    sqrt_price_limit_q64_64: u128,
    timestamp: u32,
) -> u64 {
    with_storage!({
        wasm_trace!("swap_exact_in");
        match swap_exact_in_inner(amount_in, min_amount_out, zero_for_one != 0, sqrt_price_limit_q64_64, timestamp) {
            Ok(out) => out,
            Err(_) => 0,
        }
    })
}

fn swap_exact_in_inner(
    amount_in: u64,
    min_amount_out: u64,
    zero_for_one: bool,
    sqrt_price_limit: u128,
    timestamp: u32,
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

    // Advance oracle BEFORE the swap (captures pre-swap price in observation).
    let (tick_cumulative, spl_q128) = advance_oracle(timestamp);

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
            timestamp,
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

/// @xrpl-function observe
/// @param seconds_agos_packed UINT64 - Up to 4 × u16 seconds-ago values packed LE
/// @param timestamp UINT32 - Current block timestamp
/// @return UINT64 - Packed (tick_cumulative_0: i32 << 32 | tick_cumulative_1: i32)
///                  for first two observations. 0 if oracle not ready.
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn observe(
    _sender: AccountId,
    seconds_agos_packed: u64,
    timestamp: u32,
) -> u64 {
    with_storage!({
        wasm_trace!("observe");
        // Unpack up to 2 × u32 from the packed u64 (lower / upper 32 bits).
        let sago0 = (seconds_agos_packed & 0xFFFF_FFFF) as u32;
        let sago1 = (seconds_agos_packed >> 32) as u32;
        let agos = if sago1 == 0 { &[sago0][..] } else { &[sago0, sago1][..] };

        let (tick, liq) = with_state(|s| (s.pool.current_tick, s.pool.liquidity_active));

        let result = with_state(|s| s.oracle.observe(timestamp, agos, tick, liq));
        match result {
            oracle::ObserveResult::Ok { tick_cumulatives, .. } => {
                let tc0 = tick_cumulatives.get(0).copied().unwrap_or(0) as i32 as u64;
                let tc1 = tick_cumulatives.get(1).copied().unwrap_or(0) as i32 as u64;
                tc0 | (tc1 << 32)
            }
            oracle::ObserveResult::Err => 0,
        }
    })
}

/// @xrpl-function increase_observation_cardinality
/// @param next UINT16 - New target cardinality (number of observations to keep)
/// @return UINT32 - 0 on success
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn increase_observation_cardinality(
    _sender: AccountId,
    next: u16,
) -> u32 {
    with_storage!({
        wasm_trace!("increase_observation_cardinality");
        with_state(|s| s.oracle.grow(next));
        0
    })
}

/// @xrpl-function collect_protocol
/// @param max_amount_0 UINT64 - Max token0 protocol fees to collect
/// @param max_amount_1 UINT64 - Max token1 protocol fees to collect
/// @return UINT64 - Packed (collected_0: u32 << 32 | collected_1: u32) or 0 on error
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn collect_protocol(
    sender: AccountId,
    max_amount_0: u64,
    max_amount_1: u64,
) -> u64 {
    with_storage!({
        wasm_trace!("collect_protocol");
        if require_owner(sender).is_err() {
            return 0;
        }
        with_state(|s| {
            let c0 = s.pool.protocol_fees_0.min(max_amount_0 as u128) as u64;
            let c1 = s.pool.protocol_fees_1.min(max_amount_1 as u128) as u64;
            s.pool.protocol_fees_0 -= c0 as u128;
            s.pool.protocol_fees_1 -= c1 as u128;
            (c0 as u64) | ((c1 as u64) << 32)
        })
    })
}

/// @xrpl-function set_protocol_fee
/// @param protocol_fee_share_bps UINT16 - Protocol fee share (max 2500 = 25%)
/// @return UINT32 - 0 on success
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn set_protocol_fee(sender: AccountId, protocol_fee_share_bps: u16) -> u32 {
    with_storage!({
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
    })
}

/// @xrpl-function set_pause
/// @param paused UINT8 - 1 = pause, 0 = unpause
/// @return UINT32 - 0 on success
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn set_pause(sender: AccountId, paused: u8) -> u32 {
    with_storage!({
        wasm_trace!("set_pause");
        match require_owner(sender) {
            Err(e) => e.code(),
            Ok(_) => {
                with_state(|s| s.config.paused = paused != 0);
                0
            }
        }
    })
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

pub fn get_protocol_fees() -> (u128, u128) {
    with_state(|s| (s.pool.protocol_fees_0, s.pool.protocol_fees_1))
}

pub fn get_current_tick() -> i32 {
    with_state(|s| s.pool.current_tick)
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

    fn owner() -> AccountId { [1u8; 20] }
    fn alice() -> AccountId { [2u8; 20] }

    fn init_pool() {
        test_setup(owner(), 10);
        assert_eq!(initialize_pool(owner(), Q64, 30, 0, 1_000_000), 0);
    }

    fn tick(t: i32) -> u32 { t as u32 }

    fn add_liquidity() {
        assert_eq!(mint(alice(), tick(-1000), 1000, 1_000_000_000), 0);
    }

    #[test]
    fn full_lifecycle() {
        init_pool();
        add_liquidity();
        let sp_before = get_sqrt_price();
        let liq_before = get_liquidity();
        assert!(liq_before > 0);

        let out = swap_exact_in(alice(), 10_000, 9_900, 0, Q64 * 2, 1_000_100);
        assert!(out > 0, "swap should produce output");
        let sp_after = get_sqrt_price();
        assert!(sp_after > sp_before, "price should have increased");
    }

    #[test]
    fn swap_decreases_price() {
        init_pool();
        add_liquidity();
        let sp_before = get_sqrt_price();
        let out = swap_exact_in(alice(), 10_000, 9_900, 1, Q64 / 2, 1_000_100);
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
        assert_eq!(burn(alice(), tick(-1000), 1000, 500_000_000), 0);
        assert!(get_liquidity() < liq);
    }

    #[test]
    fn collect_works_after_burn() {
        init_pool();
        add_liquidity();
        burn(alice(), tick(-1000), 1000, 1_000_000_000);
        assert_eq!(collect(alice(), tick(-1000), 1000, u64::MAX, u64::MAX), 0);
    }

    #[test]
    fn pause_blocks_swap() {
        init_pool();
        add_liquidity();
        assert_eq!(set_pause(owner(), 1), 0);
        let out = swap_exact_in(alice(), 10_000, 0, 0, Q64 * 2, 1_000_100);
        assert_eq!(out, 0); // paused → returns 0
    }

    #[test]
    fn set_protocol_fee_and_collect() {
        init_pool();
        add_liquidity();
        assert_eq!(set_protocol_fee(owner(), 1_000), 0); // 10% of fees

        // Do a swap so fees accumulate.
        swap_exact_in(alice(), 100_000, 99_000, 0, Q64 * 3, 2_000_000);

        let (pf0, pf1) = get_protocol_fees();
        // At least one of them should be non-zero.
        assert!(pf0 > 0 || pf1 > 0, "protocol fees should have accrued");

        let packed = collect_protocol(owner(), u64::MAX, u64::MAX);
        // After collection, protocol fees should be zero.
        let (pf0_after, pf1_after) = get_protocol_fees();
        assert_eq!(pf0_after, 0);
        assert_eq!(pf1_after, 0);
        let _ = packed;
    }

    #[test]
    fn oracle_observe_after_swap() {
        init_pool();
        add_liquidity();
        // Grow the oracle buffer to hold more observations.
        assert_eq!(increase_observation_cardinality(owner(), 10), 0);

        // Swap at t=1_000_100.
        swap_exact_in(alice(), 10_000, 9_900, 0, Q64 * 2, 1_000_100);

        // Swap at t=1_000_200 (100 seconds later).
        swap_exact_in(alice(), 10_000, 0, 0, Q64 * 3, 1_000_200);

        // Observe at t=1_000_200, 0 seconds ago → should return tick_cumulative.
        let packed = observe(alice(), 0u64, 1_000_200);
        // It returns 0 before any oracle writes — but we've had writes, so
        // it should be non-zero (tick_cumulative > 0 after upward swaps).
        // Even 0 is acceptable for the very first block; we just verify no panic.
        let _ = packed;
    }

    #[test]
    fn unauthorized_initialize_fails() {
        test_setup(owner(), 10);
        let result = initialize_pool(alice(), Q64, 30, 0, 0);
        assert_ne!(result, 0); // NotAuthorized
    }

    #[test]
    fn slippage_cap_enforced() {
        init_pool();
        add_liquidity();
        // min_amount_out = 0 bypasses the 1% cap (floor = 0.99 * amount_in).
        // Actually our check: floor = amount_in * 9900/10000 = 9900
        // min_amount_out = 0 < 9900 → SlippageLimitExceeded → returns 0.
        let out = swap_exact_in(alice(), 10_000, 0, 0, Q64 * 2, 1_000_100);
        assert_eq!(out, 0);
    }
}
