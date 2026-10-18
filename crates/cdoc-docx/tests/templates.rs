//! Templates and macro-enabled packages: `.dotx` reads and writes like `.docx` with its kind
//! reported; `.docm` / `.dotm` read with their macros refused (never carried, never written,
//! each part reported as removed).

mod common;

use cdoc_docx::{read_docx, write_docx, Kind, MemoryMedia, ReadLimits, WriteOptions};
use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_model::Loss;
use std::path::Path;

const TEMPLATE_MAIN: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml";
const DOCUMENT_MAIN: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const MACRO_TEMPLATE_MAIN: &str = "application/vnd.ms-word.template.macroEnabledTemplate.main+xml";
const VBA_REL: &str = "http://schemas.microsoft.com/office/2006/relationships/vbaProject";
const VBA: &str = "VBA project stand-in: not a real compound file";
const VBA_DATA: &str =
    "<wne:vbaSuppData xmlns:wne=\"http://schemas.microsoft.com/office/word/2006/wordml\"/>";

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .expect("fixture present")
}

fn write(options: WriteOptions) -> Vec<u8> {
    let doc = kitchen_sink();
    write_docx(&doc, &common::library_for(&doc), options)
        .expect("writes")
        .0
}

fn read(bytes: &[u8]) -> cdoc_docx::Imported {
    read_docx(bytes, &ReadLimits::generous(), &mut common::IdFromBytes).expect("reads")
}

/// Rewrite one XML part of a package in place.
fn edit(parts: &mut [(String, Vec<u8>)], name: &str, f: impl Fn(&str) -> String) {
    let (_, bytes) = parts
        .iter_mut()
        .find(|(n, _)| n == name)
        .expect("part present");
    *bytes = f(std::str::from_utf8(bytes).expect("utf-8 xml")).into_bytes();
}

fn repack(parts: &[(String, Vec<u8>)]) -> Vec<u8> {
    let all: Vec<(&str, &[u8])> = parts
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice()))
        .collect();
    common::package_bytes(&all)
}

/// A `.dotm` made from a written `.docx`: the main part retyped, a VBA project and its data
/// added with the relationships Word gives them.
fn macro_template() -> Vec<u8> {
    let mut parts = common::parts(&write(WriteOptions::document()));
    edit(&mut parts, "[Content_Types].xml", |t| {
        t.replace(DOCUMENT_MAIN, MACRO_TEMPLATE_MAIN).replace(
            "</Types>",
            "<Default Extension=\"bin\" ContentType=\"application/vnd.ms-office.vbaProject\"/>\
             <Override PartName=\"/word/vbaData.xml\" ContentType=\"application/vnd.ms-word.vbaData+xml\"/></Types>",
        )
    });
    edit(&mut parts, "word/_rels/document.xml.rels", |t| {
        t.replace(
            "</Relationships>",
            &format!("<Relationship Id=\"rIdVba\" Type=\"{VBA_REL}\" Target=\"vbaProject.bin\"/></Relationships>"),
        )
    });
    let vba_rels = "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
        <Relationship Id=\"rId1\" Type=\"http://schemas.microsoft.com/office/2006/relationships/wordVbaData\" Target=\"vbaData.xml\"/></Relationships>";
    parts.extend([
        ("word/vbaProject.bin".to_owned(), VBA.as_bytes().to_vec()),
        ("word/vbaData.xml".to_owned(), VBA_DATA.as_bytes().to_vec()),
        (
            "word/_rels/vbaProject.bin.rels".to_owned(),
            vba_rels.as_bytes().to_vec(),
        ),
    ]);
    repack(&parts)
}

#[test]
fn a_template_is_written_with_the_template_main_type_and_reads_back_as_one() {
    let bytes = write(WriteOptions::template());
    let types = common::part_text(&bytes, "[Content_Types].xml");
    assert!(
        types.contains(TEMPLATE_MAIN) && !types.contains(DOCUMENT_MAIN),
        "{types}"
    );
    let back = read(&bytes);
    assert_eq!(back.kind, Kind::Template);
    assert_eq!(back.document, kitchen_sink());
    assert!(back.report.is_lossless(), "{:?}", back.report);
}

#[test]
fn a_document_is_written_with_the_document_main_type() {
    let bytes = write(WriteOptions::document());
    let types = common::part_text(&bytes, "[Content_Types].xml");
    assert!(types.contains(DOCUMENT_MAIN) && !types.contains(TEMPLATE_MAIN));
    assert_eq!(read(&bytes).kind, Kind::Document);
}

#[test]
fn macros_are_refused_reported_and_never_written_back() {
    let read = read(&macro_template());
    assert_eq!(
        read.kind,
        Kind::Template,
        "a .dotm is a template once its macros are gone"
    );
    assert_eq!(
        read.document,
        kitchen_sink(),
        "the document itself is untouched"
    );
    let removed: Vec<(&str, u64)> = read.report.macros();
    assert_eq!(
        removed,
        [
            ("word/vbaData.xml", VBA_DATA.len() as u64),
            ("word/vbaProject.bin", VBA.len() as u64)
        ]
    );
    assert!(
        read.report
            .losses
            .iter()
            .all(|l| matches!(l, Loss::MacrosRemoved { .. })),
        "nothing else is lost, and the macro parts are not also 'not read': {:?}",
        read.report
    );
    for options in [WriteOptions::document(), WriteOptions::template()] {
        let (again, _) = write_docx(
            &read.document,
            &common::library_for(&read.document),
            options,
        )
        .expect("writes");
        let names = common::part_names(&again);
        assert!(
            names
                .iter()
                .all(|n| !n.to_ascii_lowercase().contains("vba")),
            "{names:?}"
        );
        let types = common::part_text(&again, "[Content_Types].xml");
        assert!(
            !types.contains("macroEnabled") && !types.contains("vba"),
            "{types}"
        );
    }
}

#[test]
fn a_spreadsheet_main_part_is_refused() {
    let mut parts = common::parts(&write(WriteOptions::document()));
    edit(&mut parts, "[Content_Types].xml", |t| {
        t.replace(
            DOCUMENT_MAIN,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
        )
    });
    let err = read_docx(
        &repack(&parts),
        &ReadLimits::generous(),
        &mut MemoryMedia::with_prefix("i"),
    )
    .expect_err("not a Word document");
    assert!(
        err.to_string().contains("not a WordprocessingML main part"),
        "{err}"
    );
}

#[test]
fn apache_poi_template_reads_as_a_valid_template() {
    let read = read_docx(
        &fixture("poi-test.dotx"),
        &ReadLimits::generous(),
        &mut MemoryMedia::with_prefix("i"),
    )
    .expect("reads");
    assert_eq!(read.kind, Kind::Template);
    assert_eq!(read.document.validate(), Ok(()));
    assert!(read
        .document
        .inlines()
        .any(|i| matches!(&i.content, cdoc_model::InlineContent::Text(t) if t.text == "Test")));
    assert_eq!(read.report.macros(), []);
}

#[test]
fn apache_poi_macro_document_reads_without_its_macros() {
    let bytes = fixture("poi-simple-macro.docm");
    let read = read_docx(
        &bytes,
        &ReadLimits::generous(),
        &mut MemoryMedia::with_prefix("i"),
    )
    .expect("reads");
    assert_eq!(read.kind, Kind::Document);
    assert_eq!(read.document.validate(), Ok(()));
    let parts: Vec<&str> = read.report.macros().into_iter().map(|(p, _)| p).collect();
    assert_eq!(parts, ["word/vbaData.xml", "word/vbaProject.bin"]);
    assert!(read.report.macros().iter().all(|(_, bytes)| *bytes > 0));
    assert!(!read
        .report
        .has(|p| p.contains("vba"), |w| w == "part not read"));
    let (again, _) = write_docx(
        &read.document,
        &MemoryMedia::with_prefix("i"),
        WriteOptions::document(),
    )
    .expect("writes");
    assert!(common::part_names(&again)
        .iter()
        .all(|n| !n.contains("vba")));
}
