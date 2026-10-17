//! Full layout against incremental relayout on a hundred-page document.
//!
//! Each incremental case alternates between the document and its edited copy, so every
//! iteration is one relayout after a one-block edit. The control case sends `Dirty::all()` to
//! a warm session and must cost what a full layout costs.

#![allow(
    missing_docs,
    reason = "criterion's macros generate undocumented items"
)]

#[path = "../tests/long/mod.rs"]
mod long;

use cdoc_layout::test_support::default_fonts;
use cdoc_layout::{layout, BlockEdit, Dirty, LayoutOptions, LayoutSession};
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

fn relayout(c: &mut Criterion) {
    let fonts = default_fonts();
    let options = LayoutOptions::word_like();
    let doc = long::hundred_pages();
    let pages = layout(&doc, &fonts, &options)
        .expect("lays out")
        .pages
        .len();
    eprintln!("document: {pages} pages");
    c.bench_function("full layout", |b| {
        b.iter(|| layout(black_box(&doc), &fonts, &options).expect("lays out"));
    });
    let middle = long::middle(&doc);
    let reflow = " and more".repeat(60);
    let cases = [
        ("one character, middle", middle, "x"),
        ("one character, first paragraph", 0, "x"),
        ("lines added near the top", 9, reflow.as_str()),
    ];
    for (name, index, typed) in cases {
        let edited = long::typed(&doc, index, typed);
        let dirty = Dirty::edits(vec![BlockEdit::changed(0, index)]);
        let mut session = LayoutSession::new(&fonts, &options);
        session.relayout(&doc, &Dirty::all()).expect("lays out");
        let mut flip = false;
        c.bench_function(name, |b| {
            b.iter(|| {
                flip = !flip;
                let next = if flip { &edited } else { &doc };
                let r = session
                    .relayout(black_box(next), &dirty)
                    .expect("relays out");
                r.changed_pages().len()
            });
        });
    }
    let prose = long::plain_prose();
    let edited = long::typed(&prose, 0, &" and more".repeat(12));
    let dirty = Dirty::edits(vec![BlockEdit::changed(0, 0)]);
    let mut session = LayoutSession::new(&fonts, &options);
    session.relayout(&prose, &Dirty::all()).expect("lays out");
    let mut flip = false;
    c.bench_function("worst case: a line added at the top of plain prose", |b| {
        b.iter(|| {
            flip = !flip;
            let next = if flip { &edited } else { &prose };
            let r = session
                .relayout(black_box(next), &dirty)
                .expect("relays out");
            r.changed_pages().len()
        });
    });
    c.bench_function("full layout of plain prose", |b| {
        b.iter(|| layout(black_box(&prose), &fonts, &options).expect("lays out"));
    });
    let mut session = LayoutSession::new(&fonts, &options);
    c.bench_function("control: Dirty::all() on a warm session", |b| {
        b.iter(|| {
            let r = session
                .relayout(black_box(&doc), &Dirty::all())
                .expect("lays out");
            r.changed_pages().len()
        });
    });
}

criterion_group!(benches, relayout);
criterion_main!(benches);
