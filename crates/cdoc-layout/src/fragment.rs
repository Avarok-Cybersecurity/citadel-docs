//! The unit the flow places: a horizontal strip with a height, what it draws, and what it
//! marks (links, signature fields, fields to compute, headings, footnote references), all in
//! coordinates local to the strip's top-left corner.

use crate::ir::{Draw, GlyphRun, Pt100, Rect};
use cdoc_model::{BreakKind, FieldId, FieldKind, FootnoteId};

/// A page-dependent field where it was placed: shaped with the cached text until its page
/// is known, then shaped again with the value, at the same origin in the same face.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldRun {
    /// What it computes.
    pub kind: FieldKind,
    /// The run, holding the cached text until it is computed.
    pub run: GlyphRun,
    /// The features the run's style shapes with.
    pub features: cdoc_shape::Features,
}

/// Something a fragment carries besides drawings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mark {
    /// A hyperlink area.
    Link {
        /// Where.
        rect: Rect,
        /// The URL.
        href: String,
    },
    /// A signature field's box.
    Signature {
        /// The field.
        id: FieldId,
        /// Who signs.
        role: String,
        /// Where.
        rect: Rect,
    },
    /// A page-dependent field, set again once its page is known.
    Field(FieldRun),
    /// The fragment is the first line of a heading.
    Heading {
        /// Outline level.
        level: u8,
        /// Heading text.
        title: String,
    },
    /// A footnote reference: its body goes at the foot of the page this fragment lands on.
    Footnote(FootnoteId),
}

impl Mark {
    /// The same mark moved by `(dx, dy)`.
    #[must_use]
    pub fn shifted(self, dx: Pt100, dy: Pt100) -> Self {
        match self {
            Self::Link { rect, href } => Self::Link {
                rect: rect.shifted(dx, dy),
                href,
            },
            Self::Signature { id, role, rect } => Self::Signature {
                id,
                role,
                rect: rect.shifted(dx, dy),
            },
            Self::Field(field) => Self::Field(FieldRun {
                run: GlyphRun {
                    x: field.run.x + dx,
                    y: field.run.y + dy,
                    ..field.run
                },
                ..field
            }),
            other @ (Self::Heading { .. } | Self::Footnote(_)) => other,
        }
    }
}

/// A strip: one line of a paragraph, or one row of a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    /// Height, top to the next strip's top.
    pub height: Pt100,
    /// Drawings, local to the strip's top-left.
    pub draws: Vec<Draw>,
    /// Marks, local to the strip's top-left.
    pub marks: Vec<Mark>,
    /// A forced break after this strip.
    pub break_after: Option<BreakKind>,
}

impl Fragment {
    /// A strip with nothing on it.
    #[must_use]
    pub const fn empty(height: Pt100) -> Self {
        Self {
            height,
            draws: Vec::new(),
            marks: Vec::new(),
            break_after: None,
        }
    }

    /// Appends another strip's contents, moved to `(dx, dy)` within this one.
    pub fn absorb(&mut self, other: Self, dx: Pt100, dy: Pt100) {
        self.draws
            .extend(other.draws.into_iter().map(|d| d.shifted(dx, dy)));
        self.marks
            .extend(other.marks.into_iter().map(|m| m.shifted(dx, dy)));
    }

    /// The footnotes referenced from this strip.
    pub fn footnotes(&self) -> impl Iterator<Item = FootnoteId> + '_ {
        self.marks.iter().filter_map(|m| match m {
            Mark::Footnote(id) => Some(*id),
            _ => None,
        })
    }
}

/// A paragraph or a table, as the flow sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Word's four independent pagination flags"
)]
pub struct Block {
    /// Space above the first strip; dropped at the top of a column.
    pub space_before: Pt100,
    /// Space below the last strip.
    pub space_after: Pt100,
    /// The strips, top to bottom; never empty.
    pub strips: Vec<Fragment>,
    /// Never split across columns.
    pub keep_together: bool,
    /// Kept in the same column as the next block's first strip.
    pub keep_with_next: bool,
    /// Starts a new page.
    pub page_break_before: bool,
    /// Widow and orphan control: never leave a single line of it at either side of a break.
    pub widow_control: bool,
    /// Strips repeated at the head of each continuation (table header rows).
    pub repeat_head: usize,
}

impl Block {
    /// Height with no break inside it.
    #[must_use]
    pub fn height(&self) -> Pt100 {
        self.space_before + self.strips.iter().map(|s| s.height).sum::<Pt100>() + self.space_after
    }
}
