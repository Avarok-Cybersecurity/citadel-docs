//! Small paragraphs where TeX's tie-breaking (`d <= minimal_demerits`: of equal-demerit nodes
//! the later wins) or its fitness-class pruning (`minimum_demerits + adj_demerits`) decides the
//! breaks, broken by TeX itself (`tests/oracle/cases.py` wrote `golden/cases.rs`). They were
//! found by mutating each rule and keeping the paragraphs whose breaks changed.

#[path = "golden/cases.rs"]
mod cases;

use cdoc_para::{end_paragraph, Breaker, HyphenKind, Item, KnuthPlass, LineWidths, Params};

fn items(desc: &str) -> Vec<Item<()>> {
    let mut items = Vec::new();
    for token in desc.split(',') {
        let (kind, rest) = token.split_at(1);
        match kind {
            "b" => items.push(Item::boxed(rest.parse().unwrap(), ())),
            "h" => {
                let w: i32 = rest.parse().unwrap();
                items.push(Item::boxed(w / 2, ()));
                items.push(Item::Hyphen {
                    width: 40,
                    kind: HyphenKind::Soft,
                });
                items.push(Item::boxed(w - w / 2, ()));
            }
            _ => {
                let (y, z) = rest.split_once('/').unwrap();
                items.push(Item::glue(100, y.parse().unwrap(), z.parse().unwrap()));
            }
        }
    }
    end_paragraph(&mut items);
    items
}

#[test]
fn ties_and_class_pruning_match_tex() {
    assert!(cases::CASES.len() >= 10);
    for (width, tolerance, desc, tex) in cases::CASES {
        let params = Params {
            pretolerance: -1,
            tolerance: *tolerance,
            ..Params::tex_plain()
        };
        let widths = LineWidths::uniform(*width).unwrap();
        let got = KnuthPlass
            .break_lines(&items(desc), &widths, &params)
            .unwrap();
        let ends: Vec<usize> = got.lines.iter().map(|l| l.end).collect();
        assert_eq!(ends, *tex, "{width} {desc}");
    }
}
