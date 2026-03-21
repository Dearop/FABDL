/// Sparse 256-bit word tick bitmap.
///
/// Tick space is partitioned into "words" of 256 bits.
/// word_pos  = tick / 256  (signed integer)
/// bit_pos   = (tick % 256 + 256) % 256  (always 0..255)
///
/// On native (std) builds we use a BTreeMap<i16, u256_emulated>.
/// On WASM builds the host storage API replaces the map.

#[cfg(not(target_arch = "wasm32"))]
use std::collections::BTreeMap;

/// A single 256-bit word stored as four u64 limbs, little-endian.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Word256([u64; 4]);

impl Word256 {
    pub fn set_bit(&mut self, bit: u8) {
        let limb = (bit / 64) as usize;
        self.0[limb] |= 1u64 << (bit % 64);
    }

    pub fn clear_bit(&mut self, bit: u8) {
        let limb = (bit / 64) as usize;
        self.0[limb] &= !(1u64 << (bit % 64));
    }

    pub fn is_set(&self, bit: u8) -> bool {
        let limb = (bit / 64) as usize;
        self.0[limb] & (1u64 << (bit % 64)) != 0
    }

    /// Scan for next set bit at or above `start_bit` (searching upward).
    /// Returns Some(bit) or None if none set in remaining positions.
    pub fn next_initialized_above(&self, start_bit: u8) -> Option<u8> {
        for bit in start_bit..=255 {
            if self.is_set(bit) {
                return Some(bit);
            }
        }
        None
    }

    /// Scan for next set bit at or below `start_bit` (searching downward).
    pub fn next_initialized_below(&self, start_bit: u8) -> Option<u8> {
        let mut bit = start_bit;
        loop {
            if self.is_set(bit) {
                return Some(bit);
            }
            if bit == 0 {
                return None;
            }
            bit -= 1;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0 == [0u64; 4]
    }
}

// ---------------------------------------------------------------------------
// TickBitmap
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
pub struct TickBitmap {
    words: BTreeMap<i16, Word256>,
}

#[cfg(not(target_arch = "wasm32"))]
impl TickBitmap {
    pub fn new() -> Self {
        Self {
            words: BTreeMap::new(),
        }
    }

    fn word_and_bit(tick: i32, tick_spacing: i32) -> (i16, u8) {
        // Only aligned ticks are addressable.
        debug_assert_eq!(tick % tick_spacing, 0);
        let compressed = tick / tick_spacing;
        let word_pos = (compressed >> 8) as i16;
        let bit_pos = ((compressed % 256 + 256) % 256) as u8;
        (word_pos, bit_pos)
    }

    /// Flip the initialized state of a tick (toggle).
    pub fn flip_tick(&mut self, tick: i32, tick_spacing: i32) {
        let (word_pos, bit_pos) = Self::word_and_bit(tick, tick_spacing);
        let word = self.words.entry(word_pos).or_default();
        if word.is_set(bit_pos) {
            word.clear_bit(bit_pos);
        } else {
            word.set_bit(bit_pos);
        }
        // Remove empty words to keep map sparse.
        if self.words.get(&word_pos).map_or(false, |w| w.is_empty()) {
            self.words.remove(&word_pos);
        }
    }

    /// Find the next initialized tick in the swap direction.
    ///
    /// `lte = true`  → search downward (zero_for_one swap)
    /// `lte = false` → search upward   (one_for_zero swap)
    ///
    /// Returns (tick, initialized) where initialized = true if an initialized
    /// tick was found, false if we hit a word boundary with no initialized tick.
    pub fn next_initialized_tick_within_one_word(
        &self,
        tick: i32,
        tick_spacing: i32,
        lte: bool,
    ) -> (i32, bool) {
        let compressed = tick / tick_spacing;
        if lte {
            let (word_pos, bit_pos) = {
                let w = (compressed >> 8) as i16;
                let b = ((compressed % 256 + 256) % 256) as u8;
                (w, b)
            };
            if let Some(word) = self.words.get(&word_pos) {
                if let Some(found_bit) = word.next_initialized_below(bit_pos) {
                    let found_compressed =
                        (word_pos as i32) * 256 + found_bit as i32;
                    return (found_compressed * tick_spacing, true);
                }
            }
            // No initialized tick in this word — return word boundary.
            let boundary_compressed = (word_pos as i32) * 256;
            (boundary_compressed * tick_spacing, false)
        } else {
            let compressed_next = compressed + 1;
            let word_pos = (compressed_next >> 8) as i16;
            let bit_pos = ((compressed_next % 256 + 256) % 256) as u8;
            if let Some(word) = self.words.get(&word_pos) {
                if let Some(found_bit) = word.next_initialized_above(bit_pos) {
                    let found_compressed =
                        (word_pos as i32) * 256 + found_bit as i32;
                    return (found_compressed * tick_spacing, true);
                }
            }
            let boundary_compressed = (word_pos as i32 + 1) * 256 - 1;
            (boundary_compressed * tick_spacing, false)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flip_and_find() {
        let mut bm = TickBitmap::new();
        bm.flip_tick(100, 10);
        bm.flip_tick(200, 10);

        let (tick, init) = bm.next_initialized_tick_within_one_word(250, 10, true);
        assert!(init);
        assert_eq!(tick, 200);
    }

    #[test]
    fn flip_twice_removes() {
        let mut bm = TickBitmap::new();
        bm.flip_tick(100, 10);
        bm.flip_tick(100, 10); // toggle off
        let (_, init) = bm.next_initialized_tick_within_one_word(200, 10, true);
        assert!(!init, "tick should have been removed after double flip");
    }

    #[test]
    fn search_upward() {
        let mut bm = TickBitmap::new();
        bm.flip_tick(300, 10);
        let (tick, init) = bm.next_initialized_tick_within_one_word(100, 10, false);
        assert!(init);
        assert_eq!(tick, 300);
    }

    #[test]
    fn word256_bit_ops() {
        let mut w = Word256::default();
        w.set_bit(0);
        w.set_bit(63);
        w.set_bit(255);
        assert!(w.is_set(0));
        assert!(w.is_set(63));
        assert!(w.is_set(255));
        assert!(!w.is_set(1));
        w.clear_bit(63);
        assert!(!w.is_set(63));
    }
}
