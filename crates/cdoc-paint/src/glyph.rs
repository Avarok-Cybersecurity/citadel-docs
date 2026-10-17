//! Glyph outlines and face metrics, for a painter that rasterises glyphs itself (the canvas
//! editor's glyph atlas). Both are small binary records described in `docs/display-list.md`
//! § Glyph paths; coordinates are **font units, y up**, as the font stores them, so one path
//! serves every size and the painter applies `size / units_per_em` and the y flip.

use ttf_parser::{Face, GlyphId, OutlineBuilder};

/// Glyph-path format version, the record's first byte.
pub const PATH_VERSION: u8 = 1;
/// Bytes before the first command: version, reserved, command count, bounding box.
pub const PATH_HEADER_LEN: usize = 12;
/// Verb: start a subpath (`x y`).
pub const VERB_MOVE: u8 = 0;
/// Verb: a line (`x y`).
pub const VERB_LINE: u8 = 1;
/// Verb: a quadratic curve (`cx cy x y`).
pub const VERB_QUAD: u8 = 2;
/// Verb: a cubic curve (`c1x c1y c2x c2y x y`).
pub const VERB_CUBIC: u8 = 3;
/// Verb: close the subpath.
pub const VERB_CLOSE: u8 = 4;

/// Why an outline or metrics could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GlyphError {
    /// The bytes are not a TrueType or OpenType font.
    #[error("the face did not parse as a TrueType or OpenType font")]
    Unreadable,
    /// The face has no glyph with this id.
    #[error("the face has no glyph {0}")]
    NoGlyph(u16),
    /// The outline has more commands than the record's 16-bit count.
    #[error("glyph {0} has more than 65535 path commands")]
    TooComplex(u16),
}

/// A face's vertical metrics in font units: what a painter needs to size a glyph atlas cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceMetrics {
    /// Font units per em.
    pub units_per_em: u16,
    /// Ascender above the baseline (positive).
    pub ascender: i16,
    /// Descender below the baseline (negative, as fonts store it).
    pub descender: i16,
    /// Extra space between lines.
    pub line_gap: i16,
}

impl FaceMetrics {
    /// The 8-byte record: `units_per_em` u16, then `ascender`, `descender`, `line_gap` i16,
    /// little-endian.
    #[must_use]
    pub fn to_bytes(self) -> [u8; 8] {
        let mut out = [0; 8];
        out[0..2].copy_from_slice(&self.units_per_em.to_le_bytes());
        out[2..4].copy_from_slice(&self.ascender.to_le_bytes());
        out[4..6].copy_from_slice(&self.descender.to_le_bytes());
        out[6..8].copy_from_slice(&self.line_gap.to_le_bytes());
        out
    }
}

fn parse(face_bytes: &[u8]) -> Result<Face<'_>, GlyphError> {
    Face::parse(face_bytes, 0).map_err(|_| GlyphError::Unreadable)
}

/// The face's metrics (`hhea`, or `OS/2` typographic metrics when the font asks for them).
pub fn face_metrics(face_bytes: &[u8]) -> Result<FaceMetrics, GlyphError> {
    let face = parse(face_bytes)?;
    Ok(FaceMetrics {
        units_per_em: face.units_per_em(),
        ascender: face.ascender(),
        descender: face.descender(),
        line_gap: face.line_gap(),
    })
}

/// Collects outline commands as rounded 16-bit font units.
struct Path {
    out: Vec<u8>,
    commands: usize,
}

impl Path {
    fn command(&mut self, verb: u8, coords: &[f32]) {
        self.out.push(verb);
        for c in coords {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "clamped to i16; font units are 16-bit in every outline format"
            )]
            let unit = c.round().clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;
            self.out.extend_from_slice(&unit.to_le_bytes());
        }
        self.commands += 1;
    }
}

impl OutlineBuilder for Path {
    fn move_to(&mut self, x: f32, y: f32) {
        self.command(VERB_MOVE, &[x, y]);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.command(VERB_LINE, &[x, y]);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.command(VERB_QUAD, &[x1, y1, x, y]);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.command(VERB_CUBIC, &[x1, y1, x2, y2, x, y]);
    }
    fn close(&mut self) {
        self.command(VERB_CLOSE, &[]);
    }
}

/// The outline of `glyph_id` as a glyph-path record. A glyph with no outline (a space) is a
/// record with zero commands and an all-zero bounding box.
pub fn glyph_path(face_bytes: &[u8], glyph_id: u16) -> Result<Vec<u8>, GlyphError> {
    let face = parse(face_bytes)?;
    if glyph_id >= face.number_of_glyphs() {
        return Err(GlyphError::NoGlyph(glyph_id));
    }
    let mut path = Path {
        out: vec![0; PATH_HEADER_LEN],
        commands: 0,
    };
    let bbox = face.outline_glyph(GlyphId(glyph_id), &mut path);
    let count = u16::try_from(path.commands).map_err(|_| GlyphError::TooComplex(glyph_id))?;
    let mut out = path.out;
    out[0] = PATH_VERSION;
    out[2..4].copy_from_slice(&count.to_le_bytes());
    if let Some(b) = bbox {
        for (k, v) in [b.x_min, b.y_min, b.x_max, b.y_max].into_iter().enumerate() {
            out[4 + 2 * k..6 + 2 * k].copy_from_slice(&v.to_le_bytes());
        }
    }
    Ok(out)
}
