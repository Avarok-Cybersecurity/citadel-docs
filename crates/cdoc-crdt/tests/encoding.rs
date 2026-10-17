//! Every binary form round trips: an update decoded and re-encoded is the same bytes, at any
//! version boundary; a snapshot likewise; a damaged encoding is an error.

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
    fn a_truncated_encoding_is_an_error(
        d in strategies::document(),
        cut in any::<prop::sample::Index>(),
    ) {
        let doc = from_model(&d, ClientId(1)).unwrap();
        let update = doc.encode_diff(&StateVector::empty());
        let at = cut.index(update.len());
        prop_assert!(Doc::new(ClientId(2)).apply_update(&update[..at]).is_err());
        let snapshot = doc.encode_snapshot();
        let at = cut.index(snapshot.len());
        prop_assert!(Doc::decode_snapshot(&snapshot[..at], ClientId(2)).is_err());
    }
}
