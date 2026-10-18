//! Composition is a local overlay: provisional text never reaches the document, the history or
//! peers; commit inserts it as one typing step; cancel leaves nothing behind.

mod common;

use cdoc_edit::Command;
use common::ROOM;
use common::{check, editor, gap, paragraphs, peer, select, text};

#[test]
fn provisional_text_stays_out_of_the_document_until_commit() {
    let mut e = editor(&paragraphs(&["hello world"]), 1);
    let g = gap(&e, 0, 6);
    common::caret(&mut e, g);
    let sv = e.state_vector();
    let started = e.composition_start().expect("starts");
    assert_eq!(started.update, Vec::<u8>::new());
    e.composition_update("にほ", 2).expect("updates");
    e.composition_update("日本", 9).expect("updates");
    let c = e.composition().expect("open");
    assert_eq!(
        (c.text.as_str(), c.caret),
        ("日本", 2),
        "the caret is clamped to the text"
    );
    assert_eq!(text(&e, 0), "hello world");
    assert_eq!(e.state_vector(), sv, "nothing reached the replica");
    let done = e.composition_commit().expect("commits");
    assert_ne!(done.update, Vec::<u8>::new());
    assert_eq!(text(&e, 0), "hello 日本world");
    assert!(e.composition().is_none());
    e.undo().expect("undoes");
    assert_eq!(text(&e, 0), "hello world", "the commit is one undo step");
    check(&e);
}

#[test]
fn cancel_leaves_nothing_and_starting_replaces_the_selection() {
    let mut e = editor(&paragraphs(&["hello world"]), 1);
    let (s, t) = (gap(&e, 0, 0), gap(&e, 0, 6));
    select(&mut e, s, t);
    let started = e.composition_start().expect("starts");
    assert!(!started.update.is_empty(), "the selection was deleted");
    assert_eq!(text(&e, 0), "world");
    e.composition_update("x", 1).expect("updates");
    e.composition_cancel();
    assert!(e.composition().is_none());
    assert_eq!(text(&e, 0), "world");
    assert!(
        e.composition_update("y", 1).is_err(),
        "no composition is open"
    );
    assert!(e.composition_commit().is_err());
}

#[test]
fn a_composition_follows_its_text_through_remote_edits() {
    let mut a = editor(&paragraphs(&["abc"]), 1);
    let mut b = peer(&a, 2);
    let g = gap(&a, 0, 2);
    common::caret(&mut a, g);
    a.composition_start().expect("starts");
    a.composition_update("Z", 1).expect("updates");
    let typed = b
        .apply(Command::InsertText("123".to_owned()))
        .expect("types");
    a.apply_remote(&typed.update, ROOM).expect("applies");
    let done = a.composition_commit().expect("commits");
    assert_eq!(text(&a, 0), "123abZc");
    b.apply_remote(&done.update, ROOM).expect("applies");
    assert_eq!(text(&b, 0), "123abZc");
}
