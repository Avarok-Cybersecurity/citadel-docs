//! A relay reads whose clocks an update writes without applying it, and is refused the same
//! bytes `apply_update` is refused.

mod common;

use cdoc_crdt::{update_writes, ClientId, DecodeError, Doc, StateVector};
use common::ROOM;

fn typed(client: u64, text: &str) -> Doc {
    let mut d = Doc::new(ClientId(client));
    d.insert_text(0, text).expect("insert");
    d
}

#[test]
fn a_local_edit_writes_its_own_clocks_and_nothing_else() {
    let mut alice = typed(7, "hello");
    let before = alice.state_vector();
    alice.insert_text(5, " world").expect("insert");
    alice.delete(0, 1).expect("delete");
    let writes = update_writes(&alice.encode_diff(&before)).expect("decodes");
    assert_eq!(writes.runs, vec![(ClientId(7), 5..12)]);
    assert_eq!(writes.floor, StateVector::empty());
}

#[test]
fn a_merged_replica_writes_every_client_it_holds() {
    let alice = typed(7, "ab");
    let mut bob = typed(9, "xyz");
    bob.apply_update(&alice.encode_diff(&StateVector::empty()), ROOM)
        .expect("apply");
    let writes = update_writes(&bob.encode_diff(&StateVector::empty())).expect("decodes");
    let clients: Vec<_> = writes.runs.iter().map(|(c, _)| *c).collect();
    assert_eq!(clients, vec![ClientId(7), ClientId(9)]);
    assert_eq!(writes.version(), bob.state_vector());
}

#[test]
fn an_empty_diff_writes_nothing() {
    let alice = typed(7, "ab");
    let writes = update_writes(&alice.encode_diff(&alice.state_vector())).expect("decodes");
    assert_eq!(writes.runs, Vec::new());
    assert_eq!(writes.version(), StateVector::empty());
}

#[test]
fn bytes_apply_update_refuses_are_refused_alike() {
    let alice = typed(7, "hello");
    let update = alice.encode_diff(&StateVector::empty());
    let mut damaged = vec![
        update[..update.len() - 1].to_vec(),
        [update.as_slice(), &[0]].concat(),
        vec![0xAB; 16],
        alice.encode_snapshot(),
    ];
    let mut bad_tag = update.clone();
    let last = bad_tag.len() - 1;
    bad_tag[last] = 0xFF;
    damaged.push(bad_tag);
    for bytes in damaged {
        let ours = update_writes(&bytes).map(|_| ());
        let theirs = Doc::new(ClientId(3)).apply_update(&bytes, ROOM).map(|_| ());
        assert!(ours.is_err(), "decoded {bytes:?}");
        assert_eq!(ours, theirs, "for {bytes:?}");
    }
    assert!(matches!(
        update_writes(&[]),
        Err(DecodeError::UnexpectedEnd)
    ));
}
