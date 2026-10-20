//! Verified fixed-size bitmap.
//!
//! A compact bit set backed by an array of u64 words. Each bit can be
//! read, set, cleared, or toggled with machine-checked contracts. Uses
//! `bitwise::bit_at`, `set_bit`, `clear_bit`, `toggle_bit` for
//! per-word operations and `arithmetic::div_ceil` for word count.

#![allow(clippy::cast_lossless, clippy::len_without_is_empty)]

use vstd::prelude::*;

use crate::bitwise::bit_at;
#[cfg(verus_keep_ghost)]
use crate::bitwise::bit_at_spec;
use crate::bitwise::clear_bit;
#[cfg(verus_keep_ghost)]
use crate::bitwise::clear_bit_spec;
use crate::bitwise::set_bit;
#[cfg(verus_keep_ghost)]
use crate::bitwise::set_bit_spec;
use crate::bitwise::toggle_bit;
#[cfg(verus_keep_ghost)]
use crate::bitwise::toggle_bit_spec;

verus! {

/// A fixed-size bitmap backed by `Vec<u64>`.
pub struct Bitmap {
    /// Backing storage — one bit per logical position.
    pub words: Vec<u64>,
    /// Total number of bits.
    pub num_bits: usize,
}

impl Bitmap {

    /// Well-formedness invariant.
    pub open spec fn wf(&self) -> bool {
        &&& self.num_bits > 0
        &&& self.words@.len() as int == (self.num_bits as int - 1) / 64 + 1
    }

    /// Ghost view: bit value at position `pos`.
    pub open spec fn get_spec(&self, pos: int) -> bool
        recommends 0 <= pos < self.num_bits as int
    {
        bit_at_spec(self.words@[pos / 64], (pos % 64) as u64)
    }

    // r[impl bitmap.new]
    /// Creates a bitmap with `num_bits` bits, all initially clear.
    pub fn new(num_bits: usize) -> (result: Bitmap)
        requires num_bits > 0
        ensures
            result.wf(),
            result.num_bits == num_bits,
            forall|j: int| 0 <= j < result.words@.len() ==> result.words@[j] == 0u64,
    {
        let n_words = (num_bits - 1) / 64 + 1;
        let mut words = Vec::new();
        let mut i: usize = 0;
        while i < n_words
            invariant
                0 <= i <= n_words,
                words@.len() == i as int,
                n_words as int == (num_bits as int - 1) / 64 + 1,
                forall|j: int| 0 <= j < i as int ==> words@[j] == 0u64,
            decreases n_words - i
        {
            words.push(0);
            i = i + 1;
        }
        Bitmap { words, num_bits }
    }

    // r[impl bitmap.get]
    /// Returns the bit value at position `pos`.
    pub fn get(&self, pos: usize) -> (result: bool)
        requires self.wf(), pos < self.num_bits
        ensures result == self.get_spec(pos as int)
    {
        let word_idx = pos / 64;
        let bit_pos = (pos % 64) as u64;
        bit_at(self.words[word_idx], bit_pos)
    }

    // r[impl bitmap.set]
    /// Sets the bit at position `pos` to 1.
    pub fn set(&mut self, pos: usize)
        requires old(self).wf(), pos < old(self).num_bits
        ensures
            self.wf(),
            self.num_bits == old(self).num_bits,
            self.get_spec(pos as int),
            forall|i: int| 0 <= i < self.words@.len() && i != pos as int / 64
                ==> self.words@[i] == old(self).words@[i],
    {
        let word_idx = pos / 64;
        let bit_pos = (pos % 64) as u64;
        let old_word = self.words[word_idx];
        let new_word = set_bit(old_word, bit_pos);
        self.words.set(word_idx, new_word);
        proof {
            // set_bit makes the target bit read as 1
            assert(bit_pos < 64 ==>
                ((old_word | (1u64 << bit_pos)) >> bit_pos) & 1u64 == 1u64
            ) by(bit_vector);
        }
    }

    // r[impl bitmap.clear]
    /// Sets the bit at position `pos` to 0.
    pub fn clear(&mut self, pos: usize)
        requires old(self).wf(), pos < old(self).num_bits
        ensures
            self.wf(),
            self.num_bits == old(self).num_bits,
            !self.get_spec(pos as int),
            forall|i: int| 0 <= i < self.words@.len() && i != pos as int / 64
                ==> self.words@[i] == old(self).words@[i],
    {
        let word_idx = pos / 64;
        let bit_pos = (pos % 64) as u64;
        let old_word = self.words[word_idx];
        let new_word = clear_bit(old_word, bit_pos);
        self.words.set(word_idx, new_word);
        proof {
            // clear_bit makes the target bit read as 0
            assert(bit_pos < 64 ==>
                ((old_word & !(1u64 << bit_pos)) >> bit_pos) & 1u64 == 0u64
            ) by(bit_vector);
        }
    }

    // r[impl bitmap.toggle]
    /// Flips the bit at position `pos`.
    pub fn toggle(&mut self, pos: usize)
        requires old(self).wf(), pos < old(self).num_bits
        ensures
            self.wf(),
            self.num_bits == old(self).num_bits,
            old(self).get_spec(pos as int) ==> !self.get_spec(pos as int),
            !old(self).get_spec(pos as int) ==> self.get_spec(pos as int),
            forall|i: int| 0 <= i < self.words@.len() && i != pos as int / 64
                ==> self.words@[i] == old(self).words@[i],
    {
        let word_idx = pos / 64;
        let bit_pos = (pos % 64) as u64;
        let old_word = self.words[word_idx];
        let new_word = toggle_bit(old_word, bit_pos);
        self.words.set(word_idx, new_word);
        proof {
            // Bridge exec variables to spec-level expressions
            assert(word_idx as int == pos as int / 64);
            assert(bit_pos == (pos as int % 64) as u64);
            assert(self.get_spec(pos as int) == bit_at_spec(new_word, bit_pos));
            assert(old(self).get_spec(pos as int) == bit_at_spec(old_word, bit_pos));

            // Z3 default theory doesn't know x & 1 ∈ {0, 1} — tell it
            assert((old_word >> bit_pos) & 1u64 == 0u64
                || (old_word >> bit_pos) & 1u64 == 1u64
            ) by(bit_vector);

            // toggle flips the bit: was-0 → 1, was-1 → 0
            assert(bit_pos < 64 && (old_word >> bit_pos) & 1u64 == 0u64 ==>
                ((old_word ^ (1u64 << bit_pos)) >> bit_pos) & 1u64 == 1u64
            ) by(bit_vector);
            assert(bit_pos < 64 && (old_word >> bit_pos) & 1u64 == 1u64 ==>
                ((old_word ^ (1u64 << bit_pos)) >> bit_pos) & 1u64 == 0u64
            ) by(bit_vector);
        }
    }

    // r[impl bitmap.len]
    /// Returns the number of bits in the bitmap.
    pub fn len(&self) -> (result: usize)
        requires self.wf()
        ensures result == self.num_bits
    {
        self.num_bits
    }

    // r[impl bitmap.word-count]
    /// Returns the number of backing u64 words.
    pub fn word_count(&self) -> (result: usize)
        requires self.wf()
        ensures result == self.words@.len()
    {
        self.words.len()
    }

}

// --- Proofs ---

// r[verify bitmap.set]
// r[verify bitmap.clear]
/// Setting then clearing the same bit leaves it cleared.
proof fn set_clear_roundtrip(bm: Bitmap, pos: int)
    requires bm.wf(), 0 <= pos < bm.num_bits as int
    ensures
        clear_bit_spec(set_bit_spec(bm.words@[pos / 64], (pos % 64) as u64), (pos % 64) as u64)
        == clear_bit_spec(bm.words@[pos / 64], (pos % 64) as u64)
{
    let p = (pos % 64) as u64;
    let w = bm.words@[pos / 64];
    assert(p < 64 ==> (w | (1u64 << p)) & !(1u64 << p) == w & !(1u64 << p)) by(bit_vector);
}

// r[verify bitmap.toggle]
/// Toggling twice restores the original value.
proof fn toggle_involution(bm: Bitmap, pos: int)
    requires bm.wf(), 0 <= pos < bm.num_bits as int
    ensures
        toggle_bit_spec(
            toggle_bit_spec(bm.words@[pos / 64], (pos % 64) as u64),
            (pos % 64) as u64,
        ) == bm.words@[pos / 64]
{
    let p = (pos % 64) as u64;
    let w = bm.words@[pos / 64];
    assert(p < 64 ==> (w ^ (1u64 << p)) ^ (1u64 << p) == w) by(bit_vector);
}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify bitmap.new]
    #[test]
    fn test_new() {
        let bm = Bitmap::new(100);
        assert_eq!(bm.len(), 100);
        assert_eq!(bm.word_count(), 2); // ceil(100/64) = 2
        for i in 0..100_usize {
            assert!(!bm.get(i));
        }
    }

    // r[verify bitmap.new]
    #[test]
    fn test_new_exact() {
        let bm = Bitmap::new(64);
        assert_eq!(bm.word_count(), 1);
        let bm = Bitmap::new(65);
        assert_eq!(bm.word_count(), 2);
        let bm = Bitmap::new(1);
        assert_eq!(bm.word_count(), 1);
    }

    // r[verify bitmap.set]
    // r[verify bitmap.get]
    #[test]
    fn test_set_get() {
        let mut bm = Bitmap::new(128);
        bm.set(0);
        bm.set(63);
        bm.set(64);
        bm.set(127);
        assert!(bm.get(0));
        assert!(bm.get(63));
        assert!(bm.get(64));
        assert!(bm.get(127));
        assert!(!bm.get(1));
        assert!(!bm.get(65));
    }

    // r[verify bitmap.clear]
    #[test]
    fn test_clear() {
        let mut bm = Bitmap::new(64);
        bm.set(10);
        assert!(bm.get(10));
        bm.clear(10);
        assert!(!bm.get(10));
    }

    // r[verify bitmap.toggle]
    #[test]
    fn test_toggle() {
        let mut bm = Bitmap::new(64);
        assert!(!bm.get(5));
        bm.toggle(5);
        assert!(bm.get(5));
        bm.toggle(5);
        assert!(!bm.get(5));
    }

    // r[verify bitmap.set]
    // r[verify bitmap.get]
    #[test]
    fn test_set_preserves_other_bits() {
        let mut bm = Bitmap::new(128);
        bm.set(10);
        bm.set(70);
        // Setting 10 didn't affect 70 and vice versa
        assert!(bm.get(10));
        assert!(bm.get(70));
        assert!(!bm.get(11));
        assert!(!bm.get(69));
    }

    fn num_bits_and_pos() -> impl Strategy<Value = (usize, usize)> {
        (1usize..=512).prop_flat_map(|n| (Just(n), 0..n))
    }

    proptest! {
        // r[verify bitmap.set]
        // r[verify bitmap.get]
        #[test]
        fn prop_set_then_get((num_bits, pos) in num_bits_and_pos()) {
            let mut bm = Bitmap::new(num_bits);
            bm.set(pos);
            prop_assert!(bm.get(pos));
        }

        // r[verify bitmap.clear]
        #[test]
        fn prop_clear_then_get((num_bits, pos) in num_bits_and_pos()) {
            let mut bm = Bitmap::new(num_bits);
            bm.set(pos);
            bm.clear(pos);
            prop_assert!(!bm.get(pos));
        }

        // r[verify bitmap.toggle]
        #[test]
        fn prop_toggle_involution((num_bits, pos) in num_bits_and_pos()) {
            let mut bm = Bitmap::new(num_bits);
            let before = bm.get(pos);
            bm.toggle(pos);
            prop_assert_ne!(bm.get(pos), before);
            bm.toggle(pos);
            prop_assert_eq!(bm.get(pos), before);
        }

        // r[verify bitmap.set]
        // r[verify bitmap.clear]
        // r[verify bitmap.toggle]
        // r[verify bitmap.get]
        // r[verify bitmap.len]
        // r[verify bitmap.word-count]
        // r[verify bitmap.new]
        /// Model oracle: track a Vec<bool> alongside Bitmap, run random ops.
        #[test]
        fn prop_model_oracle(
            num_bits in 1usize..=256,
            ops in proptest::collection::vec((0u8..3, any::<usize>()), 0..=200),
        ) {
            let mut bm = Bitmap::new(num_bits);
            let mut oracle = vec![false; num_bits];

            prop_assert_eq!(bm.len(), num_bits);

            for &(op_kind, raw_pos) in &ops {
                let pos = raw_pos % num_bits;

                match op_kind {
                    0 => {
                        bm.set(pos);
                        oracle[pos] = true;
                    }
                    1 => {
                        bm.clear(pos);
                        oracle[pos] = false;
                    }
                    _ => {
                        bm.toggle(pos);
                        oracle[pos] = !oracle[pos];
                    }
                }

                prop_assert_eq!(bm.get(pos), oracle[pos]);
            }

            // Full scan: every position must match the oracle
            for (i, &expected) in oracle.iter().enumerate() {
                prop_assert_eq!(bm.get(i), expected);
            }
        }
    }
}
