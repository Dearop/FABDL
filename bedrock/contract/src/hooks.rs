/// Generic hook mechanism for lifecycle callbacks.
///
/// Hooks are selected at pool initialization via `HookId` and stored in
/// `ContractConfig`. Every mint/burn/swap calls the relevant before/after
/// method, which may inspect pool state and return `Err` to abort the
/// operation before any state is mutated.
///
/// All implementations are compiled into the same WASM binary — there are no
/// cross-contract calls. Adding a new hook requires:
///   1. Define a zero-size struct and `impl Hook for it`.
///   2. Add a variant to `HookId`.
///   3. Add a `static` instance and a match arm in `get()`.

use crate::types::ContractError;

// ---------------------------------------------------------------------------
// Context types
// ---------------------------------------------------------------------------

/// Read-only pool state snapshot passed into every hook call.
#[derive(Clone, Copy)]
pub struct HookContext {
    pub current_tick: i32,
    pub sqrt_price: u128,
    pub liquidity: u128,
    pub fee_bps: u16,
}

/// Swap outcome passed to `after_swap`.
#[derive(Clone, Copy)]
pub struct SwapOutcome {
    pub amount_in: u64,
    pub amount_out: u64,
    pub sqrt_price_after: u128,
    pub tick_after: i32,
    pub ticks_crossed: u32,
}

// ---------------------------------------------------------------------------
// Hook trait — all methods default to no-ops
// ---------------------------------------------------------------------------

pub trait Hook: Sync {
    fn before_swap(
        &self,
        _ctx: &HookContext,
        _zero_for_one: bool,
        _amount_in: u64,
    ) -> Result<(), ContractError> {
        Ok(())
    }

    fn after_swap(
        &self,
        _ctx: &HookContext,
        _outcome: &SwapOutcome,
    ) -> Result<(), ContractError> {
        Ok(())
    }

    fn before_mint(
        &self,
        _ctx: &HookContext,
        _lower: i32,
        _upper: i32,
        _liquidity_delta: u128,
    ) -> Result<(), ContractError> {
        Ok(())
    }

    fn after_mint(
        &self,
        _ctx: &HookContext,
        _lower: i32,
        _upper: i32,
        _liquidity_delta: u128,
    ) -> Result<(), ContractError> {
        Ok(())
    }

    fn before_burn(
        &self,
        _ctx: &HookContext,
        _lower: i32,
        _upper: i32,
        _liquidity_delta: u128,
    ) -> Result<(), ContractError> {
        Ok(())
    }

    fn after_burn(
        &self,
        _ctx: &HookContext,
        _lower: i32,
        _upper: i32,
        _liquidity_delta: u128,
    ) -> Result<(), ContractError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// HookId — 1 byte, stored in ContractConfig, persisted via codec
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum HookId {
    /// No hook. All lifecycle calls are no-ops.
    #[default]
    None = 0,
    /// Rejects swaps larger than 5% of active liquidity and positions narrower
    /// than 200 ticks. Designed for the Conservative Hedge strategy.
    ConservativeHedge = 1,
    /// Requires new positions to straddle the current tick so liquidity is
    /// immediately active. Designed for the Yield Rebalance strategy.
    YieldRebalance = 2,
}

impl HookId {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => HookId::ConservativeHedge,
            2 => HookId::YieldRebalance,
            _ => HookId::None,
        }
    }

    pub fn to_u8(self) -> u8 {
        self as u8
    }
}

// ---------------------------------------------------------------------------
// Dispatch — static vtable pointers, no heap allocation
// ---------------------------------------------------------------------------

static NOOP: NoopHook = NoopHook;
static CONSERVATIVE: ConservativeHedgeHook = ConservativeHedgeHook;
static YIELD: YieldRebalanceHook = YieldRebalanceHook;

/// Return the hook for the given id as a `&'static dyn Hook`.
pub fn get(id: HookId) -> &'static dyn Hook {
    match id {
        HookId::None => &NOOP,
        HookId::ConservativeHedge => &CONSERVATIVE,
        HookId::YieldRebalance => &YIELD,
    }
}

// ---------------------------------------------------------------------------
// Implementations
// ---------------------------------------------------------------------------

struct NoopHook;
impl Hook for NoopHook {}

/// Conservative hedge hook.
///
/// - `before_swap`: block swaps where `amount_in > 5%` of active liquidity.
///   Outsized swaps cause disproportionate IL for LP positions; this cap
///   keeps individual trade impact manageable.
///
/// - `before_mint`: require `upper - lower >= 200` ticks.
///   Narrow positions go out-of-range quickly and earn zero fees during
///   directional price moves; 200 ticks (~2% price range) is the minimum
///   width that remains active across typical daily volatility.
struct ConservativeHedgeHook;
impl Hook for ConservativeHedgeHook {
    fn before_swap(
        &self,
        ctx: &HookContext,
        _zero_for_one: bool,
        amount_in: u64,
    ) -> Result<(), ContractError> {
        if ctx.liquidity > 0 {
            let cap = (ctx.liquidity / 20) as u64; // 5% of liquidity
            if amount_in > cap {
                return Err(ContractError::SlippageLimitExceeded);
            }
        }
        Ok(())
    }

    fn before_mint(
        &self,
        _ctx: &HookContext,
        lower: i32,
        upper: i32,
        _liquidity_delta: u128,
    ) -> Result<(), ContractError> {
        if upper - lower < 200 {
            return Err(ContractError::InvalidTickRange);
        }
        Ok(())
    }
}

/// Yield rebalance hook.
///
/// - `before_mint`: require `lower < current_tick < upper`.
///   Positions that don't include the current price contribute zero active
///   liquidity and earn no fees. This hook forces providers to keep their
///   positions centred on the current price, maximising fee yield.
struct YieldRebalanceHook;
impl Hook for YieldRebalanceHook {
    fn before_mint(
        &self,
        ctx: &HookContext,
        lower: i32,
        upper: i32,
        _liquidity_delta: u128,
    ) -> Result<(), ContractError> {
        if ctx.current_tick <= lower || ctx.current_tick >= upper {
            return Err(ContractError::InvalidTickRange);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Q64;

    fn ctx_with(current_tick: i32, liquidity: u128) -> HookContext {
        HookContext { current_tick, sqrt_price: Q64, liquidity, fee_bps: 30 }
    }

    // --- NoopHook ---

    #[test]
    fn noop_allows_everything() {
        let h = get(HookId::None);
        assert!(h.before_swap(&ctx_with(0, 1_000_000), true, 999_999).is_ok());
        assert!(h.before_mint(&ctx_with(0, 0), -100, 100, 1).is_ok());
        assert!(h.before_burn(&ctx_with(0, 0), -100, 100, 1).is_ok());
        assert!(h.after_swap(&ctx_with(0, 0), &SwapOutcome {
            amount_in: 1, amount_out: 1, sqrt_price_after: Q64,
            tick_after: 0, ticks_crossed: 0,
        }).is_ok());
    }

    // --- ConservativeHedgeHook ---

    #[test]
    fn conservative_blocks_oversized_swap() {
        let h = get(HookId::ConservativeHedge);
        let c = ctx_with(0, 1_000_000);
        // cap = 1_000_000 / 20 = 50_000
        assert!(h.before_swap(&c, true, 50_001).is_err());
        assert!(h.before_swap(&c, true, 50_000).is_ok());
    }

    #[test]
    fn conservative_allows_swap_when_pool_empty() {
        // liquidity = 0 → cap branch skipped, any amount is allowed
        let h = get(HookId::ConservativeHedge);
        assert!(h.before_swap(&ctx_with(0, 0), false, u64::MAX).is_ok());
    }

    #[test]
    fn conservative_blocks_narrow_mint() {
        let h = get(HookId::ConservativeHedge);
        assert!(h.before_mint(&ctx_with(0, 0), 0, 199, 1).is_err()); // 199 ticks — too narrow
        assert!(h.before_mint(&ctx_with(0, 0), 0, 200, 1).is_ok());  // exactly 200 — allowed
        assert!(h.before_mint(&ctx_with(0, 0), -500, 500, 1).is_ok()); // wide — fine
    }

    #[test]
    fn conservative_does_not_restrict_burn() {
        let h = get(HookId::ConservativeHedge);
        // Narrow position, but burn should always be allowed
        assert!(h.before_burn(&ctx_with(0, 0), 0, 10, 1).is_ok());
    }

    // --- YieldRebalanceHook ---

    #[test]
    fn yield_requires_position_to_straddle_tick() {
        let h = get(HookId::YieldRebalance);
        let c = ctx_with(0, 1_000_000); // current_tick = 0

        // Position entirely above current tick — rejected
        assert!(h.before_mint(&c, 10, 100, 1).is_err());
        // Position entirely below current tick — rejected
        assert!(h.before_mint(&c, -100, -10, 1).is_err());
        // Current tick sits on lower boundary — rejected (lower < tick required)
        assert!(h.before_mint(&c, 0, 100, 1).is_err());
        // Current tick sits on upper boundary — rejected (tick < upper required)
        assert!(h.before_mint(&c, -100, 0, 1).is_err());
        // Position straddles current tick — allowed
        assert!(h.before_mint(&c, -10, 10, 1).is_ok());
    }

    #[test]
    fn yield_does_not_restrict_burn() {
        let h = get(HookId::YieldRebalance);
        // Can always burn regardless of whether position still straddles tick
        assert!(h.before_burn(&ctx_with(100, 0), 0, 10, 1).is_ok());
    }

    // --- HookId codec ---

    #[test]
    fn hook_id_roundtrip() {
        for &id in &[HookId::None, HookId::ConservativeHedge, HookId::YieldRebalance] {
            assert_eq!(HookId::from_u8(id.to_u8()), id);
        }
        assert_eq!(HookId::from_u8(255), HookId::None); // unknown → None
    }
}
