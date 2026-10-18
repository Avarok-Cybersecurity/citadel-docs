//! A keystroke on the hundred-page document: the command, the incremental relayout and line
//! maps, and painting the pages it dirtied, as one `Editor::apply` plus `Editor::paint`.
//!
//! Iterations alternate typing a character and deleting it in a paragraph halfway through, so
//! the document stays the same size. Besides criterion's estimate the bench prints p50 and p95
//! of its own samples. The control lays the same document out with `Dirty::all()` on a warm
//! session and paints every page: what each keystroke would cost without precise dirtiness.

#![allow(
    missing_docs,
    reason = "criterion's macros generate undocumented items"
)]

#[path = "../../cdoc-layout/tests/long/mod.rs"]
mod long;

use cdoc_crdt::ClientId;
use cdoc_edit::{Command, DeleteUnit, Editor, ModelPath, Selection};
use cdoc_layout::test_support::default_fonts;
use cdoc_layout::{Dirty, LayoutOptions, LayoutSession};
use criterion::{criterion_group, criterion_main, Criterion};
use std::time::{Duration, Instant};

const SAMPLES: usize = 400;

fn percentiles(name: &str, mut samples: Vec<Duration>) {
    samples.sort_unstable();
    let at = |q: usize| samples[(samples.len() * q / 100).min(samples.len() - 1)];
    eprintln!(
        "{name}: p50 {:?}, p95 {:?} over {} samples",
        at(50),
        at(95),
        samples.len()
    );
}

fn keystroke(editor: &mut Editor<'_>, flip: &mut bool) -> usize {
    *flip = !*flip;
    let command = if *flip {
        Command::InsertText("x".to_owned())
    } else {
        Command::DeleteBackward(DeleteUnit::Char)
    };
    let applied = editor.apply(command).expect("applies");
    editor.paint(&applied.dirty_pages).expect("paints").len()
}

/// A keystroke as the browser sees it: the request's work plus the result record (caret,
/// selection rectangles and the formatting at the caret).
fn keystroke_with_result(editor: &mut Editor<'_>, flip: &mut bool) -> usize {
    *flip = !*flip;
    let command = if *flip {
        Command::InsertText("x".to_owned())
    } else {
        Command::DeleteBackward(DeleteUnit::Char)
    };
    let applied = editor.apply(command).expect("applies");
    let painted = editor.paint(&applied.dirty_pages).expect("paints").len();
    let outcome = editor
        .outcome(applied.update, applied.dirty_pages)
        .expect("an outcome");
    painted + cdoc_edit::wire::encode_result(&outcome).len()
}

fn bench(c: &mut Criterion) {
    let fonts = default_fonts();
    let options = LayoutOptions::word_like();
    let doc = long::hundred_pages();
    let middle = long::middle(&doc);
    let mut editor = Editor::from_document(&doc, ClientId(1), &fonts, &options).expect("opens");
    eprintln!("document: {} pages", editor.page_count());
    let path = ModelPath {
        section: 0,
        block: middle,
        cells: Vec::new(),
        offset: 10,
    };
    let at = editor.position_at_path(&path).expect("a position");
    editor
        .apply(Command::SetSelection(Selection::caret(at)))
        .expect("selects");
    let mut flip = false;
    let samples = (0..SAMPLES)
        .map(|_| {
            let t = Instant::now();
            keystroke(&mut editor, &mut flip);
            t.elapsed()
        })
        .collect();
    percentiles("keystroke", samples);
    let samples = (0..SAMPLES)
        .map(|_| {
            let t = Instant::now();
            keystroke_with_result(&mut editor, &mut flip);
            t.elapsed()
        })
        .collect();
    percentiles("keystroke + result record", samples);
    c.bench_function("keystroke: apply + relayout + paint(dirty)", |b| {
        b.iter(|| keystroke(&mut editor, &mut flip));
    });

    let mut session = LayoutSession::new(&fonts, &options);
    let model = editor.model().clone();
    let control = |session: &mut LayoutSession<'_>| {
        session.relayout(&model, &Dirty::all()).expect("lays out");
        let layout = session.layout().expect("laid out");
        let pages: Vec<usize> = (0..layout.pages.len()).collect();
        cdoc_paint::encode_pages(layout, &fonts, &pages)
            .expect("paints")
            .len()
    };
    let samples = (0..SAMPLES / 10)
        .map(|_| {
            let t = Instant::now();
            control(&mut session);
            t.elapsed()
        })
        .collect();
    percentiles("control: Dirty::all() + paint(all)", samples);
    c.bench_function("control: Dirty::all() + paint(all)", |b| {
        b.iter(|| control(&mut session));
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
