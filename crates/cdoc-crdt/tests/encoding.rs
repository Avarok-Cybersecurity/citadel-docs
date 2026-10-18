//! Every binary form round trips: an update decoded and re-encoded is the same bytes, at any
//! version boundary; a snapshot likewise; a damaged encoding is an error.

mod common;

use cdoc_crdt::{from_model, ClientId, Doc, StateVector};
use cdoc_model::test_support::strategies;
use proptest::prelude::*;

/// A model document, then edits that split runs, delete across them and format.
/// Returns the snapshot at every version along the way.
fn edited(d: &cdoc_model::Document, cuts: &[u16]) -> (Doc, Vec<Vec<u8>>) {
    let mut doc = from_model(d, ClientId(1)).unwrap();
    let mut versions = vec![
        Doc::new(ClientId(1)).encode_snapshot(),
        doc.encode_snapshot(),
    ];
    for (i, &c) in cuts.iter().enumerate() {
        let at = 1 + usize::from(c) * doc.len() / 65_536;
        if i % 3 == 2 && at < doc.len() {
            doc.delete(at, (doc.len() - at).min(3)).unwrap();
        } else {
            doc.insert_text(at, "xy").unwrap();
        }
        versions.push(doc.encode_snapshot());
    }
    (doc, versions)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn updates_re_encode_to_the_same_bytes(
        d in strategies::document(),
        cuts in prop::collection::vec(any::<u16>(), 0..8),
        pick in any::<prop::sample::Index>(),
    ) {
        prop_assume!(common::distinct_comment_ids(&d));
        let (doc, versions) = edited(&d, &cuts);
        let full = doc.encode_diff(&StateVector::empty());
        let mut copy = Doc::new(ClientId(2));
        copy.apply_update(&full).unwrap();
        prop_assert_eq!(&copy.encode_diff(&StateVector::empty()), &full);
        prop_assert_eq!(copy.encode_snapshot(), doc.encode_snapshot());
        let snapshot = pick.get(&versions);
        let mid = cdoc_crdt::snapshot_version(snapshot).unwrap();
        prop_assert_eq!(copy.encode_diff(&mid), doc.encode_diff(&mid));
        let mut later = Doc::decode_snapshot(snapshot, ClientId(3)).unwrap();
        later.apply_update(&doc.encode_diff(&mid)).unwrap();
        prop_assert_eq!(later.encode_diff(&mid), doc.encode_diff(&mid));
        prop_assert_eq!(later.encode_snapshot(), doc.encode_snapshot());
    }

    #[test]
    fn a_truncated_or_padded_encoding_is_an_error(
        d in strategies::document(),
        cuts in prop::collection::vec(any::<u16>(), 0..8),
        cut in any::<prop::sample::Index>(),
    ) {
        prop_assume!(common::distinct_comment_ids(&d));
        let (doc, _) = edited(&d, &cuts);
        let mut collected = Doc::decode_snapshot(&doc.encode_snapshot(), ClientId(2)).unwrap();
        collected.gc(&[doc.state_vector()]).unwrap();
        let gc_snapshot = collected.encode_snapshot();
        prop_assert_eq!(
            Doc::decode_snapshot(&gc_snapshot, ClientId(3)).unwrap().encode_snapshot(),
            gc_snapshot.clone()
        );
        for update in [doc.encode_diff(&StateVector::empty()), collected.encode_diff(&StateVector::empty())] {
            let at = cut.index(update.len());
            prop_assert!(Doc::new(ClientId(4)).apply_update(&update[..at]).is_err());
            let padded = [update.as_slice(), &[0]].concat();
            prop_assert!(Doc::new(ClientId(4)).apply_update(&padded).is_err());
        }
        for snapshot in [doc.encode_snapshot(), gc_snapshot] {
            let at = cut.index(snapshot.len());
            prop_assert!(Doc::decode_snapshot(&snapshot[..at], ClientId(4)).is_err());
            let padded = [snapshot.as_slice(), &[0]].concat();
            prop_assert!(Doc::decode_snapshot(&padded, ClientId(4)).is_err());
        }
    }
}
