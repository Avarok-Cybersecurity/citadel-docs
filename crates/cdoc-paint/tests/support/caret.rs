//! Caret lines for the every-op pin, and arbitrary ones for the round trip.

use cdoc_layout::{CaretLine, CaretStop, Provenance, Region};
use cdoc_model::{FootnoteId, HeaderFooterKind};
use cdoc_paint::Op;
use proptest::prelude::*;

/// A page's caret lines: one of each region, two of one paragraph, multi-byte gaps and run
/// counts, negative x and a top above the previous line's, and a line of runs only.
pub fn every_caret_lines() -> Op {
    let line = |region, path: Vec<u32>, top: i32, runs: u32, stops: &[(u32, i32)]| CaretLine {
        from: Provenance { region, path },
        top,
        bottom: top + 420,
        runs,
        stops: stops.iter().map(|&(gap, x)| CaretStop { gap, x }).collect(),
    };
    let body = Region::Body { section: 2 };
    Op::CaretLines(vec![
        line(body, vec![0], -120, 3, &[(0, 7200), (1, 7800), (300, -5)]),
        line(body, vec![0], 300, 2, &[]),
        line(
            Region::Header {
                section: 0,
                kind: HeaderFooterKind::First,
            },
            vec![1, 0, 2, 0],
            90_000,
            0,
            &[(4, 0)],
        ),
        line(
            Region::Footer {
                section: 1,
                kind: HeaderFooterKind::Even,
            },
            vec![0],
            80_000,
            200,
            &[],
        ),
        line(
            Region::Footnote(FootnoteId(70_000)),
            vec![0],
            70_000,
            1,
            &[(0, 1), (1, 2)],
        ),
    ])
}

fn region() -> impl Strategy<Value = Region> {
    let kind = prop_oneof![
        Just(HeaderFooterKind::Default),
        Just(HeaderFooterKind::First),
        Just(HeaderFooterKind::Even),
    ];
    prop_oneof![
        any::<u32>().prop_map(|section| Region::Body { section }),
        (any::<u32>(), kind.clone()).prop_map(|(section, kind)| Region::Header { section, kind }),
        (any::<u32>(), kind).prop_map(|(section, kind)| Region::Footer { section, kind }),
        any::<u32>().prop_map(|id| Region::Footnote(FootnoteId(id))),
    ]
}

/// A page's caret lines: one to five, each of any region and path (or the previous line's),
/// any count of runs, stops at strictly increasing gaps (at least one on a line with no run).
pub fn caret_lines() -> impl Strategy<Value = Op> {
    prop::collection::vec((caret_line(), any::<bool>()), 1..6).prop_map(|lines| {
        let mut out: Vec<CaretLine> = Vec::new();
        for (mut line, same) in lines {
            if let (true, Some(last)) = (same, out.last()) {
                line.from = last.from.clone();
            }
            out.push(line);
        }
        Op::CaretLines(out)
    })
}

/// A caret line: any region and path, any count of runs, stops at strictly increasing gaps (at
/// least one on a line with no run).
fn caret_line() -> impl Strategy<Value = CaretLine> {
    (
        region(),
        prop::collection::vec(any::<u32>(), 0..6),
        any::<[i32; 2]>(),
        any::<u32>(),
        prop::collection::btree_map(any::<u32>(), any::<i32>(), 0..8),
    )
        .prop_map(|(region, path, [top, bottom], runs, stops)| CaretLine {
            from: Provenance { region, path },
            top,
            bottom,
            runs: if stops.is_empty() { runs.max(1) } else { runs },
            stops: stops
                .into_iter()
                .map(|(gap, x)| CaretStop { gap, x })
                .collect(),
        })
}
