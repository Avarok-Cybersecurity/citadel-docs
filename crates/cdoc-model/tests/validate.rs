//! Validation accepts the blank and kitchen-sink documents and reports each specific defect.

use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_model::{
    Block, CommentId, Document, FieldId, FootnoteId, FormFieldKind, Inline, InlineContent, Length,
    NumId, NumberingRef, RunProperties, StyleId, ValidationError,
};

fn first_paragraph(doc: &mut Document) -> &mut cdoc_model::Paragraph {
    match &mut doc.sections[0].blocks[0] {
        Block::Paragraph(p) => p,
        Block::Table(_) => panic!("fixture starts with a paragraph"),
    }
}

fn errors(doc: &Document) -> Vec<ValidationError> {
    doc.validate().err().unwrap_or_default()
}

#[test]
fn blank_and_kitchen_sink_are_valid() {
    assert_eq!(Document::blank().validate(), Ok(()));
    assert_eq!(kitchen_sink().validate(), Ok(()));
}

#[test]
fn no_sections_and_empty_containers() {
    let mut doc = Document::blank();
    doc.sections.clear();
    assert!(errors(&doc).contains(&ValidationError::NoSections));

    let mut doc = Document::blank();
    doc.sections[0].blocks.clear();
    assert!(errors(&doc)
        .iter()
        .any(|e| matches!(e, ValidationError::EmptyContainer(_))));
}

#[test]
fn geometry_errors_name_the_section() {
    let mut doc = Document::blank();
    doc.sections[0].properties.page.size.width = Length(0);
    doc.sections[0].properties.page.margins.top = Length(-1);
    doc.sections[0].properties.page.columns.count = 0;
    let errs = errors(&doc);
    assert!(errs.contains(&ValidationError::PageSize { section: 0 }));
    assert!(errs.contains(&ValidationError::NegativeMargin { section: 0 }));
    assert!(errs.contains(&ValidationError::Columns { section: 0 }));
}

#[test]
fn dangling_references_are_reported() {
    let mut doc = kitchen_sink();
    first_paragraph(&mut doc).properties.style = Some(StyleId::new("Ghost"));
    first_paragraph(&mut doc).properties.numbering = Some(NumberingRef {
        num_id: NumId(99),
        level: 0,
    });
    first_paragraph(&mut doc)
        .inlines
        .push(Inline::plain(InlineContent::FootnoteRef {
            id: FootnoteId(42),
        }));
    let mut commented = Inline::text("x", RunProperties::none());
    commented.marks.comments.push(CommentId::new("nobody"));
    first_paragraph(&mut doc).inlines.push(commented);
    let errs = errors(&doc);
    assert!(
        errs.contains(&ValidationError::Style(cdoc_model::StyleError::Unknown(
            StyleId::new("Ghost")
        )))
    );
    assert!(errs.contains(&ValidationError::UnknownNumbering(NumId(99))));
    assert!(errs.contains(&ValidationError::UnknownFootnote(FootnoteId(42))));
    assert!(errs.contains(&ValidationError::UnknownComment(CommentId::new("nobody"))));
}

#[test]
fn wrong_style_kind_and_list_level_out_of_range() {
    let mut doc = kitchen_sink();
    let hyperlink = cdoc_model::builtin::ids::hyperlink();
    first_paragraph(&mut doc).properties.style = Some(hyperlink.clone());
    first_paragraph(&mut doc).properties.numbering = Some(NumberingRef {
        num_id: NumId(1),
        level: 5,
    });
    let errs = errors(&doc);
    assert!(errs.contains(&ValidationError::StyleKind(hyperlink, "paragraph")));
    assert!(errs.contains(&ValidationError::Level {
        num_id: NumId(1),
        level: 5
    }));
}

#[test]
fn duplicates_are_reported_once_each() {
    let mut doc = kitchen_sink();
    let dup_footnote = doc.footnotes[0].clone();
    doc.footnotes.push(dup_footnote);
    let dup_comment = doc.comments[0].clone();
    doc.comments.push(dup_comment);
    let dup_style = doc.styles.styles[0].clone();
    doc.styles.styles.push(dup_style);
    let errs = errors(&doc);
    assert_eq!(
        errs.iter()
            .filter(|e| matches!(e, ValidationError::DuplicateFootnote(_)))
            .count(),
        1
    );
    assert!(errs.contains(&ValidationError::DuplicateComment(CommentId::new("c1"))));
    assert!(errs.contains(&ValidationError::DuplicateStyle(StyleId::new("Normal"))));
}

#[test]
fn empty_text_bad_dropdown_and_empty_href() {
    let mut doc = Document::blank();
    let mut linked = Inline::text("", RunProperties::none());
    linked.marks.link = Some(cdoc_model::Link {
        href: String::new(),
        title: None,
    });
    first_paragraph(&mut doc).inlines.push(linked);
    first_paragraph(&mut doc)
        .inlines
        .push(Inline::plain(InlineContent::FormField(
            cdoc_model::FormField {
                id: FieldId::new("dd"),
                name: None,
                field: FormFieldKind::Dropdown {
                    options: vec!["only".to_owned()],
                    selected: Some(1),
                },
            },
        )));
    let errs = errors(&doc);
    assert!(errs.contains(&ValidationError::EmptyText));
    assert!(errs.contains(&ValidationError::EmptyHref));
    assert!(errs.contains(&ValidationError::Dropdown(FieldId::new("dd"))));
}

#[test]
fn defaults_must_name_a_font_and_size() {
    let mut doc = Document::blank();
    doc.styles.defaults.run.font = None;
    assert!(errors(&doc).contains(&ValidationError::DefaultsIncomplete));
}

#[test]
fn table_row_wider_than_grid() {
    let mut doc = kitchen_sink();
    let table = doc.sections[0]
        .blocks
        .iter_mut()
        .find_map(|b| match b {
            Block::Table(t) => Some(t),
            Block::Paragraph(_) => None,
        })
        .unwrap();
    table.rows[0].cells[0].properties.col_span = 3;
    assert!(errors(&doc).contains(&ValidationError::Table(
        "a row spans more columns than the grid"
    )));
}
