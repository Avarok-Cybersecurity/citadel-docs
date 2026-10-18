//! Workloads shared by the benchmarks: one fixed-seed operation list, applied identically to
//! `cdoc-crdt` and to `yrs` (each edit in its own `yrs` transaction, its fastest strategy for
//! these workloads: committing squashes runs that a long transaction leaves split).

#![allow(dead_code, clippy::cast_possible_truncation)]

mod workload;

pub use workload::*;

use cdoc_crdt::Doc;
use yrs::{GetString, Text, Transact};

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
    let ops = megabyte_ops();
    let d = megabyte_doc();
    let y = yrs::Doc::with_client_id(1);
    yrs_apply(&y, &ops);
    let text = y.get_or_insert_text("t");
    assert_eq!(d.visible_text(), text.get_string(&y.transact()));
    (d, y)
}
