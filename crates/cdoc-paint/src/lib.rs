//! Citadel Docs paint path: the **display list**, a compact binary description of every page
//! that each painter (the canvas editor, the SVG preview, and later the PDF writer) draws from,
//! so no two of them can disagree about what a page shows.
//!
//! - [`lower`] turns the layout IR into a [`DisplayList`]: the layout's shaped glyph runs pass
//!   through as they are (glyph ids, advances, offsets, and the text their clusters name), so
//!   no painter ever needs a shaper.
//! - [`DisplayList::to_bytes`] writes the binary format; [`encode`] and [`encode_pages`] do
//!   both steps. `docs/display-list.md` is the byte-level contract.
//! - [`decode`] reads it back without copying: [`DisplayListView`] borrows the bytes, seeks
//!   to any page through the page index, and yields [`OpView`]s.
//! - [`glyph`] serves glyph outlines and face metrics for a canvas glyph atlas.
//! - [`svg`] paints a decoded page as SVG.
//!
//! Pure: faces arrive through `cdoc_layout::FontPort`, images through
//! `cdoc_model::ports::ImagePort`. Nothing here reads a file, the clock or randomness.

#![forbid(unsafe_code)]

mod caret;
mod codec;
mod decode;
mod doc_table;
mod encode;
mod encode_tables;
mod face_table;
pub mod format;
pub mod glyph;
mod link;
mod lower;
mod op;
mod ops_view;
mod parse;
pub mod svg;

pub use caret::{CaretLineView, CaretLinesView, Lines};
pub use decode::{decode, DisplayListView, FaceView, PageView};
pub use link::allowed_link;
pub use lower::lower;
pub use op::{
    DisplayList, DocumentInfo, FaceEntry, Glyph, GlyphMetric, Op, PageOps, Rgba, RunHead,
};
pub use ops_view::{Items, OpIter, OpView, Segments, ShapedGlyphs};

use cdoc_layout::{FaceKey, FontError, FontPort, Layout};

/// Why a display list could not be built or painted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PaintError {
    /// The font port no longer serves a face the layout used.
    #[error(transparent)]
    Font(#[from] FontError),
    /// A face's bytes did not parse.
    #[error("the face {0:?} did not parse as a TrueType or OpenType font")]
    Unreadable(FaceKey),
    /// A run names a face the layout's face table does not have.
    #[error("face {0} is not in the face table")]
    NoFace(u16),
    /// A requested page is not in the layout.
    #[error("page {0} is not in the layout")]
    NoPage(usize),
    /// A count or offset does not fit the format's 32 bits.
    #[error("the display list is larger than the format's 32-bit offsets allow")]
    TooLarge,
    /// The bytes are not a valid display list.
    #[error(transparent)]
    Decode(#[from] DecodeError),
    /// The image port had no image for a content id.
    #[error(transparent)]
    Image(#[from] cdoc_model::ports::ImageError),
}

/// Why bytes are not a display list this version reads.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    /// The magic number is wrong: not a display list at all.
    #[error("not a display list (bad magic)")]
    BadMagic,
    /// A version this decoder does not read.
    #[error("display list version {0} is not supported (this decoder reads {v})", v = format::VERSION)]
    Version(u16),
    /// The bytes end before a structure they declare.
    #[error("the display list is truncated at byte {0}")]
    Truncated(usize),
    /// A structure is inconsistent.
    #[error("malformed display list: {0}")]
    Malformed(&'static str),
    /// An opcode this version does not define.
    #[error("unknown opcode {0:#04x}")]
    UnknownOp(u8),
    /// A page slot past the end of the page index.
    #[error("no page slot {0}")]
    NoPage(usize),
}

/// The whole layout as display-list bytes.
pub fn encode(layout: &Layout, fonts: &dyn FontPort) -> Result<Vec<u8>, PaintError> {
    let all: Vec<usize> = (0..layout.pages.len()).collect();
    encode_pages(layout, fonts, &all)
}

/// Only `pages` (zero-based, in the order given) as display-list bytes, for repainting the
/// pages an edit changed. Each page keeps its number, and the face table is the layout's
/// whole table, so face ids mean the same as in a full list.
pub fn encode_pages(
    layout: &Layout,
    fonts: &dyn FontPort,
    pages: &[usize],
) -> Result<Vec<u8>, PaintError> {
    lower(layout, fonts, pages)?.to_bytes()
}
