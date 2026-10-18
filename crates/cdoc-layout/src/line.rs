// Adapted from rMazing crates/cc-export/src/layout/line.rs
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! A broken line, set: its height from the tallest piece and the line-spacing rule, its
//! pieces moved by the paragraph's alignment, and what is drawn with the glyphs (highlight,
//! underline, strikethrough, links).

use crate::context::Ctx;
use crate::fragment::Fragment;
use crate::ir::Pt100;
use crate::pieces::Piece;
use crate::setter::Setter;
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

fn metrics(ctx: &Ctx<'_, '_>, line: &Line, mark: &TextStyle) -> (Pt100, Pt100) {
    let mut ascent = 0;
    let mut descent = 0;
    let mut any = false;
    for item in &line.items {
        let (a, d) = match &item.piece {
            Piece::Text { style, .. }
            | Piece::Space { style, .. }
            | Piece::Tab { style }
            | Piece::Field { style, .. } => (style.ascent(&ctx.fonts), style.descent(&ctx.fonts)),
            Piece::Widget { height, .. } => (*height, 0),
            Piece::Break(_) => continue,
        };
        any = true;
        ascent = ascent.max(a);
        descent = descent.max(d);
    }
    if !any || descent == 0 {
        descent = descent.max(mark.descent(&ctx.fonts));
    }
    if !any {
        ascent = mark.ascent(&ctx.fonts);
    }
    (ascent, descent)
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

/// The line as a strip whose top-left is the paragraph box's left edge at the line's top.
/// Spaces arrive at the width the breaker set them to (justified or natural); only centred
/// and right-aligned lines move here.
pub fn compose(ctx: &Ctx<'_, '_>, line: Line, look: &Look) -> Fragment {
    let (ascent, descent) = metrics(ctx, &line, &look.mark);
    let height = height_for(look.spacing, ascent + descent).max(1);
    let extra = (line.available - line.width).max(0);
    let offset = match look.alignment {
        Alignment::Left | Alignment::Justify => 0,
        Alignment::Center => extra / 2,
        Alignment::Right => extra,
    };
    let mut set = Setter::new(ctx, height, height - descent);
    set.frag.break_after = line.break_after;
    for item in line.items {
        set.piece(item.piece, line.start + offset + item.x);
    }
    set.finish()
}
