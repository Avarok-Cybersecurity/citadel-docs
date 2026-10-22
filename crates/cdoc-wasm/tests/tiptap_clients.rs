//! The UI derives the migration client as a full u64 (a hash; it arrives as a `BigInt`), so
//! any client from 1 to `u64::MAX` migrates, including past 2^53 where a JavaScript number
//! would round; client 0 stays refused.

use cdoc_crdt::{snapshot_version, to_model, ClientId, Doc, PendingLimits, StateVector};
use cdoc_tiptap::test_support::CORPUS;
use cdoc_wasm::migrate::tiptap_to_crdt_snapshot;

const LIMITS: PendingLimits = PendingLimits {
    max_ops: 1 << 20,
    max_bytes: 1 << 26,
};

/// 1, the first integer a JavaScript number cannot hold, and the largest u64.
const CLIENTS: [u64; 3] = [1, (1 << 53) + 1, u64::MAX];

#[test]
fn every_non_zero_u64_writes_a_snapshot_under_exactly_that_client() {
    let (_, json) = CORPUS
        .iter()
        .find(|(n, _)| *n == "meeting-notes")
        .expect("in the corpus");
    let (mapped, _) = cdoc_tiptap::tiptap_to_document(json).expect("maps");
    let mut snapshots = Vec::new();
    for client in CLIENTS {
        let (snapshot, _) = tiptap_to_crdt_snapshot(json, client).expect("migrates");
        let version = snapshot_version(&snapshot).expect("reads its version");
        let writers: Vec<ClientId> = version.iter().map(|(c, _)| c).collect();
        assert_eq!(writers, [ClientId(client)], "{client}");
        let mut device = Doc::decode_snapshot(&snapshot, ClientId(7)).expect("decodes");
        assert_eq!(to_model(&device).expect("folds"), mapped, "{client}");
        let again = tiptap_to_crdt_snapshot(json, client)
            .expect("migrates again")
            .0;
        let other = Doc::decode_snapshot(&again, ClientId(8)).expect("decodes");
        device
            .apply_update(&other.encode_diff(&StateVector::empty()), LIMITS)
            .expect("applies");
        assert!(
            device.encode_snapshot() == snapshot,
            "{client}: merge was not a no-op"
        );
        snapshots.push(snapshot);
    }
    snapshots.dedup();
    assert_eq!(
        snapshots.len(),
        CLIENTS.len(),
        "each client writes its own bytes"
    );
}

#[test]
fn client_zero_stays_refused() {
    let (_, json) = CORPUS[0];
    let err = tiptap_to_crdt_snapshot(json, 0).unwrap_err();
    assert!(err.contains("client 0 is reserved"), "{err}");
}
