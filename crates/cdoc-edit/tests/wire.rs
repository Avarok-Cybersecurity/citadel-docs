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
        assert_eq!(&bytes[..4], b"CDE\x03");
        assert_eq!(decode_result(&bytes).expect("decodes"), outcome);
        for k in 0..bytes.len() {
            assert!(decode_result(&bytes[..k]).is_err(), "cut at {k} decoded");
        }
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(decode_result(&longer).is_err(), "a trailing byte decoded");
        for version in [1, 2] {
            let mut old = bytes.clone();
            old[3] = version;
            assert!(
                decode_result(&old).is_err(),
                "a version {version} record decoded"
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
    let bytes = encode_result(
        &e.outcome(Vec::new(), Vec::new(), Vec::new())
            .expect("an outcome"),
    );
    assert_eq!(
        bytes[bytes.len() - 3..],
        [0, 0, 0],
        "nothing mixed at a caret, nothing created"
    );
    let with = |run: u64, paragraph: u64| {
        let mut b = bytes[..bytes.len() - 3].to_vec();
        varint(run, &mut b);
        varint(paragraph, &mut b);
        b.push(0);
        b
    };
    // Twelve run keys, eleven paragraph keys.
    let all = decode_result(&with((1 << 12) - 1, (1 << 11) - 1)).expect("every key");
    assert_eq!(all.formatting.mixed_run.len(), 12);
    assert_eq!(all.formatting.mixed_paragraph.len(), 11);
    assert!(decode_result(&with(1 << 12, 0)).is_err());
    assert!(decode_result(&with(0, 1 << 11)).is_err());
}
