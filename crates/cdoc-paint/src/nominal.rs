//! A text run's glyphs as decoded: table indices turned into glyph ids and nominal advances.

use crate::face_table::FaceView;
use crate::op::{nominal_pen, GlyphMetric, NominalGlyph};
use crate::ops_view::Items;
use crate::DecodeError;
use cdoc_layout::Pt100;

/// A text run's glyphs, read in place: each index looked up in the face's glyph table and
/// placed by [`nominal_pen`].
#[derive(Debug, Clone)]
pub struct NominalGlyphs<'a> {
    indices: Items<'a, u32>,
    table: &'a [GlyphMetric],
    units_per_em: u16,
    size: Pt100,
    units: i64,
    pen: Pt100,
}

impl<'a> NominalGlyphs<'a> {
    pub(crate) fn new(indices: Items<'a, u32>, face: &'a FaceView<'a>, size: Pt100) -> Self {
        Self {
            indices,
            table: &face.glyphs,
            units_per_em: face.units_per_em,
            size,
            units: 0,
            pen: 0,
        }
    }

    #[inline]
    fn checked(&mut self) -> Result<Option<NominalGlyph>, DecodeError> {
        let Some(index) = self.indices.checked()? else {
            return Ok(None);
        };
        let m = usize::try_from(index)
            .ok()
            .and_then(|i| self.table.get(i))
            .ok_or(DecodeError::Malformed(
                "glyph index outside the face's table",
            ))?;
        self.units = self.units.saturating_add(i64::from(m.advance));
        let pen = nominal_pen(self.units, self.size, self.units_per_em);
        let advance = pen.wrapping_sub(self.pen);
        self.pen = pen;
        Ok(Some(NominalGlyph { id: m.id, advance }))
    }

    /// Every index in the table and the payload exactly used; the pen is not computed.
    pub(crate) fn check(mut self) -> Result<(), DecodeError> {
        let len = self.table.len();
        while let Some(index) = self.indices.checked()? {
            if usize::try_from(index).map_or(true, |i| i >= len) {
                return Err(DecodeError::Malformed(
                    "glyph index outside the face's table",
                ));
            }
        }
        self.indices.check()
    }
}

impl Iterator for NominalGlyphs<'_> {
    type Item = NominalGlyph;
    fn next(&mut self) -> Option<NominalGlyph> {
        self.checked().ok().flatten()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.indices.size_hint()
    }
}

impl ExactSizeIterator for NominalGlyphs<'_> {}
