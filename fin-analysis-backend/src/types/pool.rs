/// Normalised intermediate types, decoupled from both the XRPL wire format
/// and the bedrock wasm32 contract types.
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Price data
// ---------------------------------------------------------------------------

/// A single historical price observation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PricePoint {
    pub timestamp_secs: u64,
    pub xrp_usd: f64,
}

// ---------------------------------------------------------------------------
// Tick snapshot (V3-style, Bedrock contract pools only)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickSnapshot {
    pub tick: i32,
    /// Net liquidity change when crossed left-to-right.
    pub liquidity_net: i128,
    /// Fee growth outside this boundary (token0), Q128.
    pub fee_growth_outside_0_q128: u128,
    /// Fee growth outside this boundary (token1), Q128.
    pub fee_growth_outside_1_q128: u128,
}

// ---------------------------------------------------------------------------
// Position snapshot
// ---------------------------------------------------------------------------

/// One LP position held by the user in a pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSnapshot {
    pub owner: String,
    /// For native XRPL AMM (constant-product) set both to 0 — ticks are unused.
    pub lower_tick: i32,
    pub upper_tick: i32,
    /// Uniswap V3-style liquidity (raw Q128 units). For native XRPL AMM,
    /// this is the LP-token share expressed in the same units.
    pub liquidity: u128,
    /// Last-checkpointed fee growth inside the range (token0), Q128.
    pub fee_growth_inside_0_last_q128: u128,
    /// Last-checkpointed fee growth inside the range (token1), Q128.
    pub fee_growth_inside_1_last_q128: u128,
    /// Token0 (XRP, in whole XRP) held at position entry.
    /// Estimated from pool state if not stored on-chain.
    pub amount0_at_entry: f64,
    /// Token1 (issued token) held at position entry.
    pub amount1_at_entry: f64,
    /// XRP price (USD) at position entry.
    pub entry_price_usd: f64,
    /// LP tokens held by the user.
    pub lp_tokens_held: f64,
}

// ---------------------------------------------------------------------------
// Pool snapshot
// ---------------------------------------------------------------------------

/// The on-chain state of one AMM pool at query time, together with all
/// positions the user holds in it and recent price history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSnapshot {
    /// Human-readable label, e.g. "XRP/USD".
    pub pool_label: String,
    /// XRPL account address of the AMM.
    pub amm_account: String,
    /// XRP reserve in drops (1 XRP = 1_000_000 drops).
    pub reserve_xrp_drops: u128,
    /// Token reserve in raw smallest units (see `AmountField::parse_raw`).
    pub reserve_token_raw: u128,
    /// ISO-4217 or hex currency code of the token side.
    pub token_currency: String,
    /// XRPL issuer address of the token.
    pub token_issuer: String,
    /// Total LP token supply (float, from ledger).
    pub lp_token_supply: f64,
    /// Pool fee in basis points (0–1000).
    pub trading_fee_bps: u16,
    /// Current XRP/USD spot price.
    pub current_xrp_price_usd: f64,

    // -----------------------------------------------------------------------
    // V3 fields — present only when this pool is served by the Bedrock contract.
    // -----------------------------------------------------------------------

    /// Current sqrt price in Q64.64.
    pub sqrt_price_q64: Option<u128>,
    /// Current tick index.
    pub current_tick: Option<i32>,
    /// Active liquidity at current price.
    pub liquidity_active: Option<u128>,
    /// Global fee growth accumulator (token0), Q128.
    pub fee_growth_global_0_q128: Option<u128>,
    /// Global fee growth accumulator (token1), Q128.
    pub fee_growth_global_1_q128: Option<u128>,
    /// All initialised ticks in this pool (sorted ascending).
    pub ticks: Vec<TickSnapshot>,

    // -----------------------------------------------------------------------
    // User positions
    // -----------------------------------------------------------------------

    /// Positions owned by the queried wallet in this pool.
    pub positions: Vec<PositionSnapshot>,

    /// Recent XRP/USD price history for VaR / Sharpe calculations.
    pub price_history: Vec<PricePoint>,
}

impl PoolSnapshot {
    /// XRP reserve in whole XRP (convenience).
    pub fn reserve_xrp(&self) -> f64 {
        self.reserve_xrp_drops as f64 / 1_000_000.0
    }

    /// Token reserve as float with 6-decimal precision.
    pub fn reserve_token(&self) -> f64 {
        self.reserve_token_raw as f64 / 1_000_000.0
    }

    /// Whether this pool uses V3-style tick-based liquidity (Bedrock contract).
    pub fn is_v3(&self) -> bool {
        self.sqrt_price_q64.is_some()
    }

    /// Implied XRP entry price for a constant-product pool.
    /// Returns `current_xrp_price_usd` when no entry price is otherwise known.
    pub fn implied_price_usd(&self) -> f64 {
        if self.reserve_xrp_drops == 0 {
            return self.current_xrp_price_usd;
        }
        // For a constant-product AMM: price_usd = (token_reserve / xrp_reserve)
        // where token_reserve is in USD-equivalent units.
        // We rely on the current_xrp_price_usd being set correctly by the caller.
        self.current_xrp_price_usd
    }
}
