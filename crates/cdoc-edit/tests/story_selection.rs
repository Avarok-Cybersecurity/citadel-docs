//! A selection never spans two stories: the head is kept in the anchor's story, and caret
//! moves stay in the story they start in.

mod common;

use cdoc_edit::{Command, Direction, Editor, Granularity, Selection, Story};
use cdoc_model::{FootnoteId, InlineContent};
use common::{at, gap, noted, type_chars};

fn index(e: &Editor<'_>, p: cdoc_edit::Position) -> usize {
    p.index(e.doc()).expect("a gap")
}

#[test]
fn a_selection_is_clamped_to_its_anchors_story() {
    let mut e = noted();
    e.apply(Command::GoToFootnote(FootnoteId(7)))
        .expect("enters");
    type_chars(&mut e, "note text");
    let (first, last) = (
        index(&e, e.selection().head) - 9,
        index(&e, e.selection().head),
    );
    let anchor = at(&e, first + 2);
    let body = gap(&e, 0, 1);
    e.apply(Command::SetSelection(Selection {
        anchor,
        head: at(&e, body),
    }))
    .expect("selects");
    assert_eq!(
        index(&e, e.selection().head),
        first,
        "clamped at the note's start"
    );
    for (g, dir) in [
        (Granularity::Document, Direction::Forward),
        (Granularity::Line, Direction::Forward),
        (Granularity::Paragraph, Direction::Forward),
        (Granularity::Char, Direction::Forward),
    ] {
        e.move_caret(g, dir, true).expect("moves");
        assert_eq!(index(&e, e.selection().head), last, "{g:?}");
    }
    e.move_caret(Granularity::Document, Direction::Backward, false)
        .expect("moves");
    assert_eq!(index(&e, e.selection().head), first);
    e.move_caret(Granularity::Char, Direction::Backward, false)
        .expect("moves");
    assert_eq!(index(&e, e.selection().head), first, "stays at the edge");
    e.apply(Command::SelectAll).expect("selects");
    let s = e.selection();
    assert_eq!((index(&e, s.anchor), index(&e, s.head)), (first, last));
    assert!(e.selection_text().unwrap().starts_with("note text"));
}

#[test]
fn the_body_never_moves_into_a_footnote() {
    let mut e = noted();
    e.move_caret(Granularity::Document, Direction::Forward, true)
        .expect("moves");
    assert_eq!(e.story().unwrap(), Story::Body);
    e.move_caret(Granularity::Char, Direction::Forward, true)
        .expect("moves");
    assert_eq!(e.story().unwrap(), Story::Body);
    let refs = e
        .model()
        .inlines()
        .filter(|i| matches!(i.content, InlineContent::FootnoteRef { .. }))
        .count();
    assert_eq!(refs, 1);
}
