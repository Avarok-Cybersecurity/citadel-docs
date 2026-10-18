//! Text editing and sync between replicas: the sequence CRDT on its own.

mod common;

use cdoc_crdt::{ClientId, Doc, EditError, StateVector};
use common::ROOM;

fn doc(client: u64, text: &str) -> Doc {
    let mut d = Doc::new(ClientId(client));
    if !text.is_empty() {
        d.insert_text(0, text).expect("insert");
    }
    d
}

fn sync(from: &Doc, to: &mut Doc) {
    let update = from.encode_diff(&to.state_vector());
    let applied = to.apply_update(&update, ROOM).expect("apply");
    assert_eq!(applied.missing, Vec::<cdoc_crdt::Id>::new());
}

#[test]
fn local_inserts_and_deletes_edit_the_visible_text() {
    let mut d = doc(1, "hello world");
    d.insert_text(5, ",").unwrap();
    d.delete(0, 1).unwrap();
    d.insert_text(0, "J").unwrap();
    d.insert_text(d.len(), "!").unwrap();
    assert_eq!(d.visible_text(), "Jello, world!");
    assert_eq!(d.len(), 13);
}

#[test]
fn multibyte_text_splits_on_character_boundaries() {
    let mut d = doc(1, "é😀a");
    d.insert_text(1, "x").unwrap();
    d.delete(2, 1).unwrap();
    assert_eq!(d.visible_text(), "éxa");
}

#[test]
fn edits_outside_the_document_are_errors() {
    let mut d = doc(1, "abc");
    assert_eq!(
        d.insert_text(4, "x"),
        Err(EditError::OutOfRange { index: 4, len: 3 })
    );
    assert_eq!(
        d.delete(2, 2),
        Err(EditError::OutOfRange { index: 4, len: 3 })
    );
    assert_eq!(d.delete(1, 0), Err(EditError::Empty));
    assert_eq!(d.insert_text(0, ""), Err(EditError::Empty));
}

#[test]
fn a_diff_brings_a_replica_up_to_date() {
    let a = doc(1, "shared text");
    let mut b = Doc::new(ClientId(2));
    sync(&a, &mut b);
    assert_eq!(b.visible_text(), "shared text");
    assert_eq!(b.state_vector(), a.state_vector());
}

#[test]
fn concurrent_delete_and_insert_both_apply() {
    let base = doc(9, "abcdef");
    let mut a = Doc::new(ClientId(1));
    let mut b = Doc::new(ClientId(2));
    sync(&base, &mut a);
    sync(&base, &mut b);
    a.delete(1, 4).unwrap();
    b.insert_text(3, "Z").unwrap();
    sync(&a, &mut b);
    sync(&b, &mut a);
    assert_eq!(a.visible_text(), "aZf");
    assert_eq!(b.visible_text(), "aZf");
}

#[test]
fn applying_an_update_twice_changes_nothing() {
    let mut a = doc(1, "abc");
    a.delete(1, 1).unwrap();
    let update = a.encode_diff(&StateVector::empty());
    let mut b = Doc::new(ClientId(2));
    b.apply_update(&update, ROOM).unwrap();
    let once = b.encode_snapshot();
    let again = b.apply_update(&update, ROOM).unwrap();
    assert!(again.changes.is_empty());
    assert_eq!(b.encode_snapshot(), once);
    assert_eq!(b.visible_text(), "ac");
}

#[test]
fn an_update_ahead_of_its_dependencies_waits_for_them() {
    let mut a = doc(1, "abc");
    let v1 = a.state_vector();
    let first = a.encode_diff(&StateVector::empty());
    a.insert_text(3, "def").unwrap();
    a.delete(0, 1).unwrap();
    let second = a.encode_diff(&v1);
    let mut b = Doc::new(ClientId(2));
    let applied = b.apply_update(&second, ROOM).unwrap();
    assert_eq!(applied.missing, vec![cdoc_crdt::Id::new(ClientId(1), 0)]);
    assert!(b.has_pending());
    assert_eq!(b.visible_text(), "");
    let applied = b.apply_update(&first, ROOM).unwrap();
    assert_eq!(applied.missing, Vec::<cdoc_crdt::Id>::new());
    assert!(!b.has_pending());
    assert_eq!(b.visible_text(), "bcdef");
}

#[test]
fn a_snapshot_reloads_to_the_same_state() {
    let mut a = doc(1, "the quick brown fox");
    a.insert_text(4, "very ").unwrap();
    a.delete(10, 6).unwrap();
    let snap = a.encode_snapshot();
    let b = Doc::decode_snapshot(&snap, ClientId(5)).unwrap();
    assert_eq!(b.visible_text(), a.visible_text());
    assert_eq!(b.state_vector(), a.state_vector());
    assert_eq!(b.encode_snapshot(), snap);
    assert_eq!(
        cdoc_crdt::snapshot_version(&snap).unwrap(),
        a.state_vector()
    );
}

#[test]
fn a_reloaded_replica_keeps_syncing() {
    let mut a = doc(1, "base");
    let mut b = Doc::decode_snapshot(&a.encode_snapshot(), ClientId(2)).unwrap();
    b.insert_text(4, " line").unwrap();
    a.insert_text(0, ">").unwrap();
    sync(&b, &mut a);
    sync(&a, &mut b);
    assert_eq!(a.visible_text(), ">base line");
    assert_eq!(a.visible_text(), b.visible_text());
}

#[test]
fn the_diff_between_two_snapshots_replays_the_edits_between_them() {
    let mut a = doc(1, "version one");
    let old = a.encode_snapshot();
    a.insert_text(8, "number ").unwrap();
    a.delete(0, 8).unwrap();
    let new = a.encode_snapshot();
    let diff = cdoc_crdt::diff_snapshots(&old, &new).unwrap();
    let mut b = Doc::decode_snapshot(&old, ClientId(2)).unwrap();
    b.apply_update(&diff, ROOM).unwrap();
    assert_eq!(b.encode_snapshot(), new);
    assert!(cdoc_crdt::diff_snapshots(&new, &old).is_err());
}

#[test]
fn state_vectors_round_trip() {
    let mut a = doc(7, "x");
    a.insert_text(0, "yz").unwrap();
    let sv = a.state_vector();
    assert_eq!(StateVector::decode(&sv.encode()).unwrap(), sv);
    assert_eq!(StateVector::decode(&a.encode_state_vector()).unwrap(), sv);
}

#[test]
fn a_contradictory_update_is_an_error_and_keeps_what_waits() {
    // Client 5's clock 0 is an attribute write, not an item.
    let mut x = Doc::new(ClientId(5));
    x.set_attr(&cdoc_crdt::DocAttr::Meta(cdoc_model::DocumentMeta {
        title: None,
        language: None,
    }))
    .unwrap();
    let mut b = Doc::new(ClientId(2));
    b.apply_update(&x.encode_diff(&StateVector::empty()), ROOM)
        .unwrap();
    // Something waiting for its dependencies.
    let mut y = doc(1, "abc");
    let first = y.encode_diff(&StateVector::empty());
    let v1 = y.state_vector();
    y.insert_text(1, "Z").unwrap();
    b.apply_update(&y.encode_diff(&v1), ROOM).unwrap();
    assert!(b.has_pending());
    // A mark anchored on that attribute write: header, no GC floor, one client (5) from
    // clock 1, one op: MARK, lamport 1, Before (5, 0), End, key Bold, remove.
    let bad = [
        b"CDC\x02\x00".as_slice(),
        &[0, 1, 5, 1, 1, 3, 1, 2, 5, 0, 1, 3, 0],
    ]
    .concat();
    assert_eq!(
        b.apply_update(&bad, ROOM),
        Err(cdoc_crdt::DecodeError::Inconsistent(
            "a mark anchor is not an item"
        ))
    );
    assert!(b.has_pending());
    b.apply_update(&first, ROOM).unwrap();
    assert!(!b.has_pending());
    assert_eq!(b.visible_text(), "aZbc");
}
