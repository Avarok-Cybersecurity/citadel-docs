//! The `yrs` side of `workload.rs`: the same operation lists applied to `yrs`, each edit in its
//! own transaction (its fastest strategy for these workloads: committing squashes runs that a
//! long transaction leaves split). Included by path from `cdoc-bench` like `workload.rs`.

// Each includer uses a subset; positions are of small, bounded documents.
#![allow(dead_code, clippy::cast_possible_truncation)]

use super::workload::{churn_ops, megabyte_ops, TEXT};
use yrs::{ReadTxn, Text, Transact};

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

/// `workload::megabyte_doc` on `yrs`.
pub fn yrs_megabyte_doc() -> yrs::Doc {
    let y = yrs::Doc::with_client_id(1);
    yrs_apply(&y, &megabyte_ops());
    y
}

/// `workload::keystroke_diff` on `yrs`: one character typed in the middle, and the update a
/// peer that was in sync needs for it.
pub fn yrs_keystroke_diff(doc: &yrs::Doc) -> Vec<u8> {
    let text = doc.get_or_insert_text("t");
    let before = doc.transact().state_vector();
    {
        let mut txn = doc.transact_mut();
        let middle = text.len(&txn) / 2;
        text.insert(&mut txn, middle, "x");
    }
    doc.transact().encode_diff_v1(&before)
}

/// `workload::churn_doc` on `yrs` (which collects its tombstones as it goes).
pub fn yrs_churn_doc() -> yrs::Doc {
    let y = yrs::Doc::with_client_id(1);
    yrs_apply(&y, &churn_ops());
    y
}

/// A `yrs` document's full state, as a fresh peer would fetch it.
pub fn yrs_snapshot(doc: &yrs::Doc) -> Vec<u8> {
    doc.transact()
        .encode_state_as_update_v1(&yrs::StateVector::default())
}
