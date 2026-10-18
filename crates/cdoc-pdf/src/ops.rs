// Adapted from rMazing crates/cc-export/src/backends/ops.rs
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! One drawing of the layout as content-stream operations. The IR is page space (y down);
//! PDF user space is y up, so every y is flipped here and nowhere else.

use crate::font::Embedded;
use cdoc_layout::ir::to_pt;
use cdoc_layout::{Draw, Pt100, Segment};
use cdoc_model::Color;
use pdf_writer::types::{LineCapStyle, LineJoinStyle};
use pdf_writer::{Content, Name};
use std::collections::BTreeMap;

/// The resource names a page's content refers to.
pub struct Names<'a> {
    /// Embedded faces by face id; the font resource is `F{id}`.
    pub fonts: &'a BTreeMap<u16, Embedded>,
    /// Image resource names by content id.
    pub images: &'a BTreeMap<String, String>,
}

fn unit(colour: Color) -> (f32, f32, f32) {
    (
        f32::from(colour.r) / 255.0,
        f32::from(colour.g) / 255.0,
        f32::from(colour.b) / 255.0,
    )
}

fn path(content: &mut Content, segments: &[Segment], height: Pt100) {
    let y = |v: Pt100| to_pt(height - v);
    for segment in segments {
        match *segment {
            Segment::Move(px, py) => {
                content.move_to(to_pt(px), y(py));
            }
            Segment::Line(px, py) => {
                content.line_to(to_pt(px), y(py));
            }
            Segment::Cubic([x1, y1, x2, y2, x3, y3]) => {
                content.cubic_to(to_pt(x1), y(y1), to_pt(x2), y(y2), to_pt(x3), y(y3));
            }
            Segment::Close => {
                content.close_path();
            }
        }
    }
}

/// Appends one drawing to `content` for a page `height` tall.
pub fn draw(content: &mut Content, draw: &Draw, height: Pt100, names: &Names<'_>) {
    let flip = |v: Pt100| to_pt(height - v);
    match draw {
        Draw::Text(run) => {
            let Some(face) = names.fonts.get(&run.face.0) else {
                return;
            };
            let (r, g, b) = unit(run.colour);
            let font = format!("F{}", run.face.0);
            content.set_fill_rgb(r, g, b);
            content.begin_text();
            content.set_font(Name(font.as_bytes()), to_pt(run.size));
            content.set_text_matrix([1.0, 0.0, 0.0, 1.0, to_pt(run.x), flip(run.y)]);
            content.show(pdf_writer::Str(&face.encode(&run.text)));
            content.end_text();
        }
        Draw::Rule {
            from,
            to,
            width,
            colour,
        } => {
            let (r, g, b) = unit(*colour);
            content.set_stroke_rgb(r, g, b);
            content.set_line_width(to_pt(*width));
            content.set_line_cap(LineCapStyle::ButtCap);
            content.move_to(to_pt(from.0), flip(from.1));
            content.line_to(to_pt(to.0), flip(to.1));
            content.stroke();
        }
        Draw::Fill { rect, colour } => {
            let (r, g, b) = unit(*colour);
            content.set_fill_rgb(r, g, b);
            content.rect(
                to_pt(rect.x),
                flip(rect.y + rect.height),
                to_pt(rect.width),
                to_pt(rect.height),
            );
            content.fill_nonzero();
        }
        Draw::Path { segments, colour } => {
            let (r, g, b) = unit(*colour);
            content.set_fill_rgb(r, g, b);
            path(content, segments, height);
            content.fill_nonzero();
        }
        Draw::Stroke {
            segments,
            width,
            colour,
        } => {
            let (r, g, b) = unit(*colour);
            content.set_stroke_rgb(r, g, b);
            content.set_line_width(to_pt(*width));
            content.set_line_cap(LineCapStyle::RoundCap);
            content.set_line_join(LineJoinStyle::RoundJoin);
            path(content, segments, height);
            content.stroke();
        }
        Draw::Image { rect, content_id } => {
            let Some(name) = names.images.get(content_id.as_str()) else {
                placeholder(content, *rect, height);
                return;
            };
            content.save_state();
            content.transform([
                to_pt(rect.width),
                0.0,
                0.0,
                to_pt(rect.height),
                to_pt(rect.x),
                flip(rect.y + rect.height),
            ]);
            content.x_object(Name(name.as_bytes()));
            content.restore_state();
        }
    }
}

/// The box drawn for an image of a type the PDF does not draw: light grey, outlined.
fn placeholder(content: &mut Content, rect: cdoc_layout::Rect, height: Pt100) {
    content.save_state();
    content.set_fill_rgb(0.93, 0.93, 0.93);
    content.set_stroke_rgb(0.6, 0.6, 0.6);
    content.set_line_width(0.5);
    content.rect(
        to_pt(rect.x),
        to_pt(height - rect.y - rect.height),
        to_pt(rect.width),
        to_pt(rect.height),
    );
    content.fill_nonzero_and_stroke();
    content.restore_state();
}
