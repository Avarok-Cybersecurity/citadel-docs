//! Inline content: what a paragraph holds. Each inline carries [`Marks`] (hyperlink, comment
//! anchors, tracked change), which apply to any inline the way `ProseMirror` marks do, so text,
//! images and fields can all be commented, linked or marked as inserted or deleted.

use crate::fields::{FormField, SignatureField};
use crate::ids::{CommentId, ContentId, FootnoteId, RevisionId};
use crate::run::RunProperties;
use crate::units::Length;
use serde::{Deserialize, Serialize};

/// A hyperlink target.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    /// The URL.
    pub href: String,
    /// A tooltip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// What kind of tracked change a revision is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RevisionKind {
    /// Content inserted while tracking changes.
    Insertion,
    /// Content deleted while tracking changes; still shown, struck through, until accepted.
    Deletion,
}

/// A tracked change. The date is RFC 3339 text supplied by the editor's clock port; the model
/// never reads a clock.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Revision {
    /// The revision id.
    pub id: RevisionId,
    /// Insertion or deletion.
    pub kind: RevisionKind,
    /// Who made the change.
    pub author: String,
    /// When, as RFC 3339.
    pub date: String,
}

/// Range annotations on an inline.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Marks {
    /// The inline is part of a hyperlink.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<Link>,
    /// Comments anchored on this inline, in anchoring order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<CommentId>,
    /// The tracked change this inline belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<Revision>,
}

impl Marks {
    /// No annotations.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            link: None,
            comments: Vec::new(),
            revision: None,
        }
    }
}

/// A run of text with uniform character formatting.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRun {
    /// The text; never empty.
    pub text: String,
    /// Direct character formatting.
    pub properties: RunProperties,
}

/// An inline image, by content id; the bytes live in the workspace's content store and reach
/// the layout crate through its image port.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineImage {
    /// The image bytes' content id.
    pub content_id: ContentId,
    /// Display width.
    pub width: Length,
    /// Display height.
    pub height: Length,
    /// Alternative text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
}

/// A break.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BreakKind {
    /// A line break within the paragraph.
    Line,
    /// The rest of the paragraph starts on a new page.
    Page,
    /// The rest of the paragraph starts in the next column.
    Column,
}

/// A computed field; `cached` is the last value the editor rendered, so a reader that cannot
/// compute the field still shows something.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FieldKind {
    /// The current page number.
    PageNumber,
    /// The number of pages.
    PageCount,
    /// A date, in the given format (Word's date picture, e.g. `d MMMM yyyy`).
    Date {
        /// The date picture.
        format: String,
    },
}

/// A computed field with its cached result.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    /// What the field computes.
    pub field: FieldKind,
    /// The last computed text.
    pub cached: String,
}

/// The content of an inline, before its marks.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum InlineContent {
    /// Text.
    Text(TextRun),
    /// An image.
    Image(InlineImage),
    /// A break.
    Break {
        /// Line, page or column.
        kind: BreakKind,
    },
    /// A tab.
    Tab,
    /// A footnote reference mark.
    FootnoteRef {
        /// The footnote.
        id: FootnoteId,
    },
    /// A computed field.
    Field(Field),
    /// A fillable form field.
    FormField(FormField),
    /// A signature placeholder for the signing track.
    SignatureField(SignatureField),
}

/// An inline: content plus marks.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Inline {
    /// The content.
    pub content: InlineContent,
    /// Range annotations.
    pub marks: Marks,
}

impl Inline {
    /// Plain text with the given run properties and no marks.
    #[must_use]
    pub fn text(text: impl Into<String>, properties: RunProperties) -> Self {
        Self {
            content: InlineContent::Text(TextRun {
                text: text.into(),
                properties,
            }),
            marks: Marks::none(),
        }
    }

    /// Any content with no marks.
    #[must_use]
    pub const fn plain(content: InlineContent) -> Self {
        Self {
            content,
            marks: Marks::none(),
        }
    }
}
