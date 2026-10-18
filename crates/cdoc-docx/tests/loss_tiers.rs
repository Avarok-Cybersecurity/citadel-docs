//! The foreign fixtures through the severity table: parts read by nobody and refused macros
//! are Info, and no fixture loses visible content (no Major).

mod common;

use cdoc_model::{Loss, Severity};

#[test]
fn foreign_documents_lose_no_visible_content() {
    for name in [
        "pandoc.docx",
        "textutil.docx",
        "poi-simple-macro.docm",
        "poi-test.dotx",
    ] {
        let bytes = std::fs::read(format!("tests/fixtures/{name}")).expect("the fixture");
        let (_, report, _) = common::read_foreign(&bytes);
        for loss in &report.losses {
            assert_ne!(loss.severity(), Severity::Major, "{name}: {loss:?}");
            let ignored =
                matches!(loss, Loss::MacrosRemoved { .. }) || loss.what() == Some("part not read");
            if ignored {
                assert_eq!(loss.severity(), Severity::Info, "{name}: {loss:?}");
            }
        }
    }
}
