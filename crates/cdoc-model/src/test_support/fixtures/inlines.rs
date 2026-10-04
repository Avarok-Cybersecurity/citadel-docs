//! The kitchen sink's inline content: every mark and every non-text inline kind.

use crate::builtin::ids;
use crate::fields::{FormField, FormFieldKind, PageAnchor, SignatureField};
use crate::ids::{CommentId, ContentId, FieldId, FootnoteId, RevisionId};
use crate::inline::{
    BreakKind, Field, FieldKind, Inline, InlineContent, InlineImage, Link, Marks, Revision,
    RevisionKind,
};
use crate::run::RunProperties;
use crate::units::{Color, FontSize, Length};
use crate::{Underline, UnderlineStyle, VerticalAlign};

/// Every character property set at once, inside a hyperlink, two comment anchors and an
/// insertion; followed by a deleted run.
pub(super) fn marked_text() -> [Inline; 2] {
    let fancy = RunProperties {
        font: Some("Georgia".to_owned()),
        size: Some(FontSize::from_points(14)),
        bold: Some(true),
        italic: Some(true),
        underline: Some(Underline {
            style: UnderlineStyle::Double,
            color: Some(Color::rgb(0xFF, 0, 0)),
        }),
        strike: Some(false),
        vertical_align: Some(VerticalAlign::Superscript),
        color: Some(Color::rgb(0x12, 0x34, 0x56)),
        highlight: Some(Color::rgb(0xFF, 0xFF, 0)),
        small_caps: Some(true),
        all_caps: Some(false),
        style: Some(ids::hyperlink()),
    };
    let revision = |id: &str, kind, author: &str| Revision {
        id: RevisionId::new(id),
        kind,
        author: author.to_owned(),
        date: "2026-10-04T12:00:00Z".to_owned(),
    };
    let link = Inline {
        content: InlineContent::Text(crate::TextRun {
            text: "a link".to_owned(),
            properties: fancy,
        }),
        marks: Marks {
            link: Some(Link {
                href: "https://avarok.net".to_owned(),
                title: Some("Avarok".to_owned()),
            }),
            comments: vec![CommentId::new("c1"), CommentId::new("c2")],
            revision: Some(revision("r1", RevisionKind::Insertion, "Alice")),
        },
    };
    let deleted = Inline {
        content: InlineContent::Text(crate::TextRun {
            text: "gone".to_owned(),
            properties: RunProperties::none(),
        }),
        marks: Marks {
            revision: Some(revision("r2", RevisionKind::Deletion, "Bob")),
            ..Marks::none()
        },
    };
    [link, deleted]
}

/// Every non-text inline kind once.
pub(super) fn widgets() -> Vec<Inline> {
    vec![
        Inline::plain(InlineContent::Tab),
        Inline::plain(InlineContent::Image(InlineImage {
            content_id: ContentId::new("img-1"),
            width: Length::from_inches(2.0),
            height: Length::from_inches(1.0),
            alt: Some("a picture".to_owned()),
        })),
        Inline::plain(InlineContent::Break {
            kind: BreakKind::Line,
        }),
        Inline::plain(InlineContent::FootnoteRef { id: FootnoteId(1) }),
        Inline::plain(InlineContent::Field(Field {
            field: FieldKind::Date {
                format: "d MMMM yyyy".to_owned(),
            },
            cached: "4 October 2026".to_owned(),
        })),
        Inline::plain(InlineContent::FormField(FormField {
            id: FieldId::new("f1"),
            name: Some("choice".to_owned()),
            field: FormFieldKind::Dropdown {
                options: vec!["yes".to_owned(), "no".to_owned()],
                selected: Some(1),
            },
        })),
        Inline::plain(InlineContent::SignatureField(SignatureField {
            id: FieldId::new("sig1"),
            signer_role: "Client".to_owned(),
            label: Some("Sign here".to_owned()),
            anchor: Some(PageAnchor {
                page: 0,
                x: Length::from_inches(1.0),
                y: Length::from_inches(9.0),
                width: Length::from_inches(3.0),
                height: Length::from_inches(0.5),
            }),
        })),
        Inline::plain(InlineContent::Break {
            kind: BreakKind::Page,
        }),
    ]
}
