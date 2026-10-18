//! Ids the editor picks itself (footnotes, list definitions), for commands that do not take
//! one from the caller. The editor has no randomness, so an id is derived from who is writing
//! and where in its history: the replica's client and its next clock, mixed with a purpose,
//! so two replicas that pick concurrently get different ids unless 31-bit hashes collide, and
//! a collision is resolved by taking the next free value. Ids are `1..2^31`.

use cdoc_crdt::Doc;

/// What an id is for, so one transaction can pick several.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Purpose {
    Footnote = 1,
    Numbering = 2,
    Definition = 3,
}

/// `SplitMix64`'s finaliser.
const fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// A free id for `purpose`: not `taken`, and the same on every run from the same replica
/// state.
pub(crate) fn id(doc: &Doc, purpose: Purpose, taken: impl Fn(u32) -> bool) -> u32 {
    let next = doc.next_id();
    let seed = mix(next.client.0 ^ mix(next.clock ^ ((purpose as u64) << 56)));
    let mut candidate = u32::try_from(seed >> 33).expect("31 bits fit");
    while candidate == 0 || taken(candidate) {
        candidate = (candidate + 1) & 0x7FFF_FFFF;
    }
    candidate
}
