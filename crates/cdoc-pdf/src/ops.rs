// Adapted from rMazing crates/cc-export/src/backends/ops.rs
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! One display-list operation as content-stream operations. The list is page space (y down);
//! PDF user space is y up, so every y is flipped here and nowhere else. Links and signature
//! fields are annotations, not content, and draw nothing here.

use crate::font::Embedded;
use crate::text::{show, Run};
use cdoc_layout::ir::to_pt;
use cdoc_layout::{Pt100, Rect, Segment};
use cdoc_paint::{Op, Rgba};
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

/// The graphics-state resource that paints at alpha `a` (`/ca` and `/CA`).
#[must_use]
pub fn alpha_name(a: u8) -> String {
    format!("GA{a}")
}

/// The alpha an operation paints at, when it paints with a colour that is not opaque.
#[must_use]
pub const fn translucency(op: &Op) -> Option<u8> {
    let colour = match op {
        Op::FillRect { colour, .. }
        | Op::StrokeRect { colour, .. }
        | Op::Line { colour, .. }
        | Op::FillPath { colour, .. }
        | Op::StrokePath { colour, .. } => *colour,
        Op::GlyphRun { head, .. } => head.colour,
        _ => return None,
    };
    if colour.a == 255 {
        None
    } else {
        Some(colour.a)
    }
}

fn unit(colour: Rgba) -> (f32, f32, f32) {
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

fn rect(content: &mut Content, r: Rect, height: Pt100) {
    content.rect(
        to_pt(r.x),
        to_pt(height - r.y - r.height),
        to_pt(r.width),
        to_pt(r.height),
    );
}

fn fill(content: &mut Content, colour: Rgba) {
    let (r, g, b) = unit(colour);
    content.set_fill_rgb(r, g, b);
}

fn stroke(content: &mut Content, colour: Rgba, width: Pt100) {
    let (r, g, b) = unit(colour);
    content.set_stroke_rgb(r, g, b);
    content.set_line_width(to_pt(width));
}

/// Appends one operation to `content` for a page `height` tall. A translucent colour is
/// painted inside its own graphics state, at the alpha of the [`alpha_name`] resource.
pub fn draw(content: &mut Content, op: &Op, height: Pt100, names: &Names<'_>) {
    let translucent = translucency(op);
    if let Some(a) = translucent {
        content.save_state();
        content.set_parameters(Name(alpha_name(a).as_bytes()));
    }
    paint(content, op, height, names);
    if translucent.is_some() {
        content.restore_state();
    }
}

fn paint(content: &mut Content, op: &Op, height: Pt100, names: &Names<'_>) {
    let flip = |v: Pt100| to_pt(height - v);
    match *op {
        Op::GlyphRun {
            ref head,
            ref text,
            ref glyphs,
        } => {
            let Some(face) = names.fonts.get(&head.face.0) else {
                return;
            };
            fill(content, head.colour);
            let shaped = Run { head, text, glyphs };
            let font = format!("F{}", head.face.0);
            show(content, &shaped, face, &font, flip(head.y));
        }
        Op::Line {
            from,
            to,
            width,
            colour,
        } => {
            stroke(content, colour, width);
            content.set_line_cap(LineCapStyle::ButtCap);
            content.move_to(to_pt(from.0), flip(from.1));
            content.line_to(to_pt(to.0), flip(to.1));
            content.stroke();
        }
        Op::FillRect { rect: r, colour } => {
            fill(content, colour);
            rect(content, r, height);
            content.fill_nonzero();
        }
        Op::StrokeRect {
            rect: r,
            width,
            colour,
        } => {
            stroke(content, colour, width);
            content.set_line_join(LineJoinStyle::MiterJoin);
            rect(content, r, height);
            content.stroke();
        }
        Op::FillPath {
            ref segments,
            colour,
        } => {
            fill(content, colour);
            path(content, segments, height);
            content.fill_nonzero();
        }
        Op::StrokePath {
            ref segments,
            width,
            colour,
        } => {
            stroke(content, colour, width);
            content.set_line_cap(LineCapStyle::RoundCap);
            content.set_line_join(LineJoinStyle::RoundJoin);
            path(content, segments, height);
            content.stroke();
        }
        Op::Image {
            rect: r,
            ref content_id,
        } => image(content, r, names.images.get(content_id), height),
        Op::ClipPush(r) => {
            content.save_state();
            rect(content, r, height);
            content.clip_nonzero();
            content.end_path();
        }
        Op::ClipPop => {
            content.restore_state();
        }
        Op::Link { .. } | Op::SigField { .. } => {}
    }
}

/// An image `XObject` (resource `name`) filling `r`, or, for a type the PDF does not draw,
/// a light grey outlined box.
fn image(content: &mut Content, r: Rect, name: Option<&String>, height: Pt100) {
    content.save_state();
    if let Some(name) = name {
        content.transform([
            to_pt(r.width),
            0.0,
            0.0,
            to_pt(r.height),
            to_pt(r.x),
            to_pt(height - r.y - r.height),
        ]);
        content.x_object(Name(name.as_bytes()));
    } else {
        content.set_fill_rgb(0.93, 0.93, 0.93);
        content.set_stroke_rgb(0.6, 0.6, 0.6);
        content.set_line_width(0.5);
        rect(content, r, height);
        content.fill_nonzero_and_stroke();
    }
    content.restore_state();
}
