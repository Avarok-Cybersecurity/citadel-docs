//! `Changes`: which blocks an applied update dirtied, so layout can redo only those.

mod common;

use cdoc_crdt::{from_model, Atom, Changes, ClientId, Doc, DocAttr, Mark};
use cdoc_model::{
    Alignment, Block, Document, DocumentMeta, Inline, Paragraph, ParagraphProperties, RunProperties,
};
use common::ROOM;

/// Section (block 0), then paragraphs "aa", "bb", "cc" (blocks 1 to 3) at units 1, 4 and 7.
fn three_paragraphs() -> Doc {
    paragraphs(&["aa", "bb", "cc"])
}

/// Section (block 0), then one paragraph per text: paragraph `k` is block `k + 1`, its start
/// atom at unit `3k + 1` when every text is two characters.
fn paragraphs(texts: &[&str]) -> Doc {
    let mut d = Document::blank();
    d.sections[0].blocks = texts
        .iter()
        .map(|t| {
            Block::Paragraph(Paragraph {
                properties: ParagraphProperties::none(),
                inlines: vec![Inline::text(*t, RunProperties::none())],
            })
        })
        .collect();
    from_model(&d, ClientId(1)).unwrap()
}

/// What `edit` on another replica reports when applied here.
fn changes_of(edit: impl FnOnce(&mut Doc)) -> Changes {
    changes_in(three_paragraphs(), edit)
}

fn changes_in(mut here: Doc, edit: impl FnOnce(&mut Doc)) -> Changes {
    let mut there = Doc::decode_snapshot(&here.encode_snapshot(), ClientId(2)).unwrap();
    let before = there.state_vector();
    edit(&mut there);
    here.apply_update(&there.encode_diff(&before), ROOM)
        .unwrap()
        .changes
}

#[test]
fn typing_dirties_only_its_paragraph() {
    let c = changes_of(|d| d.insert_text(6, "!").unwrap());
    assert_eq!(c.blocks, vec![2..3]);
    assert_eq!(c.attrs, Vec::<String>::new());
}

#[test]
fn formatting_dirties_the_blocks_it_spans() {
    let c = changes_of(|d| {
        d.format(
            2..6,
            &Mark::Run(RunProperties {
                bold: Some(true),
                ..RunProperties::none()
            }),
        )
        .unwrap();
    });
    assert_eq!(c.blocks, vec![1..3]);
}

#[test]
fn deleting_a_paragraph_boundary_dirties_the_paragraph_it_joins() {
    let c = changes_of(|d| d.delete(4, 1).unwrap());
    assert_eq!(c.blocks, vec![1..2]);
}

#[test]
fn new_blocks_and_property_changes_are_reported() {
    let c = changes_of(|d| {
        d.insert_atom(9, &Atom::Paragraph(ParagraphProperties::none()))
            .unwrap();
        let centred = ParagraphProperties {
            alignment: Some(Alignment::Center),
            ..ParagraphProperties::none()
        };
        d.set_block(1, &Atom::Paragraph(centred)).unwrap();
    });
    assert_eq!(c.blocks, vec![1..2, 3..5]);
}

#[test]
fn attributes_are_reported_by_name() {
    let c = changes_of(|d| {
        d.set_attr(&DocAttr::Meta(DocumentMeta {
            title: Some("t".to_owned()),
            language: None,
        }))
        .unwrap();
    });
    assert_eq!(c.attrs, vec!["meta".to_owned()]);
    assert_eq!(c.blocks, Vec::<std::ops::Range<usize>>::new());
}

#[test]
fn an_update_already_held_changes_nothing() {
    let mut here = three_paragraphs();
    let update = here.encode_diff(&cdoc_crdt::StateVector::empty());
    assert!(here.apply_update(&update, ROOM).unwrap().changes.is_empty());
}

/// A large document and a small update: blocks are found one lookup at a time.
#[test]
fn a_small_update_to_a_large_document_names_its_block() {
    let texts = vec!["xx"; 300];
    let c = changes_in(paragraphs(&texts), |d| {
        d.insert_text(3 * 150 + 2, "!").unwrap();
    });
    assert_eq!(c.blocks, vec![151..152]);
}

/// A large update: blocks are found in one pass over the document; same answers.
#[test]
fn a_large_update_names_every_block_it_touched() {
    let texts = vec!["xx"; 20];
    let c = changes_in(paragraphs(&texts), |d| {
        for k in (1..20).step_by(2).rev() {
            d.insert_text(3 * k + 2, "!").unwrap();
        }
    });
    let expected: Vec<_> = (1..20).step_by(2).map(|k| k + 1..k + 2).collect();
    assert_eq!(c.blocks, expected);
}
