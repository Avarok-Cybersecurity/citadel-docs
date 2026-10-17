//! Rectangles, lines and paths as SVG elements.

use crate::{OpView, Rgba, Segments};
use cdoc_layout::ir::to_pt;
use cdoc_layout::{Rect, Segment};
use std::fmt::Write as _;

/// `fill="#RRGGBB"` (or `stroke=…`), plus an opacity attribute when the colour is not opaque.
pub fn attr_paint(attr: &str, colour: Rgba) -> String {
    if colour.a == 255 {
        format!("{attr}=\"{}\"", colour.hex())
    } else {
        format!(
            "{attr}=\"{}\" {attr}-opacity=\"{}\"",
            colour.hex(),
            f32::from(colour.a) / 255.0
        )
    }
}

fn path_data(segments: Segments<'_>) -> String {
    let mut d = String::new();
    for segment in segments {
        let _ = match segment {
            Segment::Move(x, y) => write!(d, "M{} {} ", to_pt(x), to_pt(y)),
            Segment::Line(x, y) => write!(d, "L{} {} ", to_pt(x), to_pt(y)),
            Segment::Cubic(c) => write!(
                d,
                "C{} {} {} {} {} {} ",
                to_pt(c[0]),
                to_pt(c[1]),
                to_pt(c[2]),
                to_pt(c[3]),
                to_pt(c[4]),
                to_pt(c[5])
            ),
            Segment::Close => write!(d, "Z "),
        };
    }
    d.trim_end().to_owned()
}

pub fn rect_attrs(r: Rect) -> String {
    format!(
        "x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
        to_pt(r.x),
        to_pt(r.y),
        to_pt(r.width),
        to_pt(r.height)
    )
}

/// Writes `op` if it is a shape; `false` leaves it to the caller.
pub fn shape(out: &mut String, op: &OpView<'_>) -> bool {
    match op.clone() {
        OpView::Line {
            from,
            to,
            width,
            colour,
        } => {
            let _ = write!(
                out,
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" {} stroke-width=\"{}\"/>",
                to_pt(from.0),
                to_pt(from.1),
                to_pt(to.0),
                to_pt(to.1),
                attr_paint("stroke", colour),
                to_pt(width)
            );
        }
        OpView::FillRect { rect, colour } => {
            let _ = write!(
                out,
                "<rect {} {}/>",
                rect_attrs(rect),
                attr_paint("fill", colour)
            );
        }
        OpView::StrokeRect {
            rect,
            width,
            colour,
        } => {
            let _ = write!(
                out,
                "<rect {} fill=\"none\" {} stroke-width=\"{}\"/>",
                rect_attrs(rect),
                attr_paint("stroke", colour),
                to_pt(width)
            );
        }
        OpView::FillPath { segments, colour } => {
            let _ = write!(
                out,
                "<path d=\"{}\" {}/>",
                path_data(segments),
                attr_paint("fill", colour)
            );
        }
        OpView::StrokePath {
            segments,
            width,
            colour,
        } => {
            let _ = write!(
                out,
                "<path d=\"{}\" fill=\"none\" {} stroke-width=\"{}\" \
                 stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
                path_data(segments),
                attr_paint("stroke", colour),
                to_pt(width)
            );
        }
        _ => return false,
    }
    true
}
