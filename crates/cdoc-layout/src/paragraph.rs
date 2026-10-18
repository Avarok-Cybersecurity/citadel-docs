//! A paragraph, laid out into lines in a box of a given width: indents, list label, spacing,
//! alignment, shading and borders.

use crate::context::Ctx;
use crate::fragment::{Block, Fragment, Mark};
use crate::ir::{Draw, Pt100, Rect};
use crate::labels::{Counters, Label};
use crate::line::{compose, Look};
use crate::pieces::{pieces, Piece};
use crate::style::{TextStyle, INK};
use crate::wrap::{wrap, Measure, Paragraph as Breakable};
use crate::LayoutError;
use cdoc_model::{
    Alignment, Border, BorderStyle, Borders, FirstLine, Indent, InlineContent, LineSpacing,
    Paragraph, RunProperties,
};

const SINGLE: LineSpacing = LineSpacing::Multiple { lines: 240 };

fn label_pieces(
    ctx: &mut Ctx<'_, '_>,
    paragraph: &Paragraph,
    label: &Label,
) -> Result<Vec<Piece>, LayoutError> {
    let direct = label.level.run.clone().unwrap_or_else(RunProperties::none);
    let resolved = ctx.doc.styles.resolve_run(&paragraph.properties, &direct)?;
    let language = ctx.doc.meta.language.as_deref();
    let style = TextStyle::resolve(&resolved, &mut ctx.fonts, language)?;
    let (text, glyphs, width) = ctx.set(&label.text, &style);
    Ok(vec![Piece::Text {
        chars: crate::pieces::chars_of(&text),
        text,
        style,
        width,
        glyphs,
        link: None,
        note: None,
        units: crate::ir::Units::None,
    }])
}

/// Border width in hundredths of a point (the model stores eighths).
#[must_use]
pub fn border_width(border: &Border) -> Pt100 {
    i32::from(border.width.0) * 25 / 2
}

fn edge(border: Option<&Border>) -> Option<(Pt100, cdoc_model::Color)> {
    border
        .filter(|b| b.style != BorderStyle::None)
        .map(|b| (border_width(b).max(1), b.color.unwrap_or(INK)))
}

fn decorate(
    strips: &mut [Fragment],
    borders: Option<&Borders>,
    fill: Option<cdoc_model::Color>,
    x: (Pt100, Pt100),
) {
    let (left, right) = x;
    let count = strips.len();
    for (i, strip) in strips.iter_mut().enumerate() {
        let h = strip.height;
        let mut behind = Vec::new();
        if let Some(colour) = fill {
            behind.push(Draw::Fill {
                rect: Rect {
                    x: left,
                    y: 0,
                    width: right - left,
                    height: h,
                },
                colour,
            });
        }
        if let Some(b) = borders {
            let mut rule = |from, to, e: Option<(Pt100, cdoc_model::Color)>| {
                if let Some((width, colour)) = e {
                    behind.push(Draw::Rule {
                        from,
                        to,
                        width,
                        colour,
                    });
                }
            };
            if i == 0 {
                rule((left, 0), (right, 0), edge(b.top.as_ref()));
            }
            if i + 1 == count {
                rule((left, h), (right, h), edge(b.bottom.as_ref()));
            }
            rule((left, 0), (left, h), edge(b.left.as_ref()));
            rule((right, 0), (right, h), edge(b.right.as_ref()));
        }
        behind.append(&mut strip.draws);
        strip.draws = behind;
    }
}

fn plain_text(paragraph: &Paragraph) -> String {
    paragraph
        .inlines
        .iter()
        .filter_map(|i| match &i.content {
            InlineContent::Text(run) => Some(run.text.as_str()),
            _ => None,
        })
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Broken lines set as strips, each line starting at the gap the one before it ended at.
fn composed(ctx: &Ctx<'_, '_>, lines: Vec<crate::wrap::Line>, look: &Look) -> Vec<Fragment> {
    let mut gap = 0;
    lines
        .into_iter()
        .map(|line| {
            let (strip, next) = compose(ctx, line, look, gap);
            gap = next;
            strip
        })
        .collect()
}

/// The paragraph as a block of lines in a box `width` wide, counting its list label on
/// `counters`.
pub fn paragraph(
    ctx: &mut Ctx<'_, '_>,
    paragraph: &Paragraph,
    width: Pt100,
    counters: &mut Counters<'_>,
) -> Result<Block, LayoutError> {
    let label = counters.for_paragraph(ctx.doc, paragraph)?;
    labelled(ctx, paragraph, width, label.as_ref())
}

/// The paragraph as a block of lines in a box `width` wide, with its list label already
/// counted.
pub fn labelled(
    ctx: &mut Ctx<'_, '_>,
    paragraph: &Paragraph,
    width: Pt100,
    label: Option<&Label>,
) -> Result<Block, LayoutError> {
    let resolved = ctx.doc.styles.resolve_paragraph(&paragraph.properties)?;
    let indent: Indent = paragraph
        .properties
        .indent
        .clone()
        .or_else(|| label.and_then(|l| l.level.indent.clone()))
        .or_else(|| resolved.indent.clone())
        .unwrap_or(Indent {
            left: None,
            right: None,
            first_line: None,
        });
    let left = indent.left.map_or(0, |l| l.0);
    let right = indent.right.map_or(0, |l| l.0);
    let first = match indent.first_line {
        Some(FirstLine::Indent { by }) => by.0,
        Some(FirstLine::Hanging { by }) => -by.0,
        None => 0,
    };
    let spacing = resolved.spacing.clone();
    let mark_run = ctx
        .doc
        .styles
        .resolve_run(&paragraph.properties, &RunProperties::none())?;
    let language = ctx.doc.meta.language.as_deref();
    let look = Look {
        alignment: resolved.alignment.unwrap_or(Alignment::Left),
        spacing: spacing.as_ref().and_then(|s| s.line).unwrap_or(SINGLE),
        mark: TextStyle::resolve(&mark_run, &mut ctx.fonts, language)?,
    };
    let hyphenate =
        ctx.doc.settings.auto_hyphenation && resolved.suppress_auto_hyphens != Some(true);
    let mut all = Vec::new();
    if let Some(label) = label {
        all = label_pieces(ctx, paragraph, label)?;
        let label_width: Pt100 = all
            .iter()
            .map(|p| match p {
                Piece::Text { width, .. } => *width,
                _ => 0,
            })
            .sum();
        let mark = &look.mark;
        let space = ctx.fonts.width(mark.face, mark.size, " ", mark.features);
        let gap = (left - (left + first + label_width)).max(space);
        all.push(Piece::Widget {
            width: gap,
            height: 0,
            draws: Vec::new(),
            marks: Vec::new(),
            link: None,
            unit: None,
        });
    }
    all.extend(pieces(
        ctx,
        &paragraph.properties,
        &paragraph.inlines,
        hyphenate,
    )?);
    let breakable = Breakable {
        measure: Measure {
            first: (left + first, width - left - first - right),
            rest: (left, width - left - right),
            tab: ctx.doc.settings.default_tab_stop.0,
        },
        hyphenate,
        justified: look.alignment == Alignment::Justify,
    };
    let lines = wrap(ctx, all, &breakable)?;
    let mut strips = composed(ctx, lines, &look);
    let shade = resolved.shading.as_ref().map(|s| s.fill);
    if shade.is_some() || resolved.borders.is_some() {
        decorate(
            &mut strips,
            resolved.borders.as_ref(),
            shade,
            (left.min(left + first), width - right),
        );
    }
    if let (Some(level), Some(first_strip)) = (resolved.outline_level, strips.first_mut()) {
        first_strip.marks.push(Mark::Heading {
            level,
            title: plain_text(paragraph),
        });
    }
    Ok(Block {
        space_before: spacing.as_ref().and_then(|s| s.before).map_or(0, |l| l.0),
        space_after: spacing.as_ref().and_then(|s| s.after).map_or(0, |l| l.0),
        strips,
        keep_together: resolved.keep_lines == Some(true),
        keep_with_next: resolved.keep_with_next == Some(true),
        page_break_before: resolved.page_break_before == Some(true),
        widow_control: true,
        repeat_head: 0,
    })
}
