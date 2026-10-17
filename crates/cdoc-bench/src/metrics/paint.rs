//! The display list: a laid-out document encoded to `cdoc-paint`'s binary list (lowering
//! included), and decoded with every op and glyph visited, as the canvas painter reads it. The
//! `serde_json` baseline stays in `cdoc-paint`'s criterion bench: it is a format comparison, not
//! a competitor.

use super::{paged, BenchError, Ctx, Family, Fixture, Results};
use crate::schema::Measurement;
use cdoc_layout::test_support::default_fonts;
use cdoc_layout::{layout, LayoutOptions};
use cdoc_paint::{decode, encode, OpView};
use std::hint::black_box;

fn paint(e: &dyn std::fmt::Display) -> BenchError {
    BenchError::Hook {
        family: "paint",
        message: e.to_string(),
    }
}

/// Decodes `bytes` and visits every op and glyph; returns how many it saw.
pub fn visit(bytes: &[u8]) -> Result<usize, BenchError> {
    let view = decode(bytes).map_err(|e| paint(&e))?;
    let mut seen = 0;
    for slot in 0..view.page_count() {
        for op in view.page(slot).map_err(|e| paint(&e))?.ops() {
            seen += match op {
                OpView::TextRun { glyphs, .. } => glyphs.count(),
                OpView::GlyphRun { glyphs, .. } => glyphs.count(),
                other => {
                    black_box(other);
                    1
                }
            };
        }
    }
    Ok(seen)
}

pub(super) fn run(ctx: &Ctx<'_>, fx: &Fixture, out: &mut Results) -> Result<(), BenchError> {
    let fonts = default_fonts();
    let laid_out = layout(&fx.doc, &fonts, &LayoutOptions::word_like())?;

    let key = paged(Family::PaintEncode, fx.pages);
    let t = ctx.time(&key, || {
        black_box(encode(black_box(&laid_out), &fonts).map_err(|e| paint(&e))?);
        Ok(())
    })?;
    out.insert(key, Measurement::Time(t));

    let bytes = encode(&laid_out, &fonts).map_err(|e| paint(&e))?;
    let key = paged(Family::PaintDecode, fx.pages);
    let t = ctx.time(&key, || {
        black_box(visit(black_box(&bytes))?);
        Ok(())
    })?;
    out.insert(key, Measurement::Time(t));
    Ok(())
}
