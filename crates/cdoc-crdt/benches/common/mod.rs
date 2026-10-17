//! Workloads shared by the benchmarks: one fixed-seed operation list, applied identically to
//! `cdoc-crdt` and to `yrs` (each edit in its own `yrs` transaction, its fastest strategy for
//! these workloads: committing squashes runs that a long transaction leaves split).

#![allow(dead_code, clippy::cast_possible_truncation)]

use cdoc_crdt::{ClientId, Doc};
use yrs::{GetString, Text, Transact};

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

const TEXT: &str =
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor \
                    incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis no";

pub fn cdoc_apply(d: &mut Doc, ops: &[(bool, usize, usize)]) {
    for &(insert, pos, len) in ops {
        let size = d.len();
        if insert || size == 0 {
            d.insert_text(pos % (size + 1), &TEXT[..len]).unwrap();
        } else {
            let at = pos % size;
            d.delete(at, len.min(size - at)).unwrap();
        }
    }
}

pub fn yrs_apply(doc: &yrs::Doc, ops: &[(bool, usize, usize)]) {
    let text = doc.get_or_insert_text("t");
    for &(insert, pos, len) in ops {
        let mut txn = doc.transact_mut();
        let size = text.len(&txn) as usize;
        if insert || size == 0 {
            text.insert(&mut txn, (pos % (size + 1)) as u32, &TEXT[..len]);
        } else {
            let at = pos % size;
            text.remove_range(&mut txn, at as u32, len.min(size - at) as u32);
        }
    }
}

pub fn cdoc_sequential(n: usize) -> Doc {
    let mut d = Doc::new(ClientId(1));
    for i in 0..n {
        d.insert_text(i, "a").unwrap();
    }
    d
}

pub fn yrs_sequential(n: usize) -> yrs::Doc {
    let doc = yrs::Doc::with_client_id(1);
    let text = doc.get_or_insert_text("t");
    for i in 0..n {
        text.insert(&mut doc.transact_mut(), i as u32, "a");
    }
    doc
}

/// About 1 MB of text built by random chunked edits (many items and tombstones), on both.
pub fn megabyte() -> (Doc, yrs::Doc) {
    let ops = random_ops(11, 26_000, 100, 80);
    let mut d = Doc::new(ClientId(1));
    cdoc_apply(&mut d, &ops);
    let y = yrs::Doc::with_client_id(1);
    yrs_apply(&y, &ops);
    let text = y.get_or_insert_text("t");
    assert_eq!(d.visible_text(), text.get_string(&y.transact()));
    (d, y)
}
