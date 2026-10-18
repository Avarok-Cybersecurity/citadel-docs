//! Citadel Docs layout: a [`cdoc_model::Document`] paginated into a page IR ([`Layout`]) that
//! the PDF and SVG backends in `cdoc-pdf` both draw, so the preview cannot disagree with the
//! file.
//!
//! Pure: faces arrive through [`FontPort`]; the crate reads no file, clock or randomness.
//!
//! What is laid out: page geometry from each section's `PageSetup` (size, orientation,
//! margins, columns); every run shaped (`cdoc-shape`: ligatures, kerning, mark positions) in
//! the face the style cascade resolves; paragraphs broken by the breaker the options name
//! (`cdoc-para`: Knuth-Plass or first fit), hyphenated with Liang's patterns when the document
//! asks for it, justified exactly; spacing, alignment and indents; list labels from the
//! model's numbering; tables with spans, borders, shading and repeated header rows; inline
//! images; page and column breaks, keep-with-next, keep-lines-together and widow/orphan
//! control; headers and footers with page-number fields; footnotes at the foot of the
//! referencing column; signature fields, whose rectangles are reported for
//! `SignatureField.anchor`.
//!
//! Incremental: a [`LayoutSession`] relays a document out after an edit, given a [`Dirty`]
//! description, re-measuring only what changed and stopping once the flow catches up with the
//! previous layout; [`layout`] is its full relayout, so the two cannot disagree.

#![forbid(unsafe_code)]

mod anchors;
mod context;
mod decorations;
mod flow;
pub mod fonts;
mod fragment;
mod furniture;
pub mod geometry;
mod hyphens;
pub mod ir;
pub mod labels;
mod line;
mod paragraph;
mod pieces;
mod session;
mod setter;
mod stack;
mod style;
pub mod svg_path;
mod table;
#[cfg(feature = "test-support")]
pub mod test_support;
mod widgets;
mod wrap;

pub use anchors::write_anchors;
/// The Knuth-Plass parameters [`BreakerChoice::KnuthPlass`] takes, re-exported so a caller
/// names them without depending on `cdoc-para` itself.
pub use cdoc_para::Params;
pub use context::{BreakerChoice, LayoutOptions, RevisionView};
pub use fonts::{FaceSource, FontError, FontLibrary, FontPort, Fonts};
pub use ir::{
    Draw, FaceId, FaceKey, GlyphRun, Layout, LinkArea, OutlineEntry, Page, PlacedGlyph, Pt100,
    Rect, Segment, SignaturePlacement, Warning,
};
pub use labels::list_labels;
pub use session::{BlockEdit, Dirty, LayoutSession, Relayout};

use cdoc_model::{Document, StyleError};

/// Why a document could not be laid out.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    /// The style cascade failed (unknown style or a `basedOn` cycle).
    #[error(transparent)]
    Style(#[from] StyleError),
    /// A face was missing or unreadable.
    #[error(transparent)]
    Font(#[from] FontError),
    /// A shipped hyphenation pattern set did not compile.
    #[error(transparent)]
    Patterns(#[from] cdoc_para::PatternError),
    /// The line breaker refused a paragraph's items.
    #[error(transparent)]
    Break(#[from] cdoc_para::BreakError),
    /// A run resolved to no font family or no size: the document defaults must set both.
    #[error(
        "a run has no font family or size after the cascade; set them in the document defaults"
    )]
    NoFont,
    /// The document has no sections.
    #[error("the document has no sections")]
    NoSections,
    /// An incremental relayout was asked of a session with no layout to start from.
    #[error("there is no previous layout to update; relayout with Dirty::all() first")]
    NoBaseline,
    /// The description of what changed does not fit the document (edits out of order or out
    /// of range, block counts that do not add up, sections added without a page-setup change).
    #[error("the dirty description does not match the document")]
    DirtyMismatch,
}

/// Lays the document out: a fresh session's full relayout.
pub fn layout(
    doc: &Document,
    fonts: &dyn FontPort,
    options: &LayoutOptions,
) -> Result<Layout, LayoutError> {
    let mut session = LayoutSession::new(fonts, options);
    session.relayout(doc, &Dirty::all())?;
    session.into_layout()
}
