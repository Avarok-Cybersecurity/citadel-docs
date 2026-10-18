//! Shared by the display-list tests and the golden emitter: the kitchen sink laid out with the
//! shipped faces, its image, a hand-built list that uses every opcode, and proptest strategies
//! for arbitrary lists.

#![allow(dead_code, reason = "each test binary uses a different part")]

pub mod strategies;

use cdoc_layout::{layout, FaceId, FaceKey, FontLibrary, Layout, LayoutOptions, Rect, Segment};
use cdoc_model::{ContentId, Document, ImageLibrary};
use cdoc_paint::{DisplayList, FaceEntry, Glyph, GlyphMetric, Op, PageOps, Rgba, RunHead};

pub fn fonts() -> FontLibrary {
    cdoc_layout::test_support::default_fonts()
}

pub fn images() -> ImageLibrary {
    let mut lib = ImageLibrary::empty();
    lib.add(
        ContentId::new("img-1"),
        "image/jpeg",
        include_bytes!("../../../cdoc-pdf/tests/data/photo.jpg").to_vec(),
    );
    lib
}

pub fn laid_out(doc: &Document) -> Layout {
    layout(
        doc,
        &fonts(),
        &LayoutOptions::word_like(cdoc_layout::BreakerChoice::KnuthPlass(
            cdoc_layout::Params::word_like(),
        )),
    )
    .expect("lays out")
}

/// A generated document made layable: Word's built-in style sheet (which sets a font and a
/// size) in place of the random one, and every style reference cleared, since a random id
/// names no style in it. Content, direct formatting and structure are kept.
pub fn layable(mut doc: Document) -> Document {
    doc.styles = cdoc_model::StyleSheet::word_builtin();
    doc.for_each_table_mut(&mut |t| t.properties.style = None);
    doc.for_each_paragraph_mut(&mut |p| {
        p.properties.style = None;
        for inline in &mut p.inlines {
            if let cdoc_model::InlineContent::Text(run) = &mut inline.content {
                run.properties.style = None;
            }
        }
    });
    doc
}

pub fn kitchen() -> Layout {
    laid_out(&cdoc_model::test_support::fixtures::kitchen_sink())
}

/// The bytes of a Carlito face, by style.
pub fn face_bytes(bold: bool, italic: bool) -> &'static [u8] {
    cdoc_layout::test_support::FACES
        .iter()
        .find(|(b, i, _)| *b == bold && *i == italic)
        .expect("the shipped set has every style")
        .2
}

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
    ops.extend(vec![
        Op::GlyphRun {
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
        },
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
        Op::ClipPop,
    ]);
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

/// FNV-1a, 64-bit: a checksum with no dependency, for the golden files.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xCBF2_9CE4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01B3)
    })
}
