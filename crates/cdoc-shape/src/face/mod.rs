//! A font face, parsed once: character map, metrics, glyph classes and the layout tables, ready
//! for shaping.

mod be;
mod cmap;
pub(crate) mod gdef;
pub(crate) mod layout;

use crate::error::ShapeError;
use layout::{Layout, PosSubtable};
use ttf_parser::gsub::SubstitutionSubtable;
use ttf_parser::GlyphId;

/// A face's vertical metrics in font units, from `hhea`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerticalMetrics {
    /// Ascender, above the baseline (positive).
    pub ascender: i32,
    /// Descender, below the baseline (negative).
    pub descender: i32,
    /// Extra space between lines.
    pub line_gap: i32,
}

/// A glyph's bounding box in font units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphBounds {
    /// Left edge.
    pub x_min: i32,
    /// Bottom edge.
    pub y_min: i32,
    /// Right edge.
    pub x_max: i32,
    /// Top edge.
    pub y_max: i32,
}

/// A TrueType or OpenType face, parsed and prepared for shaping. Borrows the font bytes.
pub struct Face<'a> {
    pub(crate) ttf: ttf_parser::Face<'a>,
    cmap: cmap::Cmap<'a>,
    pub(crate) gdef: gdef::Gdef<'a>,
    pub(crate) gsub: Option<Layout<'a, SubstitutionSubtable<'a>>>,
    pub(crate) gpos: Option<Layout<'a, PosSubtable<'a>>>,
    pub(crate) kern: Option<ttf_parser::kern::Table<'a>>,
}

impl std::fmt::Debug for Face<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Face")
            .field("glyphs", &self.ttf.number_of_glyphs())
            .field("units_per_em", &self.ttf.units_per_em())
            .finish_non_exhaustive()
    }
}

impl<'a> Face<'a> {
    /// Parses face `index` of a font file (0 for a single-face file).
    pub fn parse(bytes: &'a [u8], index: u32) -> Result<Self, ShapeError> {
        let ttf = ttf_parser::Face::parse(bytes, index).map_err(|_| ShapeError::Unreadable)?;
        let glyphs = ttf.number_of_glyphs();
        let tables = ttf.tables();
        Ok(Self {
            cmap: cmap::Cmap::new(&ttf)?,
            gdef: gdef::Gdef::new(&ttf),
            gsub: tables.gsub.map(|t| layout::gsub(t, glyphs)),
            gpos: tables.gpos.map(|t| layout::gpos(t, glyphs)),
            kern: tables.kern,
            ttf,
        })
    }

    /// The nominal glyph for a character, `None` when the face has none. Variation selectors
    /// are not consulted.
    #[must_use]
    pub fn glyph(&self, c: char) -> Option<u16> {
        self.cmap.glyph(u32::from(c))
    }

    pub(crate) fn glyph_of(&self, c: u32) -> Option<u16> {
        self.cmap.glyph(c)
    }

    /// The number of glyphs in the face.
    #[must_use]
    pub fn glyph_count(&self) -> u16 {
        self.ttf.number_of_glyphs()
    }

    /// Font units per em.
    #[must_use]
    pub fn units_per_em(&self) -> u16 {
        self.ttf.units_per_em()
    }

    /// The horizontal advance of a glyph from `hmtx` (0 for a glyph out of range); a face
    /// without `hmtx` advances everything by one em.
    #[must_use]
    pub fn advance(&self, glyph: u16) -> i32 {
        if self.ttf.tables().hmtx.is_none() {
            return i32::from(self.units_per_em());
        }
        self.ttf
            .glyph_hor_advance(GlyphId(glyph))
            .map_or(0, i32::from)
    }

    /// Ascender, descender and line gap from `hhea`.
    #[must_use]
    pub fn vertical_metrics(&self) -> VerticalMetrics {
        VerticalMetrics {
            ascender: i32::from(self.ttf.ascender()),
            descender: i32::from(self.ttf.descender()),
            line_gap: i32::from(self.ttf.line_gap()),
        }
    }

    /// A glyph's bounding box; `None` for an empty glyph (a space) or one out of range.
    #[must_use]
    pub fn bounds(&self, glyph: u16) -> Option<GlyphBounds> {
        self.ttf
            .glyph_bounding_box(GlyphId(glyph))
            .map(|r| GlyphBounds {
                x_min: i32::from(r.x_min),
                y_min: i32::from(r.y_min),
                x_max: i32::from(r.x_max),
                y_max: i32::from(r.y_max),
            })
    }

    /// The legacy `kern` table's adjustment for a pair, summed over its horizontal, plain
    /// (not cross-stream, not state machine) subtables; `None` when no subtable lists it.
    #[must_use]
    pub fn legacy_kern(&self, left: u16, right: u16) -> Option<i32> {
        let mut found = None;
        for sub in self.kern?.subtables {
            if sub.horizontal && !sub.variable && !sub.has_cross_stream && !sub.has_state_machine {
                if let Some(v) = sub.glyphs_kerning(GlyphId(left), GlyphId(right)) {
                    found = Some(found.unwrap_or(0) + i32::from(v));
                }
            }
        }
        found
    }

    /// The caret positions inside a ligature glyph the font's `GDEF` gives, in font units from
    /// the glyph's origin; `None` when it gives none (or gives them as outline points).
    #[must_use]
    pub fn ligature_carets(&self, glyph: u16) -> Option<Vec<i32>> {
        self.gdef.carets(glyph)
    }
}
