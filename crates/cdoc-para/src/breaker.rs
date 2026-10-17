//! What a breaker returns, and the trait the layout chooses a breaker through.

use crate::badness::fit;
use crate::error::BreakError;
use crate::item::Item;
use crate::measure::Measured;
use crate::params::Params;
use crate::widths::LineWidths;

/// One line of the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Line {
    /// Index of the line's first item (after the glue and penalties discarded at the break
    /// before it).
    pub start: usize,
    /// Index of the item broken at. It is not part of the line, except that a penalty's or
    /// hyphen's width is typeset at the line's end.
    pub end: usize,
    /// The ratio the line's finite glue is set at, in 1/[`crate::RATIO_ONE`]: positive
    /// stretches, negative shrinks. Zero when the line has infinite stretch (which then
    /// takes up all the slack) or no glue.
    pub ratio: i32,
    /// The line ends at a hyphen.
    pub hyphenated: bool,
}

/// How the paragraph went.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BreakReport {
    /// Lines (zero-based) wider than their measure even when fully shrunk.
    pub overfull_lines: Vec<usize>,
    /// Lines whose badness exceeds `tolerance`.
    pub underfull_lines: Vec<usize>,
    /// Passes run: 1 to 3 for optimum fit, 0 for first fit.
    pub passes_used: u8,
    /// No optimum-fit pass found feasible breaks, so the lines are first fit.
    pub fallback: bool,
}

/// The lines and the report.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Breaking {
    /// The lines, in order; the last ends at the paragraph's final forced break.
    pub lines: Vec<Line>,
    /// What went wrong, if anything.
    pub report: BreakReport,
}

/// A line breaker. The layout picks one explicitly ([`crate::KnuthPlass`] or
/// [`crate::FirstFit`]); both take the same items and return the same shape.
pub trait Breaker<T> {
    /// Breaks `items`, which must end in a forced break, into lines of `widths`.
    fn break_lines(
        &self,
        items: &[Item<T>],
        widths: &LineWidths,
        params: &Params,
    ) -> Result<Breaking, BreakError>;
}

/// The lines for a list of break positions, measured and reported (one place for both
/// breakers).
pub(crate) fn assemble<T>(
    m: &Measured<'_, T>,
    widths: &LineWidths,
    params: &Params,
    breaks: &[usize],
    passes_used: u8,
    fallback: bool,
) -> Breaking {
    let mut lines = Vec::with_capacity(breaks.len());
    let mut report = BreakReport {
        overfull_lines: Vec::new(),
        underfull_lines: Vec::new(),
        passes_used,
        fallback,
    };
    let mut prev = None;
    for (number, &end) in breaks.iter().enumerate() {
        let start = m.line_start(prev).min(end);
        let (width, hyphenated) = match &m.items[end] {
            Item::Penalty { width, flagged, .. } => (*width, *flagged),
            Item::Hyphen { width, .. } => (*width, true),
            Item::Glue { .. } | Item::Box { .. } => (0, false),
        };
        let f = fit(&m.line(start, end, width), widths.width(number), 0);
        if f.badness > crate::INF_BAD {
            report.overfull_lines.push(number);
        } else if f.badness > i64::from(params.tolerance) {
            report.underfull_lines.push(number);
        }
        lines.push(Line {
            start,
            end,
            ratio: f.ratio,
            hyphenated,
        });
        prev = Some(end);
    }
    Breaking { lines, report }
}
