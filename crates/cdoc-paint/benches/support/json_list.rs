//! The display list itself as JSON: the same content as the binary format, field for field,
//! so the ratio measures the encoding rather than what is encoded.

use super::json::{rect, seg, JSeg};
use serde::{Deserialize, Serialize};

/// A face: family, bold, italic, units per em, `(glyph id, advance)` table.
pub type JFace = (String, bool, bool, u16, Vec<(u16, u16)>);

/// A heading: level, page, y, text.
pub type JHeading = (u8, u32, i32, String);

/// A display list.
#[derive(Serialize, Deserialize)]
pub struct JList {
    pub faces: Vec<JFace>,
    pub document: (Option<String>, Option<String>, Vec<JHeading>),
    pub pages: Vec<(u32, i32, i32, Vec<JOp>)>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum JOp {
    Shape {
        op: u8,
        coords: Vec<i32>,
        colour: String,
    },
    Path {
        op: u8,
        width: i32,
        segments: Vec<JSeg>,
        colour: String,
    },
    Glyphs {
        face: u16,
        size: i32,
        x: i32,
        y: i32,
        colour: String,
        text: String,
        glyphs: Vec<[i32; 5]>,
    },
    Ref {
        op: u8,
        rect: [i32; 4],
        target: String,
    },
    Field {
        rect: [i32; 4],
        id: String,
        role: String,
    },
    Clip {
        rect: Option<[i32; 4]>,
    },
    Carets {
        lines: Vec<JCaret>,
    },
}

/// One caret line.
#[derive(Serialize, Deserialize)]
pub struct JCaret {
    region: (u8, u32, u8),
    path: Vec<u32>,
    extent: [i32; 2],
    runs: u32,
    stops: Vec<(u32, i32)>,
}

fn hex(c: cdoc_paint::Rgba) -> String {
    format!("#{:02X}{:02X}{:02X}{:02X}", c.r, c.g, c.b, c.a)
}

fn region(r: cdoc_layout::Region) -> (u8, u32, u8) {
    use cdoc_layout::Region;
    let kind = |k: cdoc_model::HeaderFooterKind| k as u8;
    match r {
        Region::Body { section } => (0, section, 0),
        Region::Header { section, kind: k } => (1, section, kind(k)),
        Region::Footer { section, kind: k } => (2, section, kind(k)),
        Region::Footnote(id) => (3, id.0, 0),
    }
}

fn op(o: &cdoc_paint::Op) -> JOp {
    use cdoc_paint::Op;
    match o {
        Op::FillRect { rect: r, colour } => JOp::Shape {
            op: 1,
            coords: rect(*r).to_vec(),
            colour: hex(*colour),
        },
        Op::StrokeRect {
            rect: r,
            width,
            colour,
        } => {
            let mut coords = rect(*r).to_vec();
            coords.push(*width);
            JOp::Shape {
                op: 2,
                coords,
                colour: hex(*colour),
            }
        }
        Op::Line {
            from,
            to,
            width,
            colour,
        } => JOp::Shape {
            op: 3,
            coords: vec![from.0, from.1, to.0, to.1, *width],
            colour: hex(*colour),
        },
        Op::FillPath { segments, colour } => JOp::Path {
            op: 4,
            width: 0,
            segments: segments.iter().map(seg).collect(),
            colour: hex(*colour),
        },
        Op::StrokePath {
            segments,
            width,
            colour,
        } => JOp::Path {
            op: 5,
            width: *width,
            segments: segments.iter().map(seg).collect(),
            colour: hex(*colour),
        },
        Op::GlyphRun {
            head: h,
            text,
            glyphs,
        } => JOp::Glyphs {
            face: h.face.0,
            size: h.size,
            x: h.x,
            y: h.y,
            colour: hex(h.colour),
            text: text.clone(),
            glyphs: glyphs
                .iter()
                .map(|g| {
                    let cluster = i32::try_from(g.cluster).unwrap_or(i32::MAX);
                    [i32::from(g.id), cluster, g.advance, g.x_offset, g.y_offset]
                })
                .collect(),
        },
        Op::Image {
            rect: r,
            content_id,
        } => JOp::Ref {
            op: 0x20,
            rect: rect(*r),
            target: content_id.clone(),
        },
        Op::Link { rect: r, href } => JOp::Ref {
            op: 0x21,
            rect: rect(*r),
            target: href.clone(),
        },
        Op::SigField { rect: r, id, role } => JOp::Field {
            rect: rect(*r),
            id: id.clone(),
            role: role.clone(),
        },
        Op::ClipPush(r) => JOp::Clip {
            rect: Some(rect(*r)),
        },
        Op::ClipPop => JOp::Clip { rect: None },
        Op::CaretLines(lines) => JOp::Carets {
            lines: lines
                .iter()
                .map(|l| JCaret {
                    region: region(l.from.region),
                    path: l.from.path.clone(),
                    extent: [l.top, l.bottom],
                    runs: l.runs,
                    stops: l.stops.iter().map(|s| (s.gap, s.x)).collect(),
                })
                .collect(),
        },
    }
}

/// A display list as the JSON mirror.
pub fn mirror_list(list: &cdoc_paint::DisplayList) -> JList {
    JList {
        faces: list
            .faces
            .iter()
            .map(|f| {
                let glyphs = f.glyphs.iter().map(|g| (g.id, g.advance)).collect();
                (
                    f.key.family.clone(),
                    f.key.bold,
                    f.key.italic,
                    f.units_per_em,
                    glyphs,
                )
            })
            .collect(),
        document: (
            list.document.title.clone(),
            list.document.language.clone(),
            list.document
                .outline
                .iter()
                .map(|e| (e.level, e.page, e.y, e.title.clone()))
                .collect(),
        ),
        pages: list
            .pages
            .iter()
            .map(|p| (p.number, p.width, p.height, p.ops.iter().map(op).collect()))
            .collect(),
    }
}
