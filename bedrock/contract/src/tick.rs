/// Per-tick state and update logic.

use crate::types::ContractError;

const MAX_TICKS: usize = 64;

const DEFAULT_TICK_STATE: TickState = TickState {
    liquidity_gross: 0,
    liquidity_net: 0,
    fee_growth_outside_0_q128: 0,
    fee_growth_outside_1_q128: 0,
    seconds_outside: 0,
    tick_cumulative_outside: 0,
    seconds_per_liquidity_outside_q128: 0,
};

#[derive(Clone, Copy, Default)]
pub struct TickState {
    pub liquidity_gross: u128,
    pub liquidity_net: i128,
    pub fee_growth_outside_0_q128: u128,
    pub fee_growth_outside_1_q128: u128,
    pub seconds_outside: u64,
    pub tick_cumulative_outside: i64,
    pub seconds_per_liquidity_outside_q128: u128,
}

pub struct TickMap {
    pub keys: [i32; MAX_TICKS],
    pub vals: [TickState; MAX_TICKS],
    pub len: usize,
}

impl TickMap {
    pub fn new() -> Self {
        Self {
            keys: [0; MAX_TICKS],
            vals: [DEFAULT_TICK_STATE; MAX_TICKS],
            len: 0,
        }
    }

    fn find(&self, tick: i32) -> Option<usize> {
        for i in 0..self.len {
            if self.keys[i] == tick { return Some(i); }
        }
        None
    }

    pub fn get(&self, tick: i32) -> TickState {
        match self.find(tick) {
            Some(i) => self.vals[i],
            None => DEFAULT_TICK_STATE,
        }
    }

    pub fn set(&mut self, tick: i32, state: TickState) {
        if state.liquidity_gross == 0 && state.fee_growth_outside_0_q128 == 0 {
            // Remove entry if it exists.
            if let Some(i) = self.find(tick) {
                self.len -= 1;
                self.keys[i] = self.keys[self.len];
                self.vals[i] = self.vals[self.len];
            }
        } else {
            match self.find(tick) {
                Some(i) => { self.vals[i] = state; }
                None => {
                    let i = self.len;
                    self.keys[i] = tick;
                    self.vals[i] = state;
                    self.len += 1;
                }
            }
        }
    }

    pub fn update(
        &mut self,
        tick: i32,
        current_tick: i32,
        liquidity_delta: i128,
        fee_growth_global_0: u128,
        fee_growth_global_1: u128,
        upper: bool,
    ) -> Result<(u128, bool), ContractError> {
        let mut t = self.get(tick);
        let liquidity_gross_before = t.liquidity_gross;

        let liquidity_gross_after = if liquidity_delta >= 0 {
            t.liquidity_gross
                .checked_add(liquidity_delta as u128)
                .ok_or(ContractError::MathOverflow)?
        } else {
            t.liquidity_gross
                .checked_sub((-liquidity_delta) as u128)
                .ok_or(ContractError::InvalidLiquidityDelta)?
        };

        let flipped = (liquidity_gross_after == 0) != (liquidity_gross_before == 0);

        if liquidity_gross_before == 0 {
            if tick <= current_tick {
                t.fee_growth_outside_0_q128 = fee_growth_global_0;
                t.fee_growth_outside_1_q128 = fee_growth_global_1;
            }
        }

        t.liquidity_gross = liquidity_gross_after;

        if upper {
            t.liquidity_net = t
                .liquidity_net
                .checked_sub(liquidity_delta)
                .ok_or(ContractError::MathOverflow)?;
        } else {
            t.liquidity_net = t
                .liquidity_net
                .checked_add(liquidity_delta)
                .ok_or(ContractError::MathOverflow)?;
        }

        self.set(tick, t);
        Ok((liquidity_gross_after, flipped))
    }

    pub fn cross(
        &mut self,
        tick: i32,
        fee_growth_global_0: u128,
        fee_growth_global_1: u128,
        tick_cumulative: i64,
        seconds_per_liquidity_q128: u128,
        time: u32,
    ) -> i128 {
        let mut t = self.get(tick);
        t.fee_growth_outside_0_q128 =
            fee_growth_global_0.wrapping_sub(t.fee_growth_outside_0_q128);
        t.fee_growth_outside_1_q128 =
            fee_growth_global_1.wrapping_sub(t.fee_growth_outside_1_q128);
        t.tick_cumulative_outside =
            tick_cumulative.wrapping_sub(t.tick_cumulative_outside);
        t.seconds_per_liquidity_outside_q128 =
            seconds_per_liquidity_q128.wrapping_sub(t.seconds_per_liquidity_outside_q128);
        t.seconds_outside = time.wrapping_sub(t.seconds_outside as u32) as u64;
        self.set(tick, t);
        t.liquidity_net
    }

    pub fn fee_growth_inside(
        &self,
        lower_tick: i32,
        upper_tick: i32,
        current_tick: i32,
        fee_growth_global_0: u128,
        fee_growth_global_1: u128,
    ) -> (u128, u128) {
        let lower = self.get(lower_tick);
        let upper = self.get(upper_tick);

        let (fgb0, fgb1) = if current_tick >= lower_tick {
            (lower.fee_growth_outside_0_q128, lower.fee_growth_outside_1_q128)
        } else {
            (
                fee_growth_global_0.wrapping_sub(lower.fee_growth_outside_0_q128),
                fee_growth_global_1.wrapping_sub(lower.fee_growth_outside_1_q128),
            )
        };

        let (fga0, fga1) = if current_tick < upper_tick {
            (upper.fee_growth_outside_0_q128, upper.fee_growth_outside_1_q128)
        } else {
            (
                fee_growth_global_0.wrapping_sub(upper.fee_growth_outside_0_q128),
                fee_growth_global_1.wrapping_sub(upper.fee_growth_outside_1_q128),
            )
        };

        (
            fee_growth_global_0.wrapping_sub(fgb0).wrapping_sub(fga0),
            fee_growth_global_1.wrapping_sub(fgb1).wrapping_sub(fga1),
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_update_increases_gross() {
        let mut tm = TickMap::new();
        let (gross, flipped) = tm.update(100, 0, 500, 0, 0, false).unwrap();
        assert_eq!(gross, 500);
        assert!(flipped);
    }

    #[test]
    fn tick_update_to_zero_flips_back() {
        let mut tm = TickMap::new();
        tm.update(100, 0, 500, 0, 0, false).unwrap();
        let (gross, flipped) = tm.update(100, 0, -500, 0, 0, false).unwrap();
        assert_eq!(gross, 0);
        assert!(flipped);
    }

    #[test]
    fn cross_flips_outside_accumulators() {
        let mut tm = TickMap::new();
        tm.update(100, 50, 1000, 500, 300, false).unwrap();
        let lnet = tm.cross(100, 500, 300, 0, 0, 0);
        assert_eq!(lnet, 1000);
        let t = tm.get(100);
        assert_eq!(t.fee_growth_outside_0_q128, 500);
    }

    #[test]
    fn fee_growth_inside_in_range() {
        let mut tm = TickMap::new();
        tm.update(-100, 0, 1000, 0, 0, false).unwrap();
        tm.update(100, 0, 1000, 0, 0, true).unwrap();
        let (fg0, fg1) = tm.fee_growth_inside(-100, 100, 0, 1000, 1000);
        let _ = (fg0, fg1);
    }
}
