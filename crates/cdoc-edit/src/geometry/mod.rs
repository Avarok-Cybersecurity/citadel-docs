//! Where positions are on the pages: the **line map**, derived from the layout IR.
//!
//! The layout reports, for every line of document text it sets, where the line came from (its
//! region and the block path down to its paragraph) and its caret stops as gaps of that
//! paragraph at x positions, with the caret's extent ([`cdoc_layout::CaretLine`]). [`derive`]
//! resolves the provenance through the outline ([`resolve`]) and turns gap `k` into the
//! position after the paragraph's `k`-th unit; nothing reads the drawn text. Body lines come in
//! flow order (document order); the headers', footers' and footnotes' ("asides") are sorted by
//! position. A table's header rows repeated on a continuation page carry no lines: their
//! caret stays on the original rows.

pub(crate) mod derive;
mod lines;
mod query;
mod resolve;

use crate::position::Position;
use cdoc_layout::{Pt100, Rect};

/// A caret stop: a gap at an x on a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Stop {
    pub(crate) x: Pt100,
    pub(crate) pos: Position,
}

/// One line of a page, its stops in document (and x) order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Line {
    pub(crate) top: Pt100,
    pub(crate) bottom: Pt100,
    pub(crate) stops: Vec<Stop>,
}

impl Line {
    pub(crate) fn caret(&self, x: Pt100) -> Rect {
        Rect {
            x,
            y: self.top,
            width: 0,
            height: self.bottom - self.top,
        }
    }
}

/// The lines of one page: the body's, and the headers', footers' and footnotes' ("asides"),
/// each in document order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct PageMap {
    pub(crate) lines: Vec<Line>,
    pub(crate) aside: Vec<Line>,
}

/// Every page's line map, kept across edits: an edit drops the pages the layout reports
/// changed, and they are derived again (stops name units by id, so the maps of unchanged pages
/// stay right while indices shift).
#[derive(Debug, Clone, Default)]
pub(crate) struct Geometry {
    pub(crate) pages: Vec<Option<PageMap>>,
}

impl Geometry {
    /// Forgets `changed` pages and fits the page count.
    pub(crate) fn invalidate(&mut self, changed: &[usize], page_count: usize) {
        self.pages.resize(page_count, None);
        for &p in changed {
            if let Some(slot) = self.pages.get_mut(p) {
                *slot = None;
            }
        }
    }

    /// Forgets everything (the layout was redone from scratch).
    pub(crate) fn clear(&mut self, page_count: usize) {
        self.pages = vec![None; page_count];
    }

    pub(crate) fn page(&self, p: usize) -> &PageMap {
        self.pages[p].as_ref().expect("derived before it is read")
    }
}
