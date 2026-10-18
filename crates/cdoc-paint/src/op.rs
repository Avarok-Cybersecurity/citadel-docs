//! The display list as owned values: what [`crate::lower`] builds from a layout, what
//! [`DisplayList::to_bytes`] writes, and what [`crate::DisplayListView::to_owned`] reads back.

use cdoc_layout::{CaretLine, FaceId, FaceKey, OutlineEntry, Pt100, Rect, Segment};
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

/// A glyph a face's runs use, with its advance in font units (the face's `hmtx`): what a
/// glyph atlas prepares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphMetric {
    /// The glyph id in the face.
    pub id: u16,
    /// The nominal advance, in font units.
    pub advance: u16,
}

/// A face-table entry: the face, its em, and the glyphs the list's runs set in it, in order
/// of first use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceEntry {
    /// The face as the font port served it.
    pub key: FaceKey,
    /// Font units per em; never 0.
    pub units_per_em: u16,
    /// The glyphs the runs in this list use, each once.
    pub glyphs: Vec<GlyphMetric>,
}

/// A shaped glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    /// The glyph id in the face.
    pub id: u16,
    /// Byte offset in the run's text of the first character the glyph stands for.
    pub cluster: u32,
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
    /// Shaped text: glyphs at their advances and offsets, and the characters they stand
    /// for (each glyph's cluster is a byte offset into `text`).
    GlyphRun {
        /// Face, size, colour, origin.
        head: RunHead,
        /// The characters, for selection and search.
        text: String,
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
    /// An unsigned signature field: where a signer signs. A painter may draw nothing for it;
    /// the PDF writes it as a `/Sig` form field.
    SigField {
        /// Where.
        rect: Rect,
        /// The field's id in the model.
        id: String,
        /// The signer role.
        role: String,
    },
    /// Intersect the clip with a rectangle until the matching [`Op::ClipPop`].
    ClipPush(Rect),
    /// Undo the most recent clip push.
    ClipPop,
    /// A page's lines of document text, in the layout's order: where each came from, its caret
    /// extent, its runs and the stops no run draws; never empty; painted by nobody.
    CaretLines(Vec<CaretLine>),
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

/// What the document says about itself, beside its pages: the whole layout's, in a partial
/// list too.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentInfo {
    /// The title, for a file's metadata.
    pub title: Option<String>,
    /// The language (BCP 47).
    pub language: Option<String>,
    /// The headings, in document order; each names its page by page number.
    pub outline: Vec<OutlineEntry>,
}

/// A display list: the face table, the document's own description, and some or all of a
/// layout's pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayList {
    /// The faces glyph runs refer to by [`FaceId`].
    pub faces: Vec<FaceEntry>,
    /// Title, language and outline.
    pub document: DocumentInfo,
    /// The pages, in the order they are stored.
    pub pages: Vec<PageOps>,
}
