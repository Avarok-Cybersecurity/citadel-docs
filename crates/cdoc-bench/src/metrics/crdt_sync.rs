//! CRDT sync efficiency: the update a peer that was in sync needs after one keystroke in a
//! 1 MB document (its bytes, and the time to type the key and encode the update), and the
//! snapshot of a document after 100 000 edits of churn, before and after garbage collection
//! (bytes). The workloads are `cdoc-crdt`'s benchmark workloads, included by path: one
//! definition, so a criterion number and a budget here always measure the same document.

#[path = "../../../cdoc-crdt/benches/common/workload.rs"]
mod workload;

use super::{BenchError, Ctx, Results};
use super::{CRDT_CHURN_BYTES, CRDT_CHURN_GC_BYTES, CRDT_DIFF, CRDT_DIFF_BYTES};
use crate::schema::Measurement;
use std::hint::black_box;

fn bytes(len: usize) -> Result<Measurement, BenchError> {
    let bytes = u64::try_from(len).map_err(|e| BenchError::Crdt(e.to_string()))?;
    Ok(Measurement::Size { bytes })
}

pub(super) fn run(ctx: &Ctx<'_>, out: &mut Results) -> Result<(), BenchError> {
    let mut doc = workload::megabyte_doc();
    out.insert(
        CRDT_DIFF_BYTES.to_owned(),
        bytes(workload::keystroke_diff(&mut doc).len())?,
    );
    let t = ctx.time(CRDT_DIFF, || {
        black_box(workload::keystroke_diff(&mut doc));
        Ok(())
    })?;
    out.insert(CRDT_DIFF.to_owned(), Measurement::Time(t));
    let churned = workload::churn_doc().encode_snapshot();
    out.insert(CRDT_CHURN_BYTES.to_owned(), bytes(churned.len())?);
    let collected = workload::churn_collected().encode_snapshot();
    out.insert(CRDT_CHURN_GC_BYTES.to_owned(), bytes(collected.len())?);
    Ok(())
}
