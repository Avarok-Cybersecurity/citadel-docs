//! The wasm-bindgen facade over `cdoc-model` and `cdoc-pm`. Thin by design: every export is
//! one line over [`facade`], which holds the logic and is tested natively. Built for
//! `wasm32-unknown-unknown` only; the dependency gate keeps C code and I/O crates out.

#![forbid(unsafe_code)]

pub mod facade;
pub mod render;

use wasm_bindgen::prelude::*;

fn js(err: &str) -> JsError {
    JsError::new(err)
}

/// Model JSON → ProseMirror JSON.
#[wasm_bindgen]
pub fn model_to_pm(model_json: &str) -> Result<String, JsError> {
    facade::model_to_pm(model_json).map_err(|e| js(&e))
}

/// ProseMirror JSON → model JSON.
#[wasm_bindgen]
pub fn pm_to_model(pm_json: &str) -> Result<String, JsError> {
    facade::pm_to_model(pm_json).map_err(|e| js(&e))
}

/// Validate a model document; returns a JSON array of error messages, empty when valid.
#[wasm_bindgen]
pub fn validate_model(model_json: &str) -> Result<String, JsError> {
    facade::validate_model(model_json).map_err(|e| js(&e))
}

/// Check a ProseMirror JSON tree against the schema; throws naming the first violation.
#[wasm_bindgen]
pub fn check_pm(pm_json: &str) -> Result<(), JsError> {
    facade::check_pm(pm_json).map_err(|e| js(&e))
}

/// The ProseMirror schema as JSON.
#[wasm_bindgen]
#[must_use]
pub fn schema_json() -> String {
    facade::schema()
}

/// A blank document as model JSON.
#[wasm_bindgen]
#[must_use]
pub fn blank_model_json() -> String {
    facade::blank_model()
}

/// A blank document as ProseMirror JSON.
#[wasm_bindgen]
#[must_use]
pub fn blank_pm_json() -> String {
    facade::blank_pm()
}

/// Words in the body of a model document.
#[wasm_bindgen]
pub fn word_count(model_json: &str) -> Result<u32, JsError> {
    facade::word_count(model_json).map_err(|e| js(&e))
}

/// The list label of every paragraph, in ProseMirror paragraph order, as a JSON array of
/// strings and nulls.
#[wasm_bindgen]
pub fn list_labels(pm_json: &str) -> Result<String, JsError> {
    facade::list_labels(pm_json).map_err(|e| js(&e))
}

/// Faces, images and the time a render uses, handed in from JavaScript.
#[wasm_bindgen]
pub struct RenderAssets(render::Assets);

#[wasm_bindgen]
impl RenderAssets {
    /// No faces or images yet. `fallback_family` serves any family not added or aliased;
    /// `now_rfc3339` (e.g. `new Date().toISOString()`) is the time the PDF records.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(fallback_family: &str, now_rfc3339: &str) -> Self {
        Self(render::Assets::new(fallback_family, now_rfc3339))
    }

    /// Adds a face (TrueType or OpenType bytes).
    #[wasm_bindgen(js_name = addFont)]
    pub fn add_font(&mut self, family: &str, bold: bool, italic: bool, bytes: &[u8]) {
        self.0.add_font(family, bold, italic, bytes);
    }

    /// Serves `target` whenever a document asks for `name` (e.g. Calibri → Carlito).
    #[wasm_bindgen(js_name = aliasFont)]
    pub fn alias_font(&mut self, name: &str, target: &str) {
        self.0.alias_font(name, target);
    }

    /// Adds an image under the content id documents refer to it by.
    #[wasm_bindgen(js_name = addImage)]
    pub fn add_image(&mut self, content_id: &str, mime: &str, bytes: &[u8]) {
        self.0.add_image(content_id, mime, bytes);
    }
}

/// ProseMirror JSON → PDF bytes.
#[wasm_bindgen]
pub fn pm_to_pdf(pm_json: &str, assets: &RenderAssets) -> Result<Vec<u8>, JsError> {
    render::pm_to_pdf(pm_json, &assets.0).map_err(|e| js(&e))
}

/// ProseMirror JSON → a JSON array of SVG page documents for the paged preview.
#[wasm_bindgen]
pub fn pm_to_svg_pages(pm_json: &str, assets: &RenderAssets) -> Result<String, JsError> {
    render::pm_to_svg_pages(pm_json, &assets.0).map_err(|e| js(&e))
}

/// The crate version, so the UI can log which build it loaded.
#[wasm_bindgen]
#[must_use]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
