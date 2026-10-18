//! Selection highlights over a page's lines.

use super::Line;
use cdoc_crdt::Doc;
use cdoc_layout::{Pt100, Rect};

/// The highlight of the part of `line` that `range` covers. Only a line `range` starts or ends
/// in needs every stop's index; one it covers or misses is decided by its first and last.
pub(super) fn line_rect(doc: &Doc, line: &Line, range: &std::ops::Range<usize>) -> Option<Rect> {
    let (first, last) = (line.stops.first()?, line.stops.last()?);
    if let (Ok(lo), Ok(hi)) = (first.pos.index(doc), last.pos.index(doc)) {
        if hi < range.start || lo >= range.end {
            return None;
        }
        if range.start <= lo && hi <= range.end {
            return Some(highlight(line, first.x, last.x, lo == hi));
        }
    }
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
    Some(highlight(line, from, to, lo == hi))
}

fn highlight(line: &Line, from: Pt100, to: Pt100, empty: bool) -> Rect {
    let height = line.bottom - line.top;
    let width = (to - from).max(if empty { height / 4 } else { 0 });
    Rect {
        x: from,
        y: line.top,
        width,
        height,
    }
}
