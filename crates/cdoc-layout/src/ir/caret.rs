//! Where each line of text came from in the document, and where a caret can stand along it:
//! what an editor maps document positions through, without reading the drawn text.
//!
//! A line names its paragraph by **provenance**: the block list it was laid out from (a
//! section's body, a header, a footer, a footnote) and the path of block indices down to the
//! paragraph. Its **stops** are gaps between the paragraph's **units** at x positions: the
//! paragraph's characters and inline objects (tab, break, image, field, footnote reference,
//! form field, signature field), one unit each, counted as the document holds them, hidden
//! ones (a deletion the revision view leaves out) included. Gap 0 is the paragraph's start,
//! gap `k` follows its `k`-th unit. A list label, a footnote's number and the hyphen a broken
//! word ends in are drawn but are no units, so they have no stops. A table's header rows
//! repeated on a continuation page are copies and carry no lines: the original rows do.
//!
//! The layout does not compute a stop per character: that would cost every layout, the ones
//! for export included, what only an editor needs. A line's text is its **runs**: the page's
//! glyph runs whose [`super::Units`] are not `None`, in drawing order, each line owning the
//! next `runs` of them (lines in the page's order). A line keeps only the stops no run draws
//! (a tab, an image, a form or signature field, a page field, the one stop of an empty line);
//! [`crate::caret_stops`] puts a page's lines' stops together on demand, from the runs'
//! clusters and advances, as the painter draws them.

use super::Pt100;
use cdoc_model::{FootnoteId, HeaderFooterKind};

/// The block list a line was laid out from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    /// A section's body.
    Body {
        /// The section.
        section: u32,
    },
    /// A section's header.
    Header {
        /// The section.
        section: u32,
        /// Which of its headers.
        kind: HeaderFooterKind,
    },
    /// A section's footer.
    Footer {
        /// The section.
        section: u32,
        /// Which of its footers.
        kind: HeaderFooterKind,
    },
    /// A footnote's body.
    Footnote(FootnoteId),
}

/// A line's paragraph.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Provenance {
    /// The block list.
    pub region: Region,
    /// Block indices from the region's list down to the paragraph: `[block]` for a paragraph
    /// of the list, `[table, row, cell, block]` for one in a table's cell (cells counted
    /// within their row), and so on for nested tables.
    pub path: Vec<u32>,
}

/// A gap of the paragraph at an x position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CaretStop {
    /// The gap: 0 at the paragraph's start, `k` after its `k`-th unit.
    pub gap: u32,
    /// Where on the line, page space.
    pub x: Pt100,
}

/// One drawn line of a paragraph: where it is, which runs are its text, and the stops of what
/// no run draws.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CaretLine {
    /// Its paragraph.
    pub from: Provenance,
    /// Top of the caret: the line's tallest ascent above its baseline.
    pub top: Pt100,
    /// Bottom of the caret: its deepest descent below the baseline.
    pub bottom: Pt100,
    /// How many of the page's document-text runs, after the previous lines', are this one's.
    pub runs: u32,
    /// The stops of the units no run draws, in x order; an empty line has the one of the gap
    /// it stands at.
    pub stops: Vec<CaretStop>,
}

impl CaretLine {
    /// The same line moved by `(dx, dy)`.
    #[must_use]
    pub fn shifted(mut self, dx: Pt100, dy: Pt100) -> Self {
        self.top += dy;
        self.bottom += dy;
        for s in &mut self.stops {
            s.x += dx;
        }
        self
    }
}
