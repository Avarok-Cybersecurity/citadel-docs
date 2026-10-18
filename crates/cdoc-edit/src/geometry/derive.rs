//! Deriving a page's line map: the page's text runs matched against the document's units.

use super::lines::{Builder, Drawn};
use super::matcher::{matched, pens, trailing_empties, Cursor};
use super::{Geometry, PageMap};
use crate::error::EditError;
use crate::position::Position;
use cdoc_crdt::Doc;
use cdoc_layout::{Draw, FontPort, Layout, Pt100, TextRun};
use cdoc_paint::{nominal_pen, Op};
use std::collections::BTreeMap;

/// A face's vertical metrics, in font units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FaceMetrics {
    pub(crate) units_per_em: u16,
    pub(crate) ascender: i16,
    pub(crate) descender: i16,
}

/// Where a page with no matched text puts its caret: the body's top-left corner and a line
/// height, from the first section's page setup and the default font size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Fallback {
    pub(crate) x: Pt100,
    pub(crate) y: Pt100,
    pub(crate) height: Pt100,
}

/// What derivation reads.
pub(crate) struct Ctx<'a> {
    pub(crate) doc: &'a Doc,
    pub(crate) layout: &'a Layout,
    pub(crate) fonts: &'a dyn FontPort,
    pub(crate) fallback: Fallback,
    /// The start atoms of every header, footer and footnote body, as unit indices.
    pub(crate) asides: Vec<usize>,
}

impl Geometry {
    fn metrics(&mut self, ctx: &Ctx<'_>, run: &TextRun) -> Result<FaceMetrics, EditError> {
        let key = &ctx.layout.faces[usize::from(run.face.0)];
        if let Some(m) = self.metrics.get(key) {
            return Ok(*m);
        }
        let source = ctx
            .fonts
            .face(&key.family, key.bold, key.italic)
            .map_err(cdoc_paint::PaintError::from)?;
        let m = cdoc_paint::glyph::face_metrics(source.bytes)
            .map_err(|_| EditError::Paint(cdoc_paint::PaintError::Unreadable(key.clone())))?;
        let m = FaceMetrics {
            units_per_em: m.units_per_em.max(1),
            ascender: m.ascender,
            descender: m.descender,
        };
        self.metrics.insert(key.clone(), m);
        Ok(m)
    }

    /// Derives every page not derived yet, then the page after each one that came out
    /// different or with no body line: a page's text starts where its predecessors' stops end,
    /// and the empty paragraphs before its first line depend on everything since.
    pub(crate) fn derive_missing(&mut self, ctx: &Ctx<'_>) -> Result<(), EditError> {
        let count = self.pages.len();
        let mut todo: Vec<usize> = (0..count).filter(|&p| self.pages[p].is_none()).collect();
        while !todo.is_empty() {
            let list = cdoc_paint::lower(ctx.layout, ctx.fonts, &todo)?;
            let advances: Vec<BTreeMap<u16, u16>> = list
                .faces
                .iter()
                .map(|f| f.glyphs.iter().map(|g| (g.id, g.advance)).collect())
                .collect();
            let mut next = Vec::new();
            for (ops, &p) in list.pages.iter().zip(&todo) {
                let start = self.start_of(p);
                let map = self.derive_page(ctx, p, &ops.ops, &advances, &list.faces, start)?;
                let ripples = map.lines.is_empty() || self.pages[p].as_ref() != Some(&map);
                self.pages[p] = Some(map);
                if ripples && p + 1 < count && !todo.contains(&(p + 1)) {
                    next.push(p + 1);
                }
            }
            todo = next;
        }
        Ok(())
    }

    /// Where a page's body text starts: after the last body stop of the pages before it.
    fn start_of(&self, page: usize) -> Option<Position> {
        self.pages[..page]
            .iter()
            .rev()
            .find_map(|m| Some(m.as_ref()?.lines.last()?.stops.last()?.pos))
    }

    fn derive_page(
        &mut self,
        ctx: &Ctx<'_>,
        page: usize,
        ops: &[Op],
        advances: &[BTreeMap<u16, u16>],
        faces: &[cdoc_paint::FaceEntry],
        start: Option<Position>,
    ) -> Result<PageMap, EditError> {
        let runs = ctx.layout.pages[page].draws.iter().filter_map(|d| match d {
            Draw::Text(run) => Some(run),
            _ => None,
        });
        let text_ops = ops
            .iter()
            .filter(|o| matches!(o, Op::TextRun { .. } | Op::GlyphRun { .. }));
        let gap = start.map_or(Ok(0), |s| s.index(ctx.doc))?;
        let mut body_cursor = Cursor::body(ctx.doc, gap);
        let mut asides: Vec<Cursor<'_>> = ctx
            .asides
            .iter()
            .map(|&i| Cursor::aside(ctx.doc, i))
            .collect();
        let (mut body, mut aside) = (Builder::default(), Builder::default());
        for (run, op) in runs.zip(text_ops) {
            let chars: Vec<char> = run.text.chars().collect();
            let face = usize::from(run.face.0);
            let Some((y, pens)) = pens(op, &advances[face], faces[face].units_per_em) else {
                continue;
            };
            if pens.len() != chars.len() + 1 {
                continue;
            }
            let mut found = matched(&body_cursor, &chars)?.map(|m| (m, None));
            for (k, c) in asides.iter().enumerate() {
                if found.is_some() {
                    break;
                }
                found = matched(c, &chars)?.map(|m| (m, Some(k)));
            }
            let Some((m, region)) = found else {
                continue;
            };
            let metrics = self.metrics(ctx, run)?;
            let scale = |v: i16| nominal_pen(i64::from(v), run.size, metrics.units_per_em);
            let drawn = Drawn {
                region,
                top: y - scale(metrics.ascender),
                bottom: y + scale(-metrics.descender),
                pens: &pens,
            };
            match region {
                None => body.add(&m, &drawn),
                Some(k) => {
                    if let Some(last) = aside.region().filter(|&l| l != k) {
                        aside.trail(trailing_empties(asides[last].clone()));
                    }
                    aside.add(&m, &drawn);
                }
            }
            match region {
                None => body_cursor = m.next,
                Some(k) => asides[k] = m.next,
            }
        }
        if let Some(last) = aside.region() {
            aside.trail(trailing_empties(asides[last].clone()));
        }
        if page + 1 == ctx.layout.pages.len() {
            body.trail(trailing_empties(body_cursor));
        }
        let mut aside = aside.finish(ctx.fallback);
        aside.sort_by_cached_key(|l| l.stops.first().and_then(|s| s.pos.index(ctx.doc).ok()));
        Ok(PageMap {
            lines: body.finish(ctx.fallback),
            aside,
        })
    }
}
