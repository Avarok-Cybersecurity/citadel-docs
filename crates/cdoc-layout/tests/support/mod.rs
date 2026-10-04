//! The shipped default faces, read from the repository's `fonts/` folder, for tests only: the
//! library is pure and this reads files. The aliases are the ones the UI registers.

#![allow(dead_code, reason = "each test binary uses a different part")]

use cdoc_layout::{layout, FontLibrary, Layout, LayoutOptions};
use cdoc_model::Document;

/// The four Carlito faces, with Calibri and Calibri Light served by them.
#[must_use]
pub fn fonts() -> FontLibrary {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fonts");
    let mut lib = FontLibrary::new("Carlito");
    for (file, bold, italic) in [
        ("Carlito-Regular.ttf", false, false),
        ("Carlito-Bold.ttf", true, false),
        ("Carlito-Italic.ttf", false, true),
        ("Carlito-BoldItalic.ttf", true, true),
    ] {
        let bytes = std::fs::read(dir.join(file)).expect("the default faces are committed");
        lib.add("Carlito", bold, italic, bytes);
    }
    lib.alias("Calibri", "Carlito");
    lib.alias("Calibri Light", "Carlito");
    lib
}

/// The document laid out with the shipped faces and Word's options.
#[must_use]
pub fn laid_out(doc: &Document) -> Layout {
    layout(doc, &fonts(), &LayoutOptions::word_like()).expect("lays out")
}

/// A one-section document holding `blocks`, otherwise blank.
#[must_use]
pub fn doc_with(blocks: Vec<cdoc_model::Block>) -> Document {
    let mut doc = Document::blank();
    doc.sections[0].blocks = blocks;
    doc
}

/// A Normal paragraph of plain text.
#[must_use]
pub fn para(text: &str) -> cdoc_model::Block {
    cdoc_model::Block::Paragraph(cdoc_model::Paragraph {
        properties: cdoc_model::ParagraphProperties {
            style: Some(cdoc_model::builtin::ids::normal()),
            ..cdoc_model::ParagraphProperties::none()
        },
        inlines: vec![cdoc_model::Inline::text(
            text,
            cdoc_model::RunProperties::none(),
        )],
    })
}

/// Every run on every page as `page|x|y|size|face|text`, one per line: the golden form.
#[must_use]
pub fn dump(layout: &Layout) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for (i, page) in layout.pages.iter().enumerate() {
        let _ = writeln!(out, "page {i} {}x{}", page.width, page.height);
        for run in page.runs() {
            let _ = writeln!(
                out,
                "  {} {} {} f{} {:?}",
                run.x, run.y, run.size, run.face.0, run.text
            );
        }
        let others = page
            .draws
            .iter()
            .filter(|d| !matches!(d, cdoc_layout::Draw::Text(_)))
            .count();
        let _ = writeln!(out, "  +{others} shapes, {} links", page.links.len());
    }
    out
}

/// A paragraph in `style` holding `inlines`.
#[must_use]
pub fn para_with(
    style: Option<cdoc_model::StyleId>,
    inlines: Vec<cdoc_model::Inline>,
) -> cdoc_model::Block {
    cdoc_model::Block::Paragraph(cdoc_model::Paragraph {
        properties: cdoc_model::ParagraphProperties {
            style,
            ..cdoc_model::ParagraphProperties::none()
        },
        inlines,
    })
}

/// Plain text.
#[must_use]
pub fn text(s: &str) -> cdoc_model::Inline {
    cdoc_model::Inline::text(s, cdoc_model::RunProperties::none())
}
