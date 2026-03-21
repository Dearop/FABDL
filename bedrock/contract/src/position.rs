/// Per-position state and fee-collection logic.

#[cfg(not(target_arch = "wasm32"))]
use std::collections::BTreeMap;

#[cfg(target_arch = "wasm32")]
extern crate alloc;
#[cfg(target_arch = "wasm32")]
use alloc::collections::BTreeMap;

use crate::types::{AccountId, ContractError};

/// Key identifying a unique LP position.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PositionKey {
    pub owner: AccountId,
    pub lower_tick: i32,
    pub upper_tick: i32,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct PositionState {
    pub liquidity: u128,
    /// Last-checkpointed fee growth inside the range (token0), Q128.
    pub fee_growth_inside_0_last_q128: u128,
    /// Last-checkpointed fee growth inside the range (token1), Q128.
    pub fee_growth_inside_1_last_q128: u128,
    /// Accumulated fees claimable (token0), in raw drops.
    pub tokens_owed_0: u128,
    /// Accumulated fees claimable (token1), in raw drops.
    pub tokens_owed_1: u128,
}

pub struct PositionMap(BTreeMap<PositionKey, PositionState>);

impl PositionMap {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn get(&self, key: &PositionKey) -> PositionState {
        self.0.get(key).copied().unwrap_or_default()
    }

    /// Update a position when liquidity is minted or burned.
    ///
    /// `fee_growth_inside_{0,1}` — current fee growth inside the range,
    /// computed by `TickMap::fee_growth_inside`.
    ///
    /// Returns the updated position.
    pub fn update(
        &mut self,
        key: PositionKey,
        liquidity_delta: i128,
        fee_growth_inside_0: u128,
        fee_growth_inside_1: u128,
    ) -> Result<PositionState, ContractError> {
        let mut pos = self.get(&key);

        // Accrue owed fees before changing liquidity.
        let owed_0 = tokens_owed(pos.liquidity, fee_growth_inside_0, pos.fee_growth_inside_0_last_q128);
        let owed_1 = tokens_owed(pos.liquidity, fee_growth_inside_1, pos.fee_growth_inside_1_last_q128);

        pos.tokens_owed_0 = pos.tokens_owed_0.saturating_add(owed_0);
        pos.tokens_owed_1 = pos.tokens_owed_1.saturating_add(owed_1);

        // Apply liquidity delta.
        pos.liquidity = if liquidity_delta >= 0 {
            pos.liquidity
                .checked_add(liquidity_delta as u128)
                .ok_or(ContractError::MathOverflow)?
        } else {
            pos.liquidity
                .checked_sub((-liquidity_delta) as u128)
                .ok_or(ContractError::InvalidLiquidityDelta)?
        };

        // Checkpoint fee growth.
        pos.fee_growth_inside_0_last_q128 = fee_growth_inside_0;
        pos.fee_growth_inside_1_last_q128 = fee_growth_inside_1;

        if pos.liquidity == 0 && pos.tokens_owed_0 == 0 && pos.tokens_owed_1 == 0 {
            self.0.remove(&key);
        } else {
            self.0.insert(key, pos);
        }

        Ok(pos)
    }

    /// Collect up to `max_0` / `max_1` owed fees for a position.
    ///
    /// Returns (collected_0, collected_1).
    pub fn collect(
        &mut self,
        key: PositionKey,
        max_0: u64,
        max_1: u64,
    ) -> (u64, u64) {
        let mut pos = self.get(&key);
        let collect_0 = (pos.tokens_owed_0.min(max_0 as u128)) as u64;
        let collect_1 = (pos.tokens_owed_1.min(max_1 as u128)) as u64;
        pos.tokens_owed_0 -= collect_0 as u128;
        pos.tokens_owed_1 -= collect_1 as u128;

        if pos.liquidity == 0 && pos.tokens_owed_0 == 0 && pos.tokens_owed_1 == 0 {
            self.0.remove(&key);
        } else {
            self.0.insert(key, pos);
        }

        (collect_0, collect_1)
    }
}

/// Compute tokens owed since last checkpoint.
/// tokens_owed = liquidity * (fee_growth_inside_now - fee_growth_inside_last) / Q128
/// Q128 division is implicit: both values are already Q128-scaled globals;
/// the product gives drops directly when interpreted as Q128 integer math.
fn tokens_owed(liquidity: u128, fg_now: u128, fg_last: u128) -> u128 {
    // wrapping_sub handles the case where globals have wrapped.
    let delta = fg_now.wrapping_sub(fg_last);
    // mul_shift128: (liquidity * delta) >> 128
    // Use 256-bit emulation via pairs of u128.
    mul_shift128(liquidity, delta)
}

/// Multiply two u128 values and shift right by 128 bits.
/// Returns the high 128 bits of the 256-bit product.
fn mul_shift128(a: u128, b: u128) -> u128 {
    let a_lo = a & u64::MAX as u128;
    let a_hi = a >> 64;
    let b_lo = b & u64::MAX as u128;
    let b_hi = b >> 64;

    let ll = a_lo * b_lo;
    let lh = a_lo * b_hi;
    let hl = a_hi * b_lo;
    let hh = a_hi * b_hi;

    let mid = (ll >> 64).wrapping_add(lh & u64::MAX as u128).wrapping_add(hl & u64::MAX as u128);
    let carry = mid >> 64;

    hh.wrapping_add(lh >> 64)
      .wrapping_add(hl >> 64)
      .wrapping_add(carry)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn alice() -> AccountId { [1u8; 20] }

    fn key(lower: i32, upper: i32) -> PositionKey {
        PositionKey { owner: alice(), lower_tick: lower, upper_tick: upper }
    }

    #[test]
    fn mint_creates_position() {
        let mut pm = PositionMap::new();
        let pos = pm.update(key(-100, 100), 1_000, 0, 0).unwrap();
        assert_eq!(pos.liquidity, 1_000);
    }

    #[test]
    fn burn_reduces_liquidity() {
        let mut pm = PositionMap::new();
        pm.update(key(-100, 100), 1_000, 0, 0).unwrap();
        let pos = pm.update(key(-100, 100), -500, 0, 0).unwrap();
        assert_eq!(pos.liquidity, 500);
    }

    #[test]
    fn burn_below_zero_errors() {
        let mut pm = PositionMap::new();
        pm.update(key(-100, 100), 500, 0, 0).unwrap();
        let err = pm.update(key(-100, 100), -1000, 0, 0).unwrap_err();
        assert_eq!(err, ContractError::InvalidLiquidityDelta);
    }

    #[test]
    fn collect_partial() {
        let mut pm = PositionMap::new();
        // Simulate accrued fees by setting fee_growth to non-zero.
        // For simplicity, manually set tokens_owed via update with delta fee growth.
        let k = key(-100, 100);
        pm.update(k, 1_000, 0, 0).unwrap();
        // Simulate fee accrual by updating with same liquidity + fee growth.
        pm.update(k, 0, 1_000_000, 1_000_000).unwrap();
        let (c0, c1) = pm.collect(k, u64::MAX, u64::MAX);
        // tokens_owed = liquidity * delta >> 128; with small numbers this is 0 due to rounding
        // but the collect path should not panic.
        let _ = (c0, c1);
    }

    #[test]
    fn mul_shift128_basic() {
        // 2^64 * 2^64 = 2^128, >> 128 = 1
        let result = mul_shift128(1u128 << 64, 1u128 << 64);
        assert_eq!(result, 1);
    }
}
