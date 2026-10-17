//! Knuth-Plass finds the optimum: on random paragraphs its breaks cost exactly the fewest
//! demerits an exhaustive search over every pair of breakpoints finds (`support::naive`, an
//! independent rewrite of TeX's demerits), and fewer than or as few as first fit's.

mod support;

use cdoc_para::{Breaker, FirstFit, KnuthPlass, LineWidths, Params, INF_BAD};
use proptest::prelude::*;
use support::{gen, naive};

fn one_pass(base: Params, tolerance: i32) -> Params {
    Params {
        pretolerance: -1,
        tolerance,
        emergency_stretch: 0,
        looseness: 0,
        ..base
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn knuth_plass_costs_what_exhaustive_search_finds(
        items in gen::paragraph(40, false),
        w in 1_000..=12_000i32,
        tolerance in 0..=10_000i32,
        word in any::<bool>(),
    ) {
        let params = one_pass(if word { Params::word_like() } else { Params::tex_plain() }, tolerance);
        let threshold = i64::from(tolerance).min(INF_BAD);
        let widths = LineWidths::uniform(w).unwrap();
        let got = KnuthPlass.break_lines(&items, &widths, &params).unwrap();
        let best = naive::optimum(&items, i64::from(w), &params, threshold, true);
        let ends: Vec<usize> = got.lines.iter().map(|l| l.end).collect();
        match best {
            None => prop_assert!(got.report.fallback, "no feasible breaks, so first fit"),
            Some(best) => {
                prop_assert!(!got.report.fallback);
                let ours = naive::chain(&items, i64::from(w), &params, threshold, &ends);
                prop_assert_eq!(ours, Some(best));
            }
        }
    }

    #[test]
    fn knuth_plass_is_optimal_on_prose(items in gen::prose(16), w in 1_500..=6_000i32, tolerance in 100..=10_000i32) {
        let params = one_pass(Params::tex_plain(), tolerance);
        let threshold = i64::from(tolerance).min(INF_BAD);
        let got = KnuthPlass.break_lines(&items, &LineWidths::uniform(w).unwrap(), &params).unwrap();
        let best = naive::optimum(&items, i64::from(w), &params, threshold, true);
        let ends: Vec<usize> = got.lines.iter().map(|l| l.end).collect();
        prop_assert_eq!(best.is_none(), got.report.fallback);
        if best.is_some() {
            prop_assert_eq!(naive::chain(&items, i64::from(w), &params, threshold, &ends), best);
        }
    }

    #[test]
    fn optimum_fit_is_never_costlier_than_first_fit(items in gen::prose(60), w in 3_000..=20_000i32) {
        let params = one_pass(Params::tex_plain(), 10_000);
        let widths = LineWidths::uniform(w).unwrap();
        let kp = KnuthPlass.break_lines(&items, &widths, &params).unwrap();
        let ff = FirstFit.break_lines(&items, &widths, &params).unwrap();
        let cost = |b: &cdoc_para::Breaking| {
            let ends: Vec<usize> = b.lines.iter().map(|l| l.end).collect();
            naive::chain(&items, i64::from(w), &params, INF_BAD, &ends)
        };
        if let (Some(k), Some(f)) = (cost(&kp), cost(&ff)) {
            prop_assert!(k <= f, "optimum {k} vs first fit {f}");
        }
    }
}
