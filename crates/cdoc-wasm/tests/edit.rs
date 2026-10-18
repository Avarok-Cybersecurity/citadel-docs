//! The editor through the facade, as the browser drives it: request records in, result records
//! out, updates relayed between two sessions, pages painted and clicks hit-tested.

use cdoc_edit::wire::{decode_peer_view, decode_position, decode_result, encode_request, Request};
use cdoc_edit::{Command, Selection};
use cdoc_layout::test_support::{ALIASES, FACES, FALLBACK};
use cdoc_wasm::edit_facade::{EditFonts, EditSession, SealedFonts};
use std::sync::OnceLock;

fn fonts() -> SealedFonts {
    static SEALED: OnceLock<SealedFonts> = OnceLock::new();
    *SEALED.get_or_init(|| {
        let mut f = EditFonts::new(FALLBACK);
        for (bold, italic, bytes) in FACES {
            f.add_font(FALLBACK, bold, italic, bytes);
        }
        for (name, target) in ALIASES {
            f.alias_font(name, target);
        }
        f.seal()
    })
}

fn send(s: &mut EditSession, req: &Request) -> cdoc_edit::wire::Outcome {
    decode_result(&s.request(&encode_request(req)).expect("runs")).expect("a result")
}

fn typed(s: &mut EditSession, text: &str) -> cdoc_edit::wire::Outcome {
    send(s, &Request::Command(Command::InsertText(text.to_owned())))
}

#[test]
fn two_sessions_edit_one_document_through_records() {
    let mut a = EditSession::blank(1, fonts()).expect("opens");
    let mut b = EditSession::open(&a.snapshot(), 2, fonts()).expect("opens");
    let first = typed(&mut a, "Hello");
    assert!(!first.update.is_empty() && first.caret.is_some());
    assert_eq!(first.dirty_pages, vec![0]);
    let seen = decode_result(&b.apply_remote(&first.update).expect("applies")).expect("a result");
    assert_eq!(seen.update, Vec::<u8>::new());
    assert_eq!(seen.dirty_pages, vec![0]);
    let second = typed(&mut b, " world");
    a.apply_remote(&second.update).expect("applies");
    assert_eq!(a.snapshot().len(), b.snapshot().len());
    assert_eq!(
        a.encode_diff(&b.state_vector()).map(|d| d.len()),
        b.encode_diff(&a.state_vector()).map(|d| d.len())
    );
    let undone = send(&mut a, &Request::Undo);
    assert!(!undone.update.is_empty(), "a's own typing is undone");
    b.apply_remote(&undone.update).expect("applies");
    let view = decode_peer_view(&a.peer_view(&b.awareness()).expect("a view")).expect("decodes");
    assert_eq!(view.client, cdoc_crdt::ClientId(2));
    assert!(view.caret.is_some());
}

#[test]
fn pages_paint_and_clicks_land_on_positions() {
    let mut a = EditSession::blank(1, fonts()).expect("opens");
    let out = typed(&mut a, "click here");
    assert_eq!(a.page_count(), 1);
    let [w, h] = a.page_size(0).expect("a page");
    assert!(w > 0 && h > 0);
    assert!(a.page_size(1).is_none());
    let list = a.paint(&[0]).expect("paints");
    assert!(cdoc_paint::decode(&list).is_ok(), "a display list");
    let (_, caret) = out.caret.expect("a caret");
    let hit = decode_position(
        &a.hit_test(0, caret.x, caret.y + caret.height / 2)
            .expect("hits"),
    )
    .expect("a position");
    assert_eq!(hit, out.selection.head);
    assert!(a.hit_test(5, 0, 0).is_err());
    let back = send(
        &mut a,
        &Request::Command(Command::SetSelection(Selection {
            anchor: hit,
            head: hit,
        })),
    );
    assert_eq!(back.selection.head, hit);
}

#[test]
fn bad_records_are_refused() {
    let mut a = EditSession::blank(1, fonts()).expect("opens");
    assert!(a.request(&[]).is_err());
    assert!(
        a.request(&[0x01, 0x05, b'a']).is_err(),
        "a truncated string"
    );
    assert!(a.apply_remote(b"nope").is_err());
    assert!(a.peer_view(b"CDA\x01").is_err());
    assert!(EditSession::open(b"not a snapshot", 2, fonts()).is_err());
}
