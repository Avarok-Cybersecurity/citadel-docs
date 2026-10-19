//! Checking a `GLYPH_RUN` once, when its page is opened, in time proportional to its listed
//! glyphs rather than all of them: a predicted glyph cannot be wrong (its cluster is one
//! character on, its glyph the char table's or index 0), so only the columns are read: every
//! listed index is in range, every listed cluster is on a character boundary and not before
//! the glyph ahead of it, and each column ends exactly. A run whose text is not all ASCII is
//! walked glyph by glyph instead, since its clusters move by more than one byte.

use crate::codec::unzigzag;
use crate::run_view::{malformed, ShapedGlyphs, ADVANCES, CLUSTERS, IDS, X_OFFSETS, Y_OFFSETS};
use crate::sparse::{Sparse, NONE};
use crate::DecodeError;

/// Reads a whole column, handing each entry (glyph index, value) to `each`; every entry must
/// be for a glyph of the run.
fn entries(
    mut column: Sparse<'_>,
    len: u32,
    mut each: impl FnMut(u32, u32) -> Result<(), DecodeError>,
) -> Result<(), DecodeError> {
    while column.at != NONE {
        if column.at >= len {
            return Err(malformed("a column lists a glyph past the run's last"));
        }
        let at = column.at;
        each(at, column.value)?;
        column.read(at + 1)?;
    }
    Ok(())
}

impl ShapedGlyphs<'_> {
    /// Checks the run; iterating a checked run cannot fail.
    pub(crate) fn check(self) -> Result<(), DecodeError> {
        let len = self.len;
        if len > 0 && self.strike.glyphs.is_empty() {
            return Err(malformed("a run in a face with no glyphs has glyphs"));
        }
        let used = len & 7;
        if used != 0 && self.plus_one.last().is_some_and(|b| b >> used != 0) {
            return Err(malformed("the advance bitmap's padding bits are set"));
        }
        let table = self.strike.glyphs.len();
        entries(self.columns[IDS], len, |_, index| {
            if (index as usize) < table {
                Ok(())
            } else {
                Err(malformed("a glyph index is past the glyph table"))
            }
        })?;
        for k in [ADVANCES, X_OFFSETS, Y_OFFSETS] {
            entries(self.columns[k], len, |_, _| Ok(()))?;
        }
        if self.text.is_ascii() {
            self.check_ascii_clusters()
        } else {
            self.walk()
        }
    }

    /// In ASCII text every character is one byte, so glyph `i` after a listed cluster `c` at
    /// glyph `k` is predicted at `min(c + i - k, text length)`.
    fn check_ascii_clusters(&self) -> Result<(), DecodeError> {
        let end = i64::try_from(self.text.len()).map_err(|_| malformed("text length"))?;
        let (mut from, mut cluster) = (0_i64, 0_i64);
        entries(self.columns[CLUSTERS], self.len, |at, delta| {
            let at = i64::from(at);
            let predicted = (cluster + at - from).min(end);
            let ahead = if at == 0 {
                0
            } else {
                (cluster + at - 1 - from).min(end)
            };
            let actual = predicted + i64::from(unzigzag(delta));
            if actual < ahead || actual > end {
                return Err(malformed(
                    "a cluster decreases or is not a character boundary of the text",
                ));
            }
            (from, cluster) = (at, actual);
            Ok(())
        })
    }

    /// Steps through every glyph (for text with multi-byte characters).
    fn walk(mut self) -> Result<(), DecodeError> {
        while self.i < self.len {
            self.step()?;
        }
        Ok(())
    }
}
