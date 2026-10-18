//! A shaped run: glyphs in font units, and the conversions layout needs (to hundredths of a
//! point at a font size, and between characters and glyphs).

mod hit;

pub use hit::ClusterSpan;

/// Hundredths of a PostScript point. 7200 is an inch. The one definition of the unit:
/// `cdoc-layout` re-exports it as its IR's unit, and the model's `Length` holds the same
/// integer.
pub type Pt100 = i32;

/// One positioned glyph, in font units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Glyph {
    /// Glyph id in the face.
    pub id: u16,
    /// Byte offset into the shaped text of the first character this glyph stands for. Glyphs
    /// of one cluster share it; clusters increase along the run.
    pub cluster: u32,
    /// How far the pen moves right after this glyph.
    pub x_advance: i32,
    /// How far the pen moves up after this glyph (0 in horizontal text).
    pub y_advance: i32,
    /// Horizontal displacement of the drawing from the pen, rightwards.
    pub x_offset: i32,
    /// Vertical displacement of the drawing from the pen, upwards.
    pub y_offset: i32,
}

/// The glyphs of one run, in visual (left to right) order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapedRun {
    /// The glyphs.
    pub glyphs: Vec<Glyph>,
}

/// `units` font units at `half_points` (the model's font size unit) in a face of
/// `units_per_em`, in hundredths of a point, rounded half away from zero. Integer arithmetic
/// throughout, so every platform gets the same answer.
#[must_use]
pub fn scale(units: i64, half_points: u32, units_per_em: u16) -> Pt100 {
    scale_wide(units, i64::from(half_points) * 50, units_per_em)
}

/// `units` font units at `size` hundredths of a point (a reduced super- or subscript size, a
/// small capital), rounded as [`scale`] rounds: the one rule every size is scaled by.
#[must_use]
pub fn scale_size(units: i64, size: Pt100, units_per_em: u16) -> Pt100 {
    scale_wide(units, i64::from(size), units_per_em)
}

fn scale_wide(units: i64, size: i64, units_per_em: u16) -> Pt100 {
    let numerator = units.saturating_mul(size);
    let denominator = i64::from(units_per_em.max(1));
    let half = denominator / 2;
    let rounded = if numerator >= 0 {
        numerator.saturating_add(half) / denominator
    } else {
        numerator.saturating_sub(half) / denominator
    };
    Pt100::try_from(rounded).unwrap_or(if rounded < 0 { Pt100::MIN } else { Pt100::MAX })
}

impl ShapedRun {
    /// A run with no glyphs, to shape into.
    #[must_use]
    pub fn empty() -> Self {
        Self { glyphs: Vec::new() }
    }

    /// The run's total advance in font units.
    #[must_use]
    pub fn advance(&self) -> i64 {
        self.glyphs.iter().map(|g| i64::from(g.x_advance)).sum()
    }

    /// The run's total advance in hundredths of a point.
    #[must_use]
    pub fn advance_pt100(&self, half_points: u32, units_per_em: u16) -> Pt100 {
        scale(self.advance(), half_points, units_per_em)
    }

    /// Where each glyph is drawn, `(x, y)` from the run's origin in hundredths of a point (y
    /// upwards). Each position is scaled from the exact font-unit sum, so rounding never
    /// accumulates along a line.
    #[must_use]
    pub fn positions_pt100(&self, half_points: u32, units_per_em: u16) -> Vec<(Pt100, Pt100)> {
        let mut pen = (0_i64, 0_i64);
        self.glyphs
            .iter()
            .map(|g| {
                let at = (
                    scale(pen.0 + i64::from(g.x_offset), half_points, units_per_em),
                    scale(pen.1 + i64::from(g.y_offset), half_points, units_per_em),
                );
                pen.0 += i64::from(g.x_advance);
                pen.1 += i64::from(g.y_advance);
                at
            })
            .collect()
    }
}
