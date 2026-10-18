//! Encode then decode gives back exactly what was encoded: for the kitchen sink, for random
//! documents laid out with the shipped faces, and for arbitrary lists that reach every
//! opcode and value range.

mod support;

use cdoc_layout::{layout, BreakerChoice, LayoutOptions};
use cdoc_paint::{decode, encode, lower, Op};
use proptest::prelude::*;

#[test]
fn the_kitchen_sink_survives_the_round_trip() {
    let laid = support::kitchen();
    let fonts = support::fonts();
    let all: Vec<usize> = (0..laid.pages.len()).collect();
    let list = lower(&laid, &fonts, &all).expect("lowers");
    let bytes = encode(&laid, &fonts).expect("encodes");
    assert_eq!(bytes, list.to_bytes().expect("writes"));
    let back = decode(&bytes).expect("decodes").to_owned().expect("pages");
    assert_eq!(back, list);
    let keys: Vec<_> = back.faces.iter().map(|f| f.key.clone()).collect();
    assert_eq!(keys, laid.faces);
    assert_eq!(back.pages.len(), laid.pages.len());
    let runs: usize = laid.pages.iter().map(|p| p.runs().count()).sum();
    let text_ops = back
        .pages
        .iter()
        .flat_map(|p| &p.ops)
        .filter(|op| matches!(op, Op::GlyphRun { .. }))
        .count();
    assert_eq!(text_ops, runs, "every IR run is one glyph-run op");
    assert!(runs > 0);
}

#[test]
fn every_opcode_survives_the_round_trip() {
    let list = support::every_op();
    let bytes = list.to_bytes().expect("writes");
    assert_eq!(decode(&bytes).expect("decodes").to_owned(), Ok(list));
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    #[test]
    fn arbitrary_lists_survive_the_round_trip(list in support::strategies::display_list()) {
        let bytes = list.to_bytes().expect("writes");
        let view = decode(&bytes).expect("decodes");
        prop_assert_eq!(view.page_count(), list.pages.len());
        prop_assert_eq!(view.to_owned().expect("pages"), list);
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    #[test]
    fn random_documents_survive_the_round_trip(
        doc in cdoc_model::test_support::strategies::document()
    ) {
        let fonts = support::fonts();
        let doc = support::layable(doc);
        let Ok(laid) = layout(&doc, &fonts, &LayoutOptions::word_like(BreakerChoice::KnuthPlass(cdoc_layout::Params::word_like()))) else {
            return Err(TestCaseError::reject("the layout refused this document"));
        };
        let all: Vec<usize> = (0..laid.pages.len()).collect();
        let list = lower(&laid, &fonts, &all).expect("lowers");
        let bytes = encode(&laid, &fonts).expect("encodes");
        prop_assert_eq!(decode(&bytes).expect("decodes").to_owned().expect("pages"), list);
    }
}
