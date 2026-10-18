//! Building a page's lines from matched runs, and the synthesised lines of empty paragraphs.

use super::derive::Fallback;
use super::matcher::Matched;
use super::{Line, Stop};
use crate::position::Position;
use cdoc_crdt::{Doc, Id};
use cdoc_layout::{Pt100, Rect};

/// The lines of one region kind (body or asides) as a page's runs are matched.
#[derive(Default)]
pub(super) struct Builder {
    lines: Vec<Line>,
    /// Empty paragraphs, as "before line `i`".
    empties: Vec<(usize, Id)>,
    /// The aside the last run came from.
    region: Option<usize>,
}

/// One drawn run with its vertical extent and pen positions.
pub(super) struct Drawn<'a> {
    pub(super) region: Option<usize>,
    pub(super) top: Pt100,
    pub(super) bottom: Pt100,
    pub(super) pens: &'a [Pt100],
}

impl Builder {
    /// Adds matched run `m`, drawn as `d`, starting a line when it moved to a new one.
    pub(super) fn add(&mut self, m: &Matched<'_>, d: &Drawn<'_>) {
        let len = d.pens.len() - 1;
        self.empties
            .extend(m.empties.iter().map(|&e| (self.lines.len(), e)));
        // A run continues the line unless it starts left of the line's end or beside it
        // vertically (a raised or lowered run still overlaps its line).
        let continues = self.lines.last().is_some_and(|l| {
            l.stops.last().is_none_or(|s| d.pens[0] >= s.x) && d.top < l.bottom && l.top < d.bottom
        });
        let new_line = m.crossed || self.region != d.region || !continues || !m.empties.is_empty();
        if new_line {
            self.lines.push(Line {
                top: d.top,
                bottom: d.bottom,
                stops: Vec::new(),
            });
        }
        self.region = d.region;
        let line = self.lines.last_mut().expect("pushed above");
        (line.top, line.bottom) = (line.top.min(d.top), line.bottom.max(d.bottom));
        if let Some(pos) = m
            .before
            .filter(|p| line.stops.last().is_none_or(|s| s.pos != *p))
        {
            line.stops.push(Stop { x: d.pens[0], pos });
        }
        for (k, &(id, _)) in m.chars.iter().enumerate() {
            let end = m.chars.get(k + 1).map_or(len, |&(_, a)| a);
            let zero_next = m.chars.get(k + 1).is_some_and(|&(_, a)| {
                let after = m.chars.get(k + 2).map_or(len, |&(_, b)| b);
                d.pens[after] == d.pens[a]
            });
            if !zero_next {
                line.stops.push(Stop {
                    x: d.pens[end],
                    pos: Position::after(id),
                });
            }
        }
    }

    /// The aside the last run came from.
    pub(super) const fn region(&self) -> Option<usize> {
        self.region
    }

    /// Empty paragraphs after everything added so far.
    pub(super) fn trail(&mut self, ids: Vec<Id>) {
        let at = self.lines.len();
        self.empties.extend(ids.into_iter().map(|e| (at, e)));
    }

    /// The finished lines, with a line for each empty paragraph.
    pub(super) fn finish(self, fb: Fallback) -> Vec<Line> {
        with_empties(self.lines, &self.empties, fb)
    }
}

/// `lines` with a zero-width line for each empty paragraph, `(i, id)` meaning "before line
/// `i`": stacked under the line before it, or over the line after it, or at the fallback.
fn with_empties(lines: Vec<Line>, empties: &[(usize, Id)], fb: Fallback) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::with_capacity(lines.len() + empties.len());
    let mut pending = empties.iter().peekable();
    let total = lines.len();
    let mut lines = lines.into_iter();
    for i in 0..=total {
        let group: Vec<Id> =
            std::iter::from_fn(|| pending.next_if(|(at, _)| *at == i).map(|e| e.1)).collect();
        let next = lines.next();
        if !group.is_empty() {
            let (x, height) = match (out.last(), &next) {
                (Some(l), _) | (None, Some(l)) => {
                    (l.stops.first().map_or(fb.x, |s| s.x), l.bottom - l.top)
                }
                (None, None) => (fb.x, fb.height),
            };
            let n = Pt100::try_from(group.len()).unwrap_or(Pt100::MAX);
            let mut top = match (out.last(), &next) {
                (Some(l), _) => l.bottom,
                (None, Some(l)) => l.top - height * n,
                (None, None) => fb.y,
            };
            for id in group {
                let stops = vec![Stop {
                    x,
                    pos: Position::after(id),
                }];
                out.push(Line {
                    top,
                    bottom: top + height,
                    stops,
                });
                top += height;
            }
        }
        out.extend(next);
    }
    out
}

/// The highlight of the part of `line` that `range` covers.
pub(super) fn line_rect(doc: &Doc, line: &Line, range: &std::ops::Range<usize>) -> Option<Rect> {
    let idx: Vec<(usize, Pt100)> = line
        .stops
        .iter()
        .filter_map(|s| Some((s.pos.index(doc).ok()?, s.x)))
        .collect();
    let (&(lo, x0), &(hi, x1)) = (idx.first()?, idx.last()?);
    if hi < range.start || lo >= range.end {
        return None;
    }
    let from = idx
        .iter()
        .find(|(i, _)| *i >= range.start)
        .map_or(x0, |p| p.1);
    let to = idx
        .iter()
        .rev()
        .find(|(i, _)| *i <= range.end)
        .map_or(x1, |p| p.1);
    let height = line.bottom - line.top;
    let width = (to - from).max(if lo == hi { height / 4 } else { 0 });
    Some(Rect {
        x: from,
        y: line.top,
        width,
        height,
    })
}
