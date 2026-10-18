//! The face table as read: each face's name and style, its em, the glyph table its text runs
//! index into and the char table they are predicted from, then the strike table.

use crate::chars::CharTable;
use crate::codec::Reader;
use crate::format::{STYLE_BOLD, STYLE_ITALIC};
use crate::op::{FaceEntry, GlyphMetric};
use crate::strike::{read_strikes, StrikeView};
use crate::DecodeError;
use cdoc_layout::FaceKey;

/// One face-table entry, its strings borrowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceView<'a> {
    /// The family the font port served.
    pub family: &'a str,
    /// Bold.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// Font units per em; never 0.
    pub units_per_em: u16,
    /// The glyphs text runs index into, with their advances in font units.
    pub glyphs: Vec<GlyphMetric>,
    /// Characters → glyph-table indices, what a run's glyphs are predicted from.
    pub(crate) chars: CharTable,
}

impl FaceView<'_> {
    /// The face as the layout names it.
    #[must_use]
    pub fn key(&self) -> FaceKey {
        FaceKey {
            family: self.family.to_owned(),
            bold: self.bold,
            italic: self.italic,
        }
    }

    /// The entry as an owned value.
    #[must_use]
    pub fn to_owned(&self) -> FaceEntry {
        FaceEntry {
            key: self.key(),
            units_per_em: self.units_per_em,
            glyphs: self.glyphs.clone(),
        }
    }
}

fn u16_of(value: u32, what: &'static str) -> Result<u16, DecodeError> {
    u16::try_from(value).map_err(|_| DecodeError::Malformed(what))
}

fn read_chars(r: &mut Reader<'_>, glyphs: usize) -> Result<CharTable, DecodeError> {
    let mut table = CharTable::default();
    let mut next: u64 = 0;
    for _ in 0..r.len()? {
        let code_point = u32::try_from(next + u64::from(r.uv()?))
            .ok()
            .filter(|cp| char::from_u32(*cp).is_some())
            .ok_or(DecodeError::Malformed(
                "a char table entry is not a character",
            ))?;
        let index = r.uv()?;
        if usize::try_from(index).map_or(true, |i| i >= glyphs) {
            return Err(DecodeError::Malformed(
                "a char table index is past the glyph table",
            ));
        }
        table.push(code_point, index);
        next = u64::from(code_point) + 1;
    }
    Ok(table)
}

/// Reads the whole face table section (faces, then strikes); it must end exactly after the
/// last strike.
pub fn read_faces(
    mut r: Reader<'_>,
    section: usize,
) -> Result<(Vec<FaceView<'_>>, Vec<StrikeView>), DecodeError> {
    let count = r.len()?;
    let mut faces = Vec::with_capacity(count.min(section));
    for _ in 0..count {
        let style = r.u8()?;
        if style & !(STYLE_BOLD | STYLE_ITALIC) != 0 {
            return Err(DecodeError::Malformed("unknown face style bits"));
        }
        let len = r.len()?;
        let family = std::str::from_utf8(r.take(len)?)
            .map_err(|_| DecodeError::Malformed("string is not UTF-8"))?;
        let units_per_em = u16_of(r.uv()?, "units per em above 65535")?;
        if units_per_em == 0 {
            return Err(DecodeError::Malformed("units per em is 0"));
        }
        let count = r.len()?;
        let mut glyphs = Vec::with_capacity(count.min(section));
        for _ in 0..count {
            glyphs.push(GlyphMetric {
                id: u16_of(r.uv()?, "glyph id above 65535")?,
                advance: u16_of(r.uv()?, "advance above 65535")?,
            });
        }
        let chars = read_chars(&mut r, glyphs.len())?;
        faces.push(FaceView {
            family,
            bold: style & STYLE_BOLD != 0,
            italic: style & STYLE_ITALIC != 0,
            units_per_em,
            glyphs,
            chars,
        });
    }
    let strikes = read_strikes(&mut r, &faces)?;
    if r.is_empty() {
        Ok((faces, strikes))
    } else {
        Err(DecodeError::Malformed("face table longer than its entries"))
    }
}
