//! The foreign fixtures through the severity table: no fixture loses visible content.

mod common;

use cdoc_model::Severity;

#[test]
fn foreign_documents_lose_no_visible_content() {
    for name in [
        "_annotation.odt",
        "bullet_list.odt",
        "changeTracked.odt",
        "HeaderFooter.odt",
        "TestTextTable.odt",
    ] {
        let bytes = std::fs::read(format!("tests/fixtures/{name}")).expect("the fixture");
        let (_, report, _) = common::read_foreign(&bytes);
        let major: Vec<_> = report
            .losses
            .iter()
            .filter(|l| l.severity() == Severity::Major)
            .collect();
        assert!(major.is_empty(), "{name}: {major:?}");
    }
}
