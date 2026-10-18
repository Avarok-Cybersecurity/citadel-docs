//! Documents authored by other programs: pandoc (reference-template styles, numbering,
//! footnotes, an embedded image) and macOS `textutil` (Apple's text system, which writes no
//! `styles.xml` at all). Neither was built by this crate, so they exercise what the reader
//! meets in the wild. The fixtures are generated locally (`tests/fixtures/source.*`), not
//! downloaded.

mod common;

use cdoc_model::{Block, Document, InlineContent, StyleId};
use std::path::Path;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .expect("fixture present")
}

fn paragraph_texts(doc: &Document) -> Vec<String> {
    doc.paragraphs()
        .map(|p| {
            p.inlines
                .iter()
                .filter_map(|i| match &i.content {
                    InlineContent::Text(t) => Some(t.text.as_str()),
                    _ => None,
                })
                .collect()
        })
        .collect()
}

#[test]
fn pandoc_document_reads_with_its_structure() {
    let (doc, report, media) = common::read_foreign(&fixture("pandoc.docx"));
    assert_eq!(
        doc.validate(),
        Ok(()),
        "a foreign document must come out valid"
    );
    assert_eq!(doc.meta.title.as_deref(), Some("Quarterly Report"));
    assert_eq!(doc.meta.language.as_deref(), Some("en-GB"));
    let styles: Vec<_> = doc
        .paragraphs()
        .filter_map(|p| p.properties.style.as_ref().map(StyleId::as_str))
        .collect();
    assert!(
        styles.contains(&"Heading1")
            && styles.contains(&"Heading2")
            && styles.contains(&"BlockText")
    );
    let texts = paragraph_texts(&doc);
    assert!(texts
        .iter()
        .any(|t| t.starts_with("A paragraph with bold, italic, code and a link")));
    let link = doc
        .inlines()
        .find_map(|i| i.marks.link.as_ref())
        .expect("a hyperlink");
    assert_eq!(link.href, "https://avarok.net/docs");
    assert_eq!(doc.footnotes.len(), 1);
    let table = doc.sections[0]
        .blocks
        .iter()
        .find_map(|b| {
            if let Block::Table(t) = b {
                Some(t)
            } else {
                None
            }
        })
        .expect("a table");
    assert_eq!((table.column_widths.len(), table.rows.len()), (2, 3));
    assert!(
        table.rows[0].cells.iter().all(|c| c.header),
        "pandoc marks the header row"
    );
    let image = doc
        .inlines()
        .find_map(|i| match &i.content {
            InlineContent::Image(img) => Some(img),
            _ => None,
        })
        .expect("an image");
    assert_eq!(
        (image.width.0, image.height.0),
        (7200, 3600),
        "1in by 0.5in"
    );
    assert_eq!(image.alt.as_deref(), Some("A red pixel"));
    assert_eq!(media.ids().len(), 1);
    let stored = cdoc_model::ImagePort::image(&media, &media.ids()[0]).expect("stored");
    assert!(stored.bytes.starts_with(b"\x89PNG"));
    assert!(
        doc.paragraphs().any(|p| p.properties.numbering.is_some()),
        "lists keep their numbering"
    );
    assert!(
        report.count_of("w:bookmarkStart") > 0,
        "bookmarks are reported, not dropped silently"
    );
    assert!(report.has(|p| p.ends_with("fontTable.xml"), |w| w == "part not read"));
}

#[test]
fn textutil_document_without_styles_reads_with_substituted_styles() {
    let (doc, report, _) = common::read_foreign(&fixture("textutil.docx"));
    assert_eq!(doc.validate(), Ok(()));
    assert!(report.has(|_| true, |w| w.starts_with("styles.xml absent")));
    let texts = paragraph_texts(&doc);
    assert_eq!(texts[0], "Heading One");
    assert!(texts[1].starts_with("Plain, bold, italic, underlined and a link"));
    let bold = doc
        .inlines()
        .find(|i| matches!(&i.content, InlineContent::Text(t) if t.text == "bold"))
        .expect("bold run");
    let InlineContent::Text(run) = &bold.content else {
        unreachable!()
    };
    assert_eq!(run.properties.bold, Some(true));
}

#[test]
fn a_document_read_from_elsewhere_survives_another_trip() {
    for name in ["pandoc.docx", "textutil.docx"] {
        let (first, _, media) = common::read_foreign(&fixture(name));
        let (bytes, written) =
            cdoc_docx::write_docx(&first, &media, cdoc_docx::WriteOptions::document())
                .expect("writes");
        assert!(written.is_lossless(), "{name}: {written:?}");
        let mut again = cdoc_docx::MemoryMedia::with_prefix("img-");
        let second = cdoc_docx::read_docx(&bytes, &cdoc_docx::ReadLimits::generous(), &mut again)
            .expect("reads")
            .document;
        assert_eq!(
            second,
            first,
            "{name}: {:?}",
            common::first_diff(&second, &first)
        );
    }
}
