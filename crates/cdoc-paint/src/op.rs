//! The display list as owned values: what [`crate::lower`] builds from a layout, what
//! [`DisplayList::to_bytes`] writes, and what [`crate::DisplayListView::to_owned`] reads back.

use cdoc_layout::{FaceId, FaceKey, Pt100, Rect, Segment};
use cdoc_model::Color;

/// A colour with straight (not premultiplied) alpha, 255 opaque.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgba {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha, 255 opaque.
    pub a: u8,
}

impl Rgba {
    /// An opaque model colour.
    #[must_use]
    pub const fn opaque(colour: Color) -> Self {
        Self {
            r: colour.r,
            g: colour.g,
            b: colour.b,
            a: 255,
        }
    }

    /// `#RRGGBB`, the alpha left out (see [`Rgba::a`]).
    #[must_use]
    pub fn hex(self) -> String {
        Color::rgb(self.r, self.g, self.b).hex()
    }
}

/// What every glyph run starts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunHead {
    /// The face, an index into the face table.
    pub face: FaceId,
    /// The font size.
    pub size: Pt100,
    /// The fill colour.
    pub colour: Rgba,
    /// The baseline's left end.
    pub x: Pt100,
    /// The baseline.
    pub y: Pt100,
}

/// A glyph placed by its nominal advance, as a decoded text run yields it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NominalGlyph {
    /// The glyph id in the face.
    pub id: u16,
    /// How far the pen moves after it.
    pub advance: Pt100,
}

/// A glyph a face's text runs use, with its advance in font units (the face's `hmtx`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphMetric {
    /// The glyph id in the face.
    pub id: u16,
    /// The nominal advance, in font units.
    pub advance: u16,
}

/// A face-table entry: the face, its em, and the glyphs text runs set in it, in the order
/// their indices are written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceEntry {
    /// The face as the font port served it.
    pub key: FaceKey,
    /// Font units per em; never 0.
    pub units_per_em: u16,
    /// The glyphs text runs in this list use, each once.
    pub glyphs: Vec<GlyphMetric>,
}

/// Where the pen stands after `units` font units of advances at `size`: `round(units * size /
/// units_per_em)` hundredths of a point, rounding half up. The layout measures with the same
/// rule, so a run's pen ends exactly at its measured width.
#[must_use]
pub fn nominal_pen(units: i64, size: Pt100, units_per_em: u16) -> Pt100 {
    let per_em = i64::from(units_per_em.max(1));
    let value = units
        .saturating_mul(i64::from(size))
        .saturating_add(per_em / 2)
        .div_euclid(per_em);
    Pt100::try_from(value).unwrap_or(if value < 0 { Pt100::MIN } else { Pt100::MAX })
}

/// A shaped glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    /// The glyph id in the face.
    pub id: u16,
    /// How far the pen moves after it.
    pub advance: Pt100,
    /// Horizontal displacement from the pen, not moving it.
    pub x_offset: Pt100,
    /// Vertical displacement from the pen, y down, not moving it.
    pub y_offset: Pt100,
}

/// One painting operation, in page space (y down, hundredths of a point).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// Fill a rectangle.
    FillRect {
        /// Where.
        rect: Rect,
        /// Colour.
        colour: Rgba,
    },
    /// Stroke a rectangle's border, centred on its edges.
    StrokeRect {
        /// Where.
        rect: Rect,
        /// Stroke width.
        width: Pt100,
        /// Colour.
        colour: Rgba,
    },
    /// A straight stroke with butt caps.
    Line {
        /// Start.
        from: (Pt100, Pt100),
        /// End.
        to: (Pt100, Pt100),
        /// Stroke width.
        width: Pt100,
        /// Colour.
        colour: Rgba,
    },
    /// Fill a path, nonzero winding.
    FillPath {
        /// The outline.
        segments: Vec<Segment>,
        /// Colour.
        colour: Rgba,
    },
    /// Stroke a path with round caps and joins.
    StrokePath {
        /// The path.
        segments: Vec<Segment>,
        /// Stroke width.
        width: Pt100,
        /// Colour.
        colour: Rgba,
    },
    /// Unshaped text: glyph ids from the face's `cmap`, placed by nominal advances (see
    /// [`nominal_pen`]); every id is in the face entry's glyph table.
    TextRun {
        /// Face, size, colour, origin.
        head: RunHead,
        /// The glyph ids.
        glyphs: Vec<u16>,
    },
    /// Shaped text.
    GlyphRun {
        /// Face, size, colour, origin.
        head: RunHead,
        /// The glyphs.
        glyphs: Vec<Glyph>,
    },
    /// An image filling a rectangle; the bytes come from the painter's image port.
    Image {
        /// Where.
        rect: Rect,
        /// The model's content id.
        content_id: String,
    },
    /// A clickable area, its target exactly as the model holds it (see
    /// [`crate::allowed_link`]).
    Link {
        /// Where.
        rect: Rect,
        /// The target.
        href: String,
    },
    /// Intersect the clip with a rectangle until the matching [`Op::ClipPop`].
    ClipPush(Rect),
    /// Undo the most recent clip push.
    ClipPop,
}

/// One page's operations, in painting order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageOps {
    /// The zero-based page number in the layout.
    pub number: u32,
    /// Width.
    pub width: Pt100,
    /// Height.
    pub height: Pt100,
    /// Operations.
    pub ops: Vec<Op>,
}

/// A display list: the face table and some or all of a layout's pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayList {
    /// The faces glyph runs refer to by [`FaceId`].
    pub faces: Vec<FaceEntry>,
    /// The pages, in the order they are stored.
    pub pages: Vec<PageOps>,
}
