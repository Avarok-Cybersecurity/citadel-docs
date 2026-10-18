//! The format's constants: magic, version, section sizes and opcodes. `docs/display-list.md`
//! describes every byte; this module is where the numbers live.
//!
//! Changing any layout the document describes is a format change: bump [`VERSION`], update
//! the document, and regenerate the golden files (`cargo run -p cdoc-paint --example
//! emit_golden`).

/// The first four bytes of every display list: `CDPL`.
pub const MAGIC: [u8; 4] = *b"CDPL";

/// The format version this crate writes and the only one it reads.
pub const VERSION: u16 = 3;

/// Bytes in the fixed header.
pub const HEADER_LEN: usize = 24;

/// Bytes in one page-index entry.
pub const INDEX_ENTRY_LEN: usize = 20;

/// Face-table style bit: bold.
pub const STYLE_BOLD: u8 = 1;

/// Face-table style bit: italic.
pub const STYLE_ITALIC: u8 = 2;

/// Fill a rectangle.
pub const OP_FILL_RECT: u8 = 0x01;
/// Stroke a rectangle's border, centred on its edges.
pub const OP_STROKE_RECT: u8 = 0x02;
/// A straight stroke with butt caps (rules, underlines, strikethrough, table borders).
pub const OP_LINE: u8 = 0x03;
/// Fill a path (nonzero winding).
pub const OP_FILL_PATH: u8 = 0x04;
/// Stroke a path with round caps and joins.
pub const OP_STROKE_PATH: u8 = 0x05;
/// Shaped glyphs: advances, per-glyph offsets and clusters, with the text they stand for.
/// (`0x10`, version 1's unshaped `TEXT_RUN`, is retired: every run is shaped.)
pub const OP_GLYPH_RUN: u8 = 0x11;
/// An image filling a rectangle.
pub const OP_IMAGE: u8 = 0x20;
/// A clickable area.
pub const OP_LINK: u8 = 0x21;
/// An unsigned signature field.
pub const OP_SIG_FIELD: u8 = 0x22;
/// Intersect the clip with a rectangle until the matching pop.
pub const OP_CLIP_PUSH: u8 = 0x30;
/// Undo the most recent clip push.
pub const OP_CLIP_POP: u8 = 0x31;

/// Path verb: start a subpath at one point.
pub const SEG_MOVE: u8 = 0;
/// Path verb: a straight line to one point.
pub const SEG_LINE: u8 = 1;
/// Path verb: a cubic Bézier through two control points to an end point.
pub const SEG_CUBIC: u8 = 2;
/// Path verb: close the subpath.
pub const SEG_CLOSE: u8 = 3;
