//! Two replicas editing at once: they converge, a selection stays on its text while a peer
//! edits around it, and awareness records draw a peer's caret and selection.

mod common;

use cdoc_crdt::ClientId;
use cdoc_edit::{Awareness, Command, DeleteUnit, EditError, Editor, Selection};
use common::ops::{apply_on_peer, op, refused, Op};
use common::ROOM;
use common::{at, check, editor, fonts, options, peer, select};
use proptest::prelude::*;

fn deliver(to: &mut Editor<'_>, from: &mut Vec<Vec<u8>>) {
    for update in from.drain(..) {
        let r = to.apply_remote(&update, ROOM).expect("applies");
        assert!(r.missing.is_empty(), "updates arrive in order");
    }
}

/// The text between the selection's ends.
fn selected(e: &Editor<'_>) -> String {
    let range = e.selection().range(e.doc()).expect("a range");
    range
        .filter_map(|i| match e.doc().unit_at(i) {
            Some(cdoc_crdt::Unit::Char(c)) => Some(c),
            _ => None,
        })
        .collect()
}

/// Runs `ops` (replica A when the first flag is set, delivering both ways when the last is) on
/// the kitchen sink; the replicas converge and stay valid.
fn converge(ops: &[(bool, Op, bool)]) {
    let mut a = editor(&cdoc_model::test_support::fixtures::kitchen_sink(), 1);
    let mut b = peer(&a, 2);
    let (mut to_b, mut to_a) = (Vec::new(), Vec::new());
    for (k, (on_a, o, sync_now)) in ops.iter().enumerate() {
        if *on_a {
            to_b.extend(apply_on_peer(&mut a, o, k));
        } else {
            to_a.extend(apply_on_peer(&mut b, o, k));
        }
        if *sync_now {
            deliver(&mut b, &mut to_b);
            deliver(&mut a, &mut to_a);
        }
    }
    deliver(&mut b, &mut to_b);
    deliver(&mut a, &mut to_a);
    assert_eq!(a.model(), b.model());
    check(&a);
    check(&b);
}

/// The shrunk case of seed `65f8e951…`: one replica anchors a comment and undoes it while the
/// other, having seen the comment, undoes a replacement that restores text anchored to it.
/// The restored anchoring outlives the comment; it must read as no anchoring on both.
#[test]
fn a_comment_undone_while_its_text_is_restored_leaves_no_anchoring() {
    converge(&[
        (false, Op::Cmd(11, 0, 0), false),
        (false, Op::Cmd(13, 0, 0), true),
        (false, Op::Cmd(13, 0, 0), false),
        (true, Op::Cmd(31, 0, 0), false),
        (false, Op::Select(19556, 27957), false),
        (false, Op::Cmd(1, 0, 1), true),
        (false, Op::Cmd(28, 25055, 12678), true),
        (false, Op::Cmd(32, 55503, 51448), false),
        (false, Op::Undo, false),
        (true, Op::Undo, true),
    ]);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// Both replicas edit (and undo) concurrently, exchanging updates at random moments;
    /// once everything is delivered they hold the same document, and both stay valid.
    #[test]
    fn concurrent_editing_converges(
        ops in proptest::collection::vec((any::<bool>(), op(), any::<bool>()), 1..24),
    ) {
        converge(&ops);
    }

    /// While a peer inserts and deletes everywhere outside it, a selection keeps exactly the
    /// text it covered.
    #[test]
    fn a_selection_keeps_its_text_under_remote_edits(
        edits in proptest::collection::vec((any::<u16>(), 0u8..3), 1..32),
    ) {
        let mut a = editor(&common::paragraphs(&["alpha beta gamma", "delta epsilon"]), 1);
        let mut b = peer(&a, 2);
        let (s, t) = (common::gap(&a, 0, 6), common::gap(&a, 1, 5));
        select(&mut a, s, t);
        let before = selected(&a);
        for (g, kind) in edits {
            let (s, t) = {
                let r = a.selection().range(b.doc()).expect("b holds the text");
                (r.start, r.end)
            };
            let outside = |gap: usize| match kind {
                1 => gap <= s || gap > t,
                _ => gap < s || gap >= t,
            };
            let gap = 1 + usize::from(g) % b.doc().len();
            if cdoc_edit::Position::at(b.doc(), gap).is_none() {
                continue;
            }
            let p = at(&b, gap);
            b.apply(Command::SetSelection(Selection::caret(p))).expect("selects");
            // The caret may have moved to the nearest caret gap; that must be outside too.
            if !outside(b.selection().head.index(b.doc()).expect("a gap")) {
                continue;
            }
            let cmd = match kind {
                0 => Command::InsertText("xy".to_owned()),
                1 => Command::DeleteBackward(DeleteUnit::Char),
                _ => Command::DeleteForward(DeleteUnit::Char),
            };
            match b.apply(cmd) {
                Ok(applied) if !applied.update.is_empty() => {
                    a.apply_remote(&applied.update, ROOM).expect("applies");
                }
                Ok(_) => {}
                Err(err) => prop_assert!(refused(&err), "{:?}", err),
            }
            prop_assert_eq!(selected(&a), before.clone());
        }
        check(&a);
    }
}

#[test]
fn awareness_draws_a_peer_where_it_is() {
    let mut a = editor(&common::paragraphs(&["shared text here"]), 1);
    let mut b = peer(&a, 2);
    let (s, t) = (common::gap(&a, 0, 7), common::gap(&a, 0, 11));
    select(&mut a, s, t);
    let record = a.awareness();
    let decoded = Awareness::decode(&record).expect("decodes");
    assert_eq!(decoded.client, cdoc_crdt::ClientId(1));
    assert_eq!(decoded.selection, a.selection());
    let view = b.peer_view(&record).expect("a view");
    assert_eq!(view.client, cdoc_crdt::ClientId(1));
    assert_eq!(
        view.caret,
        Some(b.caret_rect(a.selection().head).expect("a caret"))
    );
    assert_eq!(view.rects.len(), 1);
    let typed = b
        .apply(Command::InsertText("new ".to_owned()))
        .expect("types");
    let moved = b.peer_view(&record).expect("a view");
    assert!(
        moved.rects[0].1.x > view.rects[0].1.x,
        "the peer's selection moved with its text"
    );
    a.apply_remote(&typed.update, ROOM).expect("applies");
    assert_eq!(
        a.peer_view(&b.awareness()).expect("a view").rects,
        Vec::new()
    );
    assert!(Awareness::decode(&record[..record.len() - 1]).is_err());
    assert!(Awareness::decode(b"XXXX").is_err());
}

/// Two replicas opened blank apart are one document: one types, the other takes the update
/// and draws its caret. Client 0 authors the blank's base and no editor may write as it.
#[test]
fn blanks_opened_apart_converge_and_see_each_other() {
    let open_blank = |c| Editor::blank(ClientId(c), fonts(), options()).expect("opens");
    let (mut a, mut b) = (open_blank(1), open_blank(2));
    assert_eq!(a.snapshot(), b.snapshot());
    let typed = a.apply(Command::InsertText("hello".into())).unwrap();
    b.apply_remote(&typed.update, ROOM).unwrap();
    assert_eq!(a.model(), b.model());
    check(&b);
    let view = b.peer_view(&a.awareness()).expect("a view");
    assert!(
        view.caret.is_some(),
        "b holds the text a's caret is anchored to"
    );
    let reserved = |r: Result<cdoc_edit::Editor<'_>, EditError>| {
        assert_eq!(
            r.err(),
            Some(EditError::Bridge(cdoc_crdt::BridgeError::ReservedClient))
        );
    };
    reserved(Editor::blank(ClientId::BLANK, fonts(), options()));
    reserved(Editor::open(
        &a.snapshot(),
        ClientId::BLANK,
        fonts(),
        options(),
    ));
    reserved(Editor::from_document(
        &a.model().clone(),
        ClientId::BLANK,
        fonts(),
        options(),
    ));
}

/// A peer undoes inserting a footnote whose reference this replica had replaced, and this
/// replica then undoes the replacement: the reference must not come back without its body.
#[test]
fn an_undone_footnote_stays_gone_when_a_peer_restores_its_reference() {
    use common::ops::Op::{Cmd, Select, Undo};
    let ops = [
        (true, Select(3815, 1295), false),
        (true, Cmd(27, 0, 0), false),
        (true, Select(14707, 55), false),
        (false, Select(4920, 0), false),
        (false, Cmd(28, 0, 0), true),
        (true, Cmd(27, 0, 0), false),
        (false, Undo, false),
        (true, Undo, false),
    ];
    let mut a = editor(&cdoc_model::test_support::fixtures::kitchen_sink(), 1);
    let mut b = peer(&a, 2);
    let (mut to_b, mut to_a) = (Vec::new(), Vec::new());
    for (k, (on_a, o, sync_now)) in ops.iter().enumerate() {
        if *on_a {
            to_b.extend(apply_on_peer(&mut a, o, k));
        } else {
            to_a.extend(apply_on_peer(&mut b, o, k));
        }
        if *sync_now {
            deliver(&mut b, &mut to_b);
            deliver(&mut a, &mut to_a);
        }
    }
    deliver(&mut b, &mut to_b);
    deliver(&mut a, &mut to_a);
    assert_eq!(a.model(), b.model());
    check(&a);
    check(&b);
}
