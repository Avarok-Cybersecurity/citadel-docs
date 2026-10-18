//! A kitchen-sink document: valid, and using every node and mark kind at least once, including
//! a nested table, all three header variants, a footnote, a comment thread, both revision
//! kinds, a form field and a signature field. Exporters use it as their first golden input.

mod inlines;
mod table;

use crate::builtin::ids;
use crate::ids::{AbstractNumId, CommentId, FootnoteId, NumId};
use crate::inline::{Field, FieldKind, Inline, InlineContent};
use crate::notes::{Comment, Footnote};
use crate::numbering::{
    AbstractNumbering, Level, LevelOverride, NumberFormat, Numbering, NumberingInstance,
};
use crate::paragraph::{Alignment, Indent, NumberingRef, Paragraph, ParagraphProperties};
use crate::run::RunProperties;
use crate::section::{HeaderFooter, HeaderFooterSet, Section, SectionStart};
use crate::units::Length;
use crate::{Block, Document};

pub(super) fn para(style: Option<crate::StyleId>, inlines: Vec<Inline>) -> Block {
    Block::Paragraph(Paragraph {
        properties: ParagraphProperties {
            style,
            ..ParagraphProperties::none()
        },
        inlines,
    })
}

pub(super) fn text(s: &str) -> Inline {
    Inline::text(s, RunProperties::none())
}

fn body() -> Vec<Block> {
    let bold = RunProperties {
        bold: Some(true),
        ..RunProperties::none()
    };
    let mut rich = vec![text("Body ")];
    rich.extend(inlines::marked_text());
    rich.extend(inlines::widgets());
    let listed = Block::Paragraph(Paragraph {
        properties: ParagraphProperties {
            style: Some(ids::list_paragraph()),
            numbering: Some(NumberingRef {
                num_id: NumId(1),
                level: 1,
            }),
            suppress_auto_hyphens: Some(true),
            ..ParagraphProperties::none()
        },
        inlines: vec![text("second level")],
    });
    vec![
        para(Some(ids::title()), vec![text("Kitchen sink")]),
        para(
            Some(ids::heading(1).expect("1")),
            vec![text("Every "), Inline::text("kind", bold)],
        ),
        para(Some(ids::normal()), rich),
        listed,
        table::table(),
        para(Some(ids::quote()), vec![text("Quoted")]),
    ]
}

fn numbering() -> Numbering {
    let level = |n: u8| Level {
        start: 1,
        format: NumberFormat::Decimal,
        text: format!("%{}.", n + 1),
        alignment: Alignment::Left,
        indent: Some(Indent {
            left: Some(Length::from_twips(720 * (i32::from(n) + 1))),
            right: None,
            first_line: Some(crate::FirstLine::Hanging {
                by: Length::from_twips(360),
            }),
        }),
        run: None,
    };
    Numbering {
        definitions: vec![AbstractNumbering {
            id: AbstractNumId(1),
            levels: vec![level(0), level(1)],
        }],
        instances: vec![NumberingInstance {
            id: NumId(1),
            abstract_id: AbstractNumId(1),
            overrides: vec![LevelOverride { level: 1, start: 3 }],
        }],
    }
}

fn header_footer(label: &str) -> HeaderFooter {
    HeaderFooter {
        blocks: vec![para(
            Some(ids::header()),
            vec![
                text(label),
                Inline::plain(InlineContent::Field(Field {
                    field: FieldKind::PageNumber,
                    cached: "1".to_owned(),
                })),
            ],
        )],
    }
}

/// The kitchen-sink document.
#[must_use]
pub fn kitchen_sink() -> Document {
    let mut doc = Document::blank();
    doc.meta.title = Some("Kitchen sink".to_owned());
    doc.settings.even_and_odd_headers = true;
    doc.settings.auto_hyphenation = true;
    doc.numbering = numbering();
    let section = &mut doc.sections[0];
    section.properties.start = SectionStart::OddPage;
    section.properties.title_page = true;
    section.properties.page.size = section.properties.page.size.rotated();
    section.properties.page.columns.count = 2;
    section.properties.headers = HeaderFooterSet {
        default: Some(header_footer("default header")),
        first: Some(header_footer("first header")),
        even: Some(header_footer("even header")),
    };
    section.properties.footers = HeaderFooterSet {
        default: Some(header_footer("footer")),
        first: None,
        even: None,
    };
    section.blocks = body();
    doc.sections.push(Section {
        properties: doc.sections[0].properties.clone(),
        blocks: vec![para(Some(ids::normal()), vec![text("Second section")])],
    });
    doc.footnotes = vec![Footnote {
        id: FootnoteId(1),
        blocks: vec![para(Some(ids::footnote_text()), vec![text("A note")])],
    }];
    doc.comments = vec![
        Comment {
            id: CommentId::new("c1"),
            author: "Alice".to_owned(),
            date: "2026-10-04T12:00:00Z".to_owned(),
            text: "Really?".to_owned(),
            resolved: false,
            parent: None,
        },
        Comment {
            id: CommentId::new("c2"),
            author: "Bob".to_owned(),
            date: "2026-10-04T12:05:00Z".to_owned(),
            text: "Yes.".to_owned(),
            resolved: false,
            parent: Some(CommentId::new("c1")),
        },
    ];
    doc
}
