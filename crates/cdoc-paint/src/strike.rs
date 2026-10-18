//! A strike: one face at one size, with the base advance (Pt100) of every glyph in the face's
//! glyph table. A run names its strike instead of its face and size, and its advances are
//! coded against the bases.

use crate::codec::Reader;
use crate::face_table::FaceView;
use crate::DecodeError;

/// A strike as the writer builds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Strike {
    /// The face id.
    pub face: u16,
    /// The font size, Pt100.
    pub size: i32,
    /// The base advance of each glyph in the face's glyph table, Pt100.
    pub bases: Vec<i32>,
}

/// A glyph in a strike: its id and base advance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub id: u16,
    pub base: i32,
}

/// A strike as the reader uses it: every glyph-table index's id and base together, so a
/// glyph is one lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrikeView {
    /// The face id.
    pub face: u16,
    /// The font size, Pt100.
    pub size: i32,
    /// By glyph-table index.
    pub glyphs: Vec<Slot>,
}

/// Reads the strike table that follows the faces.
pub fn read_strikes(
    r: &mut Reader<'_>,
    faces: &[FaceView<'_>],
) -> Result<Vec<StrikeView>, DecodeError> {
    let count = r.len()?;
    let mut strikes = Vec::with_capacity(count.min(4096));
    for _ in 0..count {
        let face = u16::try_from(r.uv()?).map_err(|_| DecodeError::Malformed("face id"))?;
        let entry = faces
            .get(usize::from(face))
            .ok_or(DecodeError::Malformed("a strike's face id out of range"))?;
        let size = r.zz()?;
        let mut glyphs = Vec::with_capacity(entry.glyphs.len());
        for g in &entry.glyphs {
            glyphs.push(Slot {
                id: g.id,
                base: r.zz()?,
            });
        }
        strikes.push(StrikeView { face, size, glyphs });
    }
    Ok(strikes)
}
