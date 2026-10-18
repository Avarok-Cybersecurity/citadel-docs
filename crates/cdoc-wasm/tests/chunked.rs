//! `EditorHandle.encodeDiffChunked` through the facade: a session's whole state from an empty
//! log (`None`) or a peer's diff, cut under a byte limit, applied in order on a replica, is the
//! session's state; a limit below an update's frame and a malformed state vector are refused.

use cdoc_crdt::{ClientId, Doc, Id};
use cdoc_edit::wire::{decode_chunks, encode_request, Request};
use cdoc_edit::Command;
use cdoc_layout::test_support::{ALIASES, FACES, FALLBACK};
use cdoc_wasm::edit_facade::{EditFonts, EditSession, SealedFonts};

const LIMIT: usize = 4_096;

fn fonts() -> SealedFonts {
    let mut f = EditFonts::new(FALLBACK);
    for (bold, italic, bytes) in FACES {
        f.add_font(FALLBACK, bold, italic, bytes);
    }
    for (name, target) in ALIASES {
        f.alias_font(name, target);
    }
    f.seal()
}

fn typed(s: &mut EditSession, text: &str) {
    let req = Request::Command(Command::InsertText(text.to_owned()));
    s.request(&encode_request(&req)).expect("runs");
}

fn apply_in_order(d: &mut Doc, record: &[u8]) -> usize {
    let chunks = decode_chunks(record).expect("a chunks record");
    for (i, chunk) in chunks.iter().enumerate() {
        assert!(chunk.len() <= LIMIT, "chunk {i} is {} bytes", chunk.len());
        let applied = d.apply_update(chunk).expect("a chunk applies");
        assert_eq!(applied.missing, Vec::<Id>::new(), "chunk {i} waits");
    }
    chunks.len()
}

#[test]
fn the_whole_state_from_an_empty_log_arrives_in_order_under_the_limit() {
    let mut s = EditSession::blank(1, fonts()).expect("opens");
    typed(
        &mut s,
        &"twenty thousand characters of pasted prose. ".repeat(450),
    );
    let mut fresh = Doc::new(ClientId(9));
    let n = apply_in_order(&mut fresh, &s.encode_diff_chunked(None, LIMIT).unwrap());
    assert!(n >= 5, "{n} chunks");
    assert_eq!(fresh.encode_snapshot(), s.snapshot());
}

#[test]
fn a_peers_diff_arrives_in_order_under_the_limit() {
    let mut a = EditSession::blank(1, fonts()).expect("opens");
    let peer = Doc::decode_snapshot(&a.snapshot(), ClientId(2)).unwrap();
    typed(&mut a, &"more text than one update may carry. ".repeat(400));
    let mut b = peer;
    let record = a
        .encode_diff_chunked(Some(&b.encode_state_vector()), LIMIT)
        .unwrap();
    assert!(apply_in_order(&mut b, &record) >= 3);
    assert_eq!(b.encode_snapshot(), a.snapshot());
}

#[test]
fn a_limit_below_the_frame_and_a_bad_state_vector_are_refused_by_name() {
    let s = EditSession::blank(1, fonts()).expect("opens");
    let small = s.encode_diff_chunked(None, 4).unwrap_err();
    assert!(small.contains("below an update's"), "{small}");
    let bad = s.encode_diff_chunked(Some(&[5]), LIMIT).unwrap_err();
    assert!(bad.contains("state vector length out of range"), "{bad}");
}
