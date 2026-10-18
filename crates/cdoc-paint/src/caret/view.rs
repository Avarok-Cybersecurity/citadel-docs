//! Caret lines read in place, and the checks a page's caret lines pass when it is opened.

use super::provenance;
use crate::codec::{Decode, Reader};
use crate::format::{LINE_NEW, LINE_NO_STOPS, LINE_SAME_HEIGHT, LINE_SAME_PITCH};
use crate::ops_view::Items;
use crate::DecodeError;
use cdoc_layout::{CaretLine, CaretStop, Provenance, Pt100, Region};

/// A caret line read in place.
#[derive(Debug, Clone)]
pub struct CaretLineView<'a> {
    /// The block list it was laid out from.
    pub region: Region,
    /// Block indices down to its paragraph.
    pub path: Items<'a, u32>,
    /// Caret top.
    pub top: Pt100,
    /// Caret bottom.
    pub bottom: Pt100,
    /// How many of the page's document-text runs are its text.
    pub runs: u32,
    /// The stops no run draws, in gap order.
    pub stops: Items<'a, CaretStop>,
}

impl CaretLineView<'_> {
    /// The line as an owned value.
    #[must_use]
    pub fn to_owned(&self) -> CaretLine {
        CaretLine {
            from: Provenance {
                region: self.region,
                path: self.path.clone().collect(),
            },
            top: self.top,
            bottom: self.bottom,
            runs: self.runs,
            stops: self.stops.clone().collect(),
        }
    }
}

/// A page's caret lines read in place; the page was checked when it was opened.
#[derive(Debug, Clone)]
pub struct CaretLinesView<'a> {
    left: usize,
    r: Reader<'a>,
}

impl<'a> CaretLinesView<'a> {
    /// How many lines.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.left
    }

    /// Whether there are none (never, in a list that decoded).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.left == 0
    }

    /// The lines, in order.
    #[must_use]
    pub fn lines(&self) -> Lines<'a> {
        Lines {
            left: self.left,
            r: self.r.clone(),
            last: None,
            pitch: 0,
        }
    }
}

/// A page's caret lines, one after another.
#[derive(Debug, Clone)]
pub struct Lines<'a> {
    left: usize,
    r: Reader<'a>,
    last: Option<CaretLineView<'a>>,
    /// The previous line's top less the one before it.
    pitch: Pt100,
}

impl<'a> Lines<'a> {
    fn checked(&mut self) -> Result<Option<CaretLineView<'a>>, DecodeError> {
        if self.left == 0 {
            return Ok(None);
        }
        self.left -= 1;
        let r = &mut self.r;
        let head = r.u8()?;
        if head & !(LINE_NEW | LINE_SAME_PITCH | LINE_SAME_HEIGHT | LINE_NO_STOPS) != 0 {
            return Err(DecodeError::Malformed("unknown caret line head"));
        }
        let (region, path) = match (head & LINE_NEW != 0, &self.last) {
            (true, _) => provenance(r)?,
            (false, Some(last)) => (last.region, last.path.clone()),
            (false, None) => {
                return Err(DecodeError::Malformed(
                    "the first caret line names no paragraph",
                ));
            }
        };
        let (top, height) = self
            .last
            .as_ref()
            .map_or((0, 0), |l| (l.top, l.bottom.wrapping_sub(l.top)));
        if head & LINE_SAME_PITCH == 0 {
            self.pitch = r.zz()?;
        }
        let top = top.wrapping_add(self.pitch);
        let height = if head & LINE_SAME_HEIGHT == 0 {
            r.zz()?
        } else {
            height
        };
        let bottom = top.wrapping_add(height);
        let runs = r.uv()?;
        let count = if head & LINE_NO_STOPS == 0 {
            r.len()?
        } else {
            0
        };
        let stops = Items::counted(r.clone(), count);
        for _ in 0..count {
            CaretStop::decode(r)?;
        }
        let line = CaretLineView {
            region,
            path,
            top,
            bottom,
            runs,
            stops,
        };
        self.last = Some(line.clone());
        Ok(Some(line))
    }
}

impl<'a> Iterator for Lines<'a> {
    type Item = CaretLineView<'a>;
    fn next(&mut self) -> Option<CaretLineView<'a>> {
        self.checked().ok().flatten()
    }
}

/// Reads the line count; the lines are left in place.
pub fn parse<'a>(r: &mut Reader<'a>) -> Result<CaretLinesView<'a>, DecodeError> {
    let left = r.len()?;
    Ok(CaretLinesView { left, r: r.clone() })
}

/// Checks the lines: at least one, each well formed, the first naming its paragraph, a line
/// with no run having a stop, gaps strictly increasing, and nothing after the last.
pub fn check(lines: &CaretLinesView<'_>) -> Result<(), DecodeError> {
    if lines.is_empty() {
        return Err(DecodeError::Malformed("a caret lines op has no lines"));
    }
    let mut all = lines.lines();
    while let Some(line) = all.checked()? {
        if line.runs == 0 && line.stops.len() == 0 {
            return Err(DecodeError::Malformed(
                "a caret line has no runs and no stops",
            ));
        }
        let mut last: Option<u32> = None;
        for s in line.stops {
            if last.is_some_and(|l| s.gap <= l) {
                return Err(DecodeError::Malformed("caret stops' gaps do not increase"));
            }
            last = Some(s.gap);
        }
    }
    if all.r.is_empty() {
        Ok(())
    } else {
        Err(DecodeError::Malformed("payload longer than its lines"))
    }
}

#[cfg(test)]
mod tests {
    use super::{check, parse};
    use crate::codec::Reader;
    use crate::DecodeError;

    fn checked(bytes: &[u8]) -> Result<(), DecodeError> {
        let mut r = Reader::new(bytes, 0);
        check(&parse(&mut r)?)
    }

    #[test]
    fn a_line_names_its_paragraph_first_and_nothing_follows_the_last() {
        // One line: new paragraph, no stops; body of section 0, path [0]; step 0, height 0;
        // one run.
        let line = [1, 0x09, 0, 0, 1, 0, 0, 0, 1];
        assert_eq!(checked(&line), Ok(()));
        assert_eq!(
            checked(&[1, 0x08, 0, 0, 1]),
            Err(DecodeError::Malformed(
                "the first caret line names no paragraph"
            ))
        );
        assert_eq!(
            checked(&[1, 0x19, 0, 0, 1, 0, 0, 0, 1]),
            Err(DecodeError::Malformed("unknown caret line head"))
        );
        let mut longer = line.to_vec();
        longer.push(0);
        assert_eq!(
            checked(&longer),
            Err(DecodeError::Malformed("payload longer than its lines"))
        );
    }
}
