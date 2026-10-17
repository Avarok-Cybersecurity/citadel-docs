//! `cdoc-crdt` against `yrs` 0.21.3 (native; a dev-dependency only), sampled by criterion at
//! sizes `yrs` can be sampled at. `full_size` times the specified sizes once each.
//!
//! Run: `cargo bench -p cdoc-crdt --bench vs_yrs`.

#![allow(missing_docs)]

mod common;

use cdoc_crdt::{ClientId, Doc, StateVector};
use common::{cdoc_apply, cdoc_sequential, megabyte, random_ops, yrs_apply, yrs_sequential};
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use yrs::updates::decoder::Decode;
use yrs::{ReadTxn, Transact, Update};

fn sequential(c: &mut Criterion) {
    let mut g = c.benchmark_group("sequential_insert");
    g.sample_size(10);
    g.bench_function("cdoc_100k", |b| b.iter(|| cdoc_sequential(100_000)));
    g.bench_function("yrs_100k", |b| b.iter(|| yrs_sequential(100_000)));
    g.bench_function("cdoc_1M", |b| b.iter(|| cdoc_sequential(1_000_000)));
    g.finish();
}

fn random(c: &mut Criterion) {
    let small = random_ops(7, 20_000, 5, 70);
    let full = random_ops(7, 100_000, 5, 70);
    let mut g = c.benchmark_group("random_edit");
    g.sample_size(10);
    g.bench_function("cdoc_20k", |b| {
        b.iter(|| {
            let mut d = Doc::new(ClientId(1));
            cdoc_apply(&mut d, &small);
            d
        });
    });
    g.bench_function("yrs_20k", |b| {
        b.iter(|| {
            let doc = yrs::Doc::with_client_id(1);
            yrs_apply(&doc, &small);
            doc
        });
    });
    g.bench_function("cdoc_100k", |b| {
        b.iter(|| {
            let mut d = Doc::new(ClientId(1));
            cdoc_apply(&mut d, &full);
            d
        });
    });
    g.finish();
}

fn load(c: &mut Criterion) {
    let (d, y) = megabyte();
    let ours = d.encode_snapshot();
    let theirs = y
        .transact()
        .encode_state_as_update_v1(&yrs::StateVector::default());
    println!(
        "1 MB document: {} visible chars; snapshot {} bytes (cdoc) vs {} bytes (yrs update)",
        d.len(),
        ours.len(),
        theirs.len()
    );
    let mut g = c.benchmark_group("encode_1MB");
    g.bench_function("cdoc", |b| b.iter(|| d.encode_snapshot()));
    g.bench_function("yrs", |b| {
        b.iter(|| {
            y.transact()
                .encode_state_as_update_v1(&yrs::StateVector::default())
        });
    });
    g.finish();
    let mut g = c.benchmark_group("decode_1MB");
    g.bench_function("cdoc", |b| {
        b.iter(|| Doc::decode_snapshot(&ours, ClientId(2)).unwrap());
    });
    let update = d.encode_diff(&StateVector::empty());
    g.bench_function("cdoc_update_path", |b| {
        b.iter(|| {
            let mut fresh = Doc::new(ClientId(2));
            fresh.apply_update(&update).unwrap();
            fresh
        });
    });
    g.bench_function("yrs", |b| {
        b.iter(|| {
            let fresh = yrs::Doc::with_client_id(2);
            fresh
                .transact_mut()
                .apply_update(Update::decode_v1(&theirs).unwrap())
                .unwrap();
            fresh
        });
    });
    g.finish();
}

fn merge(c: &mut Criterion) {
    let base_ops = random_ops(3, 2_000, 8, 70);
    let mut base = Doc::new(ClientId(1));
    cdoc_apply(&mut base, &base_ops);
    let base_snap = base.encode_snapshot();
    let sv = base.state_vector();
    let mut alice = Doc::decode_snapshot(&base_snap, ClientId(2)).unwrap();
    let mut bob = Doc::decode_snapshot(&base_snap, ClientId(3)).unwrap();
    cdoc_apply(&mut alice, &random_ops(4, 10_000, 5, 70));
    cdoc_apply(&mut bob, &random_ops(5, 10_000, 5, 70));
    let (ua, ub) = (alice.encode_diff(&sv), bob.encode_diff(&sv));

    let ybase = yrs::Doc::with_client_id(1);
    yrs_apply(&ybase, &base_ops);
    let ybase_update = ybase
        .transact()
        .encode_state_as_update_v1(&yrs::StateVector::default());
    let ysv = ybase.transact().state_vector();
    let replica = |client, seed| {
        let r = yrs::Doc::with_client_id(client);
        r.transact_mut()
            .apply_update(Update::decode_v1(&ybase_update).unwrap())
            .unwrap();
        yrs_apply(&r, &random_ops(seed, 10_000, 5, 70));
        let diff = r.transact().encode_diff_v1(&ysv);
        diff
    };
    let (ya, yb) = (replica(2, 4), replica(3, 5));

    let mut g = c.benchmark_group("merge_2x10k");
    g.bench_function("cdoc", |bench| {
        bench.iter_batched(
            || Doc::decode_snapshot(&base_snap, ClientId(9)).unwrap(),
            |mut d| {
                d.apply_update(&ua).unwrap();
                d.apply_update(&ub).unwrap();
                d
            },
            BatchSize::LargeInput,
        );
    });
    g.bench_function("yrs", |bench| {
        bench.iter_batched(
            || {
                let d = yrs::Doc::with_client_id(9);
                d.transact_mut()
                    .apply_update(Update::decode_v1(&ybase_update).unwrap())
                    .unwrap();
                d
            },
            |d| {
                {
                    let mut txn = d.transact_mut();
                    txn.apply_update(Update::decode_v1(&ya).unwrap()).unwrap();
                    txn.apply_update(Update::decode_v1(&yb).unwrap()).unwrap();
                }
                d
            },
            BatchSize::LargeInput,
        );
    });
    g.finish();
}

criterion_group!(benches, sequential, random, load, merge);
criterion_main!(benches);
