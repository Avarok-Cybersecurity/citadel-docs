//! Pieces onto a line strip: glyph runs gathered while their style and link agree, with the
//! highlight, underline and strikethrough drawn under and over them; fields and widgets placed
//! at their positions.

use crate::context::Ctx;
use crate::decorations::{glyphs, Pending};
use crate::fragment::{FieldRun, Fragment, Mark};
use crate::ir::{GlyphRun, PlacedGlyph, Pt100, Rect, SpanKind, UnitSpan, Units};
use crate::pieces::Piece;
use crate::stops::Stops;
use crate::style::TextStyle;

/// Glyphs to set, with how and what they say.
struct Set {
    style: TextStyle,
    link: Option<String>,
    text: String,
    glyphs: Vec<PlacedGlyph>,
    width: Pt100,
    units: Units,
    chars: u32,
}

/// A line strip being drawn.
pub struct Setter<'c, 'd, 'f> {
    ctx: &'c Ctx<'d, 'f>,
    height: Pt100,
    base: Pt100,
    run: Option<Pending>,
    /// The strip.
    pub frag: Fragment,
    /// The caret stops of the units set so far that no run draws.
    pub stops: Stops,
    /// The runs of document text drawn so far.
    pub runs: u32,
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
            stops: Stops(Vec::new()),
            runs: 0,
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
            let text = p.units != Units::None && !p.glyphs.is_empty();
            self.runs += u32::from(text);
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
            units,
            chars,
        } = set;
        match &mut self.run {
            Some(p) if joinable && p.style == style && p.link == link && p.x + p.width == x => {
                let base = u32::try_from(p.text.len()).unwrap_or(u32::MAX);
                p.units.append(&units, p.chars);
                p.chars += chars;
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
                    units,
                    chars,
                });
            }
        }
    }

    /// Any piece at `x`, `width` wide as set.
    pub fn piece(&mut self, piece: Piece, x: Pt100, width: Pt100) {
        match piece {
            Piece::Text {
                text,
                style,
                link,
                note,
                width,
                glyphs,
                units,
                chars,
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
                    units,
                    chars,
                };
                self.extend(set, x, note.is_none());
            }
            Piece::Space {
                style,
                link,
                width,
                glyphs,
                unit,
            } => {
                let set = Set {
                    style,
                    link,
                    text: " ".to_owned(),
                    glyphs,
                    width,
                    units: Units::One(UnitSpan {
                        at: 0,
                        unit,
                        kind: SpanKind::Each,
                    }),
                    chars: 1,
                };
                self.extend(set, x, true);
            }
            Piece::Tab { unit, .. } => {
                self.stops.unit(unit, x, width);
                self.flush();
            }
            Piece::Break { .. } => self.flush(),
            Piece::Field {
                kind,
                text,
                style,
                glyphs,
                unit,
                ..
            } => {
                self.stops.unit(unit, x, width);
                self.flush();
                let run = GlyphRun {
                    text,
                    glyphs,
                    x,
                    y: self.base - style.rise,
                    size: style.size,
                    face: style.face,
                    colour: style.colour,
                    units: Units::None,
                };
                let features = style.features;
                self.frag.marks.push(Mark::Field(FieldRun {
                    kind,
                    run,
                    features,
                }));
            }
            widget @ Piece::Widget { .. } => self.widget(widget, x, width),
        }
    }

    /// A box sitting on the baseline at `x`.
    fn widget(&mut self, widget: Piece, x: Pt100, width: Pt100) {
        let Piece::Widget {
            height,
            draws,
            marks,
            link,
            unit,
            ..
        } = widget
        else {
            return;
        };
        if let Some(unit) = unit {
            self.stops.unit(unit, x, width);
        }
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

    /// The finished strip, the stops no run draws, and how many runs of document text it
    /// drew.
    pub fn finish(mut self) -> (Fragment, Stops, u32) {
        self.flush();
        (self.frag, self.stops, self.runs)
    }
}
