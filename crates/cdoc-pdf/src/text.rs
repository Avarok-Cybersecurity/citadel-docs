//! A display-list glyph run as text operations: the run's glyph ids shown with `TJ`, each displacement
//! between where the reader's pen lands (the font's width for the glyph) and where the
//! layout's pen is (its advance and offsets) written as a `TJ` adjustment, and a vertical
//! offset as text rise. A run its `ToUnicode` map would not read back as its text is wrapped in
//! an `/ActualText` span carrying the text.

use crate::font::Embedded;
use cdoc_layout::ir::to_pt;
use cdoc_layout::Pt100;
use cdoc_paint::{Glyph, RunHead};
use pdf_writer::{Content, Name, Str, TextStr};

/// An adjustment smaller than this (thousandths of an em) is a rounding difference, not a
/// displacement, and is not written.
const NEGLIGIBLE: f32 = 0.0005;

#[allow(clippy::cast_precision_loss)]
fn hundredths(v: Pt100) -> f32 {
    v as f32
}

/// The `TJ` adjustment (thousandths of an em, positive leftwards) moving the reader's pen `by`
/// hundredths of a point rightwards at `size`.
fn adjustment(by: f32, size: Pt100) -> f32 {
    -by * 1000.0 / hundredths(size.max(1))
}

fn rise(y_offset: Pt100) -> f32 {
    -to_pt(y_offset)
}

/// A run as the text operators draw it: its head, its text and its glyphs.
pub struct Run<'a> {
    /// Face, size and origin (the colour is set by the caller).
    pub head: &'a RunHead,
    /// The characters the glyphs stand for.
    pub text: &'a str,
    /// The glyphs.
    pub glyphs: &'a [Glyph],
}

/// Shows `run` in `face` (resource `font`) with its baseline at `y` in user space.
pub fn show(content: &mut Content, run: &Run<'_>, face: &Embedded, font: &str, y: f32) {
    let spanned = !face.extracts(run.text, run.glyphs);
    if spanned {
        content
            .begin_marked_content_with_properties(Name(b"Span"))
            .properties()
            .actual_text(TextStr(run.text));
    }
    content.begin_text();
    content.set_font(Name(font.as_bytes()), to_pt(run.head.size));
    content.set_text_matrix([1.0, 0.0, 0.0, 1.0, to_pt(run.head.x), y]);
    let em = hundredths(run.head.size) / 1000.0;
    let mut lifted = 0;
    let mut drift = run.glyphs.first().map_or(0.0, |g| hundredths(g.x_offset));
    let mut start = 0;
    while start < run.glyphs.len() {
        let level = run.glyphs[start].y_offset;
        if level != lifted {
            content.set_rise(rise(level));
            lifted = level;
        }
        let end = run.glyphs[start..]
            .iter()
            .position(|g| g.y_offset != level)
            .map_or(run.glyphs.len(), |n| start + n);
        let mut shown = content.show_positioned();
        let mut items = shown.items();
        for (i, glyph) in run.glyphs[start..end].iter().enumerate() {
            let adjust = adjustment(drift, run.head.size);
            if adjust.abs() > NEGLIGIBLE {
                items.adjust(adjust);
            }
            items.show(Str(&glyph.id.to_be_bytes()));
            let next = run.glyphs.get(start + i + 1).map_or(0, |g| g.x_offset);
            drift = hundredths(glyph.advance + next - glyph.x_offset) - face.width(glyph.id) * em;
        }
        drop(items);
        drop(shown);
        start = end;
    }
    if lifted != 0 {
        content.set_rise(0.0);
    }
    content.end_text();
    if spanned {
        content.end_marked_content();
    }
}
