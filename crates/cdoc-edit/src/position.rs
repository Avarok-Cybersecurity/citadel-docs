//! Positions and selections. A [`Position`] is a gap in the document named by the unit just
//! before it, by that unit's CRDT id, so a remote insertion before the caret moves the caret
//! with the text instead of onto another character, and a remote deletion of that unit leaves
//! the caret where the unit was (deleted units keep their place in the sequence).

use crate::error::EditError;
use cdoc_crdt::{AtomKind, Doc, Id, Unit};

/// A caret position: the gap just after one unit (a character, an inline atom or a
/// paragraph's start atom). The gap after a paragraph atom is the start of that paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Position {
    /// The unit just before the gap.
    pub after: Id,
}

impl Position {
    /// The gap after unit `after`.
    #[must_use]
    pub const fn after(after: Id) -> Self {
        Self { after }
    }

    /// The gap's index: how many visible units come before it.
    pub fn index(self, doc: &Doc) -> Result<usize, EditError> {
        let at = doc.locate(self.after).ok_or(EditError::UnknownPosition)?;
        Ok(at.index + usize::from(at.visible))
    }

    /// The gap at `index` (`index >= 1`: the gap before everything is not a caret position).
    #[must_use]
    pub fn at(doc: &Doc, index: usize) -> Option<Self> {
        let before = index.checked_sub(1)?;
        doc.id_at(before).map(Self::after)
    }
}

/// Whether the caret may stand in the gap at `index`: after a paragraph's start atom or after
/// an inline unit.
#[must_use]
pub fn is_caret_gap(doc: &Doc, index: usize) -> bool {
    index > 0
        && matches!(
            doc.unit_at(index - 1),
            Some(Unit::Char(_) | Unit::Atom(AtomKind::Inline | AtomKind::Paragraph))
        )
}

/// The caret gap nearest `index`: `index` itself if it is one, else the first one after it,
/// else the last one before it.
pub fn caret_gap(doc: &Doc, index: usize) -> Result<usize, EditError> {
    let len = doc.len();
    let index = index.min(len);
    (index..=len)
        .find(|&g| is_caret_gap(doc, g))
        .or_else(|| (1..index).rev().find(|&g| is_caret_gap(doc, g)))
        .ok_or(EditError::NotApplicable("the document has no paragraph"))
}

/// Which way a caret moves or a selection runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Towards the start (left, up).
    Backward,
    /// Towards the end (right, down).
    Forward,
}

/// The unit a caret moves or a deletion removes by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Granularity {
    /// One character (cluster).
    Char,
    /// To the next word boundary (Ctrl+arrow).
    Word,
    /// To the start or end of the line (Home, End).
    LineBoundary,
    /// One line up or down, keeping the goal x.
    Line,
    /// To the start or end of the paragraph (Ctrl+up/down).
    Paragraph,
    /// To the start or end of the document (Ctrl+Home, Ctrl+End).
    Document,
}

/// A selection: where it started (`anchor`) and where the caret is (`head`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Selection {
    /// The fixed end.
    pub anchor: Position,
    /// The moving end, where the caret is drawn.
    pub head: Position,
}

impl Selection {
    /// A caret.
    #[must_use]
    pub const fn caret(at: Position) -> Self {
        Self {
            anchor: at,
            head: at,
        }
    }

    /// Whether anchor and head are the same gap.
    pub fn is_collapsed(&self, doc: &Doc) -> Result<bool, EditError> {
        Ok(self.anchor.index(doc)? == self.head.index(doc)?)
    }

    /// Which way the selection runs from anchor to head (`Forward` when collapsed).
    pub fn direction(&self, doc: &Doc) -> Result<Direction, EditError> {
        Ok(if self.head.index(doc)? < self.anchor.index(doc)? {
            Direction::Backward
        } else {
            Direction::Forward
        })
    }

    /// The selected gaps as an ascending index range.
    pub fn range(&self, doc: &Doc) -> Result<std::ops::Range<usize>, EditError> {
        let (a, h) = (self.anchor.index(doc)?, self.head.index(doc)?);
        Ok(a.min(h)..a.max(h))
    }
}
