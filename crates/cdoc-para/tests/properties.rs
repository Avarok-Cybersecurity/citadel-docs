//! Properties of both breakers on random and adversarial paragraphs: they never fail on valid
//! input, breaks are legal and increasing, forced breaks are taken, every line that is not
//! reported fits its measure, justification is exact, and the same input gives the same output.

mod support;

use cdoc_para::{
    badness, justify, Breaker, Breaking, FirstFit, Item, KnuthPlass, LineWidths, Params,
    INFINITE_PENALTY,
};
use proptest::prelude::*;
use support::gen;
use support::naive::{breakpoint, start_after};

/// (natural width, finite stretch, shrink, infinite stretch?) recomputed from the items.
fn measure(items: &[Item<u32>], start: usize, end: usize) -> (i64, i64, i64, bool) {
    let mut out = (0, 0, 0, false);
    for item in &items[start..end] {
        match item {
            Item::Box { width, .. } => out.0 += i64::from(*width),
            Item::Glue {
                width,
                stretch,
                shrink,
            } => {
                out.0 += i64::from(*width);
                out.2 += i64::from(*shrink);
                if stretch.order == cdoc_para::Order::Normal {
                    out.1 += i64::from(stretch.amount);
                } else if stretch.amount > 0 {
                    out.3 = true;
                }
            }
            Item::Penalty { .. } | Item::Hyphen { .. } => {}
        }
    }
    if let Some(Item::Penalty { width, .. } | Item::Hyphen { width, .. }) = items.get(end) {
        out.0 += i64::from(*width);
    }
    out
}

fn check(items: &[Item<u32>], widths: &LineWidths, params: &Params, got: &Breaking) {
    let n = items.len();
    assert_ne!(got.lines.len(), 0);
    assert_eq!(got.lines.last().map(|l| l.end), Some(n - 1));
    let mut prev = None;
    for (k, line) in got.lines.iter().enumerate() {
        assert!(prev.is_none_or(|p| line.end > p), "breaks increase");
        assert!(breakpoint(items, line.end, params).is_some(), "legal break");
        let start = prev.map_or(0, |p| start_after(items, p)).min(line.end);
        assert_eq!(line.start, start);
        let (nat, st, sh, inf) = measure(items, start, line.end);
        let w = i64::from(widths.width(k));
        let over = got.report.overfull_lines.contains(&k);
        assert_eq!(
            over,
            nat - sh > w,
            "line {k}: overfull iff it cannot shrink to fit"
        );
        let under = got.report.underfull_lines.contains(&k);
        if !over && !under && nat < w && !inf {
            assert!(
                badness(w - nat, st) <= i64::from(params.tolerance),
                "line {k}"
            );
        }
        let j = justify(items, line, widths.width(k));
        let boxes: i64 = nat - measure_glue(items, start, line.end);
        let set: i64 = j.glue.iter().map(|g| i64::from(*g)).sum::<i64>() + boxes;
        assert_eq!(
            set + j.residual,
            w,
            "line {k}: justified widths plus residual"
        );
        if !over && (nat >= w || st > 0 || inf) {
            assert_eq!(
                j.residual, 0,
                "line {k}: exact when the line can reach the measure"
            );
        }
        prev = Some(line.end);
    }
    for (i, item) in items.iter().enumerate() {
        if let Item::Penalty { penalty, .. } = item {
            if *penalty <= -INFINITE_PENALTY {
                assert!(
                    got.lines.iter().any(|l| l.end == i),
                    "forced break {i} taken"
                );
            }
        }
    }
}

fn measure_glue(items: &[Item<u32>], start: usize, end: usize) -> i64 {
    items[start..end]
        .iter()
        .map(|i| match i {
            Item::Glue { width, .. } => i64::from(*width),
            _ => 0,
        })
        .sum()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn knuth_plass_lines_are_legal_and_fit(
        items in gen::paragraph(120, false), widths in gen::widths(false), params in gen::params()
    ) {
        let got = KnuthPlass.break_lines(&items, &widths, &params).unwrap();
        check(&items, &widths, &params, &got);
        if !got.report.fallback {
            prop_assert!(got.report.overfull_lines.is_empty(), "optimum fit is never overfull");
            prop_assert!((1..=3).contains(&got.report.passes_used));
        }
    }

    #[test]
    fn first_fit_lines_are_legal(
        items in gen::paragraph(120, false), widths in gen::widths(false), params in gen::params()
    ) {
        let got = FirstFit.break_lines(&items, &widths, &params).unwrap();
        check(&items, &widths, &params, &got);
    }

    #[test]
    fn adversarial_paragraphs_never_fail(
        items in gen::paragraph(60, true), widths in gen::widths(true), params in gen::params()
    ) {
        for got in [
            KnuthPlass.break_lines(&items, &widths, &params).unwrap(),
            FirstFit.break_lines(&items, &widths, &params).unwrap(),
        ] {
            check(&items, &widths, &params, &got);
        }
    }

    #[test]
    fn the_same_input_gives_the_same_output(
        items in gen::paragraph(120, false), widths in gen::widths(false), params in gen::params()
    ) {
        let a = KnuthPlass.break_lines(&items, &widths, &params).unwrap();
        let b = KnuthPlass.break_lines(&items.clone(), &widths.clone(), &params).unwrap();
        prop_assert_eq!(a, b);
    }

    #[test]
    fn first_fit_never_overfills_when_every_word_fits(items in gen::prose(80), w in 3_000..=30_000i32) {
        let widths = LineWidths::uniform(w).unwrap();
        let got = FirstFit.break_lines(&items, &widths, &Params::tex_plain()).unwrap();
        prop_assert_eq!(got.report.overfull_lines, Vec::<usize>::new());
    }

    #[test]
    fn prose_breaks_are_never_worse_than_first_fit(items in gen::prose(80), w in 4_000..=30_000i32) {
        let widths = LineWidths::uniform(w).unwrap();
        let params = Params { pretolerance: -1, ..Params::word_like() };
        let kp = KnuthPlass.break_lines(&items, &widths, &params).unwrap();
        let ff = FirstFit.break_lines(&items, &widths, &params).unwrap();
        prop_assert!(kp.report.overfull_lines.len() <= ff.report.overfull_lines.len());
    }
}
