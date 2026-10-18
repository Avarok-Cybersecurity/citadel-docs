//! A paragraph's inlines as the pieces line breaking works on: words, spaces, tabs, widgets
//! (images, fields, signature boxes) and breaks, each already styled and measured.

use crate::context::{Ctx, RevisionView};
use crate::fragment::Mark;
use crate::ir::{Draw, PlacedGlyph, Pt100};
use crate::style::{capitalise, small_cap_size, TextStyle};
use crate::{widgets, LayoutError};
use cdoc_model::{
    BreakKind, Color, FieldKind, FootnoteId, Inline, InlineContent, ParagraphProperties,
    RevisionKind, RunProperties, UnderlineStyle,
};

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
    },
    /// A tab: advances to the next default tab stop.
    Tab {
        /// For its height.
        style: TextStyle,
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
    },
    /// A forced break.
    Break(BreakKind),
}

fn text_pieces(
    ctx: &mut Ctx<'_, '_>,
    text: &str,
    style: &TextStyle,
    link: Option<&String>,
    out: &mut Vec<Piece>,
) {
    for (part, reduced) in capitalise(text, style.caps) {
        let style = if reduced {
            TextStyle {
                size: small_cap_size(style.size),
                ..style.clone()
            }
        } else {
            style.clone()
        };
        let mut word = String::new();
        let mut space: Option<(Vec<PlacedGlyph>, Pt100)> = None;
        let flush = |ctx: &mut Ctx<'_, '_>, word: &mut String, out: &mut Vec<Piece>| {
            if !word.is_empty() {
                let (text, glyphs, width) = ctx.set(word, &style);
                out.push(Piece::Text {
                    text,
                    style: style.clone(),
                    width,
                    glyphs,
                    link: link.cloned(),
                    note: None,
                });
                word.clear();
            }
        };
        for ch in part.chars() {
            match ch {
                '\n' | '\u{2028}' => {
                    flush(ctx, &mut word, out);
                    out.push(Piece::Break(BreakKind::Line));
                }
                '\t' => {
                    flush(ctx, &mut word, out);
                    out.push(Piece::Tab {
                        style: style.clone(),
                    });
                }
                c if c.is_whitespace() && c != '\u{a0}' => {
                    flush(ctx, &mut word, out);
                    let (glyphs, width) = space.get_or_insert_with(|| {
                        let (_, glyphs, width) = ctx.set(" ", &style);
                        (glyphs, width)
                    });
                    out.push(Piece::Space {
                        width: *width,
                        glyphs: glyphs.clone(),
                        style: style.clone(),
                        link: link.cloned(),
                    });
                }
                c => word.push(c),
            }
        }
        flush(ctx, &mut word, out);
    }
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
    for inline in inlines {
        let revision = inline.marks.revision.as_ref().map(|r| r.kind);
        let view = ctx.options.revisions;
        if view == RevisionView::Final && revision == Some(RevisionKind::Deletion) {
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
        match &inline.content {
            InlineContent::Text(run) => {
                text_pieces(ctx, &run.text, &style, link.as_ref(), &mut out);
            }
            InlineContent::Tab => out.push(Piece::Tab { style }),
            InlineContent::Break { kind } => out.push(Piece::Break(*kind)),
            InlineContent::FootnoteRef { id } => {
                let style = style.superscript();
                let number = ctx.note_number(*id).to_string();
                let (text, glyphs, width) = ctx.set(&number, &style);
                out.push(Piece::Text {
                    text,
                    style,
                    width,
                    glyphs,
                    link,
                    note: Some(*id),
                });
            }
            InlineContent::Field(field) => match (&field.field, ctx.fields.clone()) {
                (FieldKind::Date { .. }, _) => {
                    text_pieces(ctx, &field.cached, &style, link.as_ref(), &mut out);
                }
                (FieldKind::PageNumber, Some(values)) => {
                    text_pieces(ctx, &values.page, &style, link.as_ref(), &mut out);
                }
                (FieldKind::PageCount, Some(values)) => {
                    text_pieces(ctx, &values.pages, &style, link.as_ref(), &mut out);
                }
                (kind, None) => {
                    let (text, glyphs, width) = ctx.set(&field.cached, &style);
                    out.push(Piece::Field {
                        kind: kind.clone(),
                        text,
                        style,
                        width,
                        glyphs,
                    });
                }
            },
            InlineContent::Image(image) => out.push(widgets::image(image, link)),
            InlineContent::FormField(field) => out.push(widgets::form(ctx, field, &style)),
            InlineContent::SignatureField(field) => {
                out.push(widgets::signature(ctx, field, &style));
            }
        }
    }
    Ok(out)
}
