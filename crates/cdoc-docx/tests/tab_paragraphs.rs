//! A document whose lists are typed, not numbered: Apple's `TextEdit` writes each item as a
//! paragraph `<w:tab/>•<w:tab/>text` with a hanging indent and no `word/numbering.xml`. The
//! reader keeps them as the paragraphs they are (no list is guessed); the Markdown export
//! writes no `&#9;` entity: the leading tab, which would open an indented code block, becomes
//! one space and is reported, and the tab after the bullet stays a literal tab.

mod common;

#[test]
fn typed_bullets_export_without_tab_entities_and_report_the_leading_tab() {
    let bytes = std::fs::read("tests/fixtures/textutil.docx").expect("the fixture");
    let (doc, _, _) = common::read_foreign(&bytes);
    assert!(doc.paragraphs().all(|p| p.properties.numbering.is_none()));
    let (md, report) = cdoc_md::document_to_md(&doc);
    assert!(!md.contains("&#9;"), "{md}");
    for item in [" •\tapple\n", " •\tpear\n", " 1\tone\n", " 2\ttwo\n"] {
        assert!(md.contains(&format!("\n{item}")), "{item:?} in {md}");
    }
    assert_eq!(
        report.count_of("leading tab dropped: Markdown has no leading tab"),
        4,
        "{report:?}"
    );
}
