//! Reading the line maps: the page and line holding a gap, caret and selection rectangles, hit
//! testing and vertical moves.

use super::lines::line_rect;
use super::{Geometry, Line, PageMap, Stop};
use crate::error::EditError;
use crate::position::Position;
use cdoc_crdt::Doc;
use cdoc_layout::{Pt100, Rect};

/// Where a gap is: page, line, and the stop at or before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Spot {
    pub(crate) page: usize,
    pub(crate) line: usize,
    pub(crate) stop: usize,
    /// In a header, footer or footnote: the line is one of the page's asides.
    pub(crate) aside: bool,
}

impl PageMap {
    const fn of(&self, aside: bool) -> &Vec<Line> {
        if aside {
            &self.aside
        } else {
            &self.lines
        }
    }
}

fn last_index(doc: &Doc, line: &Line) -> Option<usize> {
    line.stops.last().and_then(|s| s.pos.index(doc).ok())
}

fn stop_at(doc: &Doc, stops: &[Stop], gap: usize) -> usize {
    partition(stops.len(), |s| {
        stops[s].pos.index(doc).is_ok_and(|i| i <= gap)
    })
    .saturating_sub(1)
}

fn first_index(doc: &Doc, line: &Line) -> Option<usize> {
    line.stops.first().and_then(|s| s.pos.index(doc).ok())
}

impl Geometry {
    fn line_ref(&self, s: Spot) -> &Line {
        &self.page(s.page).of(s.aside)[s.line]
    }

    /// The first page and aside line spanning gap `gap`, if it is in a header, footer or
    /// footnote that is drawn.
    fn aside_spot(&self, doc: &Doc, gap: usize) -> Option<Spot> {
        self.pages.iter().enumerate().find_map(|(page, m)| {
            let lines = &m.as_ref()?.aside;
            let line = lines.iter().position(|l| {
                first_index(doc, l).is_some_and(|f| f <= gap)
                    && last_index(doc, l).is_some_and(|e| gap <= e)
            })?;
            let stop = stop_at(doc, &lines[line].stops, gap);
            Some(Spot {
                page,
                line,
                stop,
                aside: true,
            })
        })
    }

    /// The first gap on page `p` or, for a page with no lines, on the next page with some.
    fn page_first(&self, doc: &Doc, p: usize) -> Option<usize> {
        self.pages[p..]
            .iter()
            .find_map(|m| m.as_ref()?.lines.iter().find_map(|l| first_index(doc, l)))
    }

    /// The page, line and stop for gap `gap`: the last stop at or before it in document
    /// order (or the very first stop).
    pub(crate) fn spot(&self, doc: &Doc, gap: usize) -> Option<Spot> {
        if let Some(s) = self.aside_spot(doc, gap) {
            return Some(s);
        }
        let pp = partition(self.pages.len(), |p| {
            self.page_first(doc, p).is_some_and(|f| f <= gap)
        });
        let candidates = (0..pp.max(1)).rev().chain(pp.max(1)..self.pages.len());
        let page = candidates
            .into_iter()
            .find(|&p| self.pages[p].as_ref().is_some_and(|m| !m.lines.is_empty()))?;
        let lines = &self.page(page).lines;
        let lp = partition(lines.len(), |l| {
            first_index(doc, &lines[l]).is_some_and(|f| f <= gap)
        });
        let line = lp.saturating_sub(1);
        let stop = stop_at(doc, &lines[line].stops, gap);
        Some(Spot {
            page,
            line,
            stop,
            aside: false,
        })
    }

    /// The caret rectangle for `pos`, with its page.
    pub(crate) fn caret(&self, doc: &Doc, pos: Position) -> Result<(usize, Rect), EditError> {
        let gap = pos.index(doc)?;
        let s = self.spot(doc, gap).ok_or(EditError::UnknownPosition)?;
        let line = self.line_ref(s);
        Ok((s.page, line.caret(line.stops[s.stop].x)))
    }

    /// One rectangle per line the gaps `range` cover, with its page.
    pub(crate) fn rects(&self, doc: &Doc, range: std::ops::Range<usize>) -> Vec<(usize, Rect)> {
        let mut out = Vec::new();
        let (Some(a), Some(b)) = (self.spot(doc, range.start), self.spot(doc, range.end)) else {
            return out;
        };
        if range.is_empty() {
            return out;
        }
        let pages = if a.aside {
            a.page..=a.page
        } else {
            a.page..=b.page
        };
        for page in pages {
            let lines = self.pages[page].iter().flat_map(|m| m.of(a.aside));
            out.extend(
                lines
                    .filter_map(|l| line_rect(doc, l, &range))
                    .map(|r| (page, r)),
            );
        }
        out
    }

    /// The nearest stop to `(x, y)` on `page`; other pages when this one has no lines.
    pub(crate) fn hit(&self, page: usize, x: Pt100, y: Pt100) -> Option<Position> {
        let map = self.pages.get(page)?.as_ref()?;
        let Some(line) = map.lines.iter().chain(&map.aside).min_by_key(|l| {
            let dy = (l.top - y).max(y - l.bottom).max(0);
            let (a, b) = (
                l.stops.first().map_or(0, |s| s.x),
                l.stops.last().map_or(0, |s| s.x),
            );
            (dy, (a - x).max(x - b).max(0))
        }) else {
            let before = self.pages[..page]
                .iter()
                .rev()
                .flatten()
                .find_map(|m| m.lines.last());
            let after = self.pages[page + 1..]
                .iter()
                .flatten()
                .find_map(|m| m.lines.first());
            return before
                .and_then(|l| l.stops.last())
                .or_else(|| after.and_then(|l| l.stops.first()))
                .map(|s| s.pos);
        };
        nearest(&line.stops, x).map(|s| s.pos)
    }

    /// The stop nearest `goal_x` on the line above (`up`) or below the one holding `gap`.
    pub(crate) fn vertical(
        &self,
        doc: &Doc,
        gap: usize,
        up: bool,
        goal_x: Pt100,
    ) -> Option<Position> {
        let s = self.spot(doc, gap)?;
        if s.aside {
            let lines = &self.page(s.page).aside;
            let line = if up {
                s.line.checked_sub(1)?
            } else {
                s.line + 1
            };
            return nearest(&lines.get(line)?.stops, goal_x).map(|s| s.pos);
        }
        let count = |p: usize| self.pages[p].as_ref().map_or(0, |m| m.lines.len());
        let (page, line) = if up {
            if s.line > 0 {
                (s.page, s.line - 1)
            } else {
                let p = (0..s.page).rev().find(|&p| count(p) > 0)?;
                (p, count(p) - 1)
            }
        } else if s.line + 1 < count(s.page) {
            (s.page, s.line + 1)
        } else {
            ((s.page + 1..self.pages.len()).find(|&p| count(p) > 0)?, 0)
        };
        nearest(&self.page(page).lines[line].stops, goal_x).map(|s| s.pos)
    }

    /// The first and last stops of the line holding `gap`.
    pub(crate) fn line_ends(&self, doc: &Doc, gap: usize) -> Option<(Position, Position)> {
        let line = self.line_ref(self.spot(doc, gap)?);
        Some((line.stops.first()?.pos, line.stops.last()?.pos))
    }
}

fn nearest(stops: &[Stop], x: Pt100) -> Option<&Stop> {
    stops.iter().min_by_key(|s| (s.x - x).abs())
}

/// The first `i` in `0..n` for which `pred` is false, `pred` being true then false.
fn partition(n: usize, pred: impl Fn(usize) -> bool) -> usize {
    let (mut lo, mut hi) = (0, n);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if pred(mid) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}
