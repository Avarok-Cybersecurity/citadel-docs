//! Undo and redo: exact inverses as models, typing groups without a clock, and undo of own
//! edits that leaves concurrent remote edits alone.

mod common;

use cdoc_edit::Command;
use common::ops::{apply, command, op};
use common::*;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, ..ProptestConfig::default() })]

    /// For any history and any command: undo gives back the model before it, redo the one
    /// after it.
    #[test]
    fn undo_and_redo_are_exact(
        prefix in proptest::collection::vec(op(), 0..12),
        (c, a, b) in (0u8..34, any::<u16>(), any::<u16>()),
    ) {
        let mut e = editor(&cdoc_model::test_support::fixtures::kitchen_sink(), 1);
        for (k, o) in prefix.iter().enumerate() {
            let _ = apply(&mut e, o, k);
        }
        e.break_undo_group();
        let before = e.model().clone();
        let cmd = command(&e, c, a, b, 99);
        let Ok(applied) = e.apply(cmd) else { return Ok(()) };
        if applied.update.is_empty() {
            return Ok(());
        }
        let after = e.model().clone();
        prop_assert!(e.undo().unwrap().is_some());
        check(&e);
        prop_assert_eq!(e.model(), &before);
        prop_assert!(e.redo().unwrap().is_some());
        check(&e);
        prop_assert_eq!(e.model(), &after);
    }
}

/// Typing over text that carries several comments, then undoing, gives the comments back
/// in their order, not in the order the undo happened to anchor them.
#[test]
fn undo_keeps_the_order_of_overlapping_comments() {
    use common::ops::Op;
    let mut e = editor(&cdoc_model::test_support::fixtures::kitchen_sink(), 1);
    let prefix = [
        Op::Cmd(13, 0, 0),
        Op::Cmd(13, 0, 0),
        Op::Select(45264, 90),
        Op::Cmd(31, 0, 0),
    ];
    for (k, o) in prefix.iter().enumerate() {
        let _ = apply(&mut e, o, k);
    }
    e.break_undo_group();
    let before = e.model().clone();
    e.apply(Command::InsertText("a".into())).unwrap();
    e.undo().unwrap();
    assert_eq!(e.model(), &before);
    check(&e);
}

#[test]
fn typing_groups_by_word_and_breaks_on_request() {
    let mut e = editor(&paragraphs(&[""]), 1);
    type_chars(&mut e, "hello world");
    e.undo().unwrap();
    assert_eq!(text(&e, 0), "hello ");
    e.undo().unwrap();
    assert_eq!(text(&e, 0), "hello");
    e.undo().unwrap();
    assert_eq!(text(&e, 0), "");
    assert!(e.undo().unwrap().is_none());
    type_chars(&mut e, "ab");
    e.break_undo_group();
    type_chars(&mut e, "cd");
    e.undo().unwrap();
    assert_eq!(text(&e, 0), "ab");
    e.redo().unwrap();
    assert_eq!(text(&e, 0), "abcd");
    check(&e);
}

#[test]
fn a_caret_move_ends_the_typing_group() {
    let mut e = editor(&paragraphs(&["x"]), 1);
    type_chars(&mut e, "ab");
    e.move_caret(
        cdoc_edit::Granularity::Char,
        cdoc_edit::Direction::Backward,
        false,
    )
    .unwrap();
    type_chars(&mut e, "cd");
    e.undo().unwrap();
    assert_eq!(text(&e, 0), "abx");
}

#[test]
fn remote_updates_never_enter_the_history() {
    let mut a = editor(&paragraphs(&["shared"]), 1);
    let mut b = peer(&a, 2);
    let update = b
        .apply(Command::InsertText("remote ".into()))
        .unwrap()
        .update;
    a.apply_remote(&update).unwrap();
    assert!(a.undo().unwrap().is_none());
    assert_eq!(text(&a, 0), "remote shared");
}

#[test]
fn undo_takes_back_own_text_and_keeps_concurrent_remote_text() {
    let mut a = editor(&paragraphs(&["base"]), 1);
    let mut b = peer(&a, 2);
    let mine = a.apply(Command::InsertText("mine ".into())).unwrap().update;
    b.apply_remote(&mine).unwrap();
    let g = gap(&b, 0, 9);
    caret(&mut b, g);
    let theirs = b
        .apply(Command::InsertText(" theirs".into()))
        .unwrap()
        .update;
    a.apply_remote(&theirs).unwrap();
    let undo = a.undo().unwrap().unwrap().update;
    assert_eq!(text(&a, 0), "base theirs");
    b.apply_remote(&undo).unwrap();
    assert_eq!(text(&b, 0), "base theirs");
    check(&a);
    check(&b);
}

#[test]
fn undoing_a_deletion_puts_the_text_back_where_it_was_after_remote_edits() {
    let mut a = editor(&paragraphs(&["one two three"]), 1);
    let mut b = peer(&a, 2);
    let (s, t) = (gap(&a, 0, 4), gap(&a, 0, 8));
    select(&mut a, s, t);
    let del = a
        .apply(Command::DeleteBackward(cdoc_edit::DeleteUnit::Char))
        .unwrap()
        .update;
    let ins = b.apply(Command::InsertText("X".into())).unwrap().update;
    a.apply_remote(&ins).unwrap();
    b.apply_remote(&del).unwrap();
    a.undo().unwrap();
    assert_eq!(text(&a, 0), "Xone two three");
    check(&a);
}

#[test]
fn undo_restores_the_selection() {
    let mut e = editor(&paragraphs(&["one two"]), 1);
    let (s, t) = (gap(&e, 0, 0), gap(&e, 0, 3));
    select(&mut e, s, t);
    let place = |e: &cdoc_edit::Editor<'_>| {
        let s = e.selection();
        (
            e.block_offset(s.anchor).unwrap(),
            e.block_offset(s.head).unwrap(),
        )
    };
    let before = place(&e);
    e.apply(Command::InsertText("1".into())).unwrap();
    e.undo().unwrap();
    // The restored text is a copy with new ids, so the selection is compared by place.
    assert_eq!(place(&e), before);
    assert_eq!(text(&e, 0), "one two");
}

/// Undoing my comment takes back my comment only: one a peer added meanwhile stays.
#[test]
fn undoing_a_comment_keeps_a_concurrent_comment() {
    let comment = |id: &str| cdoc_model::Comment {
        id: cdoc_model::CommentId::new(id),
        author: "A".to_owned(),
        date: "2026-10-17T00:00:00Z".to_owned(),
        text: "note".to_owned(),
        resolved: false,
        parent: None,
    };
    let mut a = editor(&paragraphs(&["alpha beta"]), 1);
    let mut b = peer(&a, 2);
    let mine = a.apply(Command::InsertComment(comment("mine"))).unwrap();
    let g = gap(&b, 0, 8);
    caret(&mut b, g);
    let theirs = b.apply(Command::InsertComment(comment("theirs"))).unwrap();
    a.apply_remote(&theirs.update).unwrap();
    b.apply_remote(&mine.update).unwrap();
    let undo = a.undo().unwrap().unwrap().update;
    b.apply_remote(&undo).unwrap();
    for e in [&a, &b] {
        let ids: Vec<&str> = e.model().comments.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["theirs"]);
        check(e);
    }
}

/// A comment over empty paragraphs shows on nothing, yet covers what is inserted there later:
/// undoing it must clear it there too.
#[test]
fn undoing_a_comment_over_empty_paragraphs_leaves_no_mark_behind() {
    let mut e = editor(&paragraphs(&["", "", "", "x"]), 1);
    let (s, t) = (gap(&e, 0, 0), gap(&e, 2, 0));
    select(&mut e, s, t);
    let note = cdoc_model::Comment {
        id: cdoc_model::CommentId::new("c"),
        author: "A".to_owned(),
        date: "2026-10-17T00:00:00Z".to_owned(),
        text: "note".to_owned(),
        resolved: false,
        parent: None,
    };
    e.apply(Command::InsertComment(note)).unwrap();
    e.undo().unwrap();
    let at = gap(&e, 1, 0);
    caret(&mut e, at);
    e.apply(Command::InsertImage {
        content_id: cdoc_model::ContentId::new("img"),
        size: Some((cdoc_model::Length(100), cdoc_model::Length(100))),
    })
    .unwrap();
    check(&e);
}
