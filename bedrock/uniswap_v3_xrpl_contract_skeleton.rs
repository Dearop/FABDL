#![cfg_attr(target_arch = "wasm32", no_std)]

#[cfg(not(target_arch = "wasm32"))]
extern crate std;

use core::cmp::{max, min};

pub type AccountId = [u8; 20];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AssetKind {
    Xrp,
    Issued,
}

#[derive(Clone, Copy)]
pub struct Asset {
    pub kind: AssetKind,
    pub code: [u8; 20],
    pub issuer: AccountId,
}

#[derive(Clone, Copy)]
pub struct PoolKey {
    pub asset0: Asset,
    pub asset1: Asset,
    pub fee_bps: u16,
}

#[derive(Clone, Copy)]
pub struct PoolState {
    pub sqrt_price_q64_64: u128,
    pub current_tick: i32,
    pub liquidity_active: u128,
    pub protocol_fee_share_bps: u16,
    pub fee_growth_global_0_q128: u128,
    pub fee_growth_global_1_q128: u128,
    pub protocol_fees_0: u128,
    pub protocol_fees_1: u128,
}

#[derive(Clone, Copy)]
pub struct TickState {
    pub liquidity_gross: u128,
    pub liquidity_net: i128,
    pub fee_growth_outside_0_q128: u128,
    pub fee_growth_outside_1_q128: u128,
    pub seconds_outside: u64,
    pub tick_cumulative_outside: i128,
    pub seconds_per_liquidity_outside_q128: u128,
}

#[derive(Clone, Copy)]
pub struct PositionState {
    pub owner: AccountId,
    pub lower_tick: i32,
    pub upper_tick: i32,
    pub liquidity: u128,
    pub fee_growth_inside_0_last_q128: u128,
    pub fee_growth_inside_1_last_q128: u128,
    pub tokens_owed_0: u128,
    pub tokens_owed_1: u128,
}

pub enum ContractError {
    InvalidTickRange,
    TickSpacingViolation,
    SlippageLimitExceeded,
    NotAuthorized,
    Paused,
    MathOverflow,
    InvalidLiquidityDelta,
}

pub struct SwapQuote {
    pub amount_in: u64,
    pub amount_out: u64,
    pub end_sqrt_price_q64_64: u128,
    pub crossed_ticks: u32,
}

pub struct ContractConfig {
    pub owner: AccountId,
    pub paused: bool,
    pub max_slippage_bps: u16,
    pub tick_spacing: i32,
}

pub struct UniswapV3XrplContract {
    pub config: ContractConfig,
    // In real implementation, these are persistent mappings.
    pub pool: PoolState,
}

impl UniswapV3XrplContract {
    pub fn new(owner: AccountId, tick_spacing: i32) -> Self {
        Self {
            config: ContractConfig {
                owner,
                paused: false,
                max_slippage_bps: 100,
                tick_spacing,
            },
            pool: PoolState {
                sqrt_price_q64_64: 1u128 << 64,
                current_tick: 0,
                liquidity_active: 0,
                protocol_fee_share_bps: 0,
                fee_growth_global_0_q128: 0,
                fee_growth_global_1_q128: 0,
                protocol_fees_0: 0,
                protocol_fees_1: 0,
            },
        }
    }

    fn require_not_paused(&self) -> Result<(), ContractError> {
        if self.config.paused {
            return Err(ContractError::Paused);
        }
        Ok(())
    }

    fn require_owner(&self, sender: AccountId) -> Result<(), ContractError> {
        if sender != self.config.owner {
            return Err(ContractError::NotAuthorized);
        }
        Ok(())
    }

    fn validate_ticks(&self, lower_tick: i32, upper_tick: i32) -> Result<(), ContractError> {
        if lower_tick >= upper_tick {
            return Err(ContractError::InvalidTickRange);
        }
        if lower_tick % self.config.tick_spacing != 0 || upper_tick % self.config.tick_spacing != 0 {
            return Err(ContractError::TickSpacingViolation);
        }
        Ok(())
    }

    // TODO: persist and update tick/position state mappings.
    fn update_position_accounting(
        &mut self,
        _owner: AccountId,
        _lower_tick: i32,
        _upper_tick: i32,
        _liquidity_delta: i128,
    ) -> Result<(), ContractError> {
        Ok(())
    }

    // TODO: implement v3-style single-tick step and cross-tick loop.
    fn execute_swap_math(
        &mut self,
        amount_in: u64,
        _zero_for_one: bool,
        _sqrt_price_limit_q64_64: u128,
    ) -> Result<SwapQuote, ContractError> {
        if amount_in == 0 {
            return Err(ContractError::InvalidLiquidityDelta);
        }
        let amount_out = amount_in.saturating_sub((amount_in / 1000) * 3);
        Ok(SwapQuote {
            amount_in,
            amount_out,
            end_sqrt_price_q64_64: self.pool.sqrt_price_q64_64,
            crossed_ticks: 0,
        })
    }

    /// @xrpl-function initialize_pool
    /// @param sqrt_price_q64_64 UINT128 - Initial sqrt price
    /// @param protocol_fee_share_bps UINT16 - Protocol fee share in bps
    pub fn initialize_pool(
        &mut self,
        sender: AccountId,
        sqrt_price_q64_64: u128,
        protocol_fee_share_bps: u16,
    ) -> Result<u32, ContractError> {
        self.require_owner(sender)?;
        self.pool.sqrt_price_q64_64 = max(1u128 << 32, sqrt_price_q64_64);
        self.pool.protocol_fee_share_bps = min(protocol_fee_share_bps, 2500);
        Ok(0)
    }

    /// @xrpl-function mint
    /// @param lower_tick INT32 - Lower tick boundary
    /// @param upper_tick INT32 - Upper tick boundary
    /// @param liquidity_delta INT128 - Positive liquidity amount
    pub fn mint(
        &mut self,
        sender: AccountId,
        lower_tick: i32,
        upper_tick: i32,
        liquidity_delta: i128,
    ) -> Result<u32, ContractError> {
        self.require_not_paused()?;
        self.validate_ticks(lower_tick, upper_tick)?;
        if liquidity_delta <= 0 {
            return Err(ContractError::InvalidLiquidityDelta);
        }
        self.update_position_accounting(sender, lower_tick, upper_tick, liquidity_delta)?;
        self.pool.liquidity_active = self
            .pool
            .liquidity_active
            .saturating_add(liquidity_delta as u128);
        Ok(0)
    }

    /// @xrpl-function burn
    /// @param lower_tick INT32 - Lower tick boundary
    /// @param upper_tick INT32 - Upper tick boundary
    /// @param liquidity_delta INT128 - Negative liquidity amount
    pub fn burn(
        &mut self,
        sender: AccountId,
        lower_tick: i32,
        upper_tick: i32,
        liquidity_delta: i128,
    ) -> Result<u32, ContractError> {
        self.require_not_paused()?;
        self.validate_ticks(lower_tick, upper_tick)?;
        if liquidity_delta >= 0 {
            return Err(ContractError::InvalidLiquidityDelta);
        }
        self.update_position_accounting(sender, lower_tick, upper_tick, liquidity_delta)?;
        self.pool.liquidity_active = self
            .pool
            .liquidity_active
            .saturating_sub((-liquidity_delta) as u128);
        Ok(0)
    }

    /// @xrpl-function collect
    /// @param lower_tick INT32 - Lower tick boundary
    /// @param upper_tick INT32 - Upper tick boundary
    /// @param max_amount_0 UINT64 - Max token0 to collect
    /// @param max_amount_1 UINT64 - Max token1 to collect
    pub fn collect(
        &mut self,
        _sender: AccountId,
        _lower_tick: i32,
        _upper_tick: i32,
        _max_amount_0: u64,
        _max_amount_1: u64,
    ) -> Result<u32, ContractError> {
        self.require_not_paused()?;
        Ok(0)
    }

    /// @xrpl-function swap_exact_in
    /// @param amount_in UINT64 - Exact input amount
    /// @param min_amount_out UINT64 - Slippage protected minimum output
    /// @param zero_for_one UINT8 - 1 if swapping token0->token1
    /// @param sqrt_price_limit_q64_64 UINT128 - Price boundary
    pub fn swap_exact_in(
        &mut self,
        _sender: AccountId,
        amount_in: u64,
        min_amount_out: u64,
        zero_for_one: u8,
        sqrt_price_limit_q64_64: u128,
    ) -> Result<u64, ContractError> {
        self.require_not_paused()?;
        let quote = self.execute_swap_math(amount_in, zero_for_one == 1, sqrt_price_limit_q64_64)?;
        if quote.amount_out < min_amount_out {
            return Err(ContractError::SlippageLimitExceeded);
        }
        Ok(quote.amount_out)
    }

    /// @xrpl-function set_protocol_fee
    /// @param protocol_fee_share_bps UINT16 - Fee share in bps
    pub fn set_protocol_fee(
        &mut self,
        sender: AccountId,
        protocol_fee_share_bps: u16,
    ) -> Result<u32, ContractError> {
        self.require_owner(sender)?;
        self.pool.protocol_fee_share_bps = min(protocol_fee_share_bps, 2500);
        Ok(0)
    }

    /// @xrpl-function set_pause
    /// @param paused UINT8 - 1 pauses, 0 unpauses
    pub fn set_pause(&mut self, sender: AccountId, paused: u8) -> Result<u32, ContractError> {
        self.require_owner(sender)?;
        self.config.paused = paused == 1;
        Ok(0)
    }
}
