//! The editor-to-PDF hand-off: the PDF the PDF artefact writes from the editor's model JSON and
//! drawn images (`model_to_pm`, `pm_to_pdf`) is byte for byte the PDF the full binary writes
//! from the live replica (`EditorHandle.export("pdf")`), with the same faces and time.

use cdoc_layout::test_support::{ALIASES, FACES, FALLBACK};
use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_model::{ContentId, ImagePort};
use cdoc_wasm::edit_facade::{EditFonts, EditSession};
use cdoc_wasm::facade;
use cdoc_wasm::render::{pm_to_pdf, Assets};

const PNG: &[u8] = include_bytes!("../../cdoc-docx/tests/fixtures/pixel.png");
const NOW: &str = "2026-10-18T00:00:00Z";

fn session() -> EditSession {
    let mut fonts = EditFonts::new(FALLBACK);
    for (bold, italic, bytes) in FACES {
        fonts.add_font(FALLBACK, bold, italic, bytes);
    }
    for (name, target) in ALIASES {
        fonts.alias_font(name, target);
    }
    let pm = cdoc_pm::to_pm_json(&kitchen_sink());
    let mut media = facade::new_image_store();
    media.insert(ContentId::new("img-1"), "image/png", PNG.to_vec());
    let docx = facade::pm_to_docx(&pm, &media, r#"{"kind":"document"}"#)
        .unwrap()
        .0;
    EditSession::from_document(&docx, "docx", "{}", 1, fonts.seal(), 1 << 20)
        .unwrap()
        .0
}

fn pdf_artefact(model_json: &str, images: &cdoc_docx::MemoryMedia) -> Result<Vec<u8>, String> {
    let mut assets = Assets::new(FALLBACK, NOW);
    for (bold, italic, bytes) in FACES {
        assets.add_font(FALLBACK, bold, italic, bytes);
    }
    for (name, target) in ALIASES {
        assets.alias_font(name, target);
    }
    for id in images.ids() {
        let image = images.image(id).unwrap();
        assets.add_image(id.as_str(), image.mime, image.bytes);
    }
    pm_to_pdf(&facade::model_to_pm(model_json)?, &assets)
}

#[test]
fn the_pdf_artefact_writes_the_full_binarys_pdf_from_the_handed_over_bytes() {
    let session = session();
    let none = facade::new_image_store();
    let options = format!(r#"{{"now":"{NOW}"}}"#);
    let (full, _) = session.export("pdf", &options, &none).unwrap();
    let mut drawn = facade::new_image_store();
    session.drawn_images(&mut drawn);
    assert_eq!(drawn.ids().len(), 1, "the kitchen sink draws its one image");
    assert_eq!(pdf_artefact(&session.model_json(), &drawn), Ok(full));
}

#[test]
fn without_the_drawn_images_the_hand_off_is_not_the_same_pdf() {
    let session = session();
    let (full, _) = session
        .export(
            "pdf",
            &format!(r#"{{"now":"{NOW}"}}"#),
            &facade::new_image_store(),
        )
        .unwrap();
    let missing = pdf_artefact(&session.model_json(), &facade::new_image_store());
    assert_ne!(missing, Ok(full));
}
