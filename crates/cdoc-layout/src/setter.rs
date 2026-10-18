//! Pieces onto a line strip: glyph runs gathered while their style and link agree, with the
//! highlight, underline and strikethrough drawn under and over them; fields and widgets placed
//! at their positions.

use crate::context::Ctx;
use crate::decorations::{glyphs, Pending};
use crate::fragment::{FieldRun, Fragment, Mark};
use crate::ir::{GlyphRun, PlacedGlyph, Pt100, Rect};
use crate::pieces::Piece;
use crate::style::TextStyle;

/// Glyphs to set, with how and what they say.
struct Set {
    style: TextStyle,
    link: Option<String>,
    text: String,
    glyphs: Vec<PlacedGlyph>,
    width: Pt100,
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
            let link = p.link.clone().map(|href| {
                let rect = Rect {
                    x: p.x,
                    y: 0,
                    width: p.width,
                    height: self.height,
                };
                Mark::Link { rect, href }
            });
            let draws = glyphs(self.ctx, p, self.base);
            self.frag.draws.extend(draws);
            self.frag.marks.extend(link);
        }
    }

    /// Glyphs at `x`, joined to the run being gathered when they continue it (same style and
    /// link, starting where it ends).
    fn extend(&mut self, set: Set, x: Pt100, joinable: bool) {
        let Set {
            style,
            link,
            text,
            glyphs,
            width,
        } = set;
        match &mut self.run {
            Some(p) if joinable && p.style == style && p.link == link && p.x + p.width == x => {
                let base = u32::try_from(p.text.len()).unwrap_or(u32::MAX);
                p.text.push_str(&text);
                p.glyphs.extend(glyphs.into_iter().map(|g| PlacedGlyph {
                    cluster: g.cluster.saturating_add(base),
                    ..g
                }));
                p.width += width;
            }
            _ => {
                self.flush();
                self.run = Some(Pending {
                    style,
                    link,
                    x,
                    text,
                    glyphs,
                    width,
                });
            }
        }
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
                glyphs,
            } => {
                if let Some(id) = note {
                    self.frag.marks.push(Mark::Footnote(id));
                }
                let set = Set {
                    style,
                    link,
                    text,
                    glyphs,
                    width,
                };
                self.extend(set, x, note.is_none());
            }
            Piece::Space {
                style,
                link,
                width,
                glyphs,
            } => {
                let set = Set {
                    style,
                    link,
                    text: " ".to_owned(),
                    glyphs,
                    width,
                };
                self.extend(set, x, true);
            }
            Piece::Tab { .. } | Piece::Break(_) => self.flush(),
            Piece::Field {
                kind,
                text,
                style,
                glyphs,
                ..
            } => {
                self.flush();
                let run = GlyphRun {
                    text,
                    glyphs,
                    x,
                    y: self.base - style.rise,
                    size: style.size,
                    face: style.face,
                    colour: style.colour,
                };
                let features = style.features;
                self.frag.marks.push(Mark::Field(FieldRun {
                    kind,
                    run,
                    features,
                }));
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
