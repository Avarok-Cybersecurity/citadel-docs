// Adapted from rMazing crates/cc-export/src/layout/ir.rs
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! What a page holds, before it is a PDF or a preview.
//!
//! One description of every page, drawn by two backends (the PDF and the SVG preview) so the
//! preview cannot disagree with the file.
//!
//! Coordinates are **page space**: origin at the top-left corner of the sheet, y downwards, in
//! hundredths of a point as integers, the unit of the model's `Length`. The model's
//! `PageAnchor` uses the same space, so a signature field's rectangle is copied, not converted.
//! The PDF backend flips y once, at the end.

mod caret;
mod glyphs;
mod page;
mod units;

pub use caret::{CaretLine, CaretStop, Provenance, Region};
pub use glyphs::{cluster_text, GlyphRun, PlacedGlyph};
pub use page::{Layout, LinkArea, OutlineEntry, Page, SignaturePlacement, Warning};
pub use units::{SpanKind, UnitSpan, Units};

use cdoc_model::Color;

/// Hundredths of a PostScript point. 7200 is an inch. Defined once, in `cdoc-shape` (the
/// lowest crate that measures), and re-exported here as the IR's unit.
pub use cdoc_shape::Pt100;

/// Hundredths back to points, for a backend that writes decimals.
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    reason = "hundredths of a point fit f32 exactly for any page"
)]
pub fn to_pt(value: Pt100) -> f32 {
    value as f32 / 100.0
}

/// A face as the font port served it: the family it actually is, not the one asked for.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FaceKey {
    /// The family name.
    pub family: String,
    /// Bold.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
}

/// An index into [`Layout::faces`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FaceId(pub u16);

/// An axis-aligned rectangle in page space, by its top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rect {
    /// Left edge.
    pub x: Pt100,
    /// Top edge.
    pub y: Pt100,
    /// Width.
    pub width: Pt100,
    /// Height.
    pub height: Pt100,
}

impl Rect {
    /// The same rectangle moved by `(dx, dy)`.
    #[must_use]
    pub const fn shifted(self, dx: Pt100, dy: Pt100) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            ..self
        }
    }
}

/// One segment of a path: a move, a straight line, a cubic curve (two control points and an
/// end point) or a close.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Segment {
    /// Start a subpath.
    Move(Pt100, Pt100),
    /// A straight line to a point.
    Line(Pt100, Pt100),
    /// A cubic Bézier curve.
    Cubic([Pt100; 6]),
    /// Close the subpath.
    Close,
}

/// One drawing operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Draw {
    /// Shaped text.
    Text(GlyphRun),
    /// A straight stroke.
    Rule {
        /// Start point.
        from: (Pt100, Pt100),
        /// End point.
        to: (Pt100, Pt100),
        /// Stroke width.
        width: Pt100,
        /// Colour.
        colour: Color,
    },
    /// A filled rectangle.
    Fill {
        /// Where.
        rect: Rect,
        /// Colour.
        colour: Color,
    },
    /// A filled path.
    Path {
        /// The outline.
        segments: Vec<Segment>,
        /// Colour.
        colour: Color,
    },
    /// A stroked path with round caps and joins.
    Stroke {
        /// The path.
        segments: Vec<Segment>,
        /// Stroke width.
        width: Pt100,
        /// Colour.
        colour: Color,
    },
    /// An image, by the rectangle it fills; the bytes arrive at the backend through its image
    /// port.
    Image {
        /// Where.
        rect: Rect,
        /// The model's content id.
        content_id: cdoc_model::ContentId,
    },
}

impl Draw {
    /// The same drawing moved by `(dx, dy)`.
    #[must_use]
    pub fn shifted(self, dx: Pt100, dy: Pt100) -> Self {
        let seg = |s: Segment| match s {
            Segment::Move(x, y) => Segment::Move(x + dx, y + dy),
            Segment::Line(x, y) => Segment::Line(x + dx, y + dy),
            Segment::Cubic([x1, y1, x2, y2, x3, y3]) => {
                Segment::Cubic([x1 + dx, y1 + dy, x2 + dx, y2 + dy, x3 + dx, y3 + dy])
            }
            Segment::Close => Segment::Close,
        };
        match self {
            Self::Text(run) => Self::Text(GlyphRun {
                x: run.x + dx,
                y: run.y + dy,
                ..run
            }),
            Self::Rule {
                from,
                to,
                width,
                colour,
            } => Self::Rule {
                from: (from.0 + dx, from.1 + dy),
                to: (to.0 + dx, to.1 + dy),
                width,
                colour,
            },
            Self::Fill { rect, colour } => Self::Fill {
                rect: rect.shifted(dx, dy),
                colour,
            },
            Self::Path { segments, colour } => Self::Path {
                segments: segments.into_iter().map(seg).collect(),
                colour,
            },
            Self::Stroke {
                segments,
                width,
                colour,
            } => Self::Stroke {
                segments: segments.into_iter().map(seg).collect(),
                width,
                colour,
            },
            Self::Image { rect, content_id } => Self::Image {
                rect: rect.shifted(dx, dy),
                content_id,
            },
        }
    }
}
