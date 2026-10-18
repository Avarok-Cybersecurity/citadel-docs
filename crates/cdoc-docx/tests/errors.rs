//! Bad input is an error, never a panic and never a silent empty document; and what the
//! writer cannot carry is reported.

mod common;

use cdoc_docx::{read_docx, write_docx, DocxError, MemoryMedia, ReadLimits, WriteOptions};
use cdoc_model::{ContentId, Document, Inline, InlineContent, InlineImage, Length};

fn read_err(bytes: &[u8], limits: &ReadLimits) -> DocxError {
    read_docx(bytes, limits, &mut MemoryMedia::with_prefix("i-")).expect_err("must fail")
}

#[test]
fn not_a_zip_is_a_zip_error() {
    assert!(matches!(
        read_err(b"definitely not a zip", &ReadLimits::generous()),
        DocxError::Zip(_)
    ));
    assert!(matches!(
        read_err(b"", &ReadLimits::generous()),
        DocxError::Zip(_)
    ));
}

#[test]
fn a_package_without_a_main_document_names_the_missing_part() {
    let bytes = common::package(&[("hello.txt", "hi")]);
    assert_eq!(
        read_err(&bytes, &ReadLimits::generous()),
        DocxError::MissingPart("[Content_Types].xml".into())
    );
}

#[test]
fn malformed_xml_names_the_part() {
    let bytes = common::word_package("<w:p>", "", &[]);
    let err = read_err(&bytes, &ReadLimits::generous());
    assert!(
        matches!(&err, DocxError::Xml { part, .. } if part == "word/document.xml"),
        "{err:?}"
    );
}

#[test]
fn a_document_with_no_body_is_invalid() {
    let bytes = common::package(&[
        ("[Content_Types].xml", "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"/>"),
        ("_rels/.rels", "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"r\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/></Relationships>"),
        ("word/document.xml", "<w:document xmlns:w=\"x\"/>"),
    ]);
    assert!(matches!(
        read_err(&bytes, &ReadLimits::generous()),
        DocxError::Invalid { .. }
    ));
}

#[test]
fn inflation_limits_stop_a_bomb() {
    let big = "a".repeat(200_000);
    let bytes = common::package(&[("[Content_Types].xml", &big), ("x", "y")]);
    let tight = ReadLimits {
        max_parts: 10,
        max_part_bytes: 1000,
        max_total_bytes: 1 << 20,
        ..ReadLimits::generous()
    };
    assert_eq!(read_err(&bytes, &tight), DocxError::TooLarge("part size"));
    let total = ReadLimits {
        max_parts: 10,
        max_part_bytes: 1 << 20,
        max_total_bytes: 1000,
        ..ReadLimits::generous()
    };
    assert_eq!(read_err(&bytes, &total), DocxError::TooLarge("total size"));
    let few = ReadLimits {
        max_parts: 1,
        max_part_bytes: 1 << 20,
        max_total_bytes: 1 << 20,
        ..ReadLimits::generous()
    };
    assert_eq!(read_err(&bytes, &few), DocxError::TooLarge("part count"));
    assert_eq!(
        read_err(&bytes, &ReadLimits::generous()),
        DocxError::TooLarge("compression ratio"),
        "200 kB of one letter deflates a thousandfold"
    );
}

#[test]
fn a_missing_image_fails_the_write_with_its_id() {
    let mut doc = Document::blank();
    let image = InlineImage {
        content_id: ContentId::new("nope"),
        width: Length(100),
        height: Length(100),
        alt: None,
    };
    let cdoc_model::Block::Paragraph(p) = &mut doc.sections[0].blocks[0] else {
        unreachable!()
    };
    p.inlines.push(Inline::plain(InlineContent::Image(image)));
    let err = write_docx(
        &doc,
        &MemoryMedia::with_prefix("i-"),
        WriteOptions::document(),
    )
    .expect_err("no such image");
    assert_eq!(
        err,
        DocxError::Media(cdoc_model::ImageError("nope".to_owned()))
    );
}

#[test]
fn characters_xml_cannot_carry_are_replaced_and_reported() {
    let mut doc = Document::blank();
    let cdoc_model::Block::Paragraph(p) = &mut doc.sections[0].blocks[0] else {
        unreachable!()
    };
    p.inlines.push(Inline::text(
        "a\u{1}b\u{0}c",
        cdoc_model::RunProperties::none(),
    ));
    let (bytes, report) = common::write(&doc);
    assert_eq!(
        report.count_of("characters not allowed in XML replaced with U+FFFD"),
        2
    );
    let (back, _) = common::read(&bytes);
    let InlineContent::Text(t) = &back.paragraphs().next().expect("p").inlines[0].content else {
        panic!()
    };
    assert_eq!(t.text, "a\u{FFFD}b\u{FFFD}c");
}

#[test]
fn lengths_finer_than_a_twip_are_rounded_and_reported() {
    let mut doc = Document::blank();
    doc.sections[0].properties.page.margins.top = Length(1441 * 5 + 2);
    let (bytes, report) = common::write(&doc);
    assert_eq!(report.count_of("length rounded to whole twips"), 1);
    let (back, _) = common::read(&bytes);
    assert_eq!(
        back.sections[0].properties.page.margins.top,
        Length(1441 * 5)
    );
}

#[test]
fn writing_is_deterministic() {
    let doc = cdoc_model::test_support::fixtures::kitchen_sink();
    assert_eq!(
        common::write(&doc).0,
        common::write(&doc).0,
        "no clock, no randomness"
    );
}
