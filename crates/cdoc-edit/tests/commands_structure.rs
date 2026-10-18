//! Paragraph formatting, lists and inline objects: the model each command leaves is the one
//! Word would, and stays valid.

mod common;

use cdoc_edit::Command;
use cdoc_model::{
    Alignment, Block, Comment, CommentId, ContentId, FootnoteId, InlineContent, Length, NumId,
    ParagraphProperties, StyleId,
};
use common::*;

fn with_lists(texts: &[&str]) -> cdoc_model::Document {
    let mut d = paragraphs(texts);
    d.numbering = cdoc_model::test_support::fixtures::kitchen_sink().numbering;
    d
}

#[test]
fn paragraph_properties_apply_to_every_selected_paragraph() {
    let mut e = editor(&paragraphs(&["one", "two", "three"]), 1);
    let (a, b) = (gap(&e, 0, 1), gap(&e, 1, 1));
    select(&mut e, a, b);
    let patch = ParagraphProperties {
        alignment: Some(Alignment::Center),
        ..ParagraphProperties::none()
    };
    e.apply(Command::SetParagraphProperty((&patch).into()))
        .unwrap();
    e.apply(Command::SetParagraphStyle(StyleId::new("Quote")))
        .unwrap();
    check(&e);
    assert_eq!(
        (
            props(&e, 0).alignment,
            props(&e, 1).alignment,
            props(&e, 2).alignment
        ),
        (Some(Alignment::Center), Some(Alignment::Center), None)
    );
    assert_eq!(props(&e, 1).style, Some(StyleId::new("Quote")));
}

#[test]
fn list_levels_move_with_indent_and_outdent() {
    let mut e = editor(&with_lists(&["item"]), 1);
    e.apply(Command::SetNumbering {
        num_id: NumId(1),
        level: 0,
    })
    .unwrap();
    e.apply(Command::Indent).unwrap();
    assert_eq!(props(&e, 0).numbering.map(|n| n.level), Some(1));
    e.apply(Command::Indent).unwrap();
    assert_eq!(
        props(&e, 0).numbering.map(|n| n.level),
        Some(1),
        "the definition has two levels"
    );
    e.apply(Command::Outdent).unwrap();
    e.apply(Command::Outdent).unwrap();
    assert_eq!(props(&e, 0).numbering.map(|n| n.level), Some(0));
    e.apply(Command::ClearNumbering).unwrap();
    assert_eq!(props(&e, 0).numbering, None);
    assert!(e
        .apply(Command::SetNumbering {
            num_id: NumId(9),
            level: 0
        })
        .is_err());
    check(&e);
}

#[test]
fn indent_without_a_list_moves_by_one_tab_stop() {
    let mut e = editor(&paragraphs(&["text"]), 1);
    let tab = e.model().settings.default_tab_stop;
    e.apply(Command::Indent).unwrap();
    e.apply(Command::Indent).unwrap();
    assert_eq!(
        props(&e, 0).indent.and_then(|i| i.left),
        Some(Length(tab.0 * 2))
    );
    e.apply(Command::Outdent).unwrap();
    e.apply(Command::Outdent).unwrap();
    e.apply(Command::Outdent).unwrap();
    assert_eq!(props(&e, 0).indent.and_then(|i| i.left), Some(Length(0)));
    check(&e);
}

#[test]
fn objects_and_annotations() {
    let mut e = editor(&paragraphs(&["see here"]), 1);
    let g = gap(&e, 0, 3);
    caret(&mut e, g);
    e.apply(Command::InsertImage {
        content_id: ContentId::new("pic"),
        width: Length(7200),
        height: Length(7200),
    })
    .unwrap();
    e.apply(Command::InsertFootnote(FootnoteId(7))).unwrap();
    assert!(
        e.apply(Command::InsertFootnote(FootnoteId(7))).is_err(),
        "ids are unique"
    );
    check(&e);
    assert_eq!(e.model().footnotes.len(), 1);
    let (from, to) = (gap(&e, 0, 6), gap(&e, 0, 10));
    select(&mut e, from, to);
    e.apply(Command::InsertLink("https://example.test".into()))
        .unwrap();
    let comment = Comment {
        id: CommentId::new("c1"),
        author: "Ada".into(),
        date: "2026-10-17T00:00:00Z".into(),
        text: "why".into(),
        resolved: false,
        parent: None,
    };
    e.apply(Command::InsertComment(comment)).unwrap();
    check(&e);
    let Block::Paragraph(p) = &e.model().sections[0].blocks[0] else {
        unreachable!()
    };
    let linked: String = p
        .inlines
        .iter()
        .filter(|i| i.marks.link.is_some())
        .map(|i| match &i.content {
            InlineContent::Text(r) => r.text.clone(),
            _ => String::new(),
        })
        .collect();
    assert_eq!(linked, "here");
    assert!(p
        .inlines
        .iter()
        .any(|i| i.marks.comments.contains(&CommentId::new("c1"))));
    let g = gap(&e, 0, 8);
    caret(&mut e, g);
    e.apply(Command::RemoveLink).unwrap();
    check(&e);
    let Block::Paragraph(p) = &e.model().sections[0].blocks[0] else {
        unreachable!()
    };
    assert!(p.inlines.iter().all(|i| i.marks.link.is_none()));
}
