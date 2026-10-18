//! A sparse column of a `GLYPH_RUN`: the glyphs that differ from their prediction, each as
//! a (`uv` gap, `uv` value) pair, read in place.

use crate::codec::Reader;
use crate::DecodeError;

/// The glyphs a column lists, in glyph order: (`uv` gap, value) pairs, where the glyph's
/// index is one after the previous entry's plus the gap. Kept small: a run's view is moved
/// whole through every op the iterator yields.
#[derive(Debug, Clone, Copy)]
pub struct Sparse<'a> {
    /// The entries after the pending one.
    pub rest: &'a [u8],
    /// The glyph the pending entry is for; `NONE` once there is none.
    pub at: u32,
    pub value: u32,
}

/// No pending entry.
pub const NONE: u32 = u32::MAX;

impl<'a> Sparse<'a> {
    pub const fn none() -> Self {
        Self {
            rest: &[],
            at: NONE,
            value: 0,
        }
    }

    pub fn new(entries: &'a [u8]) -> Result<Self, DecodeError> {
        let mut column = Self {
            rest: entries,
            at: 0,
            value: 0,
        };
        column.read(0)?;
        Ok(column)
    }

    #[expect(
        clippy::inline_always,
        reason = "measured: 5 % of decode on the 102-page list"
    )]
    #[inline(always)]
    pub fn read(&mut self, from: u32) -> Result<(), DecodeError> {
        match *self.rest {
            [] => {
                self.at = NONE;
                Ok(())
            }
            [gap, value, ref rest @ ..] if gap < 0x80 && value < 0x80 && from < NONE - 0x80 => {
                self.at = from + u32::from(gap);
                self.value = u32::from(value);
                self.rest = rest;
                Ok(())
            }
            _ => self.read_long(from),
        }
    }

    #[cold]
    #[inline(never)]
    fn read_long(&mut self, from: u32) -> Result<(), DecodeError> {
        let mut r = Reader::new(self.rest, 0);
        self.at = from
            .checked_add(r.uv()?)
            .filter(|at| *at != NONE)
            .ok_or(DecodeError::Malformed("a column entry's gap overflows"))?;
        self.value = r.uv()?;
        self.rest = r.rest();
        Ok(())
    }

    /// The entry for glyph `i`, if the column lists it.
    #[expect(
        clippy::inline_always,
        reason = "measured: 5 % of decode on the 102-page list"
    )]
    #[inline(always)]
    pub fn take(&mut self, i: u32) -> Result<Option<u32>, DecodeError> {
        if self.at != i {
            return Ok(None);
        }
        let value = self.value;
        self.read(i + 1)?;
        Ok(Some(value))
    }
}
