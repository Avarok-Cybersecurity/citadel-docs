//! A diff cut into updates under a byte limit: each chunk is a valid update, applied in order
//! none waits for a later one, and together they integrate to what the unchunked diff does.
//! The 1 MB document is the benchmark's (`benches/common/workload.rs`).

#[path = "../benches/common/workload.rs"]
mod workload;

mod common;

use cdoc_crdt::{from_model, ChunkError, ClientId, DecodeError, Doc, DocAttr, Id, StateVector};
use cdoc_model::{Comment, CommentId, Document};
use common::ROOM;

const LIMIT: usize = 64 * 1024;

/// A fresh replica that applied `chunks` in order, each fully integrated when applied.
fn apply_in_order(chunks: &[Vec<u8>]) -> Doc {
    let mut d = Doc::new(ClientId(999));
    for (i, chunk) in chunks.iter().enumerate() {
        let applied = d.apply_update(chunk, ROOM).expect("a chunk decodes");
        assert_eq!(
            applied.missing,
            Vec::<Id>::new(),
            "chunk {i} waits for a later one"
        );
    }
    assert!(!d.has_pending());
    d
}

fn apply_whole(update: &[u8]) -> Doc {
    let mut d = Doc::new(ClientId(999));
    d.apply_update(update, ROOM).expect("the diff applies");
    d
}

#[test]
fn the_megabyte_diff_at_64k_converges_like_the_unchunked_diff() {
    let d = workload::megabyte_doc();
    let empty = StateVector::empty();
    let whole = d.encode_diff(&empty);
    let chunks = d.encode_diff_chunked(&empty, LIMIT).unwrap();
    assert!(whole.len() > 1_000_000, "the diff is {} bytes", whole.len());
    assert!(
        chunks.len() > whole.len() / LIMIT,
        "{} chunks",
        chunks.len()
    );
    assert!(chunks.iter().all(|c| c.len() <= LIMIT));
    let overhead = chunks.iter().map(Vec::len).sum::<usize>() - whole.len();
    assert!(overhead < chunks.len() * 64, "{overhead} bytes of overhead");
    let chunked = apply_in_order(&chunks);
    assert_eq!(chunked.visible_text(), d.visible_text());
    assert_eq!(chunked.state_vector(), d.state_vector());
    assert_eq!(
        chunked.encode_snapshot(),
        apply_whole(&whole).encode_snapshot()
    );
}

#[test]
fn a_paste_larger_than_the_limit_travels_as_chained_pieces() {
    let mut d = Doc::new(ClientId(1));
    let paste: String = workload::TEXT.repeat(2_000) + "é😀";
    d.insert_text(0, &paste).unwrap();
    let chunks = d.encode_diff_chunked(&StateVector::empty(), LIMIT).unwrap();
    assert!(chunks.len() >= 5, "{} chunks", chunks.len());
    assert!(chunks.iter().all(|c| c.len() <= LIMIT));
    assert_eq!(apply_in_order(&chunks).visible_text(), paste);
}

#[test]
fn with_no_limit_the_one_chunk_is_the_unchunked_diff() {
    let mut d = workload::megabyte_doc();
    let before = d.state_vector();
    d.insert_text(10, "late").unwrap();
    for sv in [StateVector::empty(), before, d.state_vector()] {
        let chunks = d.encode_diff_chunked(&sv, usize::MAX).unwrap();
        assert_eq!(chunks, vec![d.encode_diff(&sv)]);
    }
}

#[test]
fn a_chunk_applied_early_or_missing_waits_and_names_what_it_lacks() {
    let d = workload::megabyte_doc();
    let chunks = d.encode_diff_chunked(&StateVector::empty(), LIMIT).unwrap();
    let after_first = apply_in_order(&chunks[..1]);
    let lacked = vec![Id::new(
        ClientId(1),
        after_first.state_vector().get(ClientId(1)),
    )];

    let mut early = Doc::new(ClientId(999));
    let fresh = early.encode_snapshot();
    let first = vec![Id::new(ClientId(1), 0)];
    assert_eq!(early.apply_update(&chunks[1], ROOM).unwrap().missing, first);
    assert_eq!(early.state_vector(), StateVector::empty());
    assert_eq!(early.encode_snapshot(), fresh, "nothing integrated early");

    let mut gap = apply_in_order(&chunks[..1]);
    assert_eq!(gap.apply_update(&chunks[2], ROOM).unwrap().missing, lacked);
    assert_eq!(gap.encode_snapshot(), after_first.encode_snapshot());

    // The buffered chunk integrates once its predecessor arrives.
    early.apply_update(&chunks[0], ROOM).unwrap();
    for chunk in &chunks[2..] {
        early.apply_update(chunk, ROOM).unwrap();
    }
    assert!(!early.has_pending());
    assert_eq!(early.visible_text(), d.visible_text());
}

#[test]
fn a_truncated_chunk_is_refused_and_changes_nothing() {
    let d = workload::megabyte_doc();
    let chunks = d.encode_diff_chunked(&StateVector::empty(), LIMIT).unwrap();
    let mut r = apply_in_order(&chunks[..1]);
    let before = r.encode_snapshot();
    let next = &chunks[1];
    for cut in (1..next.len()).step_by(4_093).chain([next.len() - 1]) {
        let refused = r.apply_update(&next[..cut], ROOM).unwrap_err();
        assert!(
            matches!(
                refused,
                DecodeError::UnexpectedEnd
                    | DecodeError::OutOfRange(_)
                    | DecodeError::Inconsistent(_)
            ),
            "cut at {cut}: {refused:?}"
        );
        assert_eq!(r.encode_snapshot(), before, "cut at {cut}");
    }
}

#[test]
fn an_operation_over_the_limit_is_refused_by_name() {
    let mut d = from_model(&Document::blank(), ClientId(7)).unwrap();
    let at = d.next_id();
    d.set_attr(&DocAttr::Comments(vec![Comment {
        id: CommentId::new("c"),
        author: "a".to_owned(),
        date: String::new(),
        text: "x".repeat(5_000),
        resolved: false,
        parent: None,
    }]))
    .unwrap();
    match d.encode_diff_chunked(&StateVector::empty(), 4_096) {
        Err(ChunkError::OversizeOperation {
            at: found,
            bytes,
            max_bytes: 4_096,
        }) => {
            assert_eq!(found, at);
            assert!(bytes > 5_000, "{bytes}");
        }
        other => panic!("expected OversizeOperation, got {other:?}"),
    }
    assert!(d.encode_diff_chunked(&StateVector::empty(), 8_192).is_ok());
}

#[test]
fn a_limit_below_the_frame_is_refused_by_name() {
    let d = workload::cdoc_sequential(10);
    assert_eq!(
        d.encode_diff_chunked(&StateVector::empty(), 5),
        Err(ChunkError::LimitBelowFrame {
            max_bytes: 5,
            frame: 7
        })
    );
    let empty = d.encode_diff_chunked(&d.state_vector(), 7).unwrap();
    assert_eq!(empty, vec![d.encode_diff(&d.state_vector())]);
}
