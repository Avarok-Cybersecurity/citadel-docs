//! Undoing a footnote's insertion removes its reference and keeps its body, so redo, or a peer
//! restoring the reference concurrently, finds the body it names.

mod common;

use cdoc_edit::Command;
use cdoc_model::{FootnoteId, InlineContent};
use common::{caret, check, editor, gap, paragraphs};

fn bodies(e: &cdoc_edit::Editor<'_>) -> Vec<FootnoteId> {
    e.model().footnotes.iter().map(|f| f.id).collect()
}

fn references(e: &cdoc_edit::Editor<'_>) -> usize {
    e.model()
        .inlines()
        .filter(|i| matches!(i.content, InlineContent::FootnoteRef { .. }))
        .count()
}

#[test]
fn undoing_a_footnote_keeps_its_body_for_redo() {
    let mut e = editor(&paragraphs(&["one two"]), 1);
    let at = gap(&e, 0, 3);
    caret(&mut e, at);
    e.apply(Command::InsertFootnote(Some(FootnoteId(7))))
        .unwrap();
    assert_eq!((references(&e), bodies(&e)), (1, vec![FootnoteId(7)]));
    e.undo().unwrap().expect("an undo step");
    assert_eq!((references(&e), bodies(&e)), (0, vec![FootnoteId(7)]));
    check(&e);
    e.redo().unwrap().expect("a redo step");
    assert_eq!((references(&e), bodies(&e)), (1, vec![FootnoteId(7)]));
    check(&e);
}

#[test]
fn undoing_typing_in_a_body_still_removes_the_typing() {
    let mut e = editor(&paragraphs(&["one"]), 1);
    let at = gap(&e, 0, 3);
    caret(&mut e, at);
    e.apply(Command::InsertFootnote(Some(FootnoteId(7))))
        .unwrap();
    e.break_undo_group();
    let body = e.doc().len() - 1;
    caret(&mut e, body);
    e.apply(Command::InsertText("zebra".into())).unwrap();
    let text = |e: &cdoc_edit::Editor<'_>| format!("{:?}", e.model().footnotes[0].blocks);
    assert!(text(&e).contains("zebra"));
    e.undo().unwrap().expect("an undo step");
    assert!(!text(&e).contains("zebra"));
    check(&e);
}
