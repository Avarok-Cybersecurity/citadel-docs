//! [`every_op`]: one of every operation, the layout-independent pin of the byte format.

use cdoc_layout::{FaceId, FaceKey, OutlineEntry, Rect, Segment};
use cdoc_paint::{
    DisplayList, DocumentInfo, FaceEntry, Glyph, GlyphMetric, Op, PageOps, Rgba, RunHead,
};

const fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// The shapes of [`every_op`], inside an open clip.
fn shapes(red: Rgba) -> Vec<Op> {
    vec![
        Op::ClipPush(rect(0, 0, 59_528, 84_188)),
        Op::FillRect {
            rect: rect(-5, 63, 64, -8192),
            colour: red,
        },
        Op::StrokeRect {
            rect: rect(100, 200, 300, 400),
            width: 50,
            colour: red,
        },
        Op::Line {
            from: (0, 1),
            to: (i32::MAX, i32::MIN),
            width: 75,
            colour: red,
        },
        Op::FillPath {
            segments: vec![
                Segment::Move(1, 2),
                Segment::Line(3, 4),
                Segment::Cubic([5, 6, 7, 8, 9, 10]),
                Segment::Close,
            ],
            colour: red,
        },
        Op::StrokePath {
            segments: vec![Segment::Move(0, 0), Segment::Line(-100, 100)],
            width: 100,
            colour: red,
        },
    ]
}

/// An image, two links (one not a web link) and a signature field.
fn areas() -> Vec<Op> {
    vec![
        Op::Image {
            rect: rect(1, 2, 3, 4),
            content_id: "img-1".to_owned(),
        },
        Op::Link {
            rect: rect(5, 6, 7, 8),
            href: "https://avarok.net/ünïcode".to_owned(),
        },
        Op::Link {
            rect: rect(9, 9, 9, 9),
            href: "img-1".to_owned(),
        },
        Op::SigField {
            rect: rect(-10, 20, 30_000, 4_000),
            id: "sig-1".to_owned(),
            role: "Approver".to_owned(),
        },
    ]
}

/// A document table with a title, no language, and headings on both pages.
fn every_document() -> DocumentInfo {
    DocumentInfo {
        title: Some("Kitchen ünïcode".to_owned()),
        language: None,
        outline: vec![
            OutlineEntry {
                level: 0,
                title: "One".to_owned(),
                page: 0,
                y: -5,
            },
            OutlineEntry {
                level: 3,
                title: "img-1".to_owned(),
                page: 7,
                y: 70_000,
            },
        ],
    }
}

/// One of every operation, with negative, zero and multi-byte values, on two pages: the
/// layout-independent pin of the byte format.
pub fn every_op() -> DisplayList {
    let red = Rgba {
        r: 255,
        g: 0,
        b: 0,
        a: 128,
    };
    let head = RunHead {
        face: FaceId(1),
        size: 1100,
        colour: red,
        x: 7200,
        y: -1,
    };
    let mut ops = shapes(red);
    let areas = areas();
    ops.extend(vec![Op::GlyphRun {
        head,
        text: "fi é".to_owned(),
        glyphs: vec![
            Glyph {
                id: 65_535,
                cluster: 0,
                advance: 512,
                x_offset: -20,
                y_offset: 40,
            },
            Glyph {
                id: 3,
                cluster: 2,
                advance: -7,
                x_offset: 0,
                y_offset: 0,
            },
            Glyph {
                id: 300,
                cluster: 3,
                advance: 600,
                x_offset: 0,
                y_offset: 0,
            },
            Glyph {
                id: 301,
                cluster: 3,
                advance: 0,
                x_offset: -300,
                y_offset: -200,
            },
        ],
    }]);
    ops.extend(areas);
    ops.push(Op::ClipPop);
    let face = |family: &str, bold, italic, glyphs: Vec<GlyphMetric>| FaceEntry {
        key: FaceKey {
            family: family.to_owned(),
            bold,
            italic,
        },
        units_per_em: 2048,
        glyphs,
    };
    let metric = |id, advance| GlyphMetric { id, advance };
    DisplayList {
        faces: vec![
            face("Carlito", false, false, Vec::new()),
            face(
                "Carlito",
                true,
                true,
                vec![metric(65_535, 0), metric(3, 1130)],
            ),
        ],
        document: every_document(),
        pages: vec![
            PageOps {
                number: 0,
                width: 59_528,
                height: 84_188,
                ops,
            },
            PageOps {
                number: 7,
                width: 61_200,
                height: 79_200,
                ops: Vec::new(),
            },
        ],
    }
}
