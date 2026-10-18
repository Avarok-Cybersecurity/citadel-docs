//! `.odt` / `.ott` through the facade: the same strings the browser sees.

mod common;

use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_model::{ContentId, Document};
use cdoc_wasm::facade;
use serde_json::Value;

const PNG: &[u8] = include_bytes!("../../cdoc-docx/tests/fixtures/pixel.png");

fn kitchen_pm() -> String {
    facade::model_to_pm(&serde_json::to_string(&kitchen_sink()).unwrap()).unwrap()
}

fn store_with_image() -> cdoc_docx::MemoryMedia {
    let mut media = facade::new_image_store();
    media.insert(ContentId::new("img-1"), "image/png", PNG.to_vec());
    media
}

fn read(bytes: &[u8]) -> (Value, Document, cdoc_docx::MemoryMedia) {
    let mut found = facade::new_image_store();
    let imported = facade::odt_to_pm(bytes, &mut found, common::BROWSER_READ).unwrap();
    let parsed: Value = serde_json::from_str(&imported).unwrap();
    let pm = serde_json::to_string(&parsed["pm"]).unwrap();
    let doc = serde_json::from_str(&facade::pm_to_model(&pm).unwrap()).unwrap();
    (parsed, doc, found)
}

#[test]
fn odt_round_trips_the_kitchen_sink_through_the_facade() {
    let (bytes, report) =
        facade::pm_to_odt(&kitchen_pm(), &store_with_image(), r#"{"kind":"document"}"#).unwrap();
    let report: Value = serde_json::from_str(&report).unwrap();
    let losses = report["losses"].as_array().unwrap();
    assert!(
        losses
            .iter()
            .all(|l| l["what"].as_str().unwrap().ends_with("kept in the sidecar")),
        "{losses:?}"
    );
    let (parsed, back, found) = read(&bytes);
    assert_eq!(parsed["problems"], serde_json::json!([]));
    assert_eq!(parsed["report"], serde_json::json!({"losses": []}));
    assert_eq!(parsed["kind"], "document");
    assert_eq!(found.ids().len(), 1);
    assert_eq!(
        cdoc_model::ImagePort::image(&found, &found.ids()[0])
            .unwrap()
            .bytes,
        PNG
    );
    let mut expected = kitchen_sink();
    expected.for_each_paragraph_mut(&mut |p| {
        for inline in &mut p.inlines {
            if let cdoc_model::InlineContent::Image(img) = &mut inline.content {
                img.content_id = found.ids()[0].clone();
            }
        }
    });
    assert_eq!(back, expected);
}

#[test]
fn a_template_is_written_as_ott_and_reported_as_a_template() {
    let (bytes, _) =
        facade::pm_to_odt(&kitchen_pm(), &store_with_image(), r#"{"kind":"template"}"#).unwrap();
    let template = b"application/vnd.oasis.opendocument.text-template";
    assert_eq!(&bytes[38..38 + template.len()], template.as_slice());
    let (parsed, _, _) = read(&bytes);
    assert_eq!(parsed["kind"], "template");
}

#[test]
fn odt_options_have_no_defaults_and_bad_bytes_are_errors() {
    let media = store_with_image();
    assert!(facade::pm_to_odt(&kitchen_pm(), &media, "{}")
        .unwrap_err()
        .contains("bad package options"));
    assert!(facade::pm_to_odt(&kitchen_pm(), &media, r#"{"kind":"document","x":1}"#).is_err());
    let mut found = facade::new_image_store();
    assert!(facade::odt_to_pm(b"not a zip", &mut found, common::BROWSER_READ).is_err());
}
