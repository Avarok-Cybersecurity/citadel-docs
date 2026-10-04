//! Validation: a document that passes is one every exporter and the layout crate can take
//! without defensive code. Checks are split into reference integrity (`refs`) and structural
//! and geometric constraints (`structure`); this module defines the error type and runs both.

mod refs;
mod structure;

use crate::ids::{AbstractNumId, CommentId, FieldId, FootnoteId, NumId, StyleId};
use crate::style::StyleError;
use crate::Document;

/// One thing wrong with a document. `validate` returns all of them, not the first.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    /// The document has no sections.
    #[error("the document has no sections")]
    NoSections,
    /// A container that must hold a block holds none.
    #[error("{0} has no blocks")]
    EmptyContainer(String),
    /// A text run has no characters.
    #[error("a text run is empty")]
    EmptyText,
    /// A page dimension is not positive.
    #[error("section {section}: page size must be positive")]
    PageSize {
        /// Section index.
        section: usize,
    },
    /// A margin is negative.
    #[error("section {section}: margins must not be negative")]
    NegativeMargin {
        /// Section index.
        section: usize,
    },
    /// Zero columns.
    #[error("section {section}: column count must be at least 1")]
    Columns {
        /// Section index.
        section: usize,
    },
    /// A font size of zero.
    #[error("a font size is zero")]
    FontSize,
    /// Line spacing with no height.
    #[error("line spacing must be positive")]
    LineSpacing,
    /// Document defaults leave font or size unset, so nothing could be rendered.
    #[error("document defaults must set a font and a size")]
    DefaultsIncomplete,
    /// A style reference failed to resolve.
    #[error(transparent)]
    Style(#[from] StyleError),
    /// A style of the wrong kind was referenced.
    #[error("style {0:?} is not a {1} style")]
    StyleKind(StyleId, &'static str),
    /// Two styles share an id.
    #[error("style id {0:?} is defined twice")]
    DuplicateStyle(StyleId),
    /// A paragraph refers to a numbering instance that does not exist.
    #[error("numbering instance {0:?} is not defined")]
    UnknownNumbering(NumId),
    /// An instance refers to a definition that does not exist.
    #[error("abstract numbering {0:?} is not defined")]
    UnknownAbstractNumbering(AbstractNumId),
    /// Two numbering ids collide.
    #[error("numbering id {0} is defined twice")]
    DuplicateNumbering(u32),
    /// A definition has no levels or more than nine.
    #[error("abstract numbering {0:?} must have 1 to 9 levels")]
    LevelCount(AbstractNumId),
    /// A level index is out of range for its definition.
    #[error("list level {level} is out of range for numbering {num_id:?}")]
    Level {
        /// The instance.
        num_id: NumId,
        /// The level.
        level: u8,
    },
    /// An outline level above 8.
    #[error("outline level {0} is out of range")]
    OutlineLevel(u8),
    /// A footnote reference names a missing footnote.
    #[error("footnote {0:?} is not defined")]
    UnknownFootnote(FootnoteId),
    /// Two footnotes share an id.
    #[error("footnote id {0:?} is defined twice")]
    DuplicateFootnote(FootnoteId),
    /// A comment anchor or reply names a missing comment.
    #[error("comment {0:?} is not defined")]
    UnknownComment(CommentId),
    /// Two comments share an id.
    #[error("comment id {0:?} is defined twice")]
    DuplicateComment(CommentId),
    /// A comment replies to itself.
    #[error("comment {0:?} replies to itself")]
    SelfReply(CommentId),
    /// Two fields share an id.
    #[error("field id {0:?} is defined twice")]
    DuplicateField(FieldId),
    /// A drop-down with no options or a selection out of range.
    #[error("form field {0:?} has an invalid drop-down")]
    Dropdown(FieldId),
    /// A table with no columns, no rows, an empty row, or a zero span.
    #[error("a table is malformed: {0}")]
    Table(&'static str),
    /// A hyperlink with an empty target.
    #[error("a hyperlink has an empty href")]
    EmptyHref,
    /// An image with a non-positive dimension.
    #[error("an image has a non-positive size")]
    ImageSize,
}

impl Document {
    /// Every problem with the document, or `Ok` if there are none.
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        structure::check(self, &mut errors);
        refs::check(self, &mut errors);
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Push `error` if `ids` contains a duplicate, reporting the first repeated id.
fn check_unique<T: Ord + Clone>(
    ids: impl Iterator<Item = T>,
    error: impl Fn(T) -> ValidationError,
    errors: &mut Vec<ValidationError>,
) {
    let mut seen = std::collections::BTreeSet::new();
    for id in ids {
        if !seen.insert(id.clone()) {
            errors.push(error(id));
            return;
        }
    }
}
