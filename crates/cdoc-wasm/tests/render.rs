//! Rendering through the facade, as the browser calls it: ProseMirror JSON in, PDF bytes or
//! SVG pages out, with faces registered the way the UI registers them from the font manifest.

use cdoc_layout::test_support::{ALIASES, FACES, FALLBACK};
use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_wasm::render::{pm_to_pdf, pm_to_svg_pages, Assets};

const NOW: &str = "2026-10-04T12:00:00Z";

fn assets() -> Assets {
    let mut assets = Assets::new(FALLBACK, NOW);
    for (bold, italic, bytes) in FACES {
        assets.add_font(FALLBACK, bold, italic, bytes);
    }
    for (name, target) in ALIASES {
        assets.alias_font(name, target);
    }
    assets.add_image(
        "img-1",
        "image/jpeg",
        include_bytes!("../../cdoc-pdf/tests/data/photo.jpg"),
    );
    assets
}

fn kitchen_pm() -> String {
    cdoc_pm::to_pm_json(&kitchen_sink())
}

#[test]
fn a_prosemirror_document_becomes_a_pdf_lopdf_reads() {
    let pdf = pm_to_pdf(&kitchen_pm(), &assets()).expect("renders");
    let doc = lopdf::Document::load_mem(&pdf).expect("a PDF");
    assert_eq!(doc.get_pages().len(), 3);
}

#[test]
fn svg_pages_come_back_as_a_json_array_one_per_page() {
    let json = pm_to_svg_pages(&kitchen_pm(), &assets()).expect("renders");
    let pages: Vec<String> = serde_json::from_str(&json).expect("an array of strings");
    assert_eq!(pages.len(), 3);
    assert!(pages
        .iter()
        .all(|p| p.starts_with("<svg ") && p.ends_with("</svg>")));
}

#[test]
fn no_faces_invalid_documents_and_bad_json_are_errors() {
    let empty = Assets::new(FALLBACK, NOW);
    assert!(pm_to_pdf(&kitchen_pm(), &empty)
        .unwrap_err()
        .contains("no face"));
    let mut broken = kitchen_sink();
    broken.sections.clear();
    let pm = cdoc_pm::to_pm_json(&broken);
    assert!(pm_to_pdf(&pm, &assets()).is_err());
    assert!(pm_to_svg_pages("{", &assets()).is_err());
}

#[test]
fn the_font_manifest_the_ui_loads_agrees_with_the_faces_tests_use() {
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../../fonts/manifest.json")).expect("json");
    assert_eq!(manifest["fallback"], FALLBACK);
    let aliases = manifest["aliases"].as_object().expect("aliases");
    assert_eq!(aliases.len(), ALIASES.len());
    for (name, target) in ALIASES {
        assert_eq!(aliases[name], target);
    }
    let faces = manifest["faces"].as_array().expect("faces");
    assert_eq!(faces.len(), FACES.len());
    for (face, (bold, italic, _)) in faces.iter().zip(FACES) {
        assert_eq!(
            (face["bold"].as_bool(), face["italic"].as_bool()),
            (Some(bold), Some(italic))
        );
    }
}
