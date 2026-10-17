//! The face table as read: each face's name and style, its em, and the glyph table its text
//! runs index into.

use crate::codec::Reader;
use crate::format::{STYLE_BOLD, STYLE_ITALIC};
use crate::op::{FaceEntry, GlyphMetric};
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

/// Reads the whole face table section; it must end exactly after its last entry.
pub fn read_faces(mut r: Reader<'_>, section: usize) -> Result<Vec<FaceView<'_>>, DecodeError> {
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
        let glyphs = r.len()?;
        let glyphs = (0..glyphs)
            .map(|_| {
                Ok(GlyphMetric {
                    id: u16_of(r.uv()?, "glyph id above 65535")?,
                    advance: u16_of(r.uv()?, "advance above 65535")?,
                })
            })
            .collect::<Result<_, DecodeError>>()?;
        faces.push(FaceView {
            family,
            bold: style & STYLE_BOLD != 0,
            italic: style & STYLE_ITALIC != 0,
            units_per_em,
            glyphs,
        });
    }
    if r.is_empty() {
        Ok(faces)
    } else {
        Err(DecodeError::Malformed("face table longer than its entries"))
    }
}
