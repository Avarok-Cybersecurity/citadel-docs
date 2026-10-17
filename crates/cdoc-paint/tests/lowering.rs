//! What lowering does to text: every character becomes the glyph the face's `cmap` names, the
//! face entry lists it with its `hmtx` advance, and the decoded pen stands, glyph by glyph, at
//! the width the layout measured for that prefix of the run.

mod support;

use cdoc_layout::{Draw, Fonts};
use cdoc_paint::{decode, encode, lower, Op, OpView};

#[test]
fn every_character_becomes_its_cmap_glyph_and_the_pen_follows_the_measured_width() {
    let laid = support::kitchen();
    let port = support::fonts();
    let mut fonts = Fonts::new(&port);
    for key in &laid.faces {
        fonts
            .resolve(&key.family, key.bold, key.italic)
            .expect("face");
    }
    let bytes = encode(&laid, &port).expect("encodes");
    let view = decode(&bytes).expect("decodes");
    let mut checked = 0;
    for (slot, page) in laid.pages.iter().enumerate() {
        let lowered = view.page(slot).expect("page");
        for (draw, op) in page.draws.iter().zip(lowered.ops()) {
            let (Draw::Text(run), OpView::TextRun { head, glyphs }) = (draw, &op) else {
                assert!(!matches!(draw, Draw::Text(_)), "a run lowered to {op:?}");
                continue;
            };
            let key = &laid.faces[usize::from(run.face.0)];
            let face = ttf_parser::Face::parse(support::face_bytes(key.bold, key.italic), 0)
                .expect("parses");
            let expected: Vec<u16> = run
                .text
                .chars()
                .map(|c| face.glyph_index(c).map_or(0, |g| g.0))
                .collect();
            let ids: Vec<u16> = glyphs.clone().map(|g| g.id).collect();
            assert_eq!(ids, expected, "{:?}", run.text);
            assert!(
                ids.iter().all(|id| *id != 0),
                "sanitised text has no .notdef"
            );
            assert_eq!((head.x, head.y, head.size), (run.x, run.y, run.size));
            let mut pen = 0;
            for (k, (g, ch)) in glyphs.clone().zip(run.text.chars()).enumerate() {
                pen += g.advance;
                let prefix: String = run.text.chars().take(k + 1).collect();
                assert_eq!(
                    pen,
                    fonts.width(run.face, run.size, &prefix),
                    "pen after {ch:?} in {:?}",
                    run.text
                );
            }
            checked += 1;
        }
        let links = lowered.ops().filter(|op| matches!(op, OpView::Link { .. }));
        assert_eq!(links.count(), page.links.len(), "links follow the drawings");
    }
    assert!(checked > 10, "only {checked} runs checked");
}

#[test]
fn each_face_lists_the_glyphs_its_runs_use_once_with_their_advances() {
    let laid = support::kitchen();
    let all: Vec<usize> = (0..laid.pages.len()).collect();
    let list = lower(&laid, &support::fonts(), &all).expect("lowers");
    for (id, entry) in list.faces.iter().enumerate() {
        let face =
            ttf_parser::Face::parse(support::face_bytes(entry.key.bold, entry.key.italic), 0)
                .expect("parses");
        assert_eq!(entry.units_per_em, face.units_per_em());
        let mut seen = std::collections::BTreeSet::new();
        for g in &entry.glyphs {
            assert!(seen.insert(g.id), "glyph {} listed twice", g.id);
            assert_eq!(
                Some(g.advance),
                face.glyph_hor_advance(ttf_parser::GlyphId(g.id))
            );
        }
        let used: std::collections::BTreeSet<u16> = list
            .pages
            .iter()
            .flat_map(|p| &p.ops)
            .filter_map(|op| match op {
                Op::TextRun { head, glyphs } if usize::from(head.face.0) == id => Some(glyphs),
                _ => None,
            })
            .flatten()
            .copied()
            .collect();
        assert_eq!(seen, used, "face {id} lists exactly what its runs draw");
    }
}

#[test]
fn a_face_the_port_no_longer_serves_is_an_error() {
    let laid = support::kitchen();
    let empty = cdoc_layout::FontLibrary::new("Nothing");
    assert!(matches!(
        lower(&laid, &empty, &[0]),
        Err(cdoc_paint::PaintError::Font(_))
    ));
}
