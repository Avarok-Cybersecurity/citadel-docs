//! A paragraph's inlines as the pieces line breaking works on: words, spaces, tabs, widgets
//! (images, fields, signature boxes) and breaks, each already styled and measured, and each
//! knowing which of the paragraph's units it draws (see [`crate::ir::CaretLine`]).

mod text;

use crate::context::{Ctx, RevisionView};
use crate::fragment::Mark;
use crate::ir::{Draw, PlacedGlyph, Pt100, SpanKind, UnitSpan, Units};
use crate::style::TextStyle;
use crate::{widgets, LayoutError};
use cdoc_model::{
    BreakKind, Color, FieldKind, FootnoteId, Inline, InlineContent, ParagraphProperties,
    RevisionKind, RunProperties, UnderlineStyle,
};
use text::{text_pieces, Source};

/// The colour tracked changes are marked in.
pub const MARKUP: Color = Color::rgb(0xC0, 0x00, 0x30);

/// One piece.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    /// A word: glyphs with no space inside (a hyphenation point, explicit hyphen or soft
    /// hyphen may still break it).
    Text {
        /// What is drawn.
        text: String,
        /// How.
        style: TextStyle,
        /// Its width: the sum of the glyphs' advances.
        width: Pt100,
        /// The shaped glyphs, clusters into `text`.
        glyphs: Vec<PlacedGlyph>,
        /// The hyperlink it belongs to.
        link: Option<String>,
        /// The footnote it is the reference mark of.
        note: Option<FootnoteId>,
        /// The paragraph unit each character of `text` draws; `None` for text that is no
        /// unit (a list label, a footnote's own number, a line-end hyphen).
        units: Units,
        /// How many characters `text` holds.
        chars: u32,
    },
    /// One breakable space; stretched when a line is justified.
    Space {
        /// How (underline and highlight continue across spaces).
        style: TextStyle,
        /// Its width: natural, or as set on a justified line.
        width: Pt100,
        /// The space glyph; its advance is `width`.
        glyphs: Vec<PlacedGlyph>,
        /// The hyperlink it belongs to.
        link: Option<String>,
        /// Its unit.
        unit: u32,
    },
    /// A tab: advances to the next default tab stop.
    Tab {
        /// For its height.
        style: TextStyle,
        /// Its unit.
        unit: u32,
    },
    /// A box sitting on the baseline.
    Widget {
        /// Width.
        width: Pt100,
        /// Height above the baseline.
        height: Pt100,
        /// Drawings, local to the box's top-left.
        draws: Vec<Draw>,
        /// Marks, local to the box's top-left.
        marks: Vec<Mark>,
        /// The hyperlink it belongs to.
        link: Option<String>,
        /// Its unit; `None` for a box that is none (the gap after a list label).
        unit: Option<u32>,
    },
    /// A page-dependent field in the body, measured by its cached text.
    Field {
        /// What it computes.
        kind: FieldKind,
        /// The cached text.
        text: String,
        /// How.
        style: TextStyle,
        /// Width of the cached text.
        width: Pt100,
        /// The cached text, shaped.
        glyphs: Vec<PlacedGlyph>,
        /// Its unit.
        unit: u32,
    },
    /// A forced break.
    Break {
        /// Which.
        kind: BreakKind,
        /// Its unit.
        unit: u32,
    },
}

fn marked(style: TextStyle, kind: Option<RevisionKind>) -> TextStyle {
    match kind {
        Some(RevisionKind::Insertion) => TextStyle {
            colour: MARKUP,
            underline: Some((UnderlineStyle::Single, MARKUP)),
            ..style
        },
        Some(RevisionKind::Deletion) => TextStyle {
            colour: MARKUP,
            strike: true,
            ..style
        },
        None => style,
    }
}

/// How many characters `text` holds.
pub fn chars_of(text: &str) -> u32 {
    u32::try_from(text.chars().count()).unwrap_or(u32::MAX)
}

/// How many units an inline is: a character each, or one for an inline object.
fn size(inline: &Inline) -> u32 {
    match &inline.content {
        InlineContent::Text(run) => chars_of(&run.text),
        _ => 1,
    }
}

/// The paragraph's inlines as pieces. `paragraph` is the paragraph's direct formatting: the
/// cascade is applied here, run by run. `hyphenate` says whether the paragraph is hyphenated,
/// so that text in a language without patterns is reported.
pub fn pieces(
    ctx: &mut Ctx<'_, '_>,
    paragraph: &ParagraphProperties,
    inlines: &[Inline],
    hyphenate: bool,
) -> Result<Vec<Piece>, LayoutError> {
    let mut out = Vec::new();
    let mut next = 0u32;
    for inline in inlines {
        let unit = next;
        let revision = inline.marks.revision.as_ref().map(|r| r.kind);
        let view = ctx.options.revisions;
        if view == RevisionView::Final && revision == Some(RevisionKind::Deletion) {
            next = next.saturating_add(size(inline));
            continue;
        }
        let shown = (view == RevisionView::Markup).then_some(revision).flatten();
        let link = inline.marks.link.as_ref().map(|l| l.href.clone());
        let direct = match &inline.content {
            InlineContent::Text(run) => &run.properties,
            _ => &RunProperties::none(),
        };
        let resolved = ctx.doc.styles.resolve_run(paragraph, direct)?;
        let language = ctx.doc.meta.language.as_deref();
        let style = marked(
            TextStyle::resolve(&resolved, &mut ctx.fonts, language)?,
            shown,
        );
        if hyphenate && style.patterns.is_none() {
            if let Some(tag) = resolved.lang.as_deref().or(language) {
                ctx.unhyphenated(tag);
            }
        }
        let link = link.as_ref();
        let atom = Source::Atom(unit);
        let mut size = 1;
        match &inline.content {
            InlineContent::Text(run) => {
                size = text_pieces(ctx, &run.text, Source::Chars(unit), &style, link, &mut out);
            }
            InlineContent::Tab => out.push(Piece::Tab { style, unit }),
            InlineContent::Break { kind } => out.push(Piece::Break { kind: *kind, unit }),
            InlineContent::FootnoteRef { id } => {
                let style = style.superscript();
                let number = ctx.note_number(*id).to_string();
                let (text, glyphs, width) = ctx.set(&number, &style);
                out.push(Piece::Text {
                    chars: chars_of(&text),
                    units: Units::One(UnitSpan {
                        at: 0,
                        unit,
                        kind: SpanKind::Same,
                    }),
                    text,
                    style,
                    width,
                    glyphs,
                    link: link.cloned(),
                    note: Some(*id),
                });
            }
            InlineContent::Field(field) => match (&field.field, ctx.fields.clone()) {
                (FieldKind::Date { .. }, _) => {
                    text_pieces(ctx, &field.cached, atom, &style, link, &mut out);
                }
                (FieldKind::PageNumber, Some(values)) => {
                    text_pieces(ctx, &values.page, atom, &style, link, &mut out);
                }
                (FieldKind::PageCount, Some(values)) => {
                    text_pieces(ctx, &values.pages, atom, &style, link, &mut out);
                }
                (kind, None) => {
                    let (text, glyphs, width) = ctx.set(&field.cached, &style);
                    out.push(Piece::Field {
                        kind: kind.clone(),
                        text,
                        style,
                        width,
                        glyphs,
                        unit,
                    });
                }
            },
            InlineContent::Image(image) => out.push(widgets::image(image, link.cloned(), unit)),
            InlineContent::FormField(field) => {
                out.push(widgets::form(ctx, field, &style, unit));
            }
            InlineContent::SignatureField(field) => {
                out.push(widgets::signature(ctx, field, &style, unit));
            }
        }
        next = next.saturating_add(size);
    }
    Ok(out)
}
