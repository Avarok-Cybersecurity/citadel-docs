//! Tombstone collection on request: after two replicas converge, collecting shrinks the
//! snapshot and changes neither the document nor the selection, editing and syncing go on,
//! and the refusals cdoc-crdt defines reach the caller unchanged.

mod common;

use cdoc_crdt::{GcError, StateVector};
use cdoc_edit::wire::{decode_collected, decode_known, encode_collected, encode_known};
use cdoc_edit::{Command, DeleteUnit, EditError};
use common::{caret, check, editor, gap, paragraphs, peer, select, text};

fn sv(e: &cdoc_edit::Editor<'_>) -> StateVector {
    StateVector::decode(&e.state_vector()).expect("a state vector")
}

#[test]
fn collecting_after_convergence_shrinks_the_snapshot_and_keeps_the_document() {
    let mut a = editor(&paragraphs(&["one two three four five", "six"]), 1);
    let mut b = peer(&a, 2);
    let (from, to) = (gap(&a, 0, 3), gap(&a, 0, 18));
    select(&mut a, from, to);
    let cut = a.apply(Command::DeleteBackward(DeleteUnit::Char)).unwrap();
    b.apply_remote(&cut.update).unwrap();
    let at = gap(&b, 1, 3);
    caret(&mut b, at);
    let typed = b.apply(Command::InsertText(" seven".into())).unwrap();
    a.apply_remote(&typed.update).unwrap();
    assert_eq!(a.model(), b.model());
    let (model, selection) = (a.model().clone(), a.selection());
    let before = a.snapshot().len();
    let collected = a.gc(&[sv(&b)]).unwrap();
    assert!(
        collected.units > 0 && collected.deletes > 0,
        "{collected:?}"
    );
    assert!(a.snapshot().len() < before);
    assert_eq!(a.model(), &model);
    assert_eq!(a.selection(), selection);
    check(&a);
    assert!(
        a.undo().unwrap().is_none(),
        "the history ends at a collection"
    );
    let more = a.apply(Command::InsertText("!".into())).unwrap();
    b.apply_remote(&more.update).unwrap();
    let back = b.apply(Command::InsertText("?".into())).unwrap();
    a.apply_remote(&back.update).unwrap();
    assert_eq!(a.model(), b.model());
    assert_eq!(
        (text(&a, 0), text(&a, 1)),
        ("one! five".into(), "six seven?".into())
    );
    check(&a);
    check(&b);
}

#[test]
fn the_refusals_of_the_crdt_reach_the_caller() {
    let mut a = editor(&paragraphs(&["text"]), 1);
    let mut b = peer(&a, 2);
    assert_eq!(a.gc(&[]), Err(EditError::Gc(GcError::NoPeers)));
    b.apply(Command::InsertText("x".into())).unwrap();
    assert_eq!(
        a.gc(&[sv(&b)]),
        Err(EditError::Gc(GcError::Behind(cdoc_crdt::ClientId(2))))
    );
    a.composition_start().unwrap();
    assert!(matches!(a.gc(&[sv(&a)]), Err(EditError::NotApplicable(_))));
}

#[test]
fn the_gc_records_round_trip_and_refuse_truncation() {
    let a = editor(&paragraphs(&["text"]), 1);
    let known = vec![sv(&a), StateVector::empty()];
    let bytes = encode_known(&known);
    assert_eq!(decode_known(&bytes).unwrap(), known);
    let c = cdoc_crdt::Collected {
        units: 300,
        deletes: 2,
    };
    let out = encode_collected(&c);
    assert_eq!(&out[..4], b"CDG\x01");
    assert_eq!(decode_collected(&out).unwrap(), c);
    for k in 0..bytes.len() {
        assert!(decode_known(&bytes[..k]).is_err(), "known cut at {k}");
    }
    for k in 0..out.len() {
        assert!(decode_collected(&out[..k]).is_err(), "collected cut at {k}");
    }
    assert!(decode_known(&[&bytes[..], &[0]].concat()).is_err());
}

/// A caret anchored after a unit a peer deleted stays where it is when that unit is collected.
#[test]
fn a_caret_on_a_collected_unit_keeps_its_place() {
    let mut a = editor(&paragraphs(&["one two three"]), 1);
    let mut b = peer(&a, 2);
    let at = gap(&a, 0, 7);
    caret(&mut a, at);
    let (from, to) = (gap(&b, 0, 3), gap(&b, 0, 7));
    select(&mut b, from, to);
    let cut = b.apply(Command::DeleteBackward(DeleteUnit::Char)).unwrap();
    a.apply_remote(&cut.update).unwrap();
    let place = a.selection().head.index(a.doc()).unwrap();
    assert!(a.gc(&[sv(&b)]).unwrap().units > 0);
    assert_eq!(a.selection().head.index(a.doc()).unwrap(), place);
    a.apply(Command::InsertText("_".into())).unwrap();
    assert_eq!(text(&a, 0), "one_ three");
    check(&a);
}
