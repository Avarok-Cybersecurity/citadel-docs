//! Deletions are clocked operations: a diff ships only the deletions the peer lacks, so the
//! update after one keystroke is a few bytes however much was ever deleted; a deletion that
//! arrives before what it deletes waits; and the format version that changed this refuses the
//! old one loudly.

use cdoc_crdt::{snapshot_version, ClientId, DecodeError, Doc, StateVector};

/// A document with `deletes` deletions scattered through it.
fn churned(deletes: usize) -> Doc {
    let mut d = Doc::new(ClientId(1));
    d.insert_text(0, &"abcdefghij".repeat(deletes)).unwrap();
    for k in 0..deletes {
        d.delete(k * 9 + 3, 1).unwrap();
    }
    d
}

#[test]
fn one_keystroke_ships_no_deletions_however_many_there_were() {
    for deletes in [1, 100, 2_000] {
        let mut a = churned(deletes);
        let mut b = Doc::decode_snapshot(&a.encode_snapshot(), ClientId(2)).unwrap();
        let sv = a.state_vector();
        a.insert_text(5, "x").unwrap();
        let diff = a.encode_diff(&sv);
        // Header 5, floor 1, one client: id, first clock, count, one text op.
        assert!(
            diff.len() <= 20,
            "{deletes} deletions: {} bytes",
            diff.len()
        );
        b.apply_update(&diff).unwrap();
        assert_eq!(b.visible_text(), a.visible_text());
    }
}

#[test]
fn a_deletion_travels_once_to_whoever_lacks_it() {
    let mut a = churned(10);
    let mut b = Doc::decode_snapshot(&a.encode_snapshot(), ClientId(2)).unwrap();
    let sv = b.state_vector();
    b.delete(0, 2).unwrap();
    let with = b.encode_diff(&sv);
    a.apply_update(&with).unwrap();
    assert_eq!(a.visible_text(), b.visible_text());
    // Now a has it: nothing is left to send either way.
    assert_eq!(
        b.encode_diff(&a.state_vector()),
        a.encode_diff(&b.state_vector())
    );
    assert_eq!(a.encode_diff(&b.state_vector()).len(), 7);
}

#[test]
fn a_deletion_before_what_it_deletes_waits_for_it() {
    let mut a = Doc::new(ClientId(1));
    a.insert_text(0, "hello").unwrap();
    let insert = a.encode_diff(&StateVector::empty());
    let sv = a.state_vector();
    let mut c = Doc::decode_snapshot(&a.encode_snapshot(), ClientId(3)).unwrap();
    c.delete(1, 3).unwrap();
    let delete = c.encode_diff(&sv);
    let mut b = Doc::new(ClientId(2));
    let applied = b.apply_update(&delete).unwrap();
    assert!(b.has_pending());
    // It names the last unit it deletes: holding that means holding the whole range.
    assert_eq!(applied.missing, vec![cdoc_crdt::Id::new(ClientId(1), 3)]);
    let applied = b.apply_update(&insert).unwrap();
    assert!(!b.has_pending() && applied.missing.is_empty());
    assert_eq!(b.visible_text(), "ho");
}

/// The same bytes with the version byte (after the 3-byte magic) replaced.
fn with_version(bytes: &[u8], version: u8) -> Vec<u8> {
    let mut v = bytes.to_vec();
    v[3] = version;
    v
}

#[test]
fn other_format_versions_are_refused_loudly() {
    let d = churned(3);
    let update = d.encode_diff(&StateVector::empty());
    let snapshot = d.encode_snapshot();
    assert_eq!(&update[..4], b"CDC\x02");
    for version in [0, 1, 3, 255] {
        let refused = DecodeError::Version {
            found: version,
            supported: 2,
        };
        let mut fresh = Doc::new(ClientId(2));
        assert_eq!(
            fresh.apply_update(&with_version(&update, version)),
            Err(refused.clone())
        );
        assert!(fresh.is_empty() && !fresh.has_pending());
        let old = with_version(&snapshot, version);
        assert_eq!(
            Doc::decode_snapshot(&old, ClientId(2)).unwrap_err(),
            refused
        );
        assert_eq!(snapshot_version(&old), Err(refused));
    }
    // A version 1 update as 0.1 wrote it (an unclocked delete set at the end) is refused too.
    let v1 = [
        b"CDC\x01\x00".as_slice(),
        &[1, 1, 0, 1, 0, 1, 1, b'a', 1, 1, 1, 0, 1],
    ]
    .concat();
    assert_eq!(
        Doc::new(ClientId(2)).apply_update(&v1),
        Err(DecodeError::Version {
            found: 1,
            supported: 2
        })
    );
}

#[test]
fn snapshot_operations_are_gap_encoded_in_client_order() {
    let mut a = Doc::new(ClientId(1));
    a.insert_text(0, "ab").unwrap();
    a.delete(0, 1).unwrap();
    let mut b = Doc::decode_snapshot(&a.encode_snapshot(), ClientId(2)).unwrap();
    b.delete(0, 1).unwrap();
    let snapshot = b.encode_snapshot();
    // The two delete operations close the snapshot (layout 2, `snapshot/ops.rs`). Client 1's
    // is at clock 2: the head is gap 2 then the three-bit tag 2 (delete); one range located by
    // position, the tombstone run's first unit (0), one unit long. A switch to the next client
    // (step 1, tag 3), then client 2's at clock 0: its range starts at position 1 (zigzag 2).
    let ours: &[u8] = &[2 << 3 | 2, 1 << 1, 0, 1];
    let theirs: &[u8] = &[1 << 3 | 3, 2, 1 << 1, 2, 1];
    let at = snapshot.len() - 9;
    assert_eq!(&snapshot[at..], [ours, theirs].concat());
    assert_eq!(
        Doc::decode_snapshot(&snapshot, ClientId(3))
            .unwrap()
            .encode_snapshot(),
        snapshot
    );
    // Clients only ascend: client 1's operation after the switch is read as client 2's, past
    // its clock.
    let swapped = [&snapshot[..at], theirs, ours].concat();
    assert_eq!(
        Doc::decode_snapshot(&swapped, ClientId(3)).unwrap_err(),
        DecodeError::Inconsistent("an operation past its client's clock")
    );
}

#[test]
fn a_delete_with_touching_ranges_is_refused() {
    let mut d = Doc::new(ClientId(1));
    d.insert_text(0, "ab").unwrap();
    d.delete(0, 2).unwrap();
    let update = d.encode_diff(&StateVector::empty());
    // Header, empty floor, one client (1, from clock 0, two ops): the tombstone of "ab", then
    // a delete of one range (client 1, start 0, len 2).
    let head = b"CDC\x02\x00\x00\x01\x01\x00\x02\x02\x02\x06".as_slice();
    assert_eq!(update, [head, &[1, 1, 0, 2]].concat());
    let touching = [head, &[2, 1, 0, 1, 1, 1, 1]].concat();
    assert_eq!(
        Doc::new(ClientId(2)).apply_update(&touching),
        Err(DecodeError::Inconsistent("delete ranges not canonical"))
    );
}
