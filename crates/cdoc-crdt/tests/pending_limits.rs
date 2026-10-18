//! The buffer of operations waiting for a dependency is capped by the caller's limits. What
//! is over them is dropped and counted, the client holding the most bytes first, and a later
//! diff sends it again: a dropped operation is not in the state vector.

mod common;

use cdoc_crdt::{ClientId, Doc, Id, PendingLimits};
use common::ROOM;

fn limits(max_ops: usize, max_bytes: usize) -> PendingLimits {
    PendingLimits { max_ops, max_bytes }
}

fn replica(of: &Doc, client: u64) -> Doc {
    Doc::decode_snapshot(&of.encode_snapshot(), ClientId(client)).unwrap()
}

/// `author`'s `ops` insertions after one it keeps back, as an update for `base`: every one
/// waits for the clock gap. Each insertion at 0 is an operation of its own.
fn gapped(base: &Doc, author: &mut Doc, ops: usize, text: &str) -> Vec<u8> {
    author.insert_text(0, "gap").unwrap();
    let sv = author.state_vector();
    for _ in 0..ops {
        author.insert_text(0, text).unwrap();
    }
    assert!(base.state_vector().get(author.client()) < sv.get(author.client()));
    author.encode_diff(&sv)
}

fn base() -> Doc {
    let mut d = Doc::new(ClientId(1));
    d.insert_text(0, "abc").unwrap();
    d
}

#[test]
fn operations_over_the_count_are_dropped_counted_and_sent_again() {
    let mut a = base();
    let mut b = replica(&a, 2);
    let applied = a
        .apply_update(&gapped(&a, &mut b, 5, "x"), limits(3, usize::MAX))
        .unwrap();
    assert_eq!(applied.dropped, 2);
    assert_eq!(applied.missing, vec![Id::new(ClientId(2), 0)]);
    let applied = a
        .apply_update(&b.encode_diff(&a.state_vector()), limits(3, usize::MAX))
        .unwrap();
    assert_eq!((applied.missing, applied.dropped), (vec![], 0));
    assert!(!a.has_pending());
    assert_eq!(a.visible_text(), b.visible_text());
}

#[test]
fn operations_over_the_bytes_are_dropped() {
    let mut a = base();
    let mut b = replica(&a, 2);
    let update = gapped(&a, &mut b, 1, &"long ".repeat(40));
    let applied = a.apply_update(&update, limits(usize::MAX, 100)).unwrap();
    assert_eq!((applied.missing, applied.dropped), (vec![], 1));
    assert!(!a.has_pending());
    let mut room = base();
    let applied = room.apply_update(&update, limits(usize::MAX, 300)).unwrap();
    assert_eq!(applied.dropped, 0, "the operation fits 300 bytes");
}

#[test]
fn the_peer_flooding_the_buffer_loses_its_own_operations_first() {
    let mut a = base();
    let mut flood = replica(&a, 2);
    let mut quiet = replica(&a, 3);
    let flooded = gapped(&a, &mut flood, 20, "f");
    let one = gapped(&a, &mut quiet, 1, "q");
    a.apply_update(&one, limits(4, usize::MAX)).unwrap();
    let applied = a.apply_update(&flooded, limits(4, usize::MAX)).unwrap();
    assert_eq!(applied.dropped, 17);
    assert_eq!(
        applied.missing,
        vec![Id::new(ClientId(2), 0), Id::new(ClientId(3), 0)],
        "both peers still have operations waiting"
    );
}

#[test]
fn no_room_buffers_nothing_and_a_full_diff_still_converges() {
    let mut a = base();
    let mut b = replica(&a, 2);
    let applied = a
        .apply_update(&gapped(&a, &mut b, 3, "x"), limits(0, 0))
        .unwrap();
    assert_eq!((applied.missing, applied.dropped), (vec![], 3));
    assert!(!a.has_pending());
    a.apply_update(&b.encode_diff(&a.state_vector()), limits(0, 0))
        .unwrap();
    assert_eq!(a.visible_text(), b.visible_text());
}

#[test]
fn within_the_limits_nothing_is_dropped() {
    let mut a = base();
    let mut b = replica(&a, 2);
    let applied = a.apply_update(&gapped(&a, &mut b, 5, "x"), ROOM).unwrap();
    assert_eq!(applied.dropped, 0);
    assert!(a.has_pending());
}
