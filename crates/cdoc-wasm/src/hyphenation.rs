//! Hyphenation patterns handed in from JavaScript (ARCHITECTURE.md, "WASM artefacts"). The
//! pattern files are assets, not code: `crates/cdoc-para/patterns/manifest.json` names and
//! sha256-pins each, the UI fetches the ones a document needs and registers them with
//! `EditorFonts.addPatterns` or `RenderAssets.addPatterns` by the manifest's tag. A language
//! with no file registered is not hyphenated (the layout reports it), whichever artefact runs.

use cdoc_layout::FontLibrary;
use cdoc_para::Language;
use serde_json::Value;

/// Compiles `text`, the pattern file the manifest tags `tag`, into `fonts`.
pub fn register(fonts: &mut FontLibrary, tag: &str, text: &str) -> Result<(), String> {
    let language = Language::from_tag(tag).ok_or_else(|| {
        let tags: Vec<&str> = Language::ALL.iter().map(|l| l.tag()).collect();
        format!("no pattern file is tagged {tag:?}: {}", tags.join(", "))
    })?;
    fonts
        .add_patterns(language, text)
        .map_err(|e| format!("the {tag} patterns do not compile: {e}"))
}

/// The manifest tags of the pattern files a model document's hyphenated text needs, as a JSON
/// array: the document's language first, then every other `lang` in its styles and runs, each
/// once. Empty when automatic hyphenation is off; languages with no shipped file are left out.
pub fn patterns_needed(model_json: &str) -> Result<String, String> {
    let model: Value = serde_json::from_str(model_json).map_err(|e| e.to_string())?;
    let mut tags: Vec<&str> = Vec::new();
    if model["settings"]["autoHyphenation"] == Value::Bool(true) {
        let mut found = Vec::new();
        if let Some(language) = model["meta"]["language"].as_str() {
            found.push(language);
        }
        langs(&model, &mut found);
        for language in found.into_iter().filter_map(Language::from_bcp47) {
            if !tags.contains(&language.tag()) {
                tags.push(language.tag());
            }
        }
    }
    serde_json::to_string(&tags).map_err(|e| e.to_string())
}

fn langs<'a>(value: &'a Value, out: &mut Vec<&'a str>) {
    match value {
        Value::Object(map) => {
            for (key, v) in map {
                match (key.as_str(), v.as_str()) {
                    ("lang", Some(tag)) => out.push(tag),
                    _ => langs(v, out),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|v| langs(v, out)),
        _ => {}
    }
}
