//! The format conversions through the facade: the same strings the browser sees.

use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_model::{ContentId, Document};
use cdoc_wasm::facade;
use serde_json::Value;

const PNG: &[u8] = include_bytes!("../../cdoc-docx/tests/fixtures/pixel.png");
const DOCUMENT: &str = r#"{"kind":"document"}"#;

fn kitchen_pm() -> String {
    facade::model_to_pm(&serde_json::to_string(&kitchen_sink()).unwrap()).unwrap()
}

fn store_with_image() -> cdoc_docx::MemoryMedia {
    let mut media = facade::new_image_store();
    media.insert(ContentId::new("img-1"), "image/png", PNG.to_vec());
    media
}

fn pm_part(result: &str) -> String {
    serde_json::to_string(&serde_json::from_str::<Value>(result).unwrap()["pm"]).unwrap()
}

#[test]
fn docx_round_trips_the_kitchen_sink_through_the_facade() {
    let (bytes, report) = facade::pm_to_docx(&kitchen_pm(), &store_with_image(), DOCUMENT).unwrap();
    assert_eq!(report, r#"{"losses":[]}"#);
    let mut found = facade::new_image_store();
    let imported = facade::docx_to_pm(&bytes, &mut found).unwrap();
    let parsed: Value = serde_json::from_str(&imported).unwrap();
    assert_eq!(parsed["problems"], serde_json::json!([]));
    assert_eq!(parsed["report"], serde_json::json!({"losses": []}));
    assert_eq!(parsed["kind"], "document");
    // The reader names the image provisionally; everything else is the original document.
    assert_eq!(found.ids().len(), 1);
    assert_eq!(found.ids()[0].as_str(), "docx-image-1");
    assert_eq!(
        cdoc_model::ImagePort::image(&found, &found.ids()[0])
            .unwrap()
            .bytes,
        PNG
    );
    let back: Document =
        serde_json::from_str(&facade::pm_to_model(&pm_part(&imported)).unwrap()).unwrap();
    let mut expected = kitchen_sink();
    for inline in expected.sections[0]
        .blocks
        .iter_mut()
        .flat_map(|b| match b {
            cdoc_model::Block::Paragraph(p) => p.inlines.iter_mut().collect::<Vec<_>>(),
            cdoc_model::Block::Table(_) => Vec::new(),
        })
    {
        if let cdoc_model::InlineContent::Image(img) = &mut inline.content {
            img.content_id = ContentId::new("docx-image-1");
        }
    }
    assert_eq!(back, expected);
}

#[test]
fn markdown_and_text_flow_through_the_facade() {
    let md = facade::pm_to_md(&kitchen_pm()).unwrap();
    let parsed: Value = serde_json::from_str(&md).unwrap();
    assert!(parsed["text"]
        .as_str()
        .unwrap()
        .starts_with("# Kitchen sink\n\n# Every **kind**"));
    assert!(parsed["report"]["losses"].as_array().unwrap().len() > 5);
    let options = r#"{"textWidth":43200,"imageWidth":7200,"imageHeight":3600}"#;
    let back = facade::md_to_pm("# Title\n\nSome **bold** text.\n", options).unwrap();
    let doc: Document =
        serde_json::from_str(&facade::pm_to_model(&pm_part(&back)).unwrap()).unwrap();
    assert_eq!(doc.word_count(), 4);
    assert_eq!(
        serde_json::from_str::<Value>(&back).unwrap()["problems"],
        serde_json::json!([])
    );

    let txt = facade::pm_to_txt(&kitchen_pm()).unwrap();
    assert!(serde_json::from_str::<Value>(&txt).unwrap()["text"]
        .as_str()
        .unwrap()
        .contains("Merged header"));
    let read = facade::txt_to_pm("one\ntwo\n\nthree\n", r#"{"singleNewline":"space"}"#).unwrap();
    let doc: Document =
        serde_json::from_str(&facade::pm_to_model(&pm_part(&read)).unwrap()).unwrap();
    assert_eq!(doc.paragraphs().count(), 2);
    assert_eq!(
        doc.paragraphs().next().unwrap().inlines.len(),
        1,
        "one line: \"one two\""
    );
    let read = facade::txt_to_pm("one\ntwo", r#"{"singleNewline":"break"}"#).unwrap();
    let doc: Document =
        serde_json::from_str(&facade::pm_to_model(&pm_part(&read)).unwrap()).unwrap();
    assert_eq!(
        doc.paragraphs().next().unwrap().inlines.len(),
        3,
        "text, break, text"
    );
}

#[test]
fn options_have_no_defaults_and_unknown_fields_are_rejected() {
    for bad in [
        "{}",
        r#"{"textWidth":1,"imageWidth":1}"#,
        r#"{"textWidth":1,"imageWidth":1,"imageHeight":1,"extra":2}"#,
        "[]",
    ] {
        let err = facade::md_to_pm("x", bad).unwrap_err();
        assert!(err.starts_with("bad markdown options"), "{bad}: {err}");
    }
    for bad in [
        "{}",
        r#"{"singleNewline":"sometimes"}"#,
        r#"{"singleNewline":"break","x":1}"#,
    ] {
        let err = facade::txt_to_pm("x", bad).unwrap_err();
        assert!(err.starts_with("bad text options"), "{bad}: {err}");
    }
}

#[test]
fn bad_inputs_are_errors_not_panics() {
    let mut media = facade::new_image_store();
    assert!(facade::docx_to_pm(b"not a zip", &mut media)
        .unwrap_err()
        .starts_with("zip:"));
    assert!(facade::pm_to_docx("{", &media, DOCUMENT).is_err());
    assert!(facade::pm_to_docx(&kitchen_pm(), &media, "{}")
        .unwrap_err()
        .starts_with("bad package options"));
    assert!(facade::pm_to_md("[]").is_err());
    assert!(facade::pm_to_txt("null").is_err());
    let missing = facade::pm_to_docx(&kitchen_pm(), &media, DOCUMENT).unwrap_err();
    assert!(
        missing.contains("img-1"),
        "a missing image names its id: {missing}"
    );
}

#[test]
fn the_exported_classes_carry_images_and_files() {
    use cdoc_wasm::{docx_to_pm, pm_to_docx, ImageStore};
    let mut store = ImageStore::empty();
    assert!(store.is_empty() && store.id(0).is_none());
    store.add("img-1", "image/png", PNG);
    assert_eq!(
        (store.len(), store.id(0), store.mime(0)),
        (1, Some("img-1".to_owned()), Some("image/png".to_owned()))
    );
    assert_eq!(store.bytes(0).as_deref(), Some(PNG));
    assert!(store.bytes(1).is_none());
    let export = pm_to_docx(&kitchen_pm(), &store, DOCUMENT).unwrap_or_else(|_| panic!("exports"));
    assert!(export.bytes().starts_with(b"PK"), "a zip");
    assert_eq!(export.report_json(), r#"{"losses":[]}"#);
    let mut found = ImageStore::empty();
    let imported = docx_to_pm(&export.bytes(), &mut found).unwrap_or_else(|_| panic!("imports"));
    assert!(imported.starts_with("{\"pm\":"));
    assert_eq!(
        (found.len(), found.id(0).as_deref()),
        (1, Some("docx-image-1"))
    );
    assert_eq!(found.bytes(0).as_deref(), Some(PNG));
}
