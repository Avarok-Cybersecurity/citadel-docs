//! Garbage collection of acknowledged tombstones: a collected snapshot decodes and shows the
//! same document, peers at or beyond the frontier keep converging with the collected replica,
//! and a peer behind it is refused with a typed error and left unchanged.

use cdoc_crdt::{from_model, to_model, ClientId, DecodeError, Doc, GcError, StateVector};
use cdoc_model::Document;

fn text(client: u64, t: &str) -> Doc {
    let mut d = Doc::new(ClientId(client));
    d.insert_text(0, t).unwrap();
    d
}

fn sync(from: &Doc, to: &mut Doc) {
    to.apply_update(&from.encode_diff(&to.state_vector()))
        .unwrap();
}

fn replica(of: &Doc, client: u64) -> Doc {
    Doc::decode_snapshot(&of.encode_snapshot(), ClientId(client)).unwrap()
}

#[test]
fn a_collected_churn_snapshot_is_smaller_and_shows_the_same_text() {
    let mut d = Doc::new(ClientId(1));
    for i in 0..2_000usize {
        let at = (i * 7919) % (d.len() + 1);
        d.insert_text(at, "abc").unwrap();
        if d.len() > 30 {
            d.delete((i * 104_729) % (d.len() - 4), 4).unwrap();
        }
    }
    let before = d.encode_snapshot();
    let collected = d.gc(&[d.state_vector()]).unwrap();
    let after = d.encode_snapshot();
    assert!(
        collected.units > 1_000 && collected.deletes > 1_000,
        "{collected:?}"
    );
    assert!(
        after.len() * 4 < before.len(),
        "{} -> {}",
        before.len(),
        after.len()
    );
    let old = Doc::decode_snapshot(&before, ClientId(2)).unwrap();
    let new = Doc::decode_snapshot(&after, ClientId(2)).unwrap();
    assert_eq!(new.visible_text(), old.visible_text());
    assert_eq!(new.gc_floor(), &old.state_vector());
    // Collecting again finds nothing more and changes nothing.
    let mut again = new.clone();
    let none = again.gc(&[again.state_vector()]).unwrap();
    assert_eq!((none.units, none.deletes), (0, 0));
    assert_eq!(again.encode_snapshot(), after);
}

#[test]
fn a_deletion_some_peer_has_not_seen_is_kept() {
    let mut a = text(1, "abcdef");
    let mut relay = replica(&a, 9);
    let behind = a.state_vector();
    a.delete(1, 3).unwrap();
    sync(&a, &mut relay);
    let c = relay.gc(&[a.state_vector(), behind.clone()]).unwrap();
    assert_eq!((c.units, c.deletes), (0, 0));
    // The peer that had not seen it can still sync from the relay.
    let mut late = replica(&a, 3);
    late = Doc::decode_snapshot(&late.encode_snapshot(), ClientId(3)).unwrap();
    let mut old = text(1, "abcdef");
    assert_eq!(old.state_vector(), behind);
    sync(&relay, &mut old);
    assert_eq!(old.visible_text(), "aef");
    assert_eq!(late.visible_text(), "aef");
}

#[test]
fn a_peer_behind_the_floor_is_refused_and_unchanged() {
    let mut a = text(1, "abcdef");
    let mut b = replica(&a, 2);
    let mut relay = replica(&a, 9);
    a.delete(2, 2).unwrap(); // "cd"
    sync(&a, &mut relay);
    // b is not a known peer: it never acknowledged the deletion.
    relay.gc(&[a.state_vector(), relay.state_vector()]).unwrap();
    let before = b.encode_snapshot();
    assert_eq!(
        b.apply_update(&relay.encode_diff(&b.state_vector())),
        Err(DecodeError::Compacted)
    );
    assert_eq!(b.encode_snapshot(), before);
    // b types between "c" and "d"; "d" is gone from the relay, which refuses rather than
    // guessing where it was.
    let sv = b.state_vector();
    b.insert_text(3, "X").unwrap();
    let held = relay.encode_snapshot();
    assert_eq!(
        relay.apply_update(&b.encode_diff(&sv)),
        Err(DecodeError::Compacted)
    );
    assert_eq!(relay.encode_snapshot(), held);
    // Reloaded from the relay's snapshot, b is at the floor and syncs again.
    let mut b = Doc::decode_snapshot(&held, ClientId(2)).unwrap();
    b.insert_text(2, "Y").unwrap();
    sync(&b, &mut relay);
    sync(&relay, &mut a);
    assert_eq!(a.visible_text(), "abYef");
    assert_eq!(relay.visible_text(), "abYef");
}

#[test]
fn gc_refuses_without_peers_with_pending_operations_or_behind_a_peer() {
    let mut a = text(1, "abc");
    assert_eq!(a.gc(&[]), Err(GcError::NoPeers));
    let mut ahead = replica(&a, 2);
    ahead.insert_text(0, "z").unwrap();
    assert_eq!(
        a.gc(&[ahead.state_vector()]),
        Err(GcError::Behind(ClientId(2)))
    );
    let sv = ahead.state_vector();
    ahead.insert_text(0, "y").unwrap();
    a.apply_update(&ahead.encode_diff(&sv)).unwrap();
    assert!(a.has_pending());
    assert_eq!(a.gc(&[a.state_vector()]), Err(GcError::Pending));
}

#[test]
fn a_model_document_survives_collection() {
    let mut d = from_model(
        &cdoc_model::test_support::fixtures::kitchen_sink(),
        ClientId(1),
    )
    .unwrap();
    let len = d.len();
    d.delete(len / 3, len / 3).unwrap();
    let model = to_model(&d).unwrap();
    d.gc(&[d.state_vector()]).unwrap();
    assert_eq!(to_model(&d).unwrap(), model);
    let reloaded = Doc::decode_snapshot(&d.encode_snapshot(), ClientId(5)).unwrap();
    assert_eq!(to_model(&reloaded).unwrap(), model);
    let mut fresh = Doc::new(ClientId(6));
    assert_eq!(
        fresh.apply_update(&d.encode_diff(&StateVector::empty())),
        Err(DecodeError::Compacted)
    );
    assert_eq!(to_model(&fresh), Err(cdoc_crdt::BridgeError::NoSection));
    let _blank = Document::blank();
}
