//! Bullet and ordered lists become flat numbered paragraphs, to the ninth level.

mod common;

use cdoc_model::builtin::list_indent;
use cdoc_model::NumberFormat;
use common::{read, style, t, text};
use serde_json::{json, Value};

#[test]
fn lists_are_flat_numbered_paragraphs_at_their_depth() {
    let item = |c: Vec<Value>| json!({"type": "listItem", "content": c});
    let p = |s: &str| json!({"type": "paragraph", "content": [t(s)]});
    let inner = json!({"type": "orderedList", "attrs": {"start": 3, "type": "a"}, "content": [item(vec![p("i1")])]});
    let doc = json!({"type": "doc", "content": [
        {"type": "bulletList", "content": [item(vec![p("b1"), p("b1 more"), inner]), item(vec![]), item(vec![p("b2")])]},
        {"type": "orderedList", "attrs": {"start": 1, "type": null}, "content": [item(vec![p("o1")])]},
        {"type": "bulletList", "content": [item(vec![p("b3")])]},
    ]});
    let (ps, document, report) = read(&doc);
    assert!(report.is_lossless(), "{:?}", report.losses);
    let numbers: Vec<Option<(u32, u8)>> = ps
        .iter()
        .map(|p| p.properties.numbering.map(|n| (n.num_id.0, n.level)))
        .collect();
    // b1, its further paragraph, i1 (level 1), the empty item, b2, o1, b3 (bullets shared).
    assert_eq!(
        numbers,
        [
            Some((1, 0)),
            None,
            Some((2, 1)),
            Some((1, 0)),
            Some((1, 0)),
            Some((3, 0)),
            Some((1, 0))
        ]
    );
    assert!(ps.iter().all(|p| style(p) == "ListParagraph"));
    assert_eq!(
        ps[1].properties.indent.as_ref().and_then(|i| i.left),
        Some(list_indent(0))
    );
    assert_eq!(text(&ps[3]), "");
    let n = &document.numbering;
    let letter = n.definition_for(cdoc_model::NumId(2)).expect("defined");
    assert_eq!(letter.levels[1].format, NumberFormat::LowerLetter);
    assert_eq!(n.instances[1].overrides[0].start, 3);
    assert_eq!(n.instances[1].overrides[0].level, 1);
    assert_eq!(n.instances[2].overrides, []);
    assert!(document.validate().is_ok());
}

#[test]
fn deep_lists_stop_at_the_ninth_level_and_odd_starts_are_one() {
    let mut list = json!({"type": "bulletList", "content": [{"type": "listItem", "content": [{"type": "paragraph", "content": [t("deep")]}]}]});
    for _ in 0..10 {
        list = json!({"type": "orderedList", "attrs": {"start": -2, "type": "Z"}, "content": [{"type": "listItem", "content": [list]}]});
    }
    let (ps, document, report) = read(&json!({"type": "doc", "content": [list]}));
    assert_eq!(
        ps.last()
            .and_then(|p| p.properties.numbering)
            .map(|n| n.level),
        Some(8)
    );
    assert_eq!(
        report.count_of("a list level beyond the ninth or out of sequence"),
        2
    );
    assert_eq!(
        report.count_of("a list start below 0 or not a whole number (read as 1)"),
        10
    );
    assert_eq!(report.count_of("list number format Z read as decimal"), 10);
    assert_eq!(
        report.count_of("a list item without a first paragraph (an empty one added)"),
        10
    );
    assert!(document.validate().is_ok());
}
