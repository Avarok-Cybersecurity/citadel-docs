//! Benchmarks: a 100-page document's display list encoded (lowering included) and decoded with
//! every op and glyph visited, against `serde_json` JSON of the same layout IR (text as strings)
//! and of the display list itself (the same content as the binary format), written and read
//! back. Sizes are printed first, with the fixed-width control (every varint a four-byte
//! integer) beside them. The text is a fixed pseudo-random sequence over a small vocabulary.

#[path = "support/fixed.rs"]
mod fixed;
#[path = "support/hundred.rs"]
mod hundred;
#[path = "support/json.rs"]
mod json;
#[path = "support/json_list.rs"]
mod json_list;

use cdoc_paint::{decode, encode, lower, OpView};
use criterion::Criterion;
use std::hint::black_box;

fn visit(bytes: &[u8]) -> i64 {
    let view = decode(bytes).unwrap();
    let mut sum = 0_i64;
    for slot in 0..view.page_count() {
        for op in view.page(slot).unwrap().ops() {
            if let OpView::GlyphRun { glyphs, .. } = op {
                sum += glyphs
                    .map(|g| i64::from(g.advance) + i64::from(g.id))
                    .sum::<i64>();
            }
        }
    }
    sum
}

fn visit_json(bytes: &[u8]) -> i64 {
    let back: json::JLayout = serde_json::from_slice(bytes).unwrap();
    let mut sum = 0_i64;
    for page in &back.pages {
        for draw in &page.draws {
            if let json::JDraw::Text { text, x, .. } = draw {
                sum += text.chars().map(|c| i64::from(u32::from(c))).sum::<i64>() + i64::from(*x);
            }
        }
    }
    sum
}

fn main() {
    let fonts = cdoc_layout::test_support::default_fonts();
    let laid = hundred::hundred_pages(&fonts);
    let all: Vec<usize> = (0..laid.pages.len()).collect();
    let list = lower(&laid, &fonts, &all).unwrap();
    let bytes = encode(&laid, &fonts).unwrap();
    let as_json = serde_json::to_vec(&json::mirror(&laid)).unwrap();
    let list_json = serde_json::to_vec(&json_list::mirror_list(&list)).unwrap();
    let runs: usize = laid.pages.iter().map(|p| p.runs().count()).sum();
    println!(
        "{} pages, {runs} runs: display list {} bytes, fixed-i32 control {} bytes, \
         IR as serde_json {} bytes ({}.{}x), display list as serde_json {} bytes ({}.{}x)",
        laid.pages.len(),
        bytes.len(),
        fixed::fixed_len(&list),
        as_json.len(),
        as_json.len() / bytes.len(),
        as_json.len() * 10 / bytes.len() % 10,
        list_json.len(),
        list_json.len() / bytes.len(),
        list_json.len() * 10 / bytes.len() % 10,
    );
    let mut c = Criterion::default().configure_from_args();
    let mut group = c.benchmark_group("hundred_pages");
    group.sample_size(20);
    group.bench_function("display_list_encode", |b| {
        b.iter(|| encode(black_box(&laid), &fonts).unwrap());
    });
    group.bench_function("display_list_decode_visit", |b| {
        b.iter(|| visit(black_box(&bytes)));
    });
    group.bench_function("serde_json_encode", |b| {
        b.iter(|| serde_json::to_vec(&json::mirror(black_box(&laid))).unwrap());
    });
    group.bench_function("serde_json_decode_visit", |b| {
        b.iter(|| visit_json(black_box(&as_json)));
    });
    group.bench_function("display_list_as_serde_json_decode_visit", |b| {
        b.iter(|| {
            let back: json_list::JList = serde_json::from_slice(black_box(&list_json)).unwrap();
            back.pages.iter().map(|p| p.3.len()).sum::<usize>()
        });
    });
    group.finish();
    c.final_summary();
}
