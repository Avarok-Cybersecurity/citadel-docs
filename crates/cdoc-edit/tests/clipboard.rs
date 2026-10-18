//! Copy and Cut: the selection's plain text in the result record (up to its cap) and on
//! request, and Cut deleting it as one undo step; and the caret's section setup in the record.

mod common;

use cdoc_edit::wire::{decode_request, decode_result, encode_request, encode_result, Request};
use cdoc_edit::{Command, EditError, MAX_SELECTION_TEXT_BYTES};
use common::{caret, check, editor, gap, paragraphs, select, text};

fn outcome(e: &cdoc_edit::Editor<'_>) -> cdoc_edit::wire::Outcome {
    let o = e.outcome(Vec::new(), Vec::new(), Vec::new()).unwrap();
    assert_eq!(decode_result(&encode_result(&o)).unwrap(), o);
    o
}

#[test]
fn the_selection_text_has_paragraph_breaks_and_tabs() {
    let mut e = editor(&paragraphs(&["one", "three"]), 1);
    let at = gap(&e, 0, 3);
    caret(&mut e, at);
    e.apply(Command::InsertText("\ttwo".into())).unwrap();
    e.apply(Command::InsertBreak(cdoc_model::BreakKind::Line))
        .unwrap();
    assert_eq!(outcome(&e).selection_text.as_deref(), Some(""));
    e.apply(Command::SelectAll).unwrap();
    assert_eq!(e.selection_text().unwrap(), "one\ttwo\n\nthree");
    assert_eq!(
        outcome(&e).selection_text.as_deref(),
        Some("one\ttwo\n\nthree")
    );
    let (a, b) = (gap(&e, 0, 1), gap(&e, 1, 2));
    select(&mut e, a, b);
    assert_eq!(e.selection_text().unwrap(), "ne\ttwo\n\nth");
}

#[test]
fn headers_footers_and_footnote_bodies_are_not_copied() {
    let mut e = editor(&cdoc_model::test_support::fixtures::kitchen_sink(), 1);
    e.apply(Command::SelectAll).unwrap();
    let all = e.selection_text().unwrap();
    assert!(all.starts_with("Kitchen sink\n"), "{all:?}");
    assert!(
        !all.contains("A note") && !all.contains("first header"),
        "{all:?}"
    );
}

#[test]
fn past_the_cap_the_record_says_incomplete_and_the_call_has_it_all() {
    let line = "x".repeat(1000);
    let lines: Vec<&str> = (0..70).map(|_| line.as_str()).collect();
    let mut e = editor(&paragraphs(&lines), 1);
    e.apply(Command::SelectAll).unwrap();
    let o = outcome(&e);
    assert_eq!(o.selection_text, None);
    let all = e.selection_text().unwrap();
    assert_eq!(all.len(), 70 * 1000 + 69);
    assert!(all.len() > MAX_SELECTION_TEXT_BYTES);
    let bytes = encode_result(&o);
    assert_eq!(bytes[bytes.len() - 2..], [0, 0], "incomplete, no text");
    let mut bad = bytes.clone();
    let n = bad.len();
    bad[n - 2] = 2;
    assert!(decode_result(&bad).is_err(), "complete is 0 or 1");
}

#[test]
fn cut_deletes_the_selection_in_one_undo_step() {
    let mut e = editor(&paragraphs(&["alpha beta", "gamma"]), 1);
    assert!(matches!(
        e.apply(Command::Cut),
        Err(EditError::NotApplicable(_))
    ));
    let (a, b) = (gap(&e, 0, 6), gap(&e, 1, 2));
    select(&mut e, a, b);
    assert_eq!(e.selection_text().unwrap(), "beta\nga");
    e.apply(Command::Cut).unwrap();
    assert_eq!(text(&e, 0), "alpha mma");
    check(&e);
    e.undo().unwrap();
    assert_eq!(
        (text(&e, 0), text(&e, 1)),
        ("alpha beta".into(), "gamma".into())
    );
    let req = Request::Command(Command::Cut);
    assert_eq!(encode_request(&req), [0x33]);
    assert_eq!(decode_request(&[0x33]).unwrap(), req);
}

#[test]
fn the_record_carries_the_caret_s_section_setup() {
    let mut e = editor(&paragraphs(&["x"]), 1);
    assert_eq!(outcome(&e).section, e.model().sections[0].properties.page);
    e.apply(Command::SetSectionProperty(cdoc_model::PageSetupPatch {
        size: None,
        orientation: Some(cdoc_model::Orientation::Landscape),
        margins: None,
        columns: None,
    }))
    .unwrap();
    let section = outcome(&e).section;
    assert_eq!(section.size.orientation, cdoc_model::Orientation::Landscape);
    assert!(section.size.width > section.size.height);
}
