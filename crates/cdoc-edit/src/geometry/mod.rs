//! Where positions are on the pages: the **line map**, derived from the layout IR.
//!
//! The layout IR carries text runs, not document positions, so this adapter recovers them:
//! [`derive`] walks a page's text runs in drawing order and matches each against the
//! document's units at a cursor ([`matcher`]): the body's cursor reads on from where the page
//! before stopped, skipping header, footer and footnote subtrees; each of those subtrees has
//! a cursor of its own, fresh on every page, since a header repeats. Matching skips what is
//! not drawn (spaces at line ends, atoms, block boundaries); a run that matches nowhere (a
//! list label, a lone field value, a footnote number) is not editable. Each matched
//! character's caret stops come from the display list's own glyph advances
//! (`cdoc_paint::lower` + [`cdoc_paint::nominal_pen`]), so a caret stands exactly where the
//! painter drew the glyph edge. A line is the runs that continue rightwards beside each other
//! with no block boundary between them (raised and lowered runs stay on their line); its
//! height is the faces' ascent and descent.
//!
//! **This module is the only place that knows the IR's text representation.** When the layout
//! emits shaped glyph runs with cluster maps instead of `TextRun`s, `derive.rs` and
//! `matcher.rs` read clusters from them and nothing else in the crate changes. Known
//! approximations until then (the layout does not report positions): empty paragraphs get a
//! synthesised line between their neighbours, on the page after the text before them; a run
//! whose text equals the next text at the cursor is indistinguishable from it (a numbered
//! paragraph whose text starts with its own label); the caret of a repeated header is drawn
//! on the first page showing it; deleted tracked changes hidden by the revision view stop the
//! matcher on that page.

pub(crate) mod derive;
mod lines;
mod matcher;
mod query;

use crate::position::Position;
use cdoc_layout::{FaceKey, Pt100, Rect};
use std::collections::BTreeMap;

pub(crate) use derive::FaceMetrics;

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
/// changed, and they are derived again from the page before them (stops name units by id, so
/// the maps of unchanged pages stay right while indices shift).
#[derive(Debug, Clone, Default)]
pub(crate) struct Geometry {
    pub(crate) pages: Vec<Option<PageMap>>,
    pub(crate) metrics: BTreeMap<FaceKey, FaceMetrics>,
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
