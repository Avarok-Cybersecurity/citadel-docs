//! Serde is lossless on every generated document; walking and normalising behave.

use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_model::test_support::strategies;
use cdoc_model::{Block, Document, Inline, RunProperties};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn json_round_trip_is_identity(doc in strategies::document()) {
        let json = serde_json::to_string(&doc).unwrap();
        let back: Document = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(back, doc);
    }

    #[test]
    fn normalize_is_idempotent_and_reported(mut doc in strategies::document()) {
        doc.normalize();
        prop_assert!(doc.is_normalized());
        let once = doc.clone();
        doc.normalize();
        prop_assert_eq!(doc, once);
    }
}

#[test]
fn kitchen_sink_json_round_trips_and_uses_camel_case() {
    let doc = kitchen_sink();
    let json = serde_json::to_value(&doc).unwrap();
    assert!(json["sections"][0]["properties"]["titlePage"]
        .as_bool()
        .unwrap());
    assert!(json.get("even_and_odd_headers").is_none());
    let back: Document = serde_json::from_value(json).unwrap();
    assert_eq!(back, doc);
}

#[test]
fn walkers_reach_headers_footers_footnotes_and_nested_cells() {
    let doc = kitchen_sink();
    let paragraphs = doc.paragraphs().count();
    // section 0 body: 5 top-level paragraphs + 3 cell paragraphs (one in the nested table);
    // section 1 body: 1; headers (3) + footers (1), one paragraph each, in both sections: 8;
    // footnote: 1.
    assert_eq!(paragraphs, 8 + 1 + 8 + 1);
    assert!(doc
        .inlines()
        .any(|i| matches!(i.content, cdoc_model::InlineContent::SignatureField(_))));
}

#[test]
fn word_count_joins_runs_and_ignores_headers() {
    let mut doc = Document::blank();
    if let Block::Paragraph(p) = &mut doc.sections[0].blocks[0] {
        p.inlines = vec![
            Inline::text("Hel", RunProperties::none()),
            Inline::text("lo wor", RunProperties::none()),
            Inline::text("ld", RunProperties::none()),
            Inline::plain(cdoc_model::InlineContent::Tab),
            Inline::text("again", RunProperties::none()),
        ];
    }
    assert_eq!(doc.word_count(), 3);
    // "a link" and the deleted "gone" abut, so they count as one word.
    assert_eq!(kitchen_sink().word_count(), 16);
}

#[test]
fn normalize_merges_only_identical_neighbours() {
    let mut doc = Document::blank();
    let bold = RunProperties {
        bold: Some(true),
        ..RunProperties::none()
    };
    if let Block::Paragraph(p) = &mut doc.sections[0].blocks[0] {
        p.inlines = vec![
            Inline::text("a", RunProperties::none()),
            Inline::text("b", RunProperties::none()),
            Inline::text("c", bold.clone()),
            Inline::text("d", bold),
            Inline::text("e", RunProperties::none()),
        ];
    }
    assert!(!doc.is_normalized());
    doc.normalize();
    let Block::Paragraph(p) = &doc.sections[0].blocks[0] else {
        unreachable!()
    };
    let texts: Vec<&str> = p
        .inlines
        .iter()
        .map(|i| match &i.content {
            cdoc_model::InlineContent::Text(t) => t.text.as_str(),
            _ => "",
        })
        .collect();
    assert_eq!(texts, ["ab", "cd", "e"]);
}
