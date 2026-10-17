//! The baseline the binary format is measured against: the same layout IR as JSON, the way a
//! straightforward implementation would ship it (text as strings, colours as hex, every field
//! named), through `serde_json`.

use cdoc_layout::{Draw, Layout, Segment};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct JLayout {
    pub faces: Vec<(String, bool, bool)>,
    pub pages: Vec<JPage>,
}

#[derive(Serialize, Deserialize)]
pub struct JPage {
    pub width: i32,
    pub height: i32,
    pub draws: Vec<JDraw>,
    pub links: Vec<([i32; 4], String)>,
}

#[derive(Serialize, Deserialize)]
pub enum JSeg {
    M(i32, i32),
    L(i32, i32),
    C([i32; 6]),
    Z,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum JDraw {
    Text {
        text: String,
        x: i32,
        y: i32,
        size: i32,
        face: u16,
        colour: String,
    },
    Rule {
        from: (i32, i32),
        to: (i32, i32),
        width: i32,
        colour: String,
    },
    Fill {
        rect: [i32; 4],
        colour: String,
    },
    Path {
        segments: Vec<JSeg>,
        colour: String,
    },
    Stroke {
        segments: Vec<JSeg>,
        width: i32,
        colour: String,
    },
    Image {
        rect: [i32; 4],
        content_id: String,
    },
}

pub fn seg(s: &Segment) -> JSeg {
    match *s {
        Segment::Move(x, y) => JSeg::M(x, y),
        Segment::Line(x, y) => JSeg::L(x, y),
        Segment::Cubic(c) => JSeg::C(c),
        Segment::Close => JSeg::Z,
    }
}

pub fn rect(r: cdoc_layout::Rect) -> [i32; 4] {
    [r.x, r.y, r.width, r.height]
}

fn draw(d: &Draw) -> JDraw {
    match d {
        Draw::Text(r) => JDraw::Text {
            text: r.text.clone(),
            x: r.x,
            y: r.y,
            size: r.size,
            face: r.face.0,
            colour: r.colour.hex(),
        },
        Draw::Rule {
            from,
            to,
            width,
            colour,
        } => JDraw::Rule {
            from: *from,
            to: *to,
            width: *width,
            colour: colour.hex(),
        },
        Draw::Fill { rect: r, colour } => JDraw::Fill {
            rect: rect(*r),
            colour: colour.hex(),
        },
        Draw::Path { segments, colour } => JDraw::Path {
            segments: segments.iter().map(seg).collect(),
            colour: colour.hex(),
        },
        Draw::Stroke {
            segments,
            width,
            colour,
        } => JDraw::Stroke {
            segments: segments.iter().map(seg).collect(),
            width: *width,
            colour: colour.hex(),
        },
        Draw::Image {
            rect: r,
            content_id,
        } => JDraw::Image {
            rect: rect(*r),
            content_id: content_id.as_str().to_owned(),
        },
    }
}

/// The layout as the JSON mirror.
pub fn mirror(layout: &Layout) -> JLayout {
    JLayout {
        faces: layout
            .faces
            .iter()
            .map(|f| (f.family.clone(), f.bold, f.italic))
            .collect(),
        pages: layout
            .pages
            .iter()
            .map(|p| JPage {
                width: p.width,
                height: p.height,
                draws: p.draws.iter().map(draw).collect(),
                links: p
                    .links
                    .iter()
                    .map(|l| (rect(l.rect), l.href.clone()))
                    .collect(),
            })
            .collect(),
    }
}
