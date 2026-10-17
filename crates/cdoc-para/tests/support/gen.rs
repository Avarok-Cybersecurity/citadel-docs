//! Proptest generators: paragraphs (well-behaved and adversarial), line widths and parameters.

#![allow(dead_code)]

use cdoc_para::{end_paragraph, HyphenKind, Item, LineWidths, Params, Stretch};
use proptest::prelude::*;

/// One random item; `wild` admits zero and huge widths, any penalty, infinite stretch.
fn item(wild: bool) -> BoxedStrategy<Item<u32>> {
    let w = if wild {
        0..=1_000_000_000i32
    } else {
        50..=3_000i32
    };
    let g = if wild { 0..=1_000_000i32 } else { 0..=400i32 };
    prop_oneof![
        6 => (w.clone(), any::<u32>()).prop_map(|(width, data)| Item::Box { width, data }),
        4 => (g.clone(), g.clone(), g.clone(), any::<bool>()).prop_map(move |(width, st, sh, fil)| {
            Item::Glue {
                width,
                stretch: if fil && wild { Stretch::fil(st) } else { Stretch::finite(st) },
                shrink: if wild { sh } else { sh.min(width) },
            }
        }),
        1 => (
            0..=200i32,
            if wild { -30_000..=30_000i32 } else { -500..=2_000i32 },
            any::<bool>()
        )
            .prop_map(|(width, penalty, flagged)| Item::Penalty { width, penalty, flagged }),
        1 => (0..=200i32, 0..3u8).prop_map(|(width, k)| Item::Hyphen {
            width,
            kind: [HyphenKind::Pattern, HyphenKind::Soft, HyphenKind::Explicit][usize::from(k)],
        }),
    ]
    .boxed()
}

/// A paragraph of up to `max` items, ended as TeX's `\par` ends one.
pub fn paragraph(max: usize, wild: bool) -> impl Strategy<Value = Vec<Item<u32>>> {
    prop::collection::vec(item(wild), 0..=max).prop_map(|mut items| {
        end_paragraph(&mut items);
        items
    })
}

/// Words and spaces only, as a real paragraph is.
pub fn prose(max_words: usize) -> impl Strategy<Value = Vec<Item<u32>>> {
    prop::collection::vec(
        (200..=2_500i32, prop::option::of(100..=900i32)),
        1..=max_words,
    )
    .prop_map(|words| {
        let mut items = Vec::new();
        for (k, (w, hyphen_at)) in words.into_iter().enumerate() {
            if k > 0 {
                items.push(Item::glue(333, 166, 111));
            }
            match hyphen_at {
                Some(h) if h < w => {
                    items.push(Item::boxed(h, 0));
                    items.push(Item::Hyphen {
                        width: 333,
                        kind: HyphenKind::Pattern,
                    });
                    items.push(Item::boxed(w - h, 0));
                }
                _ => items.push(Item::boxed(w, 0)),
            }
        }
        end_paragraph(&mut items);
        items
    })
}

/// Uniform or per-line widths.
pub fn widths(wild: bool) -> impl Strategy<Value = LineWidths> {
    let w = if wild { 1..=i32::MAX } else { 1_000..=40_000 };
    prop::collection::vec(w, 1..=5).prop_map(|ws| LineWidths::per_line(&ws).unwrap())
}

/// Plain TeX's or the word-like parameters, with every field possibly overridden.
pub fn params() -> impl Strategy<Value = Params> {
    (
        any::<bool>(),
        prop::option::of(-1..=10_000i32),
        prop::option::of(0..=10_000i32),
        -2..=2i32,
        prop::option::of(0..=5_000i32),
        prop::option::of(0..=20_000i64),
    )
        .prop_map(|(word, pre, tol, looseness, emergency, adj)| {
            let base = if word {
                Params::word_like()
            } else {
                Params::tex_plain()
            };
            Params {
                pretolerance: pre.unwrap_or(base.pretolerance),
                tolerance: tol.unwrap_or(base.tolerance),
                looseness,
                emergency_stretch: emergency.unwrap_or(base.emergency_stretch),
                adj_demerits: adj.unwrap_or(base.adj_demerits),
                ..base
            }
        })
}
