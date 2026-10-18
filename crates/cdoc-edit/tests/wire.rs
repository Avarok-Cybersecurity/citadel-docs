//! The binary boundary: every request and the result record round-trip, truncated records are
//! refused, and a decoded request does what the request itself does.

mod common;

use cdoc_edit::wire::{
    decode_position, decode_request, decode_result, encode_position, encode_request, encode_result,
    Request,
};
use cdoc_edit::{Command, DeleteUnit, Direction, Granularity, MarkKind, Selection};
use cdoc_model::{BreakKind, Columns, Margins, Orientation, PageSetupPatch, PageSize};
use common::ops::command;
use common::{at, editor, paragraphs, select};

fn requests(e: &cdoc_edit::Editor<'_>) -> Vec<Request> {
    let mut out: Vec<Request> = (0..34u8)
        .map(|c| Request::Command(command(e, c, u16::from(c) * 7, 3, usize::from(c))))
        .collect();
    let (a, b) = (at(e, 4), at(e, 9));
    out.extend(
        [
            Command::SetSelection(Selection { anchor: a, head: b }),
            Command::InsertBreak(BreakKind::Page),
            Command::InsertBreak(BreakKind::Column),
            Command::DeleteForward(DeleteUnit::Word),
            Command::Cut,
            Command::InsertText("ünï 𝄞 \u{301}".to_owned()),
            Command::SetSectionProperty(PageSetupPatch {
                size: Some(PageSize::LETTER_PORTRAIT),
                orientation: Some(Orientation::Landscape),
                margins: Some(Margins::NORMAL),
                columns: Some(Columns::ONE),
            }),
            Command::SetSectionProperty(PageSetupPatch {
                orientation: Some(Orientation::Portrait),
                ..PageSetupPatch::none()
            }),
        ]
        .map(Request::Command),
    );
    out.extend(
        [
            MarkKind::Bold,
            MarkKind::Strike,
            MarkKind::Subscript,
            MarkKind::Superscript,
        ]
        .map(|m| Request::Command(Command::ToggleMark(m))),
    );
    out.extend([
        Request::Undo,
        Request::Redo,
        Request::BreakUndoGroup,
        Request::SelectWord(a),
        Request::SelectParagraph(b),
        Request::CompositionStart,
        Request::CompositionUpdate("かな".to_owned(), 1),
        Request::CompositionCommit,
        Request::CompositionCancel,
        Request::SetViewport(Vec::new()),
        Request::SetViewport(vec![0, 3, 200]),
    ]);
    for granularity in [
        Granularity::Char,
        Granularity::Word,
        Granularity::LineBoundary,
        Granularity::Line,
        Granularity::Paragraph,
        Granularity::Document,
    ] {
        for (direction, extend) in [(Direction::Forward, false), (Direction::Backward, true)] {
            out.push(Request::MoveCaret {
                granularity,
                direction,
                extend,
            });
        }
    }
    out
}

#[test]
fn every_request_round_trips_and_truncations_are_refused() {
    let e = editor(&cdoc_model::test_support::fixtures::kitchen_sink(), 1);
    let all = requests(&e);
    assert!(all.len() > 60);
    for req in &all {
        let bytes = encode_request(req);
        assert_eq!(&decode_request(&bytes).expect("decodes"), req);
        for k in 0..bytes.len() {
            assert!(
                decode_request(&bytes[..k]).is_err(),
                "{req:?} cut at {k} decoded"
            );
        }
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(
            decode_request(&longer).is_err(),
            "{req:?} with a trailing byte decoded"
        );
    }
    assert!(decode_request(&[0xff]).is_err());
}

#[test]
fn a_decoded_request_does_what_the_request_does() {
    let doc = paragraphs(&["alpha beta", "gamma"]);
    let (mut direct, mut wired) = (editor(&doc, 1), editor(&doc, 1));
    let script = requests(&direct);
    for req in script {
        let decoded = decode_request(&encode_request(&req)).expect("decodes");
        let x = direct.handle(req);
        let y = wired.handle(decoded);
        assert_eq!(x.is_ok(), y.is_ok());
        assert_eq!(direct.model(), wired.model());
        assert_eq!(direct.selection(), wired.selection());
    }
}

#[test]
fn the_result_record_and_positions_round_trip() {
    let mut e = editor(&paragraphs(&["alpha beta", "gamma"]), 1);
    let (s, t) = (common::gap(&e, 0, 2), common::gap(&e, 1, 3));
    select(&mut e, s, t);
    let applied = e
        .apply(Command::ToggleMark(MarkKind::Bold))
        .expect("applies");
    let outcome = e
        .outcome(applied.update, applied.dirty_pages, applied.created)
        .expect("an outcome");
    assert!(outcome.caret.is_some() && outcome.rects.len() == 2);
    let (s, t) = (common::gap(&e, 0, 0), common::gap(&e, 1, 3));
    select(&mut e, s, t);
    let mixed = e
        .outcome(Vec::new(), Vec::new(), Vec::new())
        .expect("an outcome");
    assert!(!mixed.formatting.mixed_run.is_empty(), "plain then bold");
    for outcome in [outcome, mixed] {
        let bytes = encode_result(&outcome);
        assert_eq!(&bytes[..4], b"CDE\x04");
        assert_eq!(decode_result(&bytes).expect("decodes"), outcome);
        for k in 0..bytes.len() {
            assert!(decode_result(&bytes[..k]).is_err(), "cut at {k} decoded");
        }
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(decode_result(&longer).is_err(), "a trailing byte decoded");
        for version in [1, 2, 3, 5] {
            let mut old = bytes.clone();
            old[3] = version;
            assert_eq!(
                decode_result(&old).err().map(|e| e.to_string()),
                Some(format!(
                    "result record version {version}; this build reads only version 4"
                ))
            );
        }
    }
    let p = e.selection().head;
    assert_eq!(decode_position(&encode_position(p)).expect("decodes"), p);
    assert!(decode_position(&[]).is_err());
}

fn varint(mut v: u64, out: &mut Vec<u8>) {
    while v >= 0x80 {
        out.push(u8::try_from(v & 0x7f).unwrap() | 0x80);
        v >>= 7;
    }
    out.push(u8::try_from(v).unwrap());
}

#[test]
fn a_mixed_set_naming_a_key_past_the_last_is_refused() {
    let e = editor(&paragraphs(&["alpha"]), 1);
    let outcome = e
        .outcome(Vec::new(), Vec::new(), Vec::new())
        .expect("an outcome");
    let bytes = encode_result(&outcome);
    // After the mixed sets: nothing created, the section value, a complete empty text.
    let section = cdoc_crdt::codec::to_bytes(&outcome.section).unwrap();
    let mut tail = vec![0];
    varint(section.len() as u64, &mut tail);
    tail.extend(&section);
    tail.extend([1, 0]);
    let head = bytes.len() - tail.len() - 2;
    assert_eq!(
        bytes[head..],
        [&[0, 0][..], &tail].concat(),
        "nothing mixed at a caret"
    );
    let with = |run: u64, paragraph: u64| {
        let mut b = bytes[..head].to_vec();
        varint(run, &mut b);
        varint(paragraph, &mut b);
        b.extend(&tail);
        b
    };
    // Fifteen run keys, twelve paragraph keys.
    let all = decode_result(&with((1 << 15) - 1, (1 << 12) - 1)).expect("every key");
    assert_eq!(all.formatting.mixed_run.len(), 15);
    assert_eq!(all.formatting.mixed_paragraph.len(), 12);
    assert!(decode_result(&with(1 << 15, 0)).is_err());
    assert!(decode_result(&with(0, 1 << 12)).is_err());
}

#[test]
fn an_incomplete_selection_text_is_a_zero_flag_and_an_empty_str_present() {
    let e = editor(&paragraphs(&["alpha"]), 1);
    let mut outcome = e
        .outcome(Vec::new(), Vec::new(), Vec::new())
        .expect("an outcome");
    outcome.selection_text = None;
    let bytes = encode_result(&outcome);
    assert_eq!(
        bytes[bytes.len() - 2..],
        [0, 0],
        "complete 0, then a zero length"
    );
    assert_eq!(decode_result(&bytes).expect("decodes").selection_text, None);
    let mut texted = bytes[..bytes.len() - 1].to_vec();
    texted.extend([1, b'x']);
    assert_eq!(
        decode_result(&texted).err().map(|e| e.to_string()),
        Some(cdoc_crdt::DecodeError::Inconsistent("text with an incomplete selection").to_string())
    );
}
