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

#[test]
fn the_ribbons_read_formatting_and_set_the_page_through_records() {
    let mut a = EditSession::blank(1, fonts()).expect("opens");
    typed(&mut a, "text");
    let bold = send(
        &mut a,
        &Request::Command(Command::ToggleMark(cdoc_edit::MarkKind::Bold)),
    );
    assert_eq!(bold.formatting.run.bold, Some(true), "pending at the caret");
    let [w, h] = a.page_size(0).expect("a page");
    let turned = send(
        &mut a,
        &Request::Command(Command::SetSectionProperty(cdoc_model::PageSetupPatch {
            orientation: Some(cdoc_model::Orientation::Landscape),
            ..cdoc_model::PageSetupPatch::none()
        })),
    );
    assert_ne!(turned.update, Vec::<u8>::new());
    assert_eq!(a.page_size(0), Some([h, w]));
    send(&mut a, &Request::Undo);
    assert_eq!(a.page_size(0), Some([w, h]), "one undo step");
}

#[test]
fn a_session_collects_tombstones_through_records() {
    let mut a = EditSession::blank(1, fonts()).expect("opens");
    let b = EditSession::open(&a.snapshot(), 2, fonts()).expect("opens");
    typed(&mut a, "some text to delete");
    send(
        &mut a,
        &Request::Command(Command::DeleteBackward(cdoc_edit::DeleteUnit::Word)),
    );
    let mut b = b;
    b.apply_remote(&a.encode_diff(&b.state_vector()).expect("a diff"))
        .expect("applies");
    let before = a.snapshot().len();
    let peer = cdoc_crdt::StateVector::decode(&b.state_vector()).expect("a state vector");
    let record = a
        .gc(&cdoc_edit::wire::encode_known(&[peer]))
        .expect("collects");
    let collected = cdoc_edit::wire::decode_collected(&record).expect("decodes");
    assert!(collected.units > 0);
    assert!(a.snapshot().len() < before);
    assert!(
        a.gc(&cdoc_edit::wire::encode_known(&[])).is_err(),
        "no peers"
    );
}

#[test]
fn a_session_adds_an_image_and_inserts_it_by_its_id() {
    let mut a = EditSession::blank(1, fonts()).expect("opens");
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    png.extend(20u32.to_be_bytes());
    png.extend(10u32.to_be_bytes());
    png.extend([8, 6, 0, 0, 0, 0, 0, 0, 0]);
    let record = a.add_image(&png, 1024).expect("adds");
    let added = cdoc_edit::wire::decode_added_image(&record).expect("decodes");
    assert_eq!((added.width, added.height), (20, 10));
    assert!(a.add_image(&png, 10).is_err(), "over the limit");
    assert!(a.add_image(b"GIF89a", 1024).is_err(), "not PNG or JPEG");
    let out = send(
        &mut a,
        &Request::Command(Command::InsertImage {
            content_id: added.content_id,
            width: cdoc_model::Length(1500),
            height: cdoc_model::Length(750),
        }),
    );
    assert_ne!(out.update, Vec::<u8>::new());
}
