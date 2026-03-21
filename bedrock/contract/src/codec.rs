/// Binary serialization for ContractState persistence.
///
/// Used by `save_state_to_host()` / `load_state_from_host()` to persist the
/// entire pool state as a single blob in Bedrock host storage.
///
/// Encoding is little-endian, no padding. All maps are prefixed with a u32
/// entry count followed by (key, value) pairs in BTreeMap iteration order.

#[cfg(not(target_arch = "wasm32"))]
use std::vec::Vec;

#[cfg(target_arch = "wasm32")]
extern crate alloc;
#[cfg(target_arch = "wasm32")]
use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// Low-level byte writer / reader
// ---------------------------------------------------------------------------

pub struct ByteWriter {
    pub buf: Vec<u8>,
}

impl ByteWriter {
    pub fn new() -> Self { ByteWriter { buf: Vec::new() } }

    pub fn u8(&mut self, v: u8) { self.buf.push(v); }
    pub fn u16(&mut self, v: u16) { self.buf.extend_from_slice(&v.to_le_bytes()); }
    pub fn u32(&mut self, v: u32) { self.buf.extend_from_slice(&v.to_le_bytes()); }
    pub fn i32(&mut self, v: i32) { self.buf.extend_from_slice(&v.to_le_bytes()); }
    pub fn i16(&mut self, v: i16) { self.buf.extend_from_slice(&v.to_le_bytes()); }
    pub fn u64(&mut self, v: u64) { self.buf.extend_from_slice(&v.to_le_bytes()); }
    pub fn i64(&mut self, v: i64) { self.buf.extend_from_slice(&v.to_le_bytes()); }
    pub fn u128(&mut self, v: u128) { self.buf.extend_from_slice(&v.to_le_bytes()); }
    pub fn i128(&mut self, v: i128) { self.buf.extend_from_slice(&v.to_le_bytes()); }
    pub fn bytes20(&mut self, v: &[u8; 20]) { self.buf.extend_from_slice(v); }
}

pub struct ByteReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ByteReader<'a> {
    pub fn new(data: &'a [u8]) -> Self { ByteReader { data, pos: 0 } }

    fn take(&mut self, n: usize) -> Option<&[u8]> {
        if self.pos + n > self.data.len() { return None; }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Some(s)
    }

    pub fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    pub fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    pub fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    pub fn i16(&mut self) -> Option<i16> {
        Some(i16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    pub fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    pub fn i64(&mut self) -> Option<i64> {
        Some(i64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    pub fn u128(&mut self) -> Option<u128> {
        Some(u128::from_le_bytes(self.take(16)?.try_into().ok()?))
    }
    pub fn i128(&mut self) -> Option<i128> {
        Some(i128::from_le_bytes(self.take(16)?.try_into().ok()?))
    }
    pub fn bytes20(&mut self) -> Option<[u8; 20]> {
        let s = self.take(20)?;
        let mut arr = [0u8; 20];
        arr.copy_from_slice(s);
        Some(arr)
    }
}

// ---------------------------------------------------------------------------
// Per-struct encode / decode
// ---------------------------------------------------------------------------

use crate::position::PositionState;
use crate::tick::TickState;
use crate::tick_bitmap::Word256;

// --- PoolState fields ---
pub(crate) fn encode_pool(w: &mut ByteWriter, s: &super::PoolState) {
    w.u128(s.sqrt_price_q64_64);
    w.i32(s.current_tick);
    w.u128(s.liquidity_active);
    w.u16(s.fee_bps);
    w.u16(s.protocol_fee_share_bps);
    w.u128(s.fee_growth_global_0_q128);
    w.u128(s.fee_growth_global_1_q128);
    w.u128(s.protocol_fees_0);
    w.u128(s.protocol_fees_1);
    w.u8(s.initialized as u8);
}

pub(crate) fn decode_pool(r: &mut ByteReader) -> Option<super::PoolState> {
    Some(super::PoolState {
        sqrt_price_q64_64: r.u128()?,
        current_tick: r.i32()?,
        liquidity_active: r.u128()?,
        fee_bps: r.u16()?,
        protocol_fee_share_bps: r.u16()?,
        fee_growth_global_0_q128: r.u128()?,
        fee_growth_global_1_q128: r.u128()?,
        protocol_fees_0: r.u128()?,
        protocol_fees_1: r.u128()?,
        initialized: r.u8()? != 0,
    })
}

// --- ContractConfig (47 bytes: 20 owner + 20 manager + 1 paused + 2 slippage + 4 spacing) ---
pub(crate) fn encode_config(w: &mut ByteWriter, c: &super::ContractConfig) {
    w.bytes20(&c.owner);
    w.bytes20(&c.manager);
    w.u8(c.paused as u8);
    w.u16(c.max_slippage_bps);
    w.i32(c.tick_spacing);
}

pub(crate) fn decode_config(r: &mut ByteReader) -> Option<super::ContractConfig> {
    Some(super::ContractConfig {
        owner: r.bytes20()?,
        manager: r.bytes20()?,
        paused: r.u8()? != 0,
        max_slippage_bps: r.u16()?,
        tick_spacing: r.i32()?,
    })
}

// --- TickState (96 bytes) ---
pub fn encode_tick_state(w: &mut ByteWriter, t: &TickState) {
    w.u128(t.liquidity_gross);
    w.i128(t.liquidity_net);
    w.u128(t.fee_growth_outside_0_q128);
    w.u128(t.fee_growth_outside_1_q128);
    w.u64(t.seconds_outside);
    w.i64(t.tick_cumulative_outside);
    w.u128(t.seconds_per_liquidity_outside_q128);
}

pub fn decode_tick_state(r: &mut ByteReader) -> Option<TickState> {
    Some(TickState {
        liquidity_gross: r.u128()?,
        liquidity_net: r.i128()?,
        fee_growth_outside_0_q128: r.u128()?,
        fee_growth_outside_1_q128: r.u128()?,
        seconds_outside: r.u64()?,
        tick_cumulative_outside: r.i64()?,
        seconds_per_liquidity_outside_q128: r.u128()?,
    })
}

// --- TickMap (4 + n * 100 bytes) ---
pub(crate) fn encode_ticks(w: &mut ByteWriter, m: &crate::tick::TickMap) {
    w.u32(m.len as u32);
    for i in 0..m.len {
        w.i32(m.keys[i]);
        encode_tick_state(w, &m.vals[i]);
    }
}

pub(crate) fn decode_ticks(r: &mut ByteReader) -> Option<crate::tick::TickMap> {
    let count = r.u32()? as usize;
    let mut map = crate::tick::TickMap::new();
    for _ in 0..count {
        let k = r.i32()?;
        let v = decode_tick_state(r)?;
        map.set(k, v);
    }
    Some(map)
}

// --- TickBitmap (4 + n * 34 bytes) ---
pub(crate) fn encode_bitmap(w: &mut ByteWriter, b: &crate::tick_bitmap::TickBitmap) {
    w.u32(b.len as u32);
    for i in 0..b.len {
        w.i16(b.word_keys[i]);
        for limb in &b.word_vals[i].0 {
            w.u64(*limb);
        }
    }
}

pub(crate) fn decode_bitmap(r: &mut ByteReader) -> Option<crate::tick_bitmap::TickBitmap> {
    let count = r.u32()? as usize;
    let mut bm = crate::tick_bitmap::TickBitmap::new();
    for _ in 0..count {
        let k = r.i16()?;
        let mut limbs = [0u64; 4];
        for limb in &mut limbs {
            *limb = r.u64()?;
        }
        let word = Word256(limbs);
        if !word.is_empty() {
            let i = bm.len;
            bm.word_keys[i] = k;
            bm.word_vals[i] = word;
            bm.len += 1;
        }
    }
    Some(bm)
}

// --- PositionState (80 bytes) ---
fn encode_pos_state(w: &mut ByteWriter, p: &PositionState) {
    w.u128(p.liquidity);
    w.u128(p.fee_growth_inside_0_last_q128);
    w.u128(p.fee_growth_inside_1_last_q128);
    w.u128(p.tokens_owed_0);
    w.u128(p.tokens_owed_1);
}

fn decode_pos_state(r: &mut ByteReader) -> Option<PositionState> {
    Some(PositionState {
        liquidity: r.u128()?,
        fee_growth_inside_0_last_q128: r.u128()?,
        fee_growth_inside_1_last_q128: r.u128()?,
        tokens_owed_0: r.u128()?,
        tokens_owed_1: r.u128()?,
    })
}

// --- PositionMap (4 + n * 108 bytes) ---
pub(crate) fn encode_positions(w: &mut ByteWriter, m: &crate::position::PositionMap) {
    w.u32(m.len as u32);
    for i in 0..m.len {
        w.bytes20(&m.keys[i].owner);
        w.i32(m.keys[i].lower_tick);
        w.i32(m.keys[i].upper_tick);
        encode_pos_state(w, &m.vals[i]);
    }
}

pub(crate) fn decode_positions(r: &mut ByteReader) -> Option<crate::position::PositionMap> {
    let count = r.u32()? as usize;
    let mut pm = crate::position::PositionMap::new();
    for _ in 0..count {
        let key = crate::position::PositionKey {
            owner: r.bytes20()?,
            lower_tick: r.i32()?,
            upper_tick: r.i32()?,
        };
        let val = decode_pos_state(r)?;
        let i = pm.len;
        pm.keys[i] = key;
        pm.vals[i] = val;
        pm.len += 1;
    }
    Some(pm)
}

// ---------------------------------------------------------------------------
// Top-level encode / decode for the whole ContractState
// ---------------------------------------------------------------------------

pub(crate) fn encode_state(state: &super::ContractState) -> Vec<u8> {
    let mut w = ByteWriter::new();
    encode_pool(&mut w, &state.pool);
    encode_config(&mut w, &state.config);
    encode_ticks(&mut w, &state.ticks);
    encode_bitmap(&mut w, &state.bitmap);
    encode_positions(&mut w, &state.positions);
    w.buf
}

pub(crate) fn decode_state(data: &[u8]) -> Option<super::ContractState> {
    let mut r = ByteReader::new(data);
    let pool = decode_pool(&mut r)?;
    let config = decode_config(&mut r)?;
    let ticks = decode_ticks(&mut r)?;
    let bitmap = decode_bitmap(&mut r)?;
    let positions = decode_positions(&mut r)?;
    Some(super::ContractState { pool, config, ticks, bitmap, positions })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContractState;

    #[test]
    fn roundtrip_empty_state() {
        let state = ContractState::new();
        let bytes = encode_state(&state);
        let decoded = decode_state(&bytes).expect("decode failed");
        assert_eq!(decoded.pool.initialized, false);
        assert_eq!(decoded.config.tick_spacing, 10);
    }

    #[test]
    fn roundtrip_preserves_pool_fields() {
        let mut state = ContractState::new();
        state.pool.sqrt_price_q64_64 = 12345678901234567890u128;
        state.pool.current_tick = -500;
        state.pool.fee_bps = 100;
        state.pool.initialized = true;
        let bytes = encode_state(&state);
        let decoded = decode_state(&bytes).expect("decode failed");
        assert_eq!(decoded.pool.sqrt_price_q64_64, 12345678901234567890u128);
        assert_eq!(decoded.pool.current_tick, -500);
        assert_eq!(decoded.pool.fee_bps, 100);
        assert!(decoded.pool.initialized);
    }

    #[test]
    fn roundtrip_with_ticks() {
        use crate::tick::TickState;
        let mut state = ContractState::new();
        state.ticks.set(100, TickState {
            liquidity_gross: 999,
            liquidity_net: -999,
            fee_growth_outside_0_q128: 42,
            fee_growth_outside_1_q128: 43,
            seconds_outside: 60,
            tick_cumulative_outside: -7,
            seconds_per_liquidity_outside_q128: 100,
        });
        let bytes = encode_state(&state);
        let decoded = decode_state(&bytes).expect("decode failed");
        let t = decoded.ticks.get(100);
        assert_eq!(t.liquidity_gross, 999);
        assert_eq!(t.tick_cumulative_outside, -7);
    }
}
