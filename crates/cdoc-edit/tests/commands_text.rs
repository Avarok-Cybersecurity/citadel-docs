//! Typing and splitting, with Word's results: inherited and pending formatting, Enter on a
//! heading and on an empty list item.

mod common;

use cdoc_edit::{Command, DeleteUnit, MarkKind};
use cdoc_model::{Block, Inline, NumId, NumberingRef, ParagraphProperties, RunProperties, StyleId};
use common::*;

fn bold_middle() -> cdoc_model::Document {
    let mut d = paragraphs(&["x"]);
    let Block::Paragraph(p) = &mut d.sections[0].blocks[0] else {
        unreachable!()
    };
    p.inlines = vec![
        Inline::text("plain ", RunProperties::none()),
        Inline::text("bold", bold()),
        Inline::text(" tail", RunProperties::none()),
    ];
    d
}

#[test]
fn typed_text_takes_the_formatting_of_the_character_before_it() {
    let mut e = editor(&bold_middle(), 1);
    let g = gap(&e, 0, 10);
    caret(&mut e, g);
    type_chars(&mut e, "er");
    check(&e);
    assert_eq!(runs(&e, 0)[1], ("bolder".to_owned(), bold()));
}

#[test]
fn typed_text_at_a_paragraph_start_takes_the_formatting_after_it() {
    let mut d = bold_middle();
    let Block::Paragraph(p) = &mut d.sections[0].blocks[0] else {
        unreachable!()
    };
    p.inlines.remove(0);
    let mut e = editor(&d, 1);
    type_chars(&mut e, "Very ");
    check(&e);
    assert_eq!(runs(&e, 0)[0], ("Very bold".to_owned(), bold()));
}

#[test]
fn typing_after_a_bold_run_continues_bold_but_not_across_its_start() {
    let mut e = editor(&bold_middle(), 1);
    let g = gap(&e, 0, 6);
    caret(&mut e, g);
    type_chars(&mut e, "z");
    check(&e);
    assert_eq!(
        runs(&e, 0)[0],
        ("plain z".to_owned(), RunProperties::none())
    );
}

#[test]
fn bold_toggled_on_a_caret_applies_to_the_next_text_only() {
    let mut e = editor(&paragraphs(&["ab"]), 1);
    let g = gap(&e, 0, 1);
    caret(&mut e, g);
    e.apply(Command::ToggleMark(MarkKind::Bold)).unwrap();
    assert_eq!(e.pending_marks(), Some(&bold()));
    type_chars(&mut e, "XY");
    check(&e);
    let r = runs(&e, 0);
    assert_eq!(
        r,
        vec![
            ("a".into(), RunProperties::none()),
            ("XY".into(), bold()),
            ("b".into(), RunProperties::none())
        ]
    );
    e.apply(Command::ToggleMark(MarkKind::Bold)).unwrap();
    type_chars(&mut e, "z");
    assert_eq!(
        runs(&e, 0)[2],
        ("zb".into(), RunProperties::none()),
        "off again: no direct bold left"
    );
}

#[test]
fn pending_marks_end_when_the_caret_moves() {
    let mut e = editor(&paragraphs(&["ab"]), 1);
    e.apply(Command::ToggleMark(MarkKind::Bold)).unwrap();
    let g = gap(&e, 0, 2);
    caret(&mut e, g);
    assert_eq!(e.pending_marks(), None);
    type_chars(&mut e, "c");
    assert_eq!(runs(&e, 0), vec![("abc".into(), RunProperties::none())]);
}

#[test]
fn toggling_bold_on_a_selection_sets_it_and_toggling_again_removes_it() {
    let mut e = editor(&paragraphs(&["one two"]), 1);
    let (a, b) = (gap(&e, 0, 0), gap(&e, 0, 3));
    select(&mut e, a, b);
    e.apply(Command::ToggleMark(MarkKind::Bold)).unwrap();
    assert_eq!(runs(&e, 0)[0], ("one".into(), bold()));
    e.apply(Command::ToggleMark(MarkKind::Bold)).unwrap();
    check(&e);
    assert_eq!(runs(&e, 0), vec![("one two".into(), RunProperties::none())]);
}

#[test]
fn enter_splits_and_the_new_paragraph_copies_the_properties() {
    let mut d = paragraphs(&["Title text"]);
    let Block::Paragraph(p) = &mut d.sections[0].blocks[0] else {
        unreachable!()
    };
    p.properties.alignment = Some(cdoc_model::Alignment::Center);
    let mut e = editor(&d, 1);
    let g = gap(&e, 0, 5);
    caret(&mut e, g);
    e.apply(Command::SplitParagraph).unwrap();
    check(&e);
    assert_eq!((text(&e, 0), text(&e, 1)), ("Title".into(), " text".into()));
    assert_eq!(props(&e, 1), props(&e, 0));
}

#[test]
fn enter_at_the_end_of_a_heading_starts_its_next_style() {
    let mut d = paragraphs(&["Heading"]);
    let Block::Paragraph(p) = &mut d.sections[0].blocks[0] else {
        unreachable!()
    };
    p.properties.style = Some(StyleId::new("Heading1"));
    let mut e = editor(&d, 1);
    let g = gap(&e, 0, 7);
    caret(&mut e, g);
    e.apply(Command::SplitParagraph).unwrap();
    check(&e);
    assert_eq!(props(&e, 1).style, Some(StyleId::new("Normal")));
}

#[test]
fn enter_on_an_empty_list_item_ends_the_list() {
    let mut d = cdoc_model::test_support::fixtures::kitchen_sink();
    let numbering = ParagraphProperties {
        numbering: Some(NumberingRef {
            num_id: NumId(1),
            level: 0,
        }),
        ..ParagraphProperties::none()
    };
    d.sections[0].blocks.insert(
        0,
        Block::Paragraph(cdoc_model::Paragraph {
            properties: numbering,
            inlines: Vec::new(),
        }),
    );
    let mut e = editor(&d, 1);
    let g = gap(&e, 0, 0);
    caret(&mut e, g);
    let before = e.model().sections[0].blocks.len();
    e.apply(Command::SplitParagraph).unwrap();
    check(&e);
    assert_eq!(e.model().sections[0].blocks.len(), before);
    assert_eq!(props(&e, 0).numbering, None);
}

#[test]
fn newlines_and_tabs_in_typed_text_become_paragraphs_and_tabs() {
    let mut e = editor(&paragraphs(&[""]), 1);
    e.apply(Command::InsertText("a\tb\nc".into())).unwrap();
    check(&e);
    assert_eq!(
        (text(&e, 0), text(&e, 1)),
        ("a\u{fffc}b".to_owned(), "c".to_owned())
    );
}

#[test]
fn replace_takes_the_formatting_of_the_first_replaced_character() {
    let mut e = editor(&bold_middle(), 1);
    let (a, b) = (at(&e, gap(&e, 0, 6)), at(&e, gap(&e, 0, 10)));
    e.apply(Command::Replace {
        from: a,
        to: b,
        text: "BOLD".into(),
    })
    .unwrap();
    check(&e);
    assert_eq!(runs(&e, 0)[1], ("BOLD".into(), bold()));
}

#[test]
fn select_all_covers_the_body() {
    let mut e = editor(&paragraphs(&["one", "two"]), 1);
    e.apply(Command::SelectAll).unwrap();
    e.apply(Command::DeleteBackward(DeleteUnit::Char)).unwrap();
    check(&e);
    assert_eq!(e.model().sections[0].blocks.len(), 1);
    assert_eq!(text(&e, 0), "");
}
