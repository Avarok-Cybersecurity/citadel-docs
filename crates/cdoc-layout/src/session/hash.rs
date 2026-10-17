//! A fast, deterministic 128-bit fingerprint of model values.
//!
//! The model derives `Hash`, so a block is visited field by field, in declaration order, with
//! no serialisation. The two lanes are folded multiplies in the style of wyhash, each with its
//! own constants, so two different blocks share a fingerprint with odds near 2^-128. Not a
//! cryptographic hash: it guards a cache against accident, not against an adversary, and an
//! adversary gains nothing from a collision but a stale paragraph in their own document.

use std::hash::{Hash, Hasher};

const P0: u64 = 0xa076_1d64_78bd_642f;
const P1: u64 = 0xe703_7ed1_a0b4_28db;
const P2: u64 = 0x8ebc_6af0_9c88_c6e3;
const P3: u64 = 0x5899_65cc_7537_4cc3;

/// The 128-bit product of `a` and `b`, folded to 64 bits.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the two halves of the product are taken apart on purpose"
)]
const fn fold(a: u64, b: u64) -> u64 {
    let product = (a as u128) * (b as u128);
    (product as u64) ^ ((product >> 64) as u64)
}

/// The hasher.
#[derive(Debug, Clone)]
pub struct Fingerprint {
    a: u64,
    b: u64,
    words: u64,
}

impl Fingerprint {
    /// A fresh state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            a: P0,
            b: P2,
            words: 0,
        }
    }

    const fn word(&mut self, w: u64) {
        self.a = fold(self.a ^ P1, w ^ P0);
        self.b = fold(self.b ^ P3, w.rotate_left(29) ^ P2);
        self.words += 1;
    }

    /// The 128-bit result.
    #[must_use]
    pub const fn finish128(&self) -> u128 {
        let hi = fold(self.a ^ self.words, P3 ^ self.b);
        let lo = fold(self.b ^ self.words, P1 ^ self.a);
        ((hi as u128) << 64) | lo as u128
    }
}

impl Hasher for Fingerprint {
    fn write(&mut self, bytes: &[u8]) {
        let (words, rest) = bytes.as_chunks::<8>();
        for word in words {
            self.word(u64::from_le_bytes(*word));
        }
        let mut last = [0; 8];
        last[..rest.len()].copy_from_slice(rest);
        self.word(u64::from_le_bytes(last));
        self.word(bytes.len() as u64);
    }

    fn write_u8(&mut self, i: u8) {
        self.word(u64::from(i) | 0x100);
    }

    fn write_u32(&mut self, i: u32) {
        self.word(u64::from(i) | 0x1_0000_0000);
    }

    fn write_u64(&mut self, i: u64) {
        self.word(i);
    }

    fn write_usize(&mut self, i: usize) {
        self.word(i as u64);
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "the low half, for the std interface"
    )]
    fn finish(&self) -> u64 {
        self.finish128() as u64
    }
}

/// The fingerprint of one value.
#[must_use]
pub fn fingerprint<T: Hash + ?Sized>(value: &T) -> u128 {
    let mut hasher = Fingerprint::new();
    value.hash(&mut hasher);
    hasher.finish128()
}

#[cfg(test)]
mod tests {
    use super::fingerprint;

    #[test]
    fn equal_values_agree_and_near_values_do_not() {
        assert_eq!(fingerprint("hello"), fingerprint("hello"));
        assert_ne!(fingerprint("hello"), fingerprint("hellp"));
        assert_ne!(fingerprint(&("ab", "c")), fingerprint(&("a", "bc")));
        assert_ne!(fingerprint(&0u8), fingerprint(&0u32));
        assert_ne!(fingerprint(&[0u64; 2][..]), fingerprint(&[0u64; 3][..]));
    }

    #[test]
    fn every_bit_of_a_word_reaches_both_halves() {
        let base = fingerprint(&0u64);
        for bit in 0..64 {
            let flipped = fingerprint(&(1u64 << bit));
            assert_ne!(base >> 64, flipped >> 64, "high half, bit {bit}");
            assert_ne!(base & u128::from(u64::MAX), flipped & u128::from(u64::MAX));
        }
    }
}
