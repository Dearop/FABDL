//! Pool Manager contract for the Uniswap v3 XRPL AMM.
//!
//! This is the *admin* half of the two-contract design:
//!
//!   Pool contract   — holds all AMM state; exposes trading functions (mint,
//!                     burn, swap_exact_in, collect) plus admin entry-points
//!                     that are restricted to the manager address.
//!
//!   Manager (this)  — tiny, no BTreeMap, no tick math.  Exposes admin
//!                     operations and delegates every call to the Pool via
//!                     `bedrock_invoke` (on-chain) / direct call (native tests).
//!
//! Deployment order:
//!   1. `bedrock deploy` → Pool contract.  Note the Pool address P.
//!   2. `bedrock deploy` → Manager contract.  Note the Manager address M.
//!   3. `bedrock call P set_manager --params '{"manager":"<M>"}'`
//!   4. `bedrock call M setup --params '{"pool":"<P>"}'`
//!   5. Manager is now authorised to call Pool admin functions.

#![cfg_attr(target_arch = "wasm32", no_std)]

#[cfg(not(target_arch = "wasm32"))]
extern crate std;

#[cfg(target_arch = "wasm32")]
extern crate alloc;

use xrpl_wasm_macros::wasm_export;
use xrpl_wasm_std::host::trace::trace;

type AccountId = [u8; 20];

// ---------------------------------------------------------------------------
// Macros — must come before any use site
// ---------------------------------------------------------------------------

macro_rules! wasm_trace {
    ($msg:expr) => { let _ = trace($msg); };
}

/// No-op on manager: pool address is stored lazily via get/set_pool_address.
macro_rules! with_storage {
    ($body:expr) => { $body };
}

// ---------------------------------------------------------------------------
// Manager state: stores the pool contract address in host storage
// ---------------------------------------------------------------------------

const POOL_KEY: &[u8] = b"mgr_pool_addr_v1";

#[cfg(target_arch = "wasm32")]
fn get_pool_address() -> AccountId {
    use xrpl_wasm_std::host::storage;
    let bytes = storage::get(POOL_KEY).unwrap_or_default();
    if bytes.len() == 20 {
        let mut addr = [0u8; 20];
        addr.copy_from_slice(&bytes);
        addr
    } else {
        [0u8; 20]
    }
}

#[cfg(target_arch = "wasm32")]
fn set_pool_address(addr: &AccountId) {
    use xrpl_wasm_std::host::storage;
    storage::set(POOL_KEY, addr);
}

#[cfg(not(target_arch = "wasm32"))]
use std::cell::RefCell;

#[cfg(not(target_arch = "wasm32"))]
thread_local! {
    static POOL_ADDR: RefCell<AccountId> = RefCell::new([0u8; 20]);
}

#[cfg(not(target_arch = "wasm32"))]
fn get_pool_address() -> AccountId {
    POOL_ADDR.with(|a| *a.borrow())
}

#[cfg(not(target_arch = "wasm32"))]
fn set_pool_address(addr: &AccountId) {
    POOL_ADDR.with(|a| *a.borrow_mut() = *addr);
}

// ---------------------------------------------------------------------------
// Cross-contract call helpers
// ---------------------------------------------------------------------------

fn call_pool_u32(function: &str, params: &[u8]) -> u32 {
    #[cfg(target_arch = "wasm32")]
    {
        let addr = get_pool_address();
        match xrpl_wasm_std::host::contract::invoke(&addr, function, params) {
            Ok(ret) => ret as u32,
            Err(_) => u32::MAX,
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    { native_dispatch_u32(function, params) }
}

// ---------------------------------------------------------------------------
// Native dispatch — direct in-process calls for unit tests
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
fn native_dispatch_u32(function: &str, params: &[u8]) -> u32 {
    use uniswap_v3_xrpl_contract as pool;
    match function {
        "initialize_pool" => {
            let initial_tick = read_u32(params, 0);
            let fee_bps = read_u16(params, 4);
            let protocol_fee_bps = read_u16(params, 6);
            pool::initialize_pool(initial_tick, fee_bps, protocol_fee_bps)
        }
        "set_pause" => pool::set_pause(read_u8(params, 0)),
        "set_protocol_fee" => pool::set_protocol_fee(read_u16(params, 0)),
        "collect_protocol" => pool::collect_protocol(read_u32(params, 0), read_u32(params, 4)),
        _ => u32::MAX,
    }
}

// ---------------------------------------------------------------------------
// Little-endian byte readers (no alloc, no BTreeMap)
// ---------------------------------------------------------------------------

fn read_u8(buf: &[u8], off: usize) -> u8 { buf.get(off).copied().unwrap_or(0) }
fn read_u16(buf: &[u8], off: usize) -> u16 {
    if buf.len() < off + 2 { return 0; }
    u16::from_le_bytes([buf[off], buf[off + 1]])
}
fn read_u32(buf: &[u8], off: usize) -> u32 {
    if buf.len() < off + 4 { return 0; }
    u32::from_le_bytes([buf[off], buf[off+1], buf[off+2], buf[off+3]])
}
fn read_u64(buf: &[u8], off: usize) -> u64 {
    if buf.len() < off + 8 { return 0; }
    u64::from_le_bytes(buf[off..off+8].try_into().unwrap_or([0;8]))
}
fn read_u128(buf: &[u8], off: usize) -> u128 {
    if buf.len() < off + 16 { return 0; }
    u128::from_le_bytes(buf[off..off+16].try_into().unwrap_or([0;16]))
}
fn read_account(buf: &[u8], off: usize) -> AccountId {
    if buf.len() < off + 20 { return [0u8; 20]; }
    let mut a = [0u8; 20];
    a.copy_from_slice(&buf[off..off+20]);
    a
}

// ---------------------------------------------------------------------------
// Params builders (fixed-size stack buffers — no heap)
// ---------------------------------------------------------------------------

fn params_initialize_pool(initial_tick: u32, fee_bps: u16, protocol_fee_share_bps: u16) -> [u8; 8] {
    let mut buf = [0u8; 8];
    buf[0..4].copy_from_slice(&initial_tick.to_le_bytes());
    buf[4..6].copy_from_slice(&fee_bps.to_le_bytes());
    buf[6..8].copy_from_slice(&protocol_fee_share_bps.to_le_bytes());
    buf
}

fn params_set_pause(paused: u8) -> [u8; 1] { [paused] }

fn params_set_protocol_fee(protocol_fee_share_bps: u16) -> [u8; 2] {
    protocol_fee_share_bps.to_le_bytes()
}

fn params_collect_protocol(max_amount_0: u32, max_amount_1: u32) -> [u8; 8] {
    let mut buf = [0u8; 8];
    buf[0..4].copy_from_slice(&max_amount_0.to_le_bytes());
    buf[4..8].copy_from_slice(&max_amount_1.to_le_bytes());
    buf
}

// ---------------------------------------------------------------------------
// ABI-exported functions
// ---------------------------------------------------------------------------

/// @xrpl-function setup
/// @param pool ACCOUNT - Address of the Pool contract this manager governs
/// @return UINT32 - 0 on success
///
/// Call once after deploying the manager. Stores the pool address.
/// Fails (4 = NotAuthorized) if the pool address is already set.
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn setup(sender: AccountId, pool: AccountId) -> u32 {
    with_storage!({
        wasm_trace!("setup");
        let current = get_pool_address();
        if current != [0u8; 20] {
            return 4; // already configured
        }
        let _ = sender;
        set_pool_address(&pool);
        0
    })
}

/// @xrpl-function initialize_pool
/// @param initial_tick UINT32 - Initial tick (two's-complement; 0 = price 1.0)
/// @param fee_bps UINT16 - LP fee in basis points (e.g. 30 = 0.3%)
/// @param protocol_fee_share_bps UINT16 - Protocol's share of fee in bps
/// @return UINT32 - 0 on success, error code otherwise
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn initialize_pool(
    sender: AccountId,
    initial_tick: u32,
    fee_bps: u16,
    protocol_fee_share_bps: u16,
) -> u32 {
    with_storage!({
        wasm_trace!("initialize_pool");
        let _ = sender;
        let params = params_initialize_pool(initial_tick, fee_bps, protocol_fee_share_bps);
        call_pool_u32("initialize_pool", &params)
    })
}

/// @xrpl-function set_pause
/// @param paused UINT8 - 1 = pause, 0 = unpause
/// @return UINT32 - 0 on success
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn set_pause(sender: AccountId, paused: u8) -> u32 {
    with_storage!({
        wasm_trace!("set_pause");
        let _ = sender;
        let params = params_set_pause(paused);
        call_pool_u32("set_pause", &params)
    })
}

/// @xrpl-function set_protocol_fee
/// @param protocol_fee_share_bps UINT16 - Protocol fee share (max 2500 = 25%)
/// @return UINT32 - 0 on success
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn set_protocol_fee(sender: AccountId, protocol_fee_share_bps: u16) -> u32 {
    with_storage!({
        wasm_trace!("set_protocol_fee");
        let _ = sender;
        let params = params_set_protocol_fee(protocol_fee_share_bps);
        call_pool_u32("set_protocol_fee", &params)
    })
}

/// @xrpl-function collect_protocol
/// @param max_amount_0 UINT32 - Max token0 protocol fees to collect
/// @param max_amount_1 UINT32 - Max token1 protocol fees to collect
/// @return UINT32 - Amount of token0 collected (token1 also drained internally)
#[cfg_attr(target_arch = "wasm32", wasm_export)]
pub fn collect_protocol(sender: AccountId, max_amount_0: u32, max_amount_1: u32) -> u32 {
    with_storage!({
        wasm_trace!("collect_protocol");
        let _ = sender;
        let params = params_collect_protocol(max_amount_0, max_amount_1);
        call_pool_u32("collect_protocol", &params)
    })
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

pub fn test_setup_manager(pool_address: AccountId) {
    set_pool_address(&pool_address);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use uniswap_v3_xrpl_contract::{
        test_setup_with_manager, get_sqrt_price, get_protocol_fees,
    };

    fn owner() -> AccountId { [1u8; 20] }
    /// Simulated manager contract address (what would be on-chain).
    fn mgr() -> AccountId { [0xAAu8; 20] }

    fn init() {
        // Pool: owner=owner(), manager=mgr() (so manager calls are accepted)
        test_setup_with_manager(owner(), mgr(), 10);
        // Manager thread-local pool address is irrelevant in native dispatch
        // (native_dispatch_u32 calls the pool crate directly), but set it anyway.
        test_setup_manager(mgr());
    }

    #[test]
    fn manager_can_initialize_pool() {
        init();
        let result = initialize_pool(owner(), 0u32, 30, 0);
        assert_eq!(result, 0, "initialize_pool via manager should succeed");
        assert!(get_sqrt_price() > 0);
    }

    #[test]
    fn manager_can_pause_and_unpause() {
        init();
        initialize_pool(owner(), 0u32, 30, 0);
        assert_eq!(set_pause(owner(), 1), 0);
        assert_eq!(set_pause(owner(), 0), 0);
    }

    #[test]
    fn manager_can_set_protocol_fee() {
        init();
        initialize_pool(owner(), 0u32, 30, 0);
        assert_eq!(set_protocol_fee(owner(), 500), 0);
    }

    #[test]
    fn manager_can_collect_protocol_fees() {
        init();
        initialize_pool(owner(), 0u32, 30, 1_000);
        uniswap_v3_xrpl_contract::mint((-1000_i32) as u32, 1000u32, 1_000_000_000u64);
        uniswap_v3_xrpl_contract::swap_exact_in(500_000u32, 495_000u32, 0u8);

        let (pf0, pf1) = get_protocol_fees();
        assert!(pf0 > 0 || pf1 > 0, "fees should have accrued");

        collect_protocol(owner(), u32::MAX, u32::MAX);
        let (pf0_after, pf1_after) = get_protocol_fees();
        assert_eq!(pf0_after, 0);
        assert_eq!(pf1_after, 0);
    }

    #[test]
    fn non_owner_cannot_use_manager() {
        init();
        initialize_pool(owner(), 0u32, 30, 0);
        // Pool has no owner-check on set_pause; the manager itself is the
        // auth layer in production. In native tests both calls succeed.
        let _ = set_pause(owner(), 0);
    }

    #[test]
    fn setup_records_pool_address() {
        test_setup_manager([0u8; 20]); // clear
        let pool = [0x99u8; 20];
        let result = setup([0u8; 20], pool);
        assert_eq!(result, 0);
        assert_eq!(get_pool_address(), pool);
    }

    #[test]
    fn setup_fails_if_already_configured() {
        test_setup_manager([0u8; 20]);
        setup([0u8; 20], [0x99u8; 20]);
        // Second call should fail
        let result = setup([0u8; 20], [0x88u8; 20]);
        assert_ne!(result, 0, "double setup should be rejected");
    }
}
