//! First fit: each line takes as much as fits when its glue is fully shrunk, and breaks at the
//! last legal breakpoint that does. A line with no such breakpoint (a word wider than the
//! measure) breaks at its first breakpoint and is reported overfull. Every item is visited at
//! most twice, so it always terminates.

use crate::breaker::{assemble, Breaker, Breaking};
use crate::error::BreakError;
use crate::item::Item;
use crate::measure::Measured;
use crate::params::Params;
use crate::widths::LineWidths;

/// The first-fit breaker, as a word processor sets lines one at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FirstFit;

/// First-fit break positions. Every hyphen is a candidate; penalties only forbid or force.
pub(crate) fn first_fit_breaks<T>(
    m: &Measured<'_, T>,
    widths: &LineWidths,
    params: &Params,
) -> Vec<usize> {
    let mut breaks = Vec::new();
    let mut prev = None;
    let mut candidate = None;
    for at in 0..m.len() {
        let Some(bp) = m.breakpoint(at, params, true) else {
            continue;
        };
        for _retry in 0..2 {
            let sums = m.line(m.line_start(prev), at, bp.width);
            let fits = sums.width - sums.shrink <= i64::from(widths.width(breaks.len()));
            if fits || candidate.is_none() {
                if bp.forced() || !fits {
                    breaks.push(at);
                    prev = Some(at);
                    candidate = None;
                } else {
                    candidate = Some(at);
                }
                break;
            }
            if let Some(c) = candidate.take() {
                breaks.push(c);
                prev = Some(c);
            }
        }
    }
    breaks
}

impl<T> Breaker<T> for FirstFit {
    fn break_lines(
        &self,
        items: &[Item<T>],
        widths: &LineWidths,
        params: &Params,
    ) -> Result<Breaking, BreakError> {
        let m = Measured::new(items)?;
        let breaks = first_fit_breaks(&m, widths, params);
        Ok(assemble(&m, widths, params, &breaks, 0, false))
    }
}
