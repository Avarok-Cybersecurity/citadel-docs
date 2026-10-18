//! Inline boxes: images, form fields and signature fields, each drawn into a box that sits on
//! the line like a glyph.

use crate::context::Ctx;
use crate::fragment::Mark;
use crate::ir::{Draw, GlyphRun, Pt100, Rect, Units};
use crate::pieces::Piece;
use crate::style::{TextStyle, INK};
use crate::svg_path;
use cdoc_model::{Color, FormField, FormFieldKind, InlineImage, SignatureField};

/// The pale fill behind a fillable field, so an empty one is still visible on the page.
pub const FIELD_FILL: Color = Color::rgb(0xE8, 0xEC, 0xF2);
const LABEL_GREY: Color = Color::rgb(0x59, 0x59, 0x59);
/// The tick, on a 10 × 10 grid.
const TICK: &str = "M2 5.5 L4.2 7.8 L8 2.6";

/// An image, its bottom on the baseline.
#[must_use]
pub fn image(image: &InlineImage, link: Option<String>, unit: u32) -> Piece {
    let rect = Rect {
        x: 0,
        y: 0,
        width: image.width.0.max(1),
        height: image.height.0.max(1),
    };
    Piece::Widget {
        width: rect.width,
        height: rect.height,
        draws: vec![Draw::Image {
            rect,
            content_id: image.content_id.clone(),
        }],
        marks: Vec::new(),
        link,
        unit: Some(unit),
    }
}

fn text_box(
    ctx: &mut Ctx<'_, '_>,
    text: &str,
    min_width: Pt100,
    style: &TextStyle,
    unit: u32,
) -> Piece {
    let (text, glyphs, width) = ctx.set(text, style);
    let ascent = ctx.fonts.ascent(style.face, style.size);
    let descent = ctx.fonts.descent(style.face, style.size);
    let width = width.max(min_width);
    // The widget's bottom sits on the baseline, so its local baseline is `ascent`; the pale
    // box hangs below it by the descent, as a selection highlight does.
    let mut draws = vec![Draw::Fill {
        rect: Rect {
            x: 0,
            y: 0,
            width,
            height: ascent + descent,
        },
        colour: FIELD_FILL,
    }];
    if !glyphs.is_empty() {
        draws.push(Draw::Text(GlyphRun {
            text,
            glyphs,
            x: 0,
            y: ascent,
            size: style.size,
            face: style.face,
            colour: style.colour,
            units: Units::None,
        }));
    }
    Piece::Widget {
        width,
        height: ascent,
        draws,
        marks: Vec::new(),
        link: None,
        unit: Some(unit),
    }
}

fn checkbox(ctx: &Ctx<'_, '_>, checked: bool, style: &TextStyle, unit: u32) -> Piece {
    let side = ctx.fonts.ascent(style.face, style.size) * 4 / 5;
    let stroke = (style.size / 16).max(40);
    let square = vec![
        crate::ir::Segment::Move(0, 0),
        crate::ir::Segment::Line(side, 0),
        crate::ir::Segment::Line(side, side),
        crate::ir::Segment::Line(0, side),
        crate::ir::Segment::Close,
    ];
    let mut draws = vec![Draw::Stroke {
        segments: square,
        width: stroke,
        colour: style.colour,
    }];
    if checked {
        let tick = svg_path::segments(TICK, side / 10).expect("the tick is valid path data");
        draws.push(Draw::Stroke {
            segments: tick,
            width: stroke * 3 / 2,
            colour: style.colour,
        });
    }
    Piece::Widget {
        width: side,
        height: side,
        draws,
        marks: Vec::new(),
        link: None,
        unit: Some(unit),
    }
}

/// A form field, drawn with its current value.
pub fn form(ctx: &mut Ctx<'_, '_>, field: &FormField, style: &TextStyle, unit: u32) -> Piece {
    let em = style.size;
    match &field.field {
        FormFieldKind::Text { value, .. } => text_box(ctx, value, em * 4, style, unit),
        FormFieldKind::Checkbox { checked } => checkbox(ctx, *checked, style, unit),
        FormFieldKind::Dropdown { options, selected } => {
            let chosen = selected
                .and_then(|i| options.get(usize::try_from(i).ok()?))
                .map_or("", String::as_str);
            text_box(ctx, chosen, em * 3, style, unit)
        }
        FormFieldKind::Date { value } => {
            text_box(ctx, value.as_deref().unwrap_or(""), em * 5, style, unit)
        }
    }
}

/// A signature field: a pale box with a signing line and its label. The box's rectangle is
/// what the signing track places the signature appearance in.
pub fn signature(
    ctx: &mut Ctx<'_, '_>,
    field: &SignatureField,
    style: &TextStyle,
    unit: u32,
) -> Piece {
    let (width, height) = ctx.options.signature_box;
    let (width, height) = (width.0.max(1), height.0.max(1));
    let label_size = style.size * 4 / 5;
    let label = field
        .label
        .clone()
        .unwrap_or_else(|| format!("Signature: {}", field.signer_role));
    let label_style = TextStyle {
        size: label_size,
        ..style.clone()
    };
    let (label, glyphs, _) = ctx.set(&label, &label_style);
    let label_ascent = ctx.fonts.ascent(style.face, label_size);
    let line_y = height - height / 5;
    let draws = vec![
        Draw::Fill {
            rect: Rect {
                x: 0,
                y: 0,
                width,
                height,
            },
            colour: FIELD_FILL,
        },
        Draw::Rule {
            from: (width / 20, line_y),
            to: (width - width / 20, line_y),
            width: 50,
            colour: INK,
        },
        Draw::Text(GlyphRun {
            text: label,
            glyphs,
            x: width / 20,
            y: (label_ascent + height / 10).min(line_y),
            size: label_size,
            face: style.face,
            colour: LABEL_GREY,
            units: Units::None,
        }),
    ];
    let rect = Rect {
        x: 0,
        y: 0,
        width,
        height,
    };
    Piece::Widget {
        width,
        height,
        draws,
        marks: vec![Mark::Signature {
            id: field.id.clone(),
            role: field.signer_role.clone(),
            rect,
        }],
        link: None,
        unit: Some(unit),
    }
}
