/// Per-position state and fee-collection logic.

use crate::types::{AccountId, ContractError};

const MAX_POSITIONS: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PositionKey {
    pub owner: AccountId,
    pub lower_tick: i32,
    pub upper_tick: i32,
}

const DEFAULT_POSITION_KEY: PositionKey = PositionKey {
    owner: [0u8; 20],
    lower_tick: 0,
    upper_tick: 0,
};

#[derive(Clone, Copy, Default, Debug)]
pub struct PositionState {
    pub liquidity: u128,
    pub fee_growth_inside_0_last_q128: u128,
    pub fee_growth_inside_1_last_q128: u128,
    pub tokens_owed_0: u128,
    pub tokens_owed_1: u128,
}

const DEFAULT_POSITION_STATE: PositionState = PositionState {
    liquidity: 0,
    fee_growth_inside_0_last_q128: 0,
    fee_growth_inside_1_last_q128: 0,
    tokens_owed_0: 0,
    tokens_owed_1: 0,
};

pub struct PositionMap {
    pub keys: [PositionKey; MAX_POSITIONS],
    pub vals: [PositionState; MAX_POSITIONS],
    pub len: usize,
}

impl PositionMap {
    pub fn new() -> Self {
        Self {
            keys: [DEFAULT_POSITION_KEY; MAX_POSITIONS],
            vals: [DEFAULT_POSITION_STATE; MAX_POSITIONS],
            len: 0,
        }
    }

    fn find(&self, key: &PositionKey) -> Option<usize> {
        for i in 0..self.len {
            if self.keys[i] == *key { return Some(i); }
        }
        None
    }

    pub fn get(&self, key: &PositionKey) -> PositionState {
        match self.find(key) {
            Some(i) => self.vals[i],
            None => DEFAULT_POSITION_STATE,
        }
    }

    pub fn update(
        &mut self,
        key: PositionKey,
        liquidity_delta: i128,
        fee_growth_inside_0: u128,
        fee_growth_inside_1: u128,
    ) -> Result<PositionState, ContractError> {
        let mut pos = self.get(&key);

        let owed_0 = tokens_owed(pos.liquidity, fee_growth_inside_0, pos.fee_growth_inside_0_last_q128);
        let owed_1 = tokens_owed(pos.liquidity, fee_growth_inside_1, pos.fee_growth_inside_1_last_q128);

        pos.tokens_owed_0 = pos.tokens_owed_0.saturating_add(owed_0);
        pos.tokens_owed_1 = pos.tokens_owed_1.saturating_add(owed_1);

        pos.liquidity = if liquidity_delta >= 0 {
            pos.liquidity
                .checked_add(liquidity_delta as u128)
                .ok_or(ContractError::MathOverflow)?
        } else {
            pos.liquidity
                .checked_sub((-liquidity_delta) as u128)
                .ok_or(ContractError::InvalidLiquidityDelta)?
        };

        pos.fee_growth_inside_0_last_q128 = fee_growth_inside_0;
        pos.fee_growth_inside_1_last_q128 = fee_growth_inside_1;

        if pos.liquidity == 0 && pos.tokens_owed_0 == 0 && pos.tokens_owed_1 == 0 {
            // Remove position.
            if let Some(i) = self.find(&key) {
                self.len -= 1;
                self.keys[i] = self.keys[self.len];
                self.vals[i] = self.vals[self.len];
            }
        } else {
            match self.find(&key) {
                Some(i) => { self.vals[i] = pos; }
                None => {
                    let i = self.len;
                    self.keys[i] = key;
                    self.vals[i] = pos;
                    self.len += 1;
                }
            }
        }

        Ok(pos)
    }

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
            if let Some(i) = self.find(&key) {
                self.len -= 1;
                self.keys[i] = self.keys[self.len];
                self.vals[i] = self.vals[self.len];
            }
        } else if let Some(i) = self.find(&key) {
            self.vals[i] = pos;
        }

        (collect_0, collect_1)
    }
}

fn tokens_owed(liquidity: u128, fg_now: u128, fg_last: u128) -> u128 {
    let delta = fg_now.wrapping_sub(fg_last);
    mul_shift128(liquidity, delta)
}

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
        let k = key(-100, 100);
        pm.update(k, 1_000, 0, 0).unwrap();
        pm.update(k, 0, 1_000_000, 1_000_000).unwrap();
        let (c0, c1) = pm.collect(k, u64::MAX, u64::MAX);
        let _ = (c0, c1);
    }

    #[test]
    fn mul_shift128_basic() {
        let result = mul_shift128(1u128 << 64, 1u128 << 64);
        assert_eq!(result, 1);
    }
}
