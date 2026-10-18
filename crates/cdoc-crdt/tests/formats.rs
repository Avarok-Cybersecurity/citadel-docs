//! The format scan reports what a slice of the same range holds: the distinct paragraph
//! properties and (paragraph, character properties) pairs, in order of first appearance.

use cdoc_crdt::{formats, from_model, slice, Atom, ClientId, Doc, Formats, Piece};
use cdoc_model::test_support::{fixtures, strategies};
use cdoc_model::{Inline, InlineContent, ParagraphProperties};
use proptest::prelude::*;
use std::ops::Range;

fn from_slice(doc: &Doc, range: Range<usize>, first: ParagraphProperties) -> Formats {
    let mut out = Formats {
        paragraphs: vec![first],
        runs: Vec::new(),
    };
    let mut current = 0;
    for piece in slice(doc, range).unwrap() {
        match piece {
            Piece::Block(Atom::Paragraph(p)) => {
                current = if let Some(i) = out.paragraphs.iter().position(|q| *q == p) {
                    i
                } else {
                    out.paragraphs.push(p);
                    out.paragraphs.len() - 1
                };
            }
            Piece::Inline(Inline {
                content: InlineContent::Text(t),
                ..
            }) => {
                let pair = (current, t.properties);
                if !out.runs.contains(&pair) {
                    out.runs.push(pair);
                }
            }
            _ => {}
        }
    }
    out
}

#[test]
fn the_kitchen_sink_scans_as_it_slices() {
    let doc = from_model(&fixtures::kitchen_sink(), ClientId(1)).unwrap();
    let first = ParagraphProperties::none();
    let scanned = formats(&doc, 0..doc.len(), first.clone()).unwrap();
    assert!(scanned.paragraphs.len() > 2 && scanned.runs.len() > 2);
    assert_eq!(scanned, from_slice(&doc, 0..doc.len(), first));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn every_range_scans_as_it_slices(
        d in strategies::document(),
        a in 0usize..10_000,
        b in 0usize..10_000,
    ) {
        let doc = from_model(&d, ClientId(1)).unwrap();
        let (a, b) = (a % (doc.len() + 1), b % (doc.len() + 1));
        let range = a.min(b)..a.max(b);
        let first = ParagraphProperties::none();
        prop_assert_eq!(
            formats(&doc, range.clone(), first.clone()).unwrap(),
            from_slice(&doc, range, first)
        );
    }
}
