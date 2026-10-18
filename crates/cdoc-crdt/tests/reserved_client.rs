//! Client 0 writes only the blank document's base. A replica takes the base from a diff (a
//! replica that did not open blank needs it), but an operation claiming client 0 that is not
//! the base, past it or in its place, is refused, and is not buffered to fail again. A base
//! the document deleted travels as deleted runs, and those are the base too. A snapshot is held
//! to the same rule when it loads.

mod common;

use cdoc_crdt::{blank, to_model, ClientId, DecodeError, Doc, DocAttr, StateVector};
use common::ROOM;

const REFUSED: DecodeError = DecodeError::Inconsistent("client 0 writes only the blank document");

#[test]
fn the_blank_base_arrives_in_a_diff_to_an_empty_replica() {
    let b = blank(ClientId(1)).unwrap();
    let mut empty = Doc::new(ClientId(5));
    empty
        .apply_update(&b.encode_diff(&StateVector::empty()), ROOM)
        .unwrap();
    assert_eq!(to_model(&empty).unwrap(), to_model(&b).unwrap());
}

#[test]
fn a_deleted_base_arrives_in_a_diff_to_an_empty_replica() {
    let mut b = blank(ClientId(1)).unwrap();
    b.insert_text(0, "hello").unwrap();
    b.delete(0, b.visible_text().chars().count()).unwrap();
    let mut empty = Doc::new(ClientId(5));
    empty
        .apply_update(&b.encode_diff(&StateVector::empty()), ROOM)
        .unwrap();
    assert_eq!(empty.state_vector(), b.state_vector());
    assert_eq!(empty.visible_text(), b.visible_text());
}

#[test]
fn a_deleted_run_of_client_0_past_the_base_is_refused() {
    let mut victim = blank(ClientId(1)).unwrap();
    let mut forger = Doc::decode_snapshot(&victim.encode_snapshot(), ClientId::BLANK).unwrap();
    let sv = forger.state_vector();
    forger.insert_text(1, "planted").unwrap();
    forger.delete(1, 7).unwrap();
    let before = victim.encode_snapshot();
    assert_eq!(
        victim.apply_update(&forger.encode_diff(&sv), ROOM),
        Err(REFUSED)
    );
    assert_eq!(victim.encode_snapshot(), before);
    assert!(!victim.has_pending());
}

#[test]
fn an_operation_of_client_0_past_the_base_is_refused() {
    let mut victim = blank(ClientId(1)).unwrap();
    let mut forger = Doc::decode_snapshot(&victim.encode_snapshot(), ClientId::BLANK).unwrap();
    let sv = forger.state_vector();
    forger.insert_text(1, "planted").unwrap();
    let before = victim.encode_snapshot();
    assert_eq!(
        victim.apply_update(&forger.encode_diff(&sv), ROOM),
        Err(REFUSED)
    );
    assert_eq!(victim.encode_snapshot(), before);
    assert!(!victim.has_pending());
}

#[test]
fn another_base_under_client_0_is_refused() {
    let mut forger = Doc::new(ClientId::BLANK);
    forger.insert_text(0, "planted").unwrap();
    let mut empty = Doc::new(ClientId(5));
    assert_eq!(
        empty.apply_update(&forger.encode_diff(&StateVector::empty()), ROOM),
        Err(REFUSED)
    );
    assert!(!empty.has_pending());
    assert_eq!(empty.visible_text(), "");
}

#[test]
fn a_snapshot_whose_base_was_deleted_loads_collected_or_not() {
    let mut b = blank(ClientId(1)).unwrap();
    b.insert_text(0, "hello").unwrap();
    b.delete(0, b.visible_text().chars().count()).unwrap();
    let deleted = b.encode_snapshot();
    let sv = b.state_vector();
    b.gc(&[sv]).unwrap();
    for bytes in [deleted, b.encode_snapshot()] {
        let loaded = Doc::decode_snapshot(&bytes, ClientId(5)).unwrap();
        assert_eq!(loaded.encode_snapshot(), bytes);
    }
}

#[test]
fn a_snapshot_with_client_0_past_the_base_is_refused() {
    let victim = blank(ClientId(1)).unwrap();
    let mut forger = Doc::decode_snapshot(&victim.encode_snapshot(), ClientId::BLANK).unwrap();
    forger.insert_text(1, "planted").unwrap();
    let planted = forger.encode_snapshot();
    forger.delete(1, 7).unwrap();
    for forged in [planted, forger.encode_snapshot()] {
        assert_eq!(
            Doc::decode_snapshot(&forged, ClientId(5)).err(),
            Some(REFUSED)
        );
    }
    // A replica holding what was refused cannot collect either: collection reloads.
    let sv = forger.state_vector();
    assert!(forger.gc(&[sv]).is_err());
}

#[test]
fn a_snapshot_with_another_base_under_client_0_is_refused() {
    let mut items = Doc::new(ClientId::BLANK);
    items.insert_text(0, "planted").unwrap();
    let meta = to_model(&blank(ClientId(1)).unwrap()).unwrap().meta;
    let mut write = Doc::new(ClientId::BLANK);
    write.set_attr(&DocAttr::Meta(meta)).unwrap();
    for forged in [items, write] {
        assert_eq!(
            Doc::decode_snapshot(&forged.encode_snapshot(), ClientId(5)).err(),
            Some(REFUSED)
        );
    }
}
