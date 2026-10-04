//! Strategies for inline content and marks.

use super::props::run_properties;
use super::units::{comment_id, field_id, positive_length, revision_id, rfc3339, short_text};
use super::{opt, pick};
use crate::fields::{FormField, FormFieldKind, PageAnchor, SignatureField};
use crate::ids::{ContentId, FootnoteId};
use crate::inline::{
    BreakKind, Field, FieldKind, Inline, InlineContent, InlineImage, Link, Marks, Revision,
    RevisionKind, TextRun,
};
use proptest::prelude::*;

/// A hyperlink.
pub fn link() -> BoxedStrategy<Link> {
    (
        short_text().prop_map(|t| format!("https://example.test/{t}")),
        opt(short_text()),
    )
        .prop_map(|(href, title)| Link { href, title })
        .boxed()
}

/// A tracked change.
pub fn revision() -> BoxedStrategy<Revision> {
    (
        revision_id(),
        pick(&[RevisionKind::Insertion, RevisionKind::Deletion]),
        short_text(),
        rfc3339(),
    )
        .prop_map(|(id, kind, author, date)| Revision {
            id,
            kind,
            author,
            date,
        })
        .boxed()
}

/// Marks: link, up to two comment anchors, revision, each independently.
pub fn marks() -> BoxedStrategy<Marks> {
    (
        opt(link()),
        prop::collection::vec(comment_id(), 0..=2),
        opt(revision()),
    )
        .prop_map(|(link, comments, revision)| Marks {
            link,
            comments,
            revision,
        })
        .boxed()
}

/// A page anchor.
pub fn page_anchor() -> BoxedStrategy<PageAnchor> {
    (
        0u32..50,
        positive_length(),
        positive_length(),
        positive_length(),
        positive_length(),
    )
        .prop_map(|(page, x, y, width, height)| PageAnchor {
            page,
            x,
            y,
            width,
            height,
        })
        .boxed()
}

/// A form field of every kind. Drop-downs always have at least one option and a selection
/// within range so the generated field is valid.
pub fn form_field() -> BoxedStrategy<FormField> {
    let kind = prop_oneof![
        (short_text(), opt(1u32..100))
            .prop_map(|(value, max_length)| FormFieldKind::Text { value, max_length }),
        any::<bool>().prop_map(|checked| FormFieldKind::Checkbox { checked }),
        prop::collection::vec(short_text(), 1..=3).prop_flat_map(|options| {
            let n = u32::try_from(options.len()).expect("tiny");
            (Just(options), opt(0..n))
                .prop_map(|(options, selected)| FormFieldKind::Dropdown { options, selected })
        }),
        opt(Just("2026-10-04".to_owned())).prop_map(|value| FormFieldKind::Date { value }),
    ];
    (field_id(), opt(short_text()), kind)
        .prop_map(|(id, name, field)| FormField { id, name, field })
        .boxed()
}

/// A signature field.
pub fn signature_field() -> BoxedStrategy<SignatureField> {
    (
        field_id(),
        pick(&["Client", "Witness", "Notary"]).prop_map(str::to_owned),
        opt(short_text()),
        opt(page_anchor()),
    )
        .prop_map(|(id, signer_role, label, anchor)| SignatureField {
            id,
            signer_role,
            label,
            anchor,
        })
        .boxed()
}

/// Inline content of every variant; text is the most frequent.
pub fn inline_content() -> BoxedStrategy<InlineContent> {
    let field = prop_oneof![
        Just(FieldKind::PageNumber),
        Just(FieldKind::PageCount),
        pick(&["d MMMM yyyy", "yyyy-MM-dd"]).prop_map(|f| FieldKind::Date {
            format: f.to_owned()
        }),
    ];
    prop_oneof![
        6 => (short_text(), run_properties())
            .prop_map(|(text, properties)| InlineContent::Text(TextRun { text, properties })),
        1 => (short_text(), positive_length(), positive_length(), opt(short_text())).prop_map(
            |(cid, width, height, alt)| InlineContent::Image(InlineImage {
                content_id: ContentId(cid),
                width,
                height,
                alt,
            })
        ),
        1 => pick(&[BreakKind::Line, BreakKind::Page, BreakKind::Column])
            .prop_map(|kind| InlineContent::Break { kind }),
        1 => Just(InlineContent::Tab),
        1 => (1u32..20).prop_map(|n| InlineContent::FootnoteRef { id: FootnoteId(n) }),
        1 => (field, short_text()).prop_map(|(field, cached)| InlineContent::Field(Field {
            field,
            cached
        })),
        1 => form_field().prop_map(InlineContent::FormField),
        1 => signature_field().prop_map(InlineContent::SignatureField),
    ]
    .boxed()
}

/// An inline with marks.
pub fn inline() -> BoxedStrategy<Inline> {
    (inline_content(), marks())
        .prop_map(|(content, marks)| Inline { content, marks })
        .boxed()
}
