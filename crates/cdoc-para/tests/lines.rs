//! Justification and per-line widths.

mod support;

use cdoc_para::{justify, Breaker, Item, KnuthPlass, LineWidths, Params, INFINITE_PENALTY};
use support::fixtures::{ends, words};

#[test]
fn justify_sums_exactly_and_respects_orders() {
    let items = words(&[100, 200, 300, 400], None);
    let w = LineWidths::uniform(10_000).unwrap();
    let got = KnuthPlass
        .break_lines(&items, &w, &Params::word_like())
        .unwrap();
    let line = got.lines[0];
    let j = justify(&items, &line, 10_000);
    assert_eq!(j.residual, 0);
    assert_eq!(
        j.glue[..3],
        [100, 100, 100],
        "finite glue stays natural beside fil"
    );
    assert_eq!(j.glue[3], 10_000 - 1300, "the parfillskip takes the slack");
    let body = Item::<()>::glue(100, 7, 3);
    let three = vec![
        Item::boxed(1, ()),
        body.clone(),
        Item::boxed(1, ()),
        body.clone(),
        Item::boxed(1, ()),
        body,
        Item::boxed(1, ()),
        Item::penalty(-INFINITE_PENALTY),
    ];
    let full = cdoc_para::Line {
        start: 0,
        end: 7,
        ratio: 0,
        hyphenated: false,
    };
    let j = justify(&three, &full, 314);
    assert_eq!(
        j.glue,
        vec![103, 103, 104],
        "cumulative floors: the remainder goes last"
    );
    assert_eq!(j.residual, 0);
    let j = justify(&three, &full, 290);
    assert_eq!(
        j.glue,
        vec![97, 97, 97],
        "shrink stops at the glue's shrink"
    );
    assert_eq!(j.residual, -5);
}

#[test]
fn per_line_widths_and_the_last_repeating() {
    let items = words(&[500; 6], None);
    let w = LineWidths::per_line(&[700, 1300]).unwrap();
    let got = KnuthPlass
        .break_lines(&items, &w, &Params::word_like())
        .unwrap();
    let ends = ends(&got);
    assert_eq!(ends.len(), 4, "one word, then two per line");
    assert_eq!(ends[0], 1);
    assert_eq!(w.width(9), 1300);
}
