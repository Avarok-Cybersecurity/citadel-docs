//! Word's list styles: a multilevel list style's levels live in the `w:abstractNum` that
//! defines the style (`w:styleLink`); the one Word points its lists at only names the style
//! (`w:numStyleLink`) and has no levels. Its items keep their numbering with the style's
//! levels, and a link to a style nothing defines is reported, not guessed.

mod common;

use cdoc_model::{AbstractNumId, NumId, NumberFormat};

const W: &str = "xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"";
const NUMBERING_REL: &str = "<Relationship Id=\"rId9\" \
    Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering\" \
    Target=\"numbering.xml\"/>";

fn level(ilvl: u8, format: &str, text: &str) -> String {
    format!(
        "<w:lvl w:ilvl=\"{ilvl}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"{format}\"/>\
         <w:lvlText w:val=\"{text}\"/></w:lvl>"
    )
}

fn item(text: &str, ilvl: u8) -> String {
    format!(
        "<w:p><w:pPr><w:numPr><w:ilvl w:val=\"{ilvl}\"/><w:numId w:val=\"2\"/></w:numPr></w:pPr>\
         <w:r><w:t>{text}</w:t></w:r></w:p>"
    )
}

/// Definition 10 defines the list style `OutlineList`; definition 11 uses it; list 2 is 11.
fn package(uses: &str) -> Vec<u8> {
    let numbering = format!(
        "<w:numbering {W}>\
         <w:abstractNum w:abstractNumId=\"10\"><w:styleLink w:val=\"OutlineList\"/>{}{}</w:abstractNum>\
         <w:abstractNum w:abstractNumId=\"11\"><w:numStyleLink w:val=\"{uses}\"/></w:abstractNum>\
         <w:num w:numId=\"1\"><w:abstractNumId w:val=\"10\"/></w:num>\
         <w:num w:numId=\"2\"><w:abstractNumId w:val=\"11\"/></w:num></w:numbering>",
        level(0, "decimal", "%1."),
        level(1, "bullet", "\u{25E6}"),
    );
    let body = [item("first", 0), item("nested", 1), item("second", 0)].concat();
    common::word_package(&body, NUMBERING_REL, &[("word/numbering.xml", &numbering)])
}

#[test]
fn a_list_using_a_list_style_takes_the_style_s_levels() {
    let (doc, report, _) = common::read_foreign(&package("OutlineList"));
    let defining = doc.numbering.definition_for(NumId(1)).expect("list 1");
    let using = doc.numbering.definition_for(NumId(2)).expect("list 2");
    assert_eq!(using.id, AbstractNumId(11));
    assert_eq!(using.levels, defining.levels);
    assert_eq!(
        using.levels.iter().map(|l| l.format).collect::<Vec<_>>(),
        [NumberFormat::Decimal, NumberFormat::Bullet]
    );
    assert!(
        !report.has(|_| true, |w| w.contains("numStyleLink")),
        "{report:?}"
    );
    let (md, _) = cdoc_md::document_to_md(&doc);
    assert_eq!(md, "1. first\n    - nested\n2. second\n");
}

#[test]
fn a_link_to_a_style_nothing_defines_is_reported() {
    let (doc, report, _) = common::read_foreign(&package("Missing"));
    let levels = &doc
        .numbering
        .definition_for(NumId(2))
        .expect("list 2")
        .levels;
    assert_eq!(levels, &Vec::new(), "nothing is guessed");
    assert_eq!(
        report.count_of("w:numStyleLink to a list style no w:abstractNum defines"),
        1,
        "{report:?}"
    );
}

/// pandoc's document (bullets, a nested bullet, an ordered list) as Markdown, against the
/// committed expectation: the list syntax a `.docx` → `.md` export must produce.
#[test]
fn a_docx_with_lists_exports_the_expected_markdown() {
    let bytes = std::fs::read("tests/fixtures/pandoc.docx").expect("the fixture");
    let (doc, _, _) = common::read_foreign(&bytes);
    let (md, _) = cdoc_md::document_to_md(&doc);
    let expected = std::fs::read_to_string("tests/fixtures/pandoc.expected.md").expect("expected");
    assert_eq!(md, expected);
    assert!(
        md.contains("- first\n- second\n    - nested\n1. one\n2. two\n"),
        "{md}"
    );
}
