//! An imported document published into an empty log: the 500-page corpus document, loaded
//! into the CRDT, cut from the empty state vector into 64 KiB updates, applied in order on a
//! fresh replica, is the source replica again, snapshot for snapshot.

use cdoc_bench::corpus::generate;
use cdoc_crdt::{from_model, to_model, ClientId, Doc, Id, StateVector};

const LIMIT: usize = 64 * 1024;

#[test]
fn a_500_page_import_published_from_empty_in_64k_updates_is_the_source_again() {
    let model = generate(500);
    let source = from_model(&model, ClientId(1)).unwrap();
    let chunks = source
        .encode_diff_chunked(&StateVector::empty(), LIMIT)
        .unwrap();
    // About 2 MB of updates: the corpus is deterministic, so the count is too.
    assert_eq!(chunks.len(), 32);
    let mut replica = Doc::new(ClientId(2));
    for (i, chunk) in chunks.iter().enumerate() {
        assert!(chunk.len() <= LIMIT, "chunk {i} is {} bytes", chunk.len());
        let applied = replica.apply_update(chunk).unwrap();
        assert_eq!(applied.missing, Vec::<Id>::new(), "chunk {i} waits");
    }
    assert_eq!(replica.state_vector(), source.state_vector());
    assert_eq!(replica.encode_snapshot(), source.encode_snapshot());
    assert_eq!(to_model(&replica).unwrap(), to_model(&source).unwrap());
}
