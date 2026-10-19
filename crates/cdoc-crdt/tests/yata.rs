//! The YATA placement rule on hand-built concurrent histories, checked under every delivery
//! order.

mod common;

use cdoc_crdt::{ClientId, Doc, StateVector};
use common::ROOM;

fn replica(of: &Doc, client: u64) -> Doc {
    Doc::decode_snapshot(&of.encode_snapshot(), ClientId(client)).unwrap()
}

/// Applies `updates` to a copy of `base` in each listed order; every order must give `want`.
fn every_order(base: &Doc, updates: &[Vec<u8>], orders: &[&[usize]], want: &str) {
    for order in orders {
        let mut r = replica(base, 50);
        for &i in *order {
            r.apply_update(&updates[i], ROOM).unwrap();
        }
        assert!(!r.has_pending(), "order {order:?} left operations waiting");
        assert_eq!(r.visible_text(), want, "order {order:?}");
    }
}

#[test]
fn concurrent_inserts_at_one_place_order_lower_client_first() {
    let base = {
        let mut d = Doc::new(ClientId(9));
        d.insert_text(0, "ab").unwrap();
        d
    };
    let sv = base.state_vector();
    let mut a = replica(&base, 1);
    let mut b = replica(&base, 2);
    a.insert_text(1, "XX").unwrap();
    b.insert_text(1, "YY").unwrap();
    let updates = [a.encode_diff(&sv), b.encode_diff(&sv)];
    every_order(&base, &updates, &[&[0, 1], &[1, 0]], "aXXYYb");
}

/// A higher client's insert P sits before a lower client's sibling Q (P was typed in front of
/// Q). A concurrent insert X from a client between them must pass P provisionally, then pass
/// Q (lower client), and stop at C: `a P Q X C b`.
#[test]
fn a_concurrent_insert_passes_a_higher_sibling_to_reach_a_lower_one() {
    let base = {
        let mut d = Doc::new(ClientId(9));
        d.insert_text(0, "ab").unwrap();
        d
    };
    let sv = base.state_vector();
    let mut c3 = replica(&base, 3);
    c3.insert_text(1, "C").unwrap();
    let mut c1 = replica(&c3, 1);
    c1.insert_text(1, "Q").unwrap();
    c3.apply_update(&c1.encode_diff(&c3.state_vector()), ROOM)
        .unwrap();
    c3.insert_text(1, "P").unwrap();
    assert_eq!(c3.visible_text(), "aPQCb");
    let mut c2 = replica(&base, 2);
    c2.insert_text(1, "X").unwrap();
    let updates = [c3.encode_diff(&sv), c2.encode_diff(&sv)];
    every_order(&base, &updates, &[&[0, 1], &[1, 0]], "aPQXCb");
}

#[test]
fn interleaved_typing_does_not_interleave_characters() {
    let mut a = Doc::new(ClientId(2));
    let mut b = Doc::new(ClientId(1));
    a.insert_text(0, "a").unwrap();
    b.apply_update(&a.encode_diff(&b.state_vector()), ROOM)
        .unwrap();
    for ch in ["b", "c"] {
        a.insert_text(a.len(), ch).unwrap();
    }
    for ch in ["1", "2", "3"] {
        b.insert_text(b.len(), ch).unwrap();
    }
    let (to_b, to_a) = (
        a.encode_diff(&b.state_vector()),
        b.encode_diff(&a.state_vector()),
    );
    b.apply_update(&to_b, ROOM).unwrap();
    a.apply_update(&to_a, ROOM).unwrap();
    assert_eq!(a.visible_text(), b.visible_text());
    assert!(["abc123", "a123bc"].contains(&a.visible_text().as_str()));
}

#[test]
fn an_insert_whose_origins_come_from_another_client_waits_for_them() {
    let mut a = Doc::new(ClientId(1));
    a.insert_text(0, "abc").unwrap();
    let from_a = a.encode_diff(&StateVector::empty());
    let mut b = replica(&a, 2);
    b.insert_text(1, "X").unwrap();
    let from_b = b.encode_diff(&a.state_vector());
    let mut c = Doc::new(ClientId(3));
    let applied = c.apply_update(&from_b, ROOM).unwrap();
    assert_eq!(applied.missing, vec![cdoc_crdt::Id::new(ClientId(1), 0)]);
    assert_eq!(c.visible_text(), "");
    c.apply_update(&from_a, ROOM).unwrap();
    assert!(!c.has_pending());
    assert_eq!(c.visible_text(), "aXbc");
}
