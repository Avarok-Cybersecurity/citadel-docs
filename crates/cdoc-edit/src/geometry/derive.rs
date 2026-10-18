//! Deriving a page's line map from the layout's caret lines: each names its paragraph by
//! provenance, and its stops (put together from its runs only when a page is derived, see
//! [`cdoc_layout::caret_stops`]) by the paragraph's gaps, so a stop is the position after the
//! unit at the paragraph start's index plus the gap (gap 0 is after the paragraph's own atom).

use super::resolve::Resolver;
use super::{Geometry, Line, PageMap, Stop};
use crate::error::EditError;
use crate::outline::Outline;
use crate::position::Position;
use cdoc_crdt::{Doc, Id};
use cdoc_layout::{caret_stops, CaretStop, Layout, Region};

/// What derivation reads.
pub(crate) struct Ctx<'a> {
    pub(crate) doc: &'a Doc,
    pub(crate) layout: &'a Layout,
    pub(crate) outline: &'a Outline,
}

impl Geometry {
    /// Derives every page not derived yet. Pages are independent: each line says where it
    /// came from, so no page reads another's.
    pub(crate) fn derive_missing(&mut self, ctx: &Ctx<'_>) -> Result<(), EditError> {
        let mut resolver = None;
        for (p, slot) in self.pages.iter_mut().enumerate() {
            if slot.is_none() {
                let resolver = match &mut resolver {
                    Some(r) => r,
                    None => resolver.insert(Resolver::new(ctx.doc, ctx.outline)?),
                };
                *slot = Some(derive_page(ctx, resolver, p)?);
            }
        }
        Ok(())
    }
}

fn derive_page(
    ctx: &Ctx<'_>,
    resolver: &mut Resolver<'_>,
    page: usize,
) -> Result<PageMap, EditError> {
    let mut map = PageMap::default();
    let page = &ctx.layout.pages[page];
    let all = caret_stops(page).ok_or(EditError::Unplaced)?;
    for (line, line_stops) in page.lines.iter().zip(&all) {
        let Some(paragraph) = resolver.paragraph(&line.from)? else {
            continue;
        };
        let derived = Line {
            top: line.top,
            bottom: line.bottom,
            stops: stops(ctx.doc, paragraph, line_stops)?,
        };
        match line.from.region {
            Region::Body { .. } => map.lines.push(derived),
            Region::Header { .. } | Region::Footer { .. } | Region::Footnote(_) => {
                map.aside.push(derived);
            }
        }
    }
    map.aside
        .sort_by_cached_key(|l| l.stops.first().and_then(|s| s.pos.index(ctx.doc).ok()));
    Ok(map)
}

/// A line's stops as positions: the units from the paragraph's atom on, read once.
fn stops(doc: &Doc, paragraph: Id, line: &[CaretStop]) -> Result<Vec<Stop>, EditError> {
    let start = doc
        .locate(paragraph)
        .filter(|l| l.visible)
        .ok_or(EditError::UnknownPosition)?
        .index;
    let (Some(first), Some(last)) = (line.first(), line.last()) else {
        return Ok(Vec::new());
    };
    let gap = |s: &CaretStop| usize::try_from(s.gap).map_err(|_| EditError::Unplaced);
    let (lo, hi) = (gap(first)?, gap(last)?);
    let ids: Vec<Id> = doc
        .units_from(start + lo)
        .take(hi + 1 - lo)
        .map(|(id, _)| id)
        .collect();
    line.iter()
        .map(|s| {
            let id = ids.get(gap(s)? - lo).ok_or(EditError::Unplaced)?;
            Ok(Stop {
                x: s.x,
                pos: Position::after(*id),
            })
        })
        .collect()
}
