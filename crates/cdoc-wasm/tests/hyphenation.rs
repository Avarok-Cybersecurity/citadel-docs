//! Hyphenation patterns as the UI hands them in: by the manifest's tag, refused by name when the
//! tag or the file is wrong; `patterns_needed` names the files a document needs; the manifest
//! lists exactly the languages `cdoc-para` knows, under the same tags and file names.

use cdoc_layout::FontLibrary;
use cdoc_para::Language;
use cdoc_wasm::hyphenation::{patterns_needed, register};
use serde_json::{json, Value};

#[test]
fn the_manifest_lists_every_language_under_its_tag_and_file() {
    let manifest: Value =
        serde_json::from_str(include_str!("../../cdoc-para/patterns/manifest.json")).unwrap();
    let listed: Vec<(String, String)> = manifest["patterns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["tag"].as_str().unwrap().into(),
                p["file"].as_str().unwrap().into(),
            )
        })
        .collect();
    let known: Vec<(String, String)> = Language::ALL
        .iter()
        .map(|l| (l.tag().to_owned(), l.file().to_owned()))
        .collect();
    assert_eq!(listed, known);
}

#[test]
fn every_shipped_file_registers_under_its_tag() {
    let mut fonts = FontLibrary::new("Carlito");
    for language in Language::ALL {
        register(&mut fonts, language.tag(), language.patterns()).unwrap();
    }
}

#[test]
fn an_unknown_tag_or_a_broken_file_is_refused_by_name() {
    let mut fonts = FontLibrary::new("Carlito");
    let err = register(&mut fonts, "en-US", Language::EnUs.patterns()).unwrap_err();
    assert!(err.contains("\"en-US\"") && err.contains("en-us"), "{err}");
    let err = register(&mut fonts, "fr", r"\patterns{ 1a1 }}").unwrap_err();
    assert!(err.starts_with("the fr patterns do not compile"), "{err}");
}

fn needed(model: &Value) -> Value {
    serde_json::from_str(&patterns_needed(&model.to_string()).unwrap()).unwrap()
}

#[test]
fn a_document_needs_the_files_of_its_hyphenated_languages() {
    let mut model: Value = serde_json::from_str(&cdoc_wasm::facade::blank_model()).unwrap();
    model["meta"]["language"] = json!("en-GB");
    model["styles"]["extra"] = json!({ "run": { "lang": "de-CH" } });
    model["sections"][0]["x"] = json!([{ "lang": "nl" }, { "lang": "en-AU" }, { "lang": "fr" }]);
    assert_eq!(needed(&model), json!([]), "automatic hyphenation is off");
    model["settings"]["autoHyphenation"] = json!(true);
    let tags = needed(&model);
    assert_eq!(tags[0], json!("en-gb"), "the document's language first");
    let mut sorted: Vec<&str> = tags
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap())
        .collect();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        ["de-1996", "en-gb", "fr"],
        "nl has no file; en-AU is en-gb"
    );
}

#[test]
fn malformed_json_is_refused() {
    assert!(patterns_needed("{").is_err());
}
