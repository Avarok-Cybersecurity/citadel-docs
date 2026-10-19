//! A `GLYPH_RUN`'s glyphs read in place from its columns. Every glyph is predicted (its
//! cluster one character after the previous glyph's, its glyph from the char table, its
//! advance the strike's base or one more, no offsets) and each column lists only the glyphs
//! that differ. `run_check` checks a run once, so iterating it cannot fail.

use crate::chars::{after, char_at, predicted, CharTable};
use crate::codec::{unzigzag, Reader};
use crate::face_table::FaceView;
use crate::format::{
    RUN_ADVANCES, RUN_CLUSTERS, RUN_COLUMNS, RUN_IDS, RUN_PLUS_ONE, RUN_X_OFFSETS, RUN_Y_OFFSETS,
};
use crate::op::{Glyph, Rgba, RunHead};
use crate::sparse::{Sparse, NONE};
use crate::strike::{Slot, StrikeView};
use crate::DecodeError;
use cdoc_layout::FaceId;

/// Column order in `ShapedGlyphs::columns`.
pub(crate) const IDS: usize = 0;
pub(crate) const CLUSTERS: usize = 1;
pub(crate) const ADVANCES: usize = 2;
pub(crate) const X_OFFSETS: usize = 3;
pub(crate) const Y_OFFSETS: usize = 4;

/// A shaped run's glyphs, read in place.
#[derive(Debug, Clone)]
pub struct ShapedGlyphs<'a> {
    pub(crate) text: &'a str,
    pub(crate) chars: &'a CharTable,
    pub(crate) strike: &'a StrikeView,
    pub(crate) plus_one: &'a [u8],
    pub(crate) columns: [Sparse<'a>; 5],
    /// No column lists a glyph before this one: until here every glyph is its prediction.
    pub(crate) quiet: u32,
    pub(crate) len: u32,
    pub(crate) i: u32,
    pub(crate) cluster: u32,
    pub(crate) next_cluster: u32,
}

/// The parts of a `GLYPH_RUN` payload.
pub struct RunParts<'a> {
    pub head: RunHead,
    pub text: &'a str,
    pub glyphs: ShapedGlyphs<'a>,
}

fn column<'a>(r: &mut Reader<'a>, flags: u8, flag: u8) -> Result<Sparse<'a>, DecodeError> {
    if flags & flag == 0 {
        return Ok(Sparse::none());
    }
    let len = r.len()?;
    Sparse::new(r.take(len)?)
}

fn quiet(columns: &[Sparse<'_>; 5]) -> u32 {
    columns.iter().map(|c| c.at).min().unwrap_or(NONE)
}

/// Reads a `GLYPH_RUN` payload's head and locates its columns; the payload must end there.
pub fn parse_run<'a>(
    mut r: Reader<'a>,
    faces: &'a [FaceView<'a>],
    strikes: &'a [StrikeView],
) -> Result<RunParts<'a>, DecodeError> {
    let strike = strikes
        .get(r.len()?)
        .ok_or(DecodeError::Malformed("strike id out of range"))?;
    let colour = Rgba {
        r: r.u8()?,
        g: r.u8()?,
        b: r.u8()?,
        a: r.u8()?,
    };
    let (x, y) = (r.zz()?, r.zz()?);
    let len = r.len()?;
    let text = std::str::from_utf8(r.take(len)?)
        .map_err(|_| DecodeError::Malformed("string is not UTF-8"))?;
    let glyphs = r.uv()?;
    let flags = r.u8()?;
    if flags & !RUN_COLUMNS != 0 {
        return Err(DecodeError::Malformed("unknown glyph-run column flags"));
    }
    let ids = column(&mut r, flags, RUN_IDS)?;
    let clusters = column(&mut r, flags, RUN_CLUSTERS)?;
    let plus_one = if flags & RUN_PLUS_ONE == 0 {
        &[][..]
    } else {
        r.take(glyphs.div_ceil(8) as usize)?
    };
    let advances = column(&mut r, flags, RUN_ADVANCES)?;
    let x_offsets = column(&mut r, flags, RUN_X_OFFSETS)?;
    let y_offsets = column(&mut r, flags, RUN_Y_OFFSETS)?;
    if !r.is_empty() {
        return Err(DecodeError::Malformed("payload longer than its fields"));
    }
    let head = RunHead {
        face: FaceId(strike.face),
        size: strike.size,
        colour,
        x,
        y,
    };
    let columns = [ids, clusters, advances, x_offsets, y_offsets];
    let glyphs = ShapedGlyphs {
        text,
        chars: &faces[usize::from(strike.face)].chars,
        strike,
        plus_one,
        quiet: quiet(&columns),
        columns,
        len: glyphs,
        i: 0,
        cluster: 0,
        next_cluster: 0,
    };
    Ok(RunParts { head, text, glyphs })
}

pub(crate) const fn malformed(what: &'static str) -> DecodeError {
    DecodeError::Malformed(what)
}

impl ShapedGlyphs<'_> {
    /// The glyph predicted at `cluster` and the width of the character there.
    #[expect(
        clippy::inline_always,
        reason = "measured: 5 % of decode on the 102-page list"
    )]
    #[inline(always)]
    pub(crate) fn predict(&self, cluster: u32) -> (Option<Slot>, u32) {
        let (index, width) = match self.text.as_bytes().get(cluster as usize) {
            Some(&b) if b < 0x80 => (self.chars.predict(u32::from(b)), 1),
            _ => predicted(self.chars, char_at(self.text, cluster as usize)),
        };
        (self.strike.glyphs.get(index as usize).copied(), width)
    }

    #[inline]
    fn plus_one(&self, i: u32) -> i32 {
        i32::from(
            self.plus_one
                .get((i >> 3) as usize)
                .map_or(0, |b| (b >> (i & 7)) & 1),
        )
    }

    /// The next glyph, every prediction and exception checked.
    #[inline]
    pub(crate) fn step(&mut self) -> Result<Glyph, DecodeError> {
        let i = self.i;
        self.i += 1;
        if i < self.quiet {
            let cluster = self.next_cluster;
            let (slot, width) = self.predict(cluster);
            let slot = slot.ok_or(malformed("a run in a face with no glyphs has glyphs"))?;
            self.cluster = cluster;
            self.next_cluster = after(cluster, width);
            return Ok(Glyph {
                id: slot.id,
                cluster,
                advance: slot.base.wrapping_add(self.plus_one(i)),
                x_offset: 0,
                y_offset: 0,
            });
        }
        let glyph = self.listed(i);
        self.quiet = quiet(&self.columns);
        glyph
    }

    /// Glyph `i`, which some column lists.
    fn listed(&mut self, i: u32) -> Result<Glyph, DecodeError> {
        let mut cluster = self.next_cluster;
        if let Some(delta) = self.columns[CLUSTERS].take(i)? {
            let at = i64::from(cluster) + i64::from(unzigzag(delta));
            cluster = u32::try_from(at)
                .ok()
                .filter(|c| *c >= self.cluster && self.text.is_char_boundary(*c as usize))
                .ok_or(malformed(
                    "a cluster decreases or is not a character boundary of the text",
                ))?;
        }
        let (predicted, width) = self.predict(cluster);
        let slot = match self.columns[IDS].take(i)? {
            Some(index) => Some(
                *self
                    .strike
                    .glyphs
                    .get(index as usize)
                    .ok_or(malformed("a glyph index is past the glyph table"))?,
            ),
            None => predicted,
        };
        let slot = slot.ok_or(malformed("a run in a face with no glyphs has glyphs"))?;
        let advance = match self.columns[ADVANCES].take(i)? {
            Some(delta) => slot.base.wrapping_add(unzigzag(delta)),
            None => slot.base.wrapping_add(self.plus_one(i)),
        };
        let x_offset = self.columns[X_OFFSETS].take(i)?.map_or(0, unzigzag);
        let y_offset = self.columns[Y_OFFSETS].take(i)?.map_or(0, unzigzag);
        self.cluster = cluster;
        self.next_cluster = after(cluster, width);
        Ok(Glyph {
            id: slot.id,
            cluster,
            advance,
            x_offset,
            y_offset,
        })
    }
}

impl Iterator for ShapedGlyphs<'_> {
    type Item = Glyph;
    #[inline]
    fn next(&mut self) -> Option<Glyph> {
        if self.i >= self.len {
            return None;
        }
        self.step().ok()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = (self.len - self.i) as usize;
        (left, Some(left))
    }
}

impl ExactSizeIterator for ShapedGlyphs<'_> {}
