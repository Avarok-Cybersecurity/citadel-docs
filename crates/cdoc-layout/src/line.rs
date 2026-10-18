// Adapted from rMazing crates/cc-export/src/layout/line.rs
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! A broken line, set: its height from the tallest piece and the line-spacing rule, its
//! pieces moved by the paragraph's alignment, and what is drawn with the glyphs (highlight,
//! underline, strikethrough, links).

use crate::context::Ctx;
use crate::fragment::{Fragment, LineMark, Mark};
use crate::ir::{CaretStop, FaceId, Pt100};
use crate::pieces::Piece;
use crate::setter::Setter;
use crate::stops::Stops;
use crate::style::TextStyle;
use crate::wrap::Line;
use cdoc_model::{Alignment, LineSpacing};

/// The paragraph-level look of its lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Look {
    /// Alignment.
    pub alignment: Alignment,
    /// Line spacing.
    pub spacing: LineSpacing,
    /// The paragraph mark's style: the height of an empty line.
    pub mark: TextStyle,
}

/// A line's extents above and below its baseline.
struct Extents {
    /// The line box's: the tallest piece, at the style's line size (widgets included).
    line: (Pt100, Pt100),
    /// The caret's: the tallest text at its own size and rise (widgets do not stretch it).
    caret: (Pt100, Pt100),
}

/// What a style's extents depend on: face, size, line size, rise.
type StyleKey = (FaceId, Pt100, Pt100, Pt100);

/// The line's extents; a line with no text takes the paragraph mark's (its box only where no
/// widget gives it one).
fn extents(ctx: &Ctx<'_, '_>, line: &Line, mark: &TextStyle) -> Extents {
    let fonts = &ctx.fonts;
    let (mut ascent, mut descent) = (0, 0);
    let mut caret: Option<(Pt100, Pt100)> = None;
    let mut any = false;
    // The last style's metrics, by what they depend on: consecutive pieces mostly share it.
    let mut last: Option<(StyleKey, [Pt100; 4])> = None;
    for item in &line.items {
        let [a, d, ca, cd] = match &item.piece {
            Piece::Text { style, .. }
            | Piece::Space { style, .. }
            | Piece::Tab { style, .. }
            | Piece::Field { style, .. } => {
                let key = (style.face, style.size, style.line_size, style.rise);
                match last {
                    Some((k, e)) if k == key => e,
                    _ => {
                        let e = style.extents(fonts);
                        last = Some((key, e));
                        e
                    }
                }
            }
            Piece::Widget { height, .. } => {
                any = true;
                ascent = ascent.max(*height);
                continue;
            }
            Piece::Break { .. } => continue,
        };
        any = true;
        ascent = ascent.max(a);
        descent = descent.max(d);
        caret = Some(caret.map_or((ca, cd), |(x, y)| (x.max(ca), y.max(cd))));
    }
    let of_mark = || mark.extents(fonts);
    if !any || descent == 0 {
        descent = descent.max(of_mark()[1]);
    }
    if !any {
        ascent = of_mark()[0];
    }
    Extents {
        line: (ascent, descent),
        caret: caret.unwrap_or_else(|| {
            let [_, _, a, d] = of_mark();
            (a, d)
        }),
    }
}

fn height_for(spacing: LineSpacing, natural: Pt100) -> Pt100 {
    match spacing {
        LineSpacing::Multiple { lines } => {
            let scaled = i64::from(natural) * i64::from(lines) / 240;
            Pt100::try_from(scaled).unwrap_or(natural)
        }
        LineSpacing::Exact { height } => height.0,
        LineSpacing::AtLeast { height } => natural.max(height.0),
    }
}

/// The line as a strip whose top-left is the paragraph box's left edge at the line's top, and
/// the gap the next line starts at. Spaces arrive at the width the breaker set them to
/// (justified or natural); only centred and right-aligned lines move here. `gap` is where the
/// line starts in the paragraph's units, for a line with no unit drawn (an empty paragraph,
/// the line after a final break).
pub fn compose(ctx: &Ctx<'_, '_>, line: Line, look: &Look, gap: u32) -> (Fragment, u32) {
    let Extents {
        line: (ascent, descent),
        caret: (caret_ascent, caret_descent),
    } = extents(ctx, &line, &look.mark);
    let height = height_for(look.spacing, ascent + descent).max(1);
    let extra = (line.available - line.width).max(0);
    let offset = match look.alignment {
        Alignment::Left | Alignment::Justify => 0,
        Alignment::Center => extra / 2,
        Alignment::Right => extra,
    };
    let base = height - descent;
    let (top, bottom) = (base - caret_ascent, base + caret_descent);
    let mut set = Setter::new(ctx, height, base);
    set.frag.break_after = line.break_after;
    for item in line.items {
        set.piece(item.piece, line.start + offset + item.x, item.width);
    }
    let (mut frag, Stops(mut stops), runs) = set.finish();
    if runs == 0 && stops.is_empty() {
        stops.push(CaretStop {
            gap,
            x: line.start + offset,
        });
    }
    let next = line.end_break.map_or(gap, |unit| unit.saturating_add(1));
    frag.marks.push(Mark::Line(LineMark {
        path: Vec::new(),
        top,
        bottom,
        runs,
        stops,
    }));
    (frag, next)
}
