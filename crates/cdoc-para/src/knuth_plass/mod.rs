//! Knuth and Plass's optimum-fit line breaking, as TeX runs it: a first pass at
//! `pretolerance` without pattern hyphens, a second at `tolerance` with them, and a third with
//! `emergency_stretch` when that is set. When no pass finds feasible breaks the paragraph is
//! set first-fit and the result says so.

mod pass;

use crate::breaker::{assemble, Breaker, Breaking};
use crate::error::BreakError;
use crate::first_fit::first_fit_breaks;
use crate::item::Item;
use crate::measure::Measured;
use crate::params::Params;
use crate::widths::LineWidths;
use crate::INF_BAD;
use pass::{Active, Outcome, Pass};

/// The optimum-fit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KnuthPlass;

/// The node with the fewest demerits, then (TeX §875) the one nearest the requested
/// looseness. Returns it and the looseness achieved.
fn choose(ends: &[Active], looseness: i32) -> Option<(Active, i64)> {
    let mut best = *ends.first()?;
    for a in ends {
        if a.total < best.total {
            best = *a;
        }
    }
    if looseness == 0 {
        return Some((best, 0));
    }
    let best_line = i64::try_from(best.line).unwrap_or(i64::MAX);
    let want = i64::from(looseness);
    let mut actual = 0;
    for a in ends {
        let diff = i64::try_from(a.line).unwrap_or(i64::MAX) - best_line;
        if (diff < actual && want <= diff) || (diff > actual && want >= diff) {
            best = *a;
            actual = diff;
        } else if diff == actual && a.total < best.total {
            best = *a;
        }
    }
    Some((best, actual))
}

fn breaks_of(outcome: &Outcome, end: &Active) -> Vec<usize> {
    let mut breaks = Vec::with_capacity(end.line);
    let mut cursor = end.passive;
    while let Some(p) = cursor {
        breaks.push(outcome.passive[p].pos);
        cursor = outcome.passive[p].prev;
    }
    breaks.reverse();
    breaks
}

impl KnuthPlass {
    /// The break positions (item indices) of the passes, or `None` when every pass failed,
    /// with the number of passes run.
    fn optimum<T>(
        m: &Measured<'_, T>,
        widths: &LineWidths,
        params: &Params,
    ) -> (Option<Vec<usize>>, u8) {
        let tolerance = i64::from(params.tolerance).min(INF_BAD);
        let mut passes = Vec::with_capacity(3);
        if params.pretolerance >= 0 {
            passes.push(Pass {
                threshold: i64::from(params.pretolerance).min(INF_BAD),
                patterns: false,
                background: 0,
            });
        }
        passes.push(Pass {
            threshold: tolerance,
            patterns: true,
            background: 0,
        });
        if params.emergency_stretch > 0 {
            passes.push(Pass {
                threshold: tolerance,
                patterns: true,
                background: i64::from(params.emergency_stretch),
            });
        }
        let count = passes.len();
        for (k, p) in passes.iter().enumerate() {
            let used = u8::try_from(k + 1).unwrap_or(u8::MAX);
            let Some(outcome) = pass::run(m, widths, params, p) else {
                continue;
            };
            let Some((end, actual)) = choose(&outcome.ends, params.looseness) else {
                continue;
            };
            if actual == i64::from(params.looseness) || k + 1 == count {
                return (Some(breaks_of(&outcome, &end)), used);
            }
        }
        (None, u8::try_from(count).unwrap_or(u8::MAX))
    }
}

impl<T> Breaker<T> for KnuthPlass {
    fn break_lines(
        &self,
        items: &[Item<T>],
        widths: &LineWidths,
        params: &Params,
    ) -> Result<Breaking, BreakError> {
        let m = Measured::new(items)?;
        let (found, passes) = Self::optimum(&m, widths, params);
        let (breaks, fallback) = match found {
            Some(breaks) => (breaks, false),
            None => (first_fit_breaks(&m, widths, params), true),
        };
        Ok(assemble(&m, widths, params, &breaks, passes, fallback))
    }
}
