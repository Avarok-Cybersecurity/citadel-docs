//! Knuth-Plass against TeX itself. The Frog King paragraph of Knuth and Plass's "Breaking
//! Paragraphs into Lines", measured in cmr10 and hyphenated with the en-US patterns, was broken
//! by TeX 3.141592653 with plain TeX's parameters (and per-case overrides: widths, `\parshape`,
//! `\looseness`, `\pretolerance`, `\tolerance`, `\emergencystretch`). `tests/oracle/frog.py`
//! wrote the items and TeX's lines, passes and glue-set ratios to `golden/frog.rs`. Every line
//! break, the pass count and every ratio (to within one in TeX's fifth printed decimal) must agree.

#[path = "golden/frog.rs"]
mod frog;

use cdoc_para::{
    end_paragraph, Breaker, HyphenKind, Item, KnuthPlass, LineWidths, Params, RATIO_ONE,
};
use frog::{CASES, HYPHEN, SPACE, WORDS};

fn frog_items() -> Vec<Item<usize>> {
    let mut items = Vec::new();
    let mut ordinal = 0;
    for (w, (encoded, widths)) in WORDS.iter().enumerate() {
        if w > 0 {
            items.push(Item::glue(SPACE.0, SPACE.1, SPACE.2));
        }
        let mut widths = widths.iter();
        let mut push_box = |items: &mut Vec<Item<usize>>| {
            let width = *widths.next().expect("a width per fragment");
            ordinal += 1;
            items.push(Item::boxed(width, ordinal));
        };
        push_box(&mut items);
        for c in encoded.chars() {
            let kind = match c {
                '|' => HyphenKind::Soft,
                '/' => HyphenKind::Explicit,
                _ => continue,
            };
            let width = if kind == HyphenKind::Soft { HYPHEN } else { 0 };
            items.push(Item::Hyphen { width, kind });
            push_box(&mut items);
        }
    }
    end_paragraph(&mut items);
    items
}

fn params(case: &frog::Case) -> Params {
    Params {
        looseness: case.looseness,
        pretolerance: case.pretolerance,
        tolerance: case.tolerance,
        emergency_stretch: case.emergency_stretch,
        ..Params::tex_plain()
    }
}

#[test]
fn the_items_are_the_paragraph_tex_broke() {
    let items = frog_items();
    let boxes = items.iter().filter(|i| i.is_box()).count();
    let fragments: usize = WORDS.iter().map(|(_, w)| w.len()).sum();
    assert_eq!(boxes, fragments);
    assert_eq!(WORDS.len(), 112, "the Frog King paragraph has 112 words");
}

#[test]
fn every_break_matches_tex() {
    let items = frog_items();
    for case in CASES {
        let widths = LineWidths::per_line(case.widths).unwrap();
        let got = KnuthPlass
            .break_lines(&items, &widths, &params(case))
            .unwrap();
        let breaks: Vec<usize> = got.lines.iter().map(|l| l.end).collect();
        assert_eq!(breaks, case.breaks, "{}: breaks", case.name);
        assert!(
            !got.report.fallback,
            "{}: TeX found feasible breaks",
            case.name
        );
        assert!(got.report.overfull_lines.is_empty(), "{}", case.name);
    }
}

#[test]
fn the_pass_count_matches_tex() {
    let items = frog_items();
    for case in CASES {
        let widths = LineWidths::per_line(case.widths).unwrap();
        let got = KnuthPlass
            .break_lines(&items, &widths, &params(case))
            .unwrap();
        assert_eq!(got.report.passes_used, case.passes, "{}: passes", case.name);
    }
}

#[test]
fn every_glue_set_ratio_matches_tex() {
    let items = frog_items();
    for case in CASES {
        let widths = LineWidths::per_line(case.widths).unwrap();
        let got = KnuthPlass
            .break_lines(&items, &widths, &params(case))
            .unwrap();
        for (k, (line, tex)) in got.lines.iter().zip(case.ratios_e5).enumerate() {
            let one = i64::from(RATIO_ONE);
            let ours = (i64::from(line.ratio) * 200_000 + one).div_euclid(2 * one);
            assert!(
                (ours - tex).abs() <= 1,
                "{} line {k}: ratio {ours}e-5, TeX {tex}e-5",
                case.name
            );
        }
    }
}

#[test]
fn the_cases_cover_every_pass_and_looseness_direction() {
    let passes: Vec<u8> = CASES.iter().map(|c| c.passes).collect();
    for p in 1..=3 {
        assert!(passes.contains(&p), "a case ending in pass {p}");
    }
    let lines = |name: &str| {
        CASES
            .iter()
            .find(|c| c.name == name)
            .map(|c| c.breaks.len())
            .unwrap()
    };
    assert!(lines("looser_tolerant_300pt") > lines("hsize_300pt"));
    assert!(lines("tighter_tolerant_250pt") < lines("hsize_250pt"));
}
