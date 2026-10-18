//! What lowering does to text: every glyph run of the layout becomes one `GLYPH_RUN` with the
//! same glyphs, advances, offsets, clusters and text, so the decoded pen ends exactly where
//! the layout measured the run to end, and each face entry lists the glyphs its runs use with
//! their `hmtx` advances.

mod support;

use cdoc_layout::Draw;
use cdoc_paint::{decode, encode, lower, Op, OpView};

#[test]
fn every_glyph_run_passes_through_as_it_was_set() {
    let laid = support::kitchen();
    let port = support::fonts();
    let bytes = encode(&laid, &port).expect("encodes");
    let view = decode(&bytes).expect("decodes");
    let mut checked = 0;
    for (slot, page) in laid.pages.iter().enumerate() {
        let lowered = view.page(slot).expect("page");
        for (draw, op) in page.draws.iter().zip(lowered.ops()) {
            let (Draw::Text(run), OpView::GlyphRun { head, text, glyphs }) = (draw, &op) else {
                assert!(!matches!(draw, Draw::Text(_)), "a run lowered to {op:?}");
                continue;
            };
            assert_eq!(*text, run.text);
            assert_eq!((head.x, head.y, head.size), (run.x, run.y, run.size));
            let got: Vec<_> = glyphs
                .clone()
                .map(|g| (g.id, g.cluster, g.advance, g.x_offset, g.y_offset))
                .collect();
            let want: Vec<_> = run
                .glyphs
                .iter()
                .map(|g| (g.id, g.cluster, g.advance, g.x_offset, g.y_offset))
                .collect();
            assert_eq!(got, want, "{:?}", run.text);
            assert!(
                got.iter().all(|g| g.0 != 0),
                "sanitised text has no .notdef"
            );
            let pen: i32 = glyphs.clone().map(|g| g.advance).sum();
            assert_eq!(pen, run.width(), "{:?}", run.text);
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
                Op::GlyphRun { head, glyphs, .. } if usize::from(head.face.0) == id => Some(glyphs),
                _ => None,
            })
            .flatten()
            .map(|g| g.id)
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
