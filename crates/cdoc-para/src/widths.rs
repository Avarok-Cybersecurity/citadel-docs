//! The width of each line, as TeX's `\parshape`: a list whose last entry repeats.

use crate::error::BreakError;

/// Line widths. Line numbers are zero-based.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LineWidths {
    widths: Vec<i32>,
}

impl LineWidths {
    /// Every line the same width.
    pub fn uniform(width: i32) -> Result<Self, BreakError> {
        Self::per_line(&[width])
    }

    /// Line `i` is `widths[i]`; lines past the end take the last entry, so `[first, rest]`
    /// describes a first-line indent and `[w; n]` then `w2` a float beside the first n lines.
    pub fn per_line(widths: &[i32]) -> Result<Self, BreakError> {
        if widths.is_empty() {
            return Err(BreakError::NoLineWidths);
        }
        if let Some(line) = widths.iter().position(|w| *w <= 0) {
            return Err(BreakError::NonPositiveWidth {
                line,
                width: widths[line],
            });
        }
        Ok(Self {
            widths: widths.to_vec(),
        })
    }

    /// The width of line `line`.
    #[must_use]
    pub fn width(&self, line: usize) -> i32 {
        let last = self.widths.len() - 1;
        self.widths[line.min(last)]
    }

    /// Lines from this number on all have the same width (TeX's `last_special_line`).
    #[must_use]
    pub fn last_special(&self) -> usize {
        self.widths.len() - 1
    }
}
