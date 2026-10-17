//! Edge cases and the pass scheme, one behaviour per test.

mod support;

use cdoc_para::{
    end_paragraph, BreakError, Breaker, FirstFit, HyphenKind, Item, KnuthPlass, LineWidths, Params,
    Stretch, INFINITE_PENALTY,
};
use support::fixtures::{ends, words};

#[test]
fn an_empty_paragraph_is_one_empty_line() {
    let mut items: Vec<Item<()>> = Vec::new();
    end_paragraph(&mut items);
    let w = LineWidths::uniform(1000).unwrap();
    for got in [
        KnuthPlass
            .break_lines(&items, &w, &Params::tex_plain())
            .unwrap(),
        FirstFit
            .break_lines(&items, &w, &Params::tex_plain())
            .unwrap(),
    ] {
        assert_eq!(ends(&got), vec![2]);
        assert!(got.report.overfull_lines.is_empty(), "an empty line fits");
    }
}

#[test]
fn a_box_wider_than_the_line_is_reported_overfull() {
    let items = words(&[300, 5000, 300], None);
    let w = LineWidths::uniform(1000).unwrap();
    let got = KnuthPlass
        .break_lines(&items, &w, &Params::tex_plain())
        .unwrap();
    assert!(got.report.fallback, "no feasible breaks");
    assert_eq!(got.report.passes_used, 2);
    assert_eq!(got.report.overfull_lines, vec![1]);
    assert_eq!(ends(&got).len(), 3);
}

#[test]
fn invalid_input_is_a_typed_error() {
    let w = LineWidths::uniform(1000).unwrap();
    let p = Params::tex_plain();
    let open = vec![Item::boxed(10, ())];
    assert_eq!(
        KnuthPlass.break_lines(&open, &w, &p),
        Err(BreakError::Unterminated)
    );
    let none: [Item<()>; 0] = [];
    assert_eq!(
        FirstFit.break_lines(&none, &w, &p),
        Err(BreakError::Unterminated)
    );
    let mut neg = vec![
        Item::boxed(10, ()),
        Item::glue(10, -1, 0),
        Item::boxed(1, ()),
    ];
    end_paragraph(&mut neg);
    assert_eq!(
        KnuthPlass.break_lines(&neg, &w, &p),
        Err(BreakError::NegativeFlex { index: 1 })
    );
    assert_eq!(LineWidths::per_line(&[]), Err(BreakError::NoLineWidths));
    assert_eq!(
        LineWidths::per_line(&[10, 0]),
        Err(BreakError::NonPositiveWidth { line: 1, width: 0 })
    );
}

#[test]
fn forced_breaks_split_and_forbidden_breaks_hold() {
    let mut items = words(&[300, 300], None);
    items.insert(2, Item::penalty(-INFINITE_PENALTY));
    let w = LineWidths::uniform(10_000).unwrap();
    let got = KnuthPlass
        .break_lines(&items, &w, &Params::tex_plain())
        .unwrap();
    assert_eq!(ends(&got), vec![2, items.len() - 1]);
    let mut glued = words(&[600, 600], None);
    glued.insert(1, Item::penalty(INFINITE_PENALTY));
    glued.remove(2);
    glued.insert(
        2,
        Item::Glue {
            width: 100,
            stretch: Stretch::finite(50),
            shrink: 30,
        },
    );
    let narrow = LineWidths::uniform(700).unwrap();
    let got = FirstFit
        .break_lines(&glued, &narrow, &Params::tex_plain())
        .unwrap();
    assert_eq!(
        got.lines.len(),
        1,
        "a glue after a penalty is no breakpoint"
    );
}

#[test]
fn pattern_hyphens_wait_for_the_second_pass() {
    // 4 words of 400 in 1350: two lines fit with ratio 0.83 (badness 58) in pass one.
    let w = LineWidths::uniform(1350).unwrap();
    let pattern = words(&[400, 400, 400, 400], Some((2, HyphenKind::Pattern)));
    let soft = words(&[400, 400, 400, 400], Some((2, HyphenKind::Soft)));
    let p = Params {
        pretolerance: 100,
        ..Params::tex_plain()
    };
    let got = KnuthPlass.break_lines(&pattern, &w, &p).unwrap();
    assert_eq!(got.report.passes_used, 1);
    assert!(got.lines.iter().all(|l| !l.hyphenated));
    let tight = Params {
        pretolerance: 0,
        tolerance: 10_000,
        ..Params::tex_plain()
    };
    let got = KnuthPlass.break_lines(&pattern, &w, &tight).unwrap();
    assert_eq!(got.report.passes_used, 2);
    let got = KnuthPlass.break_lines(&soft, &w, &tight).unwrap();
    assert_eq!(
        got.report.passes_used, 2,
        "only badness, not the hyphen, failed pass one"
    );
}

#[test]
fn a_pattern_hyphen_that_alone_makes_a_line_fit_needs_pass_two() {
    // 400 + 100 + 400 + 100 + 200 + hyphen 40 = 1240: perfect only when broken at the hyphen.
    let w = LineWidths::uniform(1240).unwrap();
    let p = Params::tex_plain();
    let pattern = words(&[400, 400, 400, 400], Some((2, HyphenKind::Pattern)));
    let got = KnuthPlass.break_lines(&pattern, &w, &p).unwrap();
    assert_eq!(
        got.report.passes_used, 2,
        "pass one may not use the pattern hyphen"
    );
    assert!(got.lines[0].hyphenated);
    let soft = words(&[400, 400, 400, 400], Some((2, HyphenKind::Soft)));
    let got = KnuthPlass.break_lines(&soft, &w, &p).unwrap();
    assert_eq!(
        got.report.passes_used, 1,
        "an author's hyphen is there from pass one"
    );
    assert!(got.lines[0].hyphenated);
}

#[test]
fn first_fit_fills_each_line_before_breaking() {
    let items = words(&[300, 300, 300, 300, 300], None);
    let w = LineWidths::uniform(1040).unwrap();
    let got = FirstFit
        .break_lines(&items, &w, &Params::tex_plain())
        .unwrap();
    assert_eq!(
        ends(&got),
        vec![5, 11],
        "three words fit in 1040 when shrunk (1100 - 60)"
    );
    assert!(
        got.report.overfull_lines.is_empty(),
        "first fit backs up to the last fit"
    );
}

#[test]
fn emergency_stretch_runs_a_third_pass_and_reports_loose_lines() {
    let items = words(&[400, 400, 400, 400, 400], None);
    let w = LineWidths::uniform(1100).unwrap();
    let plain = KnuthPlass
        .break_lines(&items, &w, &Params::tex_plain())
        .unwrap();
    assert!(plain.report.fallback);
    let p = Params {
        emergency_stretch: 400,
        ..Params::tex_plain()
    };
    let got = KnuthPlass.break_lines(&items, &w, &p).unwrap();
    assert!(!got.report.fallback);
    assert_eq!(got.report.passes_used, 3);
    assert!(
        !got.report.underfull_lines.is_empty(),
        "emergency lines are loose"
    );
}
