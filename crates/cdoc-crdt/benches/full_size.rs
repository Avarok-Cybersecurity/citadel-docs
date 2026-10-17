//! The specified sizes, timed once each on both sides: 1M sequential inserts and 100k random
//! edits. `yrs` is quadratic on both (minutes per run), so criterion's repeated sampling is not
//! practical here; `vs_yrs` samples the same workloads at smaller sizes.
//!
//! Run: `cargo bench -p cdoc-crdt --bench full_size`.

#![allow(missing_docs)]

mod common;

use cdoc_crdt::{ClientId, Doc};
use common::{cdoc_apply, cdoc_sequential, random_ops, yrs_apply, yrs_sequential};
use std::time::Instant;

fn time<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let out = f();
    println!("{label}: {:?}", start.elapsed());
    out
}

fn main() {
    let d = time("sequential_insert_1M/cdoc", || cdoc_sequential(1_000_000));
    assert_eq!(d.len(), 1_000_000);
    time("sequential_insert_1M/yrs", || yrs_sequential(1_000_000));
    let ops = random_ops(7, 100_000, 5, 70);
    time("random_edit_100k/cdoc", || {
        let mut d = Doc::new(ClientId(1));
        cdoc_apply(&mut d, &ops);
        d
    });
    time("random_edit_100k/yrs", || {
        let doc = yrs::Doc::with_client_id(1);
        yrs_apply(&doc, &ops);
        doc
    });
}
