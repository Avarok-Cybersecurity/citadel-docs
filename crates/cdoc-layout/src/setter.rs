//! Pieces onto a line strip: glyph runs gathered while their style and link agree, with the
//! highlight, underline and strikethrough drawn under and over them; fields and widgets placed
//! at their positions.

use crate::context::Ctx;
use crate::fragment::{Fragment, Mark};
use crate::ir::{Draw, Pt100, Rect, TextRun};
use crate::pieces::Piece;
use crate::style::TextStyle;
use cdoc_model::UnderlineStyle;

/// Glyphs being gathered into one run.
struct Pending {
    style: TextStyle,
    link: Option<String>,
    x: Pt100,
    text: String,
    width: Pt100,
}

/// Draws a run of glyphs and its decorations.
fn glyphs(
    ctx: &Ctx<'_, '_>,
    text: String,
    style: &TextStyle,
    x: Pt100,
    w: Pt100,
    base: Pt100,
) -> Vec<Draw> {
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
    if !text.is_empty() {
        draws.push(Draw::Text(TextRun {
            text,
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

/// A line strip being drawn.
pub struct Setter<'c, 'd, 'f> {
    ctx: &'c Ctx<'d, 'f>,
    height: Pt100,
    base: Pt100,
    run: Option<Pending>,
    /// The strip.
    pub frag: Fragment,
}

impl<'c, 'd, 'f> Setter<'c, 'd, 'f> {
    /// A strip `height` tall whose baseline is `base` below its top.
    pub const fn new(ctx: &'c Ctx<'d, 'f>, height: Pt100, base: Pt100) -> Self {
        Self {
            ctx,
            height,
            base,
            run: None,
            frag: Fragment::empty(height),
        }
    }

    fn flush(&mut self) {
        if let Some(p) = self.run.take() {
            let draws = glyphs(self.ctx, p.text, &p.style, p.x, p.width, self.base);
            self.frag.draws.extend(draws);
            if let Some(href) = p.link {
                let rect = Rect {
                    x: p.x,
                    y: 0,
                    width: p.width,
                    height: self.height,
                };
                self.frag.marks.push(Mark::Link { rect, href });
            }
        }
    }

    fn extend(
        &mut self,
        style: TextStyle,
        link: Option<String>,
        x: Pt100,
        text: &str,
        width: Pt100,
        joinable: bool,
    ) {
        match &mut self.run {
            Some(p) if joinable && p.style == style && p.link == link => {
                p.text.push_str(text);
                p.width = x + width - p.x;
            }
            _ => {
                self.flush();
                self.run = Some(Pending {
                    style,
                    link,
                    x,
                    text: text.to_owned(),
                    width,
                });
            }
        }
    }

    /// A justified line's space, widened: drawn on its own so the next word starts where the
    /// layout put it.
    pub fn stretched_space(
        &mut self,
        style: TextStyle,
        link: Option<String>,
        x: Pt100,
        width: Pt100,
    ) {
        self.flush();
        self.run = Some(Pending {
            style,
            link,
            x,
            text: String::new(),
            width,
        });
        self.flush();
    }

    /// Any piece at `x`.
    pub fn piece(&mut self, piece: Piece, x: Pt100) {
        match piece {
            Piece::Text {
                text,
                style,
                link,
                note,
                width,
            } => {
                if let Some(id) = note {
                    self.frag.marks.push(Mark::Footnote(id));
                }
                self.extend(style, link, x, &text, width, note.is_none());
            }
            Piece::Space { style, link, width } => self.extend(style, link, x, " ", width, true),
            Piece::Tab { .. } | Piece::Break(_) => self.flush(),
            Piece::Field {
                kind, text, style, ..
            } => {
                self.flush();
                let run = TextRun {
                    text,
                    x,
                    y: self.base - style.rise,
                    size: style.size,
                    face: style.face,
                    colour: style.colour,
                };
                self.frag.marks.push(Mark::Field { kind, run });
            }
            Piece::Widget {
                height,
                draws,
                marks,
                link,
                width,
            } => {
                self.flush();
                let top = self.base - height;
                self.frag
                    .draws
                    .extend(draws.into_iter().map(|d| d.shifted(x, top)));
                self.frag
                    .marks
                    .extend(marks.into_iter().map(|m| m.shifted(x, top)));
                if let Some(href) = link {
                    let rect = Rect {
                        x,
                        y: top,
                        width,
                        height,
                    };
                    self.frag.marks.push(Mark::Link { rect, href });
                }
            }
        }
    }

    /// The finished strip.
    pub fn finish(mut self) -> Fragment {
        self.flush();
        self.frag
    }
}
