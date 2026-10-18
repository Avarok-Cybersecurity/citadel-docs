//! What `EditorHandle.export("pdf")` does in each artefact: the editor artefact has no PDF
//! writer and says where the PDF comes from; with `pdf` built in it writes the file.

use cdoc_layout::test_support::{FACES, FALLBACK};
use cdoc_wasm::edit_facade::{EditFonts, EditSession};
use cdoc_wasm::facade;

fn pdf_export() -> Result<Vec<u8>, String> {
    let mut fonts = EditFonts::new(FALLBACK);
    for (bold, italic, bytes) in FACES {
        fonts.add_font(FALLBACK, bold, italic, bytes);
    }
    let session = EditSession::blank(1, fonts.seal())?;
    let none = facade::new_image_store();
    session
        .export("pdf", r#"{"now":"2026-10-18T00:00:00Z"}"#, &none)
        .map(|(bytes, _)| bytes)
}

#[cfg(not(feature = "pdf"))]
#[test]
fn the_editor_artefact_refuses_pdf_naming_the_pdf_artefact() {
    let err = pdf_export().expect_err("no PDF writer here");
    assert!(err.contains("pdf artefact"), "{err}");
}

#[cfg(feature = "pdf")]
#[test]
fn a_build_with_the_pdf_writer_exports_pdf() {
    assert!(pdf_export().expect("writes").starts_with(b"%PDF-"));
}
