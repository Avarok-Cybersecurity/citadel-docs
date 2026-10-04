//! The wasm-bindgen facade over `cdoc-model` and `cdoc-pm`. Thin by design: every export is
//! one line over [`facade`], which holds the logic and is tested natively. Built for
//! `wasm32-unknown-unknown` only; the dependency gate keeps C code and I/O crates out.

#![forbid(unsafe_code)]

pub mod facade;

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

/// The crate version, so the UI can log which build it loaded.
#[wasm_bindgen]
#[must_use]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
