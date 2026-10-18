//! The read API an editor builds on: ids and indices convert both ways, deleted units keep a
//! place, the unit walk and the structure list agree with the visible text, and a slice of a
//! block range folds into exactly the blocks `to_model` produces for it.

use cdoc_crdt::{fold_blocks, from_model, slice, to_model, ClientId, Doc, Located, Piece, Unit};
use cdoc_model::test_support::{fixtures, strategies};
use cdoc_model::{Block, Document, Inline, RunProperties};
use proptest::prelude::*;

fn edited(d: &Document, edits: &[(usize, bool)]) -> Doc {
    let mut doc = from_model(d, ClientId(1)).unwrap();
    for &(at, insert) in edits {
        let text = doc.visible_text();
        let len = doc.len();
        // Only characters are touched, so the structure stays well formed.
        let chars: Vec<usize> = text
            .chars()
            .enumerate()
            .filter(|(_, c)| *c != '\u{fffc}')
            .map(|(i, _)| i)
            .collect();
        if chars.is_empty() || len == 0 {
            break;
        }
        let i = chars[at % chars.len()];
        if insert {
            doc.insert_text(i, "xy").unwrap();
        } else {
            doc.delete(i, 1).unwrap();
        }
    }
    doc
}

fn check(doc: &Doc) -> Result<(), TestCaseError> {
    let text: Vec<char> = doc.visible_text().chars().collect();
    let units: Vec<_> = doc.units_from(0).collect();
    prop_assert_eq!(units.len(), doc.len());
    for (i, (id, unit)) in units.iter().enumerate() {
        prop_assert_eq!(doc.id_at(i), Some(*id));
        prop_assert_eq!(
            doc.locate(*id),
            Some(Located {
                index: i,
                visible: true
            })
        );
        prop_assert_eq!(doc.unit_at(i), Some(*unit));
        match unit {
            Unit::Char(c) => prop_assert_eq!(text[i], *c),
            Unit::Atom(_) => prop_assert_eq!(text[i], '\u{fffc}'),
        }
        let tail: Vec<_> = doc.units_from(i).collect();
        prop_assert_eq!(&tail[..], &units[i..]);
    }
    let structure = doc.structure();
    prop_assert_eq!(structure.len(), doc.blocks_before(doc.len()));
    let mut seen = 0;
    for (i, (_, unit)) in units.iter().enumerate() {
        prop_assert_eq!(doc.blocks_before(i), seen);
        if matches!(unit, Unit::Atom(k) if k.is_structural()) {
            prop_assert_eq!(structure[seen].id, units[i].0);
            let inline_next = units
                .get(i + 1)
                .is_some_and(|(_, u)| !matches!(u, Unit::Atom(k) if k.is_structural()));
            prop_assert_eq!(structure[seen].inlines, inline_next);
            seen += 1;
        }
    }
    let model = to_model(doc).unwrap();
    let all: Vec<_> = model
        .sections
        .iter()
        .flat_map(|s| s.blocks.clone())
        .collect();
    if model.footnotes.is_empty()
        && model.sections.iter().all(|s| {
            s.properties.headers == cdoc_model::HeaderFooterSet::none()
                && s.properties.footers == cdoc_model::HeaderFooterSet::none()
        })
    {
        prop_assert_eq!(fold_blocks(slice(doc, 0..doc.len()).unwrap()), all);
    }
    Ok(())
}

#[test]
fn a_deleted_unit_keeps_its_place() {
    let mut doc = Doc::new(ClientId(1));
    doc.insert_text(0, "abcd").unwrap();
    let c = doc.id_at(2).unwrap();
    doc.delete(1, 2).unwrap();
    assert_eq!(
        doc.locate(c),
        Some(Located {
            index: 1,
            visible: false
        })
    );
    assert_eq!(
        doc.next_id().clock,
        5,
        "a deletion takes a clock (format v2)"
    );
    assert_eq!(doc.id_at(4), None);
}

#[test]
fn the_kitchen_sink_reads_back() {
    let d = fixtures::kitchen_sink();
    check(&edited(&d, &[(3, true), (40, false), (7, true)])).unwrap();
}

#[test]
fn a_slice_of_one_paragraph_folds_to_it() {
    let mut d = Document::blank();
    let Block::Paragraph(p) = d.sections[0].blocks[0].clone() else {
        unreachable!()
    };
    d.sections[0].blocks = ["one", "two", "three"]
        .iter()
        .map(|t| {
            let mut p = p.clone();
            p.inlines = vec![Inline::text(*t, RunProperties::none())];
            Block::Paragraph(p)
        })
        .collect();
    let doc = from_model(&d, ClientId(1)).unwrap();
    let structure = doc.structure();
    // Block 0 is the section atom; block 2 is the second paragraph.
    let start = doc.locate(structure[2].id).unwrap().index;
    let end = doc.locate(structure[3].id).unwrap().index;
    let blocks = fold_blocks(slice(&doc, start..end).unwrap());
    assert_eq!(blocks, vec![d.sections[0].blocks[1].clone()]);
    let partial = slice(&doc, start + 1..start + 3).unwrap();
    assert_eq!(
        partial,
        vec![Piece::Inline(Inline::text("tw", RunProperties::none()))]
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn every_generated_document_reads_back(
        d in strategies::document(),
        edits in proptest::collection::vec((0usize..1000, any::<bool>()), 0..8),
    ) {
        check(&edited(&d, &edits))?;
    }
}
