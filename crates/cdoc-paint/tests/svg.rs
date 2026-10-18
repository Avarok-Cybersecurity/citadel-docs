//! The SVG painter draws what the display list says, not what some other description says:
//! a hand-built list, with no layout behind it, comes out as exactly its own positions,
//! glyphs, colours, clips and links.

mod support;

use cdoc_layout::{FaceId, FaceKey, Rect};
use cdoc_paint::svg::render_page;
use cdoc_paint::{
    decode, DisplayList, DocumentInfo, FaceEntry, Glyph, GlyphMetric, Op, PageOps, Rgba, RunHead,
};

fn glyph(c: char) -> u16 {
    let face = ttf_parser::Face::parse(support::face_bytes(false, false), 0).expect("parses");
    face.glyph_index(c).expect("glyph").0
}

/// Carlito Regular with a glyph table of `H`, `i` and `&`, each 1000 units wide.
fn carlito() -> FaceEntry {
    FaceEntry {
        key: FaceKey {
            family: "Carlito".to_owned(),
            bold: false,
            italic: false,
        },
        units_per_em: 2048,
        glyphs: ['H', 'i', '&']
            .map(|c| GlyphMetric {
                id: glyph(c),
                advance: 1000,
            })
            .to_vec(),
    }
}

fn page(ops: Vec<Op>) -> String {
    let list = DisplayList {
        faces: vec![carlito()],
        document: DocumentInfo::default(),
        pages: vec![PageOps {
            number: 4,
            width: 10_000,
            height: 20_000,
            ops,
        }],
    };
    let bytes = list.to_bytes().expect("writes");
    let view = decode(&bytes).expect("decodes");
    render_page(&view, 0, &support::images()).expect("paints")
}

const BLUE: Rgba = Rgba {
    r: 0,
    g: 0,
    b: 255,
    a: 255,
};

fn head(x: i32, y: i32) -> RunHead {
    RunHead {
        face: FaceId(0),
        size: 1200,
        colour: BLUE,
        x,
        y,
    }
}

fn g(id: u16, cluster: u32, advance: i32, x_offset: i32, y_offset: i32) -> Glyph {
    Glyph {
        id,
        cluster,
        advance,
        x_offset,
        y_offset,
    }
}

#[test]
fn text_is_written_from_the_runs_text_at_its_glyphs_positions() {
    let ids = ['H', 'i', '&'].map(glyph);
    let svg = page(vec![
        Op::GlyphRun {
            head: head(1234, 5678),
            text: "Hi&".to_owned(),
            glyphs: vec![
                g(ids[0], 0, 600, 0, 0),
                g(ids[1], 1, 300, 0, 0),
                g(ids[2], 2, 500, 0, 0),
            ],
        },
        Op::GlyphRun {
            head: head(100, 200),
            text: "Hi".to_owned(),
            glyphs: vec![g(ids[0], 0, 650, 0, 0), g(ids[1], 1, 300, 25, -50)],
        },
    ]);
    assert!(svg.starts_with("<svg ") && svg.contains("viewBox=\"0 0 100 200\""));
    assert!(
        svg.contains(
            "<text x=\"12.34 18.34 21.34\" y=\"56.78 56.78 56.78\" font-family=\"'Carlito'\""
        ),
        "{svg}"
    );
    assert!(svg.contains("font-size=\"12\" fill=\"#0000FF\" xml:space=\"preserve\">Hi&amp;</text>"));
    assert!(svg.contains("<text x=\"1 7.75\" y=\"2 1.5\""), "{svg}");
}

#[test]
fn a_ligature_spreads_its_characters_over_its_advance_and_keeps_the_text() {
    let fi = 1234;
    let svg = page(vec![Op::GlyphRun {
        head: head(0, 1000),
        text: "fix".to_owned(),
        glyphs: vec![g(fi, 0, 1000, 0, 0), g(glyph('i'), 2, 300, 0, 0)],
    }]);
    assert!(svg.contains("<text x=\"0 5 10\" y=\"10 10 10\""), "{svg}");
    assert!(svg.contains(">fix</text>"), "{svg}");
}

#[test]
fn shapes_clips_alpha_and_links_come_from_the_list() {
    let r = Rect {
        x: 100,
        y: 200,
        width: 300,
        height: 400,
    };
    let half = Rgba { a: 51, ..BLUE };
    let svg = page(vec![
        Op::ClipPush(r),
        Op::StrokeRect {
            rect: r,
            width: 50,
            colour: half,
        },
        Op::Line {
            from: (0, 0),
            to: (1000, 0),
            width: 25,
            colour: BLUE,
        },
        Op::ClipPop,
        Op::Link {
            rect: r,
            href: "javascript:alert(1)".to_owned(),
        },
        Op::Link {
            rect: r,
            href: "https://avarok.net/?a=1&b=2".to_owned(),
        },
        Op::Image {
            rect: r,
            content_id: "img-1".to_owned(),
        },
    ]);
    assert!(
        svg.contains(
            "<clipPath id=\"p4c1\"><rect x=\"1\" y=\"2\" width=\"3\" height=\"4\"/></clipPath>\
         <g clip-path=\"url(#p4c1)\"><rect x=\"1\" y=\"2\" width=\"3\" height=\"4\" fill=\"none\" \
         stroke=\"#0000FF\" stroke-opacity=\"0.2\" stroke-width=\"0.5\"/>"
        ),
        "{svg}"
    );
    assert!(svg.contains(
        "<line x1=\"0\" y1=\"0\" x2=\"10\" y2=\"0\" stroke=\"#0000FF\" stroke-width=\"0.25\"/></g>"
    ));
    assert!(!svg.contains("javascript:"));
    assert!(svg.contains("<a href=\"https://avarok.net/?a=1&amp;b=2\">"));
    assert!(svg.contains("href=\"data:image/jpeg;base64,"));
}

#[test]
fn a_missing_image_is_an_error_not_a_blank() {
    let list = DisplayList {
        faces: Vec::new(),
        document: DocumentInfo::default(),
        pages: vec![PageOps {
            number: 0,
            width: 1,
            height: 1,
            ops: vec![Op::Image {
                rect: Rect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                },
                content_id: "nope".to_owned(),
            }],
        }],
    };
    let bytes = list.to_bytes().expect("writes");
    let view = decode(&bytes).expect("decodes");
    assert!(matches!(
        render_page(&view, 0, &support::images()),
        Err(cdoc_paint::PaintError::Image(_))
    ));
}
