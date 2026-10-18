//! Sync efficiency: the update a peer in sync needs after one keystroke in a 1 MB document, and
//! the snapshot of a document after 100k edits of churn. The workloads are the ones the
//! `cdoc-bench` metrics run (`common/workload.rs`).
//!
//! Run: `cargo bench -p cdoc-crdt --bench sync`.

#![allow(missing_docs)]

mod common;

use common::{churn_collected, churn_doc, keystroke_diff, megabyte_doc};
use criterion::{criterion_group, criterion_main, Criterion};

fn diff_after_keystroke(c: &mut Criterion) {
    let mut d = megabyte_doc();
    println!(
        "1 MB document ({} chars): one keystroke's diff is {} bytes",
        d.len(),
        keystroke_diff(&mut d).len()
    );
    let mut g = c.benchmark_group("diff_after_keystroke_1MB");
    g.bench_function("cdoc", |b| b.iter(|| keystroke_diff(&mut d)));
    g.finish();
}

fn churn(c: &mut Criterion) {
    let d = churn_doc();
    let collected = churn_collected();
    println!(
        "100k churn ({} chars left): snapshot {} bytes, {} after gc",
        d.len(),
        d.encode_snapshot().len(),
        collected.encode_snapshot().len()
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

criterion_group!(benches, diff_after_keystroke, churn);
criterion_main!(benches);
