//! The `cdoc-crdt` workloads, defined once: the criterion benchmarks use them through
//! `common`, and the `cdoc-bench` CI metrics include this file by path, so a budget and a
//! criterion number always measure the same document and the same edits.

// Each includer uses a subset; the arithmetic is on small, bounded sizes.
#![allow(dead_code, clippy::cast_possible_truncation)]

use cdoc_crdt::{ClientId, Doc};

/// xorshift64*.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// `(insert?, position, length)`: inserts with probability `insert_pct`; positions are taken
/// modulo the current length.
pub fn random_ops(
    seed: u64,
    n: usize,
    max_len: usize,
    insert_pct: usize,
) -> Vec<(bool, usize, usize)> {
    let mut rng = Rng(seed);
    (0..n)
        .map(|_| {
            (
                rng.below(100) < insert_pct,
                rng.below(usize::MAX >> 1),
                1 + rng.below(max_len),
            )
        })
        .collect()
}

pub const TEXT: &str =
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor \
                    incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis no";

pub fn cdoc_apply(d: &mut Doc, ops: &[(bool, usize, usize)]) {
    for &(insert, pos, len) in ops {
        let size = d.len();
        if insert || size == 0 {
            d.insert_text(pos % (size + 1), &TEXT[..len])
                .expect("an insert inside the document");
        } else {
            let at = pos % size;
            d.delete(at, len.min(size - at))
                .expect("a delete inside the document");
        }
    }
}

pub fn cdoc_sequential(n: usize) -> Doc {
    let mut d = Doc::new(ClientId(1));
    for i in 0..n {
        d.insert_text(i, "a").expect("an append");
    }
    d
}

/// The random edits that build the 1 MB document.
pub fn megabyte_ops() -> Vec<(bool, usize, usize)> {
    random_ops(11, 26_000, 100, 80)
}

/// About 1 MB of text built by random chunked edits: many items, many tombstones.
pub fn megabyte_doc() -> Doc {
    let mut d = Doc::new(ClientId(1));
    cdoc_apply(&mut d, &megabyte_ops());
    d
}

/// One character typed in the middle, and the update a peer that was in sync needs for it.
pub fn keystroke_diff(d: &mut Doc) -> Vec<u8> {
    let before = d.state_vector();
    let middle = d.len() / 2;
    d.insert_text(middle, "x")
        .expect("an insert inside the document");
    d.encode_diff(&before)
}

/// How many edits the churn workload makes.
pub const CHURN_OPS: usize = 100_000;

/// `CHURN_OPS` short inserts and deletes, half each.
pub fn churn_ops() -> Vec<(bool, usize, usize)> {
    random_ops(13, CHURN_OPS, 5, 50)
}

/// The churn edits applied: a document that stays small while its history (and its
/// tombstones) grows.
pub fn churn_doc() -> Doc {
    let mut d = Doc::new(ClientId(1));
    cdoc_apply(&mut d, &churn_ops());
    d
}

/// The churned document after a collection with itself as the only peer, as a relay that every
/// peer has caught up with would collect it.
pub fn churn_collected() -> Doc {
    let mut d = churn_doc();
    d.gc(&[d.state_vector()]).expect("a replica covers itself");
    d
}

/// A paragraph with four properties set, then its alignment set: the update a peer in sync
/// needs for that one field.
pub fn block_field_diff() -> Vec<u8> {
    use cdoc_crdt::{from_model, Atom};
    use cdoc_model::{Alignment, Document, ParagraphProperties};
    let mut d = from_model(&Document::blank(), ClientId(1)).expect("the blank document loads");
    let mut p = ParagraphProperties {
        keep_with_next: Some(true),
        keep_lines: Some(true),
        page_break_before: Some(true),
        outline_level: Some(2),
        ..ParagraphProperties::none()
    };
    d.set_block(1, &Atom::Paragraph(p.clone()))
        .expect("atom 1 of the blank document is its paragraph");
    let before = d.state_vector();
    p.alignment = Some(Alignment::Center);
    d.set_block(1, &Atom::Paragraph(p))
        .expect("atom 1 of the blank document is its paragraph");
    d.encode_diff(&before)
}
