//! Sync efficiency, against `yrs` where it has the operation: the update a peer in sync needs
//! after one keystroke in a 1 MB document, the snapshot of a document after 100k edits of churn
//! (before and after `Doc::gc`; `yrs` collects as it goes), and the update for one changed
//! paragraph property; and the 1 MB document's whole diff cut into 64 KiB updates, against the
//! plain diff and against chunking with no limit (the control: same path, one chunk). The
//! workloads are the ones the `cdoc-bench` metrics run
//! (`common/workload.rs`, `common/yrs_workload.rs`).
//!
//! Run: `cargo bench -p cdoc-crdt --bench sync`.

#![allow(missing_docs)]

mod common;

use cdoc_crdt::StateVector;
use common::{block_field_diff, churn_collected, churn_doc, keystroke_diff, megabyte_doc};
use common::{yrs_churn_doc, yrs_keystroke_diff, yrs_megabyte_doc, yrs_snapshot};
use criterion::{criterion_group, criterion_main, Criterion};

fn diff_after_keystroke(c: &mut Criterion) {
    let mut d = megabyte_doc();
    let y = yrs_megabyte_doc();
    println!(
        "1 MB document ({} chars): one keystroke's diff is {} bytes (cdoc) vs {} bytes (yrs)",
        d.len(),
        keystroke_diff(&mut d).len(),
        yrs_keystroke_diff(&y).len()
    );
    let mut g = c.benchmark_group("diff_after_keystroke_1MB");
    g.bench_function("cdoc", |b| b.iter(|| keystroke_diff(&mut d)));
    g.bench_function("yrs", |b| b.iter(|| yrs_keystroke_diff(&y)));
    g.finish();
}

fn diff_chunked(c: &mut Criterion) {
    const LIMIT: usize = 64 * 1024;
    let d = megabyte_doc();
    let empty = StateVector::empty();
    let plain = d.encode_diff(&empty);
    let chunks = d
        .encode_diff_chunked(&empty, LIMIT)
        .expect("the 1 MB diff cuts at 64 KiB");
    let total: usize = chunks.iter().map(Vec::len).sum();
    println!(
        "1 MB document's diff: {} bytes plain, {} chunks of at most {LIMIT} bytes, {} bytes \
         in all ({} bytes of framing)",
        plain.len(),
        chunks.len(),
        total,
        total - plain.len()
    );
    let mut g = c.benchmark_group("diff_chunked_1MB");
    g.sample_size(20);
    g.bench_function("plain", |b| b.iter(|| d.encode_diff(&empty)));
    g.bench_function("chunked_64KiB", |b| {
        b.iter(|| d.encode_diff_chunked(&empty, LIMIT));
    });
    g.bench_function("chunked_no_limit", |b| {
        b.iter(|| d.encode_diff_chunked(&empty, usize::MAX));
    });
    g.finish();
}

fn churn(c: &mut Criterion) {
    let d = churn_doc();
    let collected = churn_collected();
    println!(
        "100k churn ({} chars left): snapshot {} bytes, {} after gc (cdoc) vs {} bytes (yrs)",
        d.len(),
        d.encode_snapshot().len(),
        collected.encode_snapshot().len(),
        yrs_snapshot(&yrs_churn_doc()).len()
    );
    let mut g = c.benchmark_group("churn_100k");
    g.sample_size(10);
    g.bench_function("snapshot", |b| b.iter(|| d.encode_snapshot()));
    g.bench_function("snapshot_after_gc", |b| {
        b.iter(|| collected.encode_snapshot());
    });
    g.bench_function("gc", |b| {
        b.iter_batched(
            || d.clone(),
            |mut d| d.gc(&[d.state_vector()]).expect("a replica covers itself"),
            criterion::BatchSize::LargeInput,
        );
    });
    g.finish();
}

fn block_field(c: &mut Criterion) {
    println!(
        "one paragraph property changed: {} bytes",
        block_field_diff().len()
    );
    c.bench_function("block_field_diff", |b| b.iter(block_field_diff));
}

criterion_group!(
    benches,
    diff_after_keystroke,
    diff_chunked,
    churn,
    block_field
);
criterion_main!(benches);
