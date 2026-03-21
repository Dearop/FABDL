/// TWAP oracle: circular buffer of price and liquidity observations.
///
/// Mirrors Uniswap v3 §5: each swap writes at most one observation per block,
/// recording (timestamp, tickCumulative, secondsPerLiquidityCumulative).
/// `observe()` returns interpolated values for arbitrary past timestamps.

#[cfg(not(target_arch = "wasm32"))]
use std::vec::Vec;

#[cfg(target_arch = "wasm32")]
extern crate alloc;
#[cfg(target_arch = "wasm32")]
use alloc::vec::Vec;

/// Hard cap on oracle buffer size. Matches Uniswap v3.
pub const MAX_CARDINALITY: u16 = 65_535;

/// A single price/liquidity checkpoint.
#[derive(Clone, Copy, Default)]
pub struct Observation {
    /// Block timestamp (seconds, wraps at 2^32).
    pub timestamp: u32,
    /// Cumulative sum of current_tick × elapsed_seconds up to this point.
    pub tick_cumulative: i64,
    /// Cumulative sum of elapsed_seconds / active_liquidity, in Q128.
    pub seconds_per_liquidity_q128: u128,
    /// Whether this slot has been written.
    pub initialized: bool,
}

/// Circular buffer of oracle observations.
pub struct OracleBuffer {
    /// Ring storage. Length == cardinality_next (capacity).
    pub data: Vec<Observation>,
    /// Index of the most recently written slot.
    pub index: u16,
    /// Number of initialized observations (≤ data.len()).
    pub cardinality: u16,
    /// Requested capacity (the buffer will grow up to this on the next write).
    pub cardinality_next: u16,
}

impl OracleBuffer {
    pub fn new() -> Self {
        let mut data = Vec::with_capacity(1);
        data.push(Observation::default());
        OracleBuffer { data, index: 0, cardinality: 0, cardinality_next: 1 }
    }

    /// Write the first observation at pool initialization.
    pub fn initialize(&mut self, time: u32) {
        self.data[0] = Observation {
            timestamp: time,
            tick_cumulative: 0,
            seconds_per_liquidity_q128: 0,
            initialized: true,
        };
        self.index = 0;
        self.cardinality = 1;
    }

    /// Record a new observation (at most once per block).
    ///
    /// If the last observation has the same timestamp, this is a no-op —
    /// the price will be extended in `observe()` using the live tick.
    ///
    /// Returns the (tick_cumulative, seconds_per_liquidity_q128) at this moment.
    pub fn write(&mut self, time: u32, tick: i32, liquidity: u128) -> (i64, u128) {
        if self.cardinality == 0 {
            return (0, 0);
        }
        let last = self.data[self.index as usize];

        // Same block: don't advance the ring; return the live-extended value.
        if last.initialized && last.timestamp == time {
            return (last.tick_cumulative, last.seconds_per_liquidity_q128);
        }

        // Ensure backing storage covers cardinality_next capacity.
        if self.data.len() < self.cardinality_next as usize {
            self.data.resize(self.cardinality_next as usize, Observation::default());
        }

        // Advance ring pointer within cardinality_next (the max capacity).
        let next_index = (self.index + 1) % self.cardinality_next;

        let delta = time.wrapping_sub(last.timestamp);
        let obs = transform(&last, time, tick, liquidity, delta);

        self.data[next_index as usize] = obs;
        self.index = next_index;

        // Increment cardinality one-at-a-time as slots are actually written.
        // This ensures (index+1) % cardinality always points to the true oldest.
        if next_index >= self.cardinality {
            self.cardinality = next_index + 1;
        }

        (obs.tick_cumulative, obs.seconds_per_liquidity_q128)
    }

    /// Request the buffer to grow to `next` slots on the next `write()`.
    pub fn grow(&mut self, next: u16) {
        let next = next.min(MAX_CARDINALITY);
        if next > self.cardinality_next {
            self.cardinality_next = next;
        }
    }

    /// Return `(tick_cumulative, seconds_per_liquidity_q128)` for each entry
    /// in `seconds_agos`. `time` is the current block timestamp; `tick` and
    /// `liquidity` are the live (post-last-observation) values.
    pub fn observe(
        &self,
        time: u32,
        seconds_agos: &[u32],
        tick: i32,
        liquidity: u128,
    ) -> ObserveResult {
        if self.cardinality == 0 {
            return ObserveResult::Err;
        }
        let mut tick_cumulatives = Vec::with_capacity(seconds_agos.len());
        let mut spls = Vec::with_capacity(seconds_agos.len());

        for &seconds_ago in seconds_agos {
            match observe_single(
                &self.data,
                time,
                seconds_ago,
                tick,
                self.index,
                liquidity,
                self.cardinality,
            ) {
                Some((tc, spl)) => {
                    tick_cumulatives.push(tc);
                    spls.push(spl);
                }
                None => return ObserveResult::Err,
            }
        }
        ObserveResult::Ok { tick_cumulatives, seconds_per_liquidity: spls }
    }
}

pub enum ObserveResult {
    Ok {
        tick_cumulatives: Vec<i64>,
        seconds_per_liquidity: Vec<u128>,
    },
    Err,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Extend an observation forward by `delta` seconds.
pub fn transform(last: &Observation, time: u32, tick: i32, liquidity: u128, delta: u32) -> Observation {
    let tick_cumulative = last.tick_cumulative.wrapping_add(tick as i64 * delta as i64);
    // (delta * 2^128) / liquidity — Uniswap v3 uses uint160 for this.
    // We use a Q64-precision approximation: (delta << 64) / liquidity stored
    // as the high 64 bits of a Q128 value. Deltas are consistent so TWAP
    // range queries remain correct despite the reduced absolute precision.
    let seconds_per_liquidity_q128 = if liquidity > 0 && delta > 0 {
        let d = delta as u128;
        // d < 2^32, so d << 64 < 2^96 — always fits in u128.
        let increment = (d << 64) / liquidity;
        last.seconds_per_liquidity_q128.wrapping_add(increment)
    } else {
        last.seconds_per_liquidity_q128
    };
    Observation { timestamp: time, tick_cumulative, seconds_per_liquidity_q128, initialized: true }
}

fn observe_single(
    observations: &[Observation],
    time: u32,
    seconds_ago: u32,
    tick: i32,
    index: u16,
    liquidity: u128,
    cardinality: u16,
) -> Option<(i64, u128)> {
    let target = time.wrapping_sub(seconds_ago);
    let last = &observations[index as usize];

    // Live state (seconds_ago == 0) or target is at/after last observation.
    if seconds_ago == 0 {
        let delta = time.wrapping_sub(last.timestamp);
        if delta == 0 {
            return Some((last.tick_cumulative, last.seconds_per_liquidity_q128));
        }
        let obs = transform(last, time, tick, liquidity, delta);
        return Some((obs.tick_cumulative, obs.seconds_per_liquidity_q128));
    }

    // target is after the last written observation (but seconds_ago > 0):
    // extend from last up to target.
    if target >= last.timestamp {
        let delta = target.wrapping_sub(last.timestamp);
        let obs = transform(last, target, tick, liquidity, delta);
        return Some((obs.tick_cumulative, obs.seconds_per_liquidity_q128));
    }

    // target is strictly before last — binary search in [oldest, last).
    if cardinality == 1 {
        // Only one observation and target is before it.
        return None;
    }

    let oldest_idx = (index as usize + 1) % cardinality as usize;
    let oldest = &observations[oldest_idx];
    if !oldest.initialized || target < oldest.timestamp {
        return None; // before our oldest record
    }

    observe_in_range(observations, target, index, cardinality)
}

/// Binary search in the circular buffer for `target`.
/// Precondition: oldest.timestamp <= target < last.timestamp.
fn observe_in_range(
    observations: &[Observation],
    target: u32,
    index: u16,
    cardinality: u16,
) -> Option<(i64, u128)> {
    // Map circular positions to linear offsets from oldest.
    // offset 0 = oldest = (index+1) % cardinality
    // offset cardinality-1 = newest = index
    let (mut lo, mut hi) = (0usize, (cardinality - 1) as usize);
    // Binary search: find last offset whose timestamp <= target.
    let mut before_off = 0usize;
    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        let idx = (index as usize + 1 + mid) % cardinality as usize;
        let obs = &observations[idx];
        if obs.initialized && obs.timestamp <= target {
            before_off = mid;
            lo = mid + 1;
        } else {
            if mid == 0 { break; }
            hi = mid - 1;
        }
    }
    let before_idx = (index as usize + 1 + before_off) % cardinality as usize;
    let after_idx = (index as usize + 1 + before_off + 1) % cardinality as usize;
    let before = &observations[before_idx];
    let after = &observations[after_idx];

    if before.timestamp == target {
        return Some((before.tick_cumulative, before.seconds_per_liquidity_q128));
    }
    if !after.initialized {
        return Some((before.tick_cumulative, before.seconds_per_liquidity_q128));
    }

    // Linear interpolation between before and after.
    let dt = after.timestamp.wrapping_sub(before.timestamp) as i64;
    let dt_target = target.wrapping_sub(before.timestamp) as i64;
    if dt == 0 {
        return Some((before.tick_cumulative, before.seconds_per_liquidity_q128));
    }

    let tc = before.tick_cumulative.wrapping_add(
        after.tick_cumulative.wrapping_sub(before.tick_cumulative) * dt_target / dt,
    );
    let spl_delta = after.seconds_per_liquidity_q128.wrapping_sub(before.seconds_per_liquidity_q128);
    let spl = before.seconds_per_liquidity_q128.wrapping_add(
        spl_delta * dt_target as u128 / dt as u128,
    );
    Some((tc, spl))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn buf_with(timestamps: &[(u32, i32, u128)]) -> OracleBuffer {
        let mut b = OracleBuffer::new();
        b.grow(timestamps.len() as u16 + 1);
        if let Some(&(t0, _, _)) = timestamps.first() {
            b.initialize(t0);
            for &(t, tick, liq) in &timestamps[1..] {
                b.write(t, tick, liq);
            }
        }
        b
    }

    #[test]
    fn initialize_writes_first_slot() {
        let mut b = OracleBuffer::new();
        b.initialize(1000);
        assert_eq!(b.cardinality, 1);
        assert!(b.data[0].initialized);
        assert_eq!(b.data[0].timestamp, 1000);
    }

    #[test]
    fn write_advances_index() {
        let b = buf_with(&[(1000, 100, 1_000_000), (1060, 100, 1_000_000)]);
        // tick_cumulative at t=1060: 100 * 60 = 6000
        assert_eq!(b.data[b.index as usize].tick_cumulative, 6000);
    }

    #[test]
    fn observe_now_seconds_ago_zero() {
        let b = buf_with(&[(1000, 100, 1_000_000), (1060, 100, 1_000_000)]);
        match b.observe(1060, &[0], 100, 1_000_000) {
            ObserveResult::Ok { tick_cumulatives, .. } => {
                assert_eq!(tick_cumulatives[0], 6000);
            }
            ObserveResult::Err => panic!("observe failed"),
        }
    }

    #[test]
    fn observe_past_exact_match() {
        let b = buf_with(&[
            (1000, 100, 1_000_000),
            (1060, 100, 1_000_000),
            (1120, 100, 1_000_000),
        ]);
        // At t=1060 tick_cumulative = 6000
        match b.observe(1120, &[60], 100, 1_000_000) {
            ObserveResult::Ok { tick_cumulatives, .. } => {
                assert_eq!(tick_cumulatives[0], 6000);
            }
            ObserveResult::Err => panic!("observe failed"),
        }
    }

    #[test]
    fn observe_interpolates_between_checkpoints() {
        let b = buf_with(&[
            (1000, 100, 1_000_000),
            (1060, 100, 1_000_000), // tc = 6000
            (1120, 100, 1_000_000), // tc = 12000
        ]);
        // 30s before now (t=1090) should give tc = 6000 + 100*30 = 9000
        match b.observe(1120, &[30], 100, 1_000_000) {
            ObserveResult::Ok { tick_cumulatives, .. } => {
                assert_eq!(tick_cumulatives[0], 9000);
            }
            ObserveResult::Err => panic!("observe failed"),
        }
    }

    #[test]
    fn grow_increases_cardinality_next() {
        let mut b = OracleBuffer::new();
        b.grow(100);
        assert_eq!(b.cardinality_next, 100);
    }

    #[test]
    fn same_timestamp_is_idempotent() {
        let mut b = buf_with(&[(1000, 100, 1_000_000)]);
        let before_idx = b.index;
        b.write(1000, 200, 1_000_000); // same timestamp
        assert_eq!(b.index, before_idx); // index unchanged
    }
}
