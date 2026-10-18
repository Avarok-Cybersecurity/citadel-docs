//! Deleting with Word's results: paragraph joins that keep the right properties, numbering
//! removed before a join, word deletion, and selections across paragraphs.

mod common;

use cdoc_edit::{Command, DeleteUnit};
use cdoc_model::{Block, NumId, NumberingRef};
use common::*;

#[test]
fn backspace_at_a_paragraph_start_joins_it_to_the_one_before() {
    let mut d = paragraphs(&["one", "two"]);
    let Block::Paragraph(p) = &mut d.sections[0].blocks[1] else {
        unreachable!()
    };
    p.properties.alignment = Some(cdoc_model::Alignment::Right);
    let mut e = editor(&d, 1);
    let g = gap(&e, 1, 0);
    caret(&mut e, g);
    e.apply(Command::DeleteBackward(DeleteUnit::Char)).unwrap();
    check(&e);
    assert_eq!(text(&e, 0), "onetwo");
    assert_eq!(
        props(&e, 0).alignment,
        None,
        "the upper paragraph's properties win"
    );
    assert_eq!(e.block_offset(e.selection().head).unwrap().offset, 3);
}

#[test]
fn backspace_into_an_empty_paragraph_keeps_the_lower_ones_properties() {
    let mut d = paragraphs(&["", "two"]);
    let Block::Paragraph(p) = &mut d.sections[0].blocks[1] else {
        unreachable!()
    };
    p.properties.alignment = Some(cdoc_model::Alignment::Right);
    let mut e = editor(&d, 1);
    let g = gap(&e, 1, 0);
    caret(&mut e, g);
    e.apply(Command::DeleteBackward(DeleteUnit::Char)).unwrap();
    check(&e);
    assert_eq!(e.model().sections[0].blocks.len(), 1);
    assert_eq!(props(&e, 0).alignment, Some(cdoc_model::Alignment::Right));
}

#[test]
fn backspace_at_a_list_item_start_removes_the_numbering_first() {
    let mut d = paragraphs(&["a", "item"]);
    let Block::Paragraph(p) = &mut d.sections[0].blocks[1] else {
        unreachable!()
    };
    p.properties.numbering = Some(NumberingRef {
        num_id: NumId(1),
        level: 0,
    });
    d.numbering = cdoc_model::test_support::fixtures::kitchen_sink().numbering;
    let mut e = editor(&d, 1);
    let g = gap(&e, 1, 0);
    caret(&mut e, g);
    e.apply(Command::DeleteBackward(DeleteUnit::Char)).unwrap();
    check(&e);
    assert_eq!(
        (props(&e, 1).numbering, text(&e, 1)),
        (None, "item".to_owned())
    );
}

#[test]
fn delete_at_a_paragraph_end_pulls_the_next_one_up() {
    let mut e = editor(&paragraphs(&["one", "two"]), 1);
    let g = gap(&e, 0, 3);
    caret(&mut e, g);
    e.apply(Command::DeleteForward(DeleteUnit::Char)).unwrap();
    check(&e);
    assert_eq!(text(&e, 0), "onetwo");
}

#[test]
fn word_deletion_goes_to_the_word_boundary() {
    let mut e = editor(&paragraphs(&["alpha beta  gamma"]), 1);
    let g = gap(&e, 0, 12);
    caret(&mut e, g);
    e.apply(Command::DeleteBackward(DeleteUnit::Word)).unwrap();
    assert_eq!(text(&e, 0), "alpha gamma");
    e.apply(Command::DeleteForward(DeleteUnit::Word)).unwrap();
    assert_eq!(text(&e, 0), "alpha ");
    check(&e);
}

#[test]
fn a_selection_across_paragraphs_deletes_into_one() {
    let mut e = editor(&paragraphs(&["one", "two", "three"]), 1);
    let (a, b) = (gap(&e, 0, 1), gap(&e, 2, 2));
    select(&mut e, a, b);
    e.apply(Command::InsertText("X".into())).unwrap();
    check(&e);
    assert_eq!(e.model().sections[0].blocks.len(), 1);
    assert_eq!(text(&e, 0), "oXree");
}

/// A paragraph that was empty when the structure last changed, typed into since: Delete at its
/// end pulls the next paragraph up and keeps the typed text in its own paragraph.
#[test]
fn delete_after_typing_into_an_empty_paragraph_joins_the_next_one() {
    let mut e = editor(&paragraphs(&["", "next"]), 1);
    let g = gap(&e, 0, 0);
    caret(&mut e, g);
    e.apply(Command::InsertText("a".into())).unwrap();
    e.apply(Command::DeleteForward(DeleteUnit::Char)).unwrap();
    check(&e);
    assert_eq!(e.model().sections[0].blocks.len(), 1);
    assert_eq!(text(&e, 0), "anext");
    e.apply(Command::InsertText("b".into())).unwrap();
    assert_eq!(
        text(&e, 0),
        "abnext",
        "the caret stays after the typed text"
    );
}
