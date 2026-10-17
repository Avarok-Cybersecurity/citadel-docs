//! Benchmarks: Knuth-Plass on a 2000-word paragraph at three measures, against the O(n^2)
//! exhaustive search it prunes with its active list; Liang hyphenation of 10 000 words.
//! Widths are hundredths of a point; each letter is 5pt wide, a space 3.33pt plus 1.67pt
//! minus 1.11pt, a hyphen 3.33pt. The text is the Frog King's words in a fixed shuffled order.

#[path = "../tests/support/naive.rs"]
mod naive;

use cdoc_para::{
    end_paragraph, Breaker, HyphenKind, Hyphenator, Item, KnuthPlass, Language, LineWidths, Params,
};
use criterion::{BenchmarkId, Criterion};
use std::hint::black_box;

const LETTER: i32 = 500;

fn text(count: usize) -> Vec<&'static str> {
    let vocabulary: Vec<&str> = include_str!("../tests/oracle/frog.txt")
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphabetic()))
        .collect();
    // A fixed linear congruential sequence: reproducible, no randomness source.
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    (0..count)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let k = usize::try_from(state >> 33).unwrap_or(0) % vocabulary.len();
            vocabulary[k]
        })
        .collect()
}

fn paragraph(words: &[&str], h: &Hyphenator) -> Vec<Item<usize>> {
    let mut items = Vec::new();
    let mut breaks = Vec::new();
    for (k, word) in words.iter().enumerate() {
        if k > 0 {
            items.push(Item::glue(333, 167, 111));
        }
        h.hyphenate_into(word, &mut breaks);
        let mut last = 0;
        for &b in &breaks {
            let letters = i32::try_from(word[last..b].chars().count()).unwrap_or(0);
            items.push(Item::boxed(letters * LETTER, k));
            items.push(Item::Hyphen {
                width: 333,
                kind: HyphenKind::Pattern,
            });
            last = b;
        }
        let letters = i32::try_from(word[last..].chars().count()).unwrap_or(0);
        items.push(Item::boxed(letters * LETTER, k));
    }
    end_paragraph(&mut items);
    items
}

fn breaking(c: &mut Criterion) {
    let h = Hyphenator::for_language(Language::EnUs).unwrap();
    let items = paragraph(&text(2000), &h);
    let params = Params {
        pretolerance: -1,
        ..Params::tex_plain()
    };
    let mut group = c.benchmark_group("knuth_plass_2000_words");
    for measure in [25_000, 40_000, 60_000] {
        let widths = LineWidths::uniform(measure).unwrap();
        group.bench_with_input(BenchmarkId::new("active_list", measure), &widths, |b, w| {
            b.iter(|| {
                KnuthPlass
                    .break_lines(black_box(&items), w, &params)
                    .unwrap()
            });
        });
    }
    group.sample_size(10);
    for measure in [25_000, 40_000, 60_000] {
        let threshold = i64::from(params.tolerance);
        let width = i64::from(measure);
        group.bench_with_input(BenchmarkId::new("naive_n2", measure), &width, |b, w| {
            b.iter(|| naive::optimum(black_box(&items), *w, &params, threshold, false));
        });
    }
    group.finish();
}

fn hyphenation(c: &mut Criterion) {
    let h = Hyphenator::for_language(Language::EnUs).unwrap();
    let words = text(10_000);
    let mut out = Vec::with_capacity(16);
    c.bench_function("hyphenate_10000_words", |b| {
        b.iter(|| {
            let mut total = 0;
            for w in &words {
                h.hyphenate_into(black_box(w), &mut out);
                total += out.len();
            }
            total
        });
    });
    c.bench_function("compile_en_us_patterns", |b| {
        b.iter(|| Hyphenator::for_language(black_box(Language::EnUs)).unwrap());
    });
}

fn main() {
    let mut c = Criterion::default().configure_from_args();
    breaking(&mut c);
    hyphenation(&mut c);
    c.final_summary();
}
