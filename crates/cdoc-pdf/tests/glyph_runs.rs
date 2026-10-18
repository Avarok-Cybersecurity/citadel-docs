//! The PDF draws the shaper's glyphs: a ligature reaches the content stream as the one CID the
//! layout set (the glyph id itself), and the `ToUnicode` map, built from the run's clusters,
//! reads it back as the characters it stands for.

mod support;

use cdoc_model::{Block, Inline, InlineContent, TextRun};

fn document(text: &str) -> cdoc_model::Document {
    let mut doc = cdoc_model::test_support::fixtures::kitchen_sink();
    let Some(Block::Paragraph(p)) = doc.sections[0].blocks.first_mut() else {
        panic!("the kitchen sink opens with a paragraph");
    };
    p.inlines = vec![Inline {
        content: InlineContent::Text(TextRun {
            text: text.to_owned(),
            properties: cdoc_model::RunProperties::none(),
        }),
        marks: cdoc_model::Marks::none(),
    }];
    doc
}

#[test]
fn a_ligature_is_one_cid_that_reads_back_as_its_characters() {
    let layout = support::laid_out(&document("first office"));
    let run = layout.pages[0]
        .runs()
        .find(|r| r.text.starts_with("first"))
        .expect("the run");
    let fi = run.glyphs[0];
    assert_eq!(run.glyphs[1].cluster, 2, "f and i are one glyph: {run:?}");
    let doc = support::load(&support::pdf_of(&layout));
    let page = *doc.get_pages().values().next().expect("page");
    let fonts = doc.get_page_fonts(page).expect("fonts");
    let font = fonts
        .get(format!("F{}", run.face.0).as_bytes())
        .expect("the run's font");
    let map = support::to_unicode(&doc, font);
    assert_eq!(map.get(&fi.id).map(String::as_str), Some("fi"));
    let read = support::pdf_runs(&doc, page);
    assert!(
        read.iter().any(|r| r.3 == run.text && !r.4),
        "read back through ToUnicode alone: {read:?}"
    );
}
