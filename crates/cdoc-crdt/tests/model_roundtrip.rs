//! The model bridge is lossless: `to_model(from_model(d)) == d` normalised, for the kitchen
//! sink and for every generated document, directly and through a snapshot and an update.

mod common;

use cdoc_crdt::{from_model, to_model, ClientId, Doc, StateVector};
use cdoc_model::test_support::{fixtures, strategies};
use cdoc_model::{Block, CommentId, Document, Inline, Link, RunProperties};
use common::ROOM;
use proptest::prelude::*;

fn normalised(d: &Document) -> Document {
    let mut d = d.clone();
    d.normalize();
    d
}

fn check(d: &Document) -> Result<(), TestCaseError> {
    let expected = normalised(d);
    let doc = from_model(d, ClientId(1)).unwrap();
    prop_assert_eq!(&to_model(&doc).unwrap(), &expected);
    let reloaded = Doc::decode_snapshot(&doc.encode_snapshot(), ClientId(2)).unwrap();
    prop_assert_eq!(&to_model(&reloaded).unwrap(), &expected);
    let mut synced = Doc::new(ClientId(3));
    synced
        .apply_update(&doc.encode_diff(&StateVector::empty()), ROOM)
        .unwrap();
    prop_assert_eq!(&to_model(&synced).unwrap(), &expected);
    prop_assert_eq!(synced.encode_snapshot(), doc.encode_snapshot());
    Ok(())
}

#[test]
fn the_kitchen_sink_round_trips() {
    check(&fixtures::kitchen_sink()).unwrap();
}

/// Adjacent runs that differ only in their marks (a link, a comment) stay separate inlines.
#[test]
fn runs_differing_only_in_marks_stay_apart() {
    let mut d = Document::blank();
    let Block::Paragraph(p) = &mut d.sections[0].blocks[0] else {
        unreachable!()
    };
    let plain = Inline::text("plain ", RunProperties::none());
    let mut linked = Inline::text("linked", RunProperties::none());
    linked.marks.link = Some(Link {
        href: "https://x.test".to_owned(),
        title: None,
    });
    let mut commented = Inline::text(" noted", RunProperties::none());
    commented.marks.comments = vec![CommentId::new("c1")];
    p.inlines = vec![plain, linked, commented];
    check(&d).unwrap();
}

#[test]
fn a_blank_document_round_trips() {
    check(&Document::blank()).unwrap();
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn every_generated_document_round_trips(d in strategies::document()) {
        check(&d)?;
    }
}
