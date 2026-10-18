//! A gathered run of glyphs drawn with what goes with it: the highlight behind, the
//! underline and strikethrough over.

use crate::context::Ctx;
use crate::ir::{Draw, GlyphRun, PlacedGlyph, Pt100, Rect};
use crate::style::TextStyle;
use cdoc_model::UnderlineStyle;

/// Glyphs being gathered into one run.
pub struct Pending {
    /// How.
    pub style: TextStyle,
    /// The hyperlink.
    pub link: Option<String>,
    /// Where the run starts.
    pub x: Pt100,
    /// The characters.
    pub text: String,
    /// The glyphs, clusters into `text`.
    pub glyphs: Vec<PlacedGlyph>,
    /// The sum of the advances.
    pub width: Pt100,
}

/// Draws a run of glyphs and its decorations.
pub fn glyphs(ctx: &Ctx<'_, '_>, p: Pending, base: Pt100) -> Vec<Draw> {
    let Pending {
        style,
        text,
        glyphs,
        x,
        width: w,
        ..
    } = p;
    let style = &style;
    let mut draws = Vec::new();
    let y = base - style.rise;
    if let Some(colour) = style.highlight {
        let top = y - ctx.fonts.ascent(style.face, style.size);
        let bottom = y + ctx.fonts.descent(style.face, style.size);
        draws.push(Draw::Fill {
            rect: Rect {
                x,
                y: top,
                width: w,
                height: bottom - top,
            },
            colour,
        });
    }
    if !glyphs.is_empty() {
        draws.push(Draw::Text(GlyphRun {
            text,
            glyphs,
            x,
            y,
            size: style.size,
            face: style.face,
            colour: style.colour,
        }));
    }
    let thickness = (style.size / 18).max(25);
    let rule = |at: Pt100, width: Pt100, colour| Draw::Rule {
        from: (x, at),
        to: (x + w, at),
        width,
        colour,
    };
    if let Some((kind, colour)) = style.underline {
        let at = y + style.size / 9;
        match kind {
            UnderlineStyle::Double => {
                draws.push(rule(at, thickness, colour));
                draws.push(rule(at + thickness * 2, thickness, colour));
            }
            UnderlineStyle::Thick => draws.push(rule(at, thickness * 2, colour)),
            _ => draws.push(rule(at, thickness, colour)),
        }
    }
    if style.strike {
        draws.push(rule(y - style.size * 3 / 10, thickness, style.colour));
    }
    draws
}
