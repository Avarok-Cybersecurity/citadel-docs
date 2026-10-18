//! The wasm-bindgen facade over `cdoc-model`, `cdoc-pm`, the renderers and the editor. Thin by
//! design: every export is one line over [`facade`], [`render`] or [`edit_facade`], which
//! hold the logic and are tested natively. Built for `wasm32-unknown-unknown` only; the
//! dependency gate keeps C code and I/O crates out. [`migrate`] moves a document stored as
//! ProseMirror JSON onto a replica (`pm_to_crdt_snapshot`, `docs/tiptap-schema.md`).

#![forbid(unsafe_code)]

pub mod convert;
mod edit_bindings;
pub mod edit_facade;
mod edit_io;
pub mod facade;
mod files;
pub mod migrate;
pub mod render;
mod render_bindings;

pub use edit_bindings::*;
pub use files::{docx_to_pm, odt_to_pm, pm_to_docx, pm_to_odt, FileExport, ImageStore};
pub use render_bindings::*;

use wasm_bindgen::prelude::*;

pub(crate) fn js(err: &str) -> JsError {
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

/// Markdown to JSON `{"pm", "report", "problems"}`. Options JSON:
/// `{"textWidth", "imageWidth", "imageHeight"}` in hundredths of a point.
#[wasm_bindgen]
pub fn md_to_pm(md: &str, options_json: &str) -> Result<String, JsError> {
    convert::md_to_pm(md, options_json).map_err(|e| js(&e))
}

/// ProseMirror JSON to JSON `{"text", "report"}` holding Markdown.
#[wasm_bindgen]
pub fn pm_to_md(pm_json: &str) -> Result<String, JsError> {
    convert::pm_to_md(pm_json).map_err(|e| js(&e))
}

/// Plain text to JSON `{"pm", "report", "problems"}`. Options JSON:
/// `{"singleNewline": "break" | "space"}`.
#[wasm_bindgen]
pub fn txt_to_pm(text: &str, options_json: &str) -> Result<String, JsError> {
    convert::txt_to_pm(text, options_json).map_err(|e| js(&e))
}

/// ProseMirror JSON to JSON `{"text", "report"}` holding plain text.
#[wasm_bindgen]
pub fn pm_to_txt(pm_json: &str) -> Result<String, JsError> {
    convert::pm_to_txt(pm_json).map_err(|e| js(&e))
}

/// ProseMirror JSON to a CRDT snapshot written as `client`, for moving a stored document onto
/// collaborative editing; the report JSON is `{"report", "problems"}`.
#[wasm_bindgen]
pub fn pm_to_crdt_snapshot(pm_json: &str, client: u64) -> Result<Migrated, JsError> {
    migrate::pm_to_crdt_snapshot(pm_json, client)
        .map(|(snapshot, report)| Migrated { snapshot, report })
        .map_err(|e| js(&e))
}

/// A CRDT snapshot made from a stored document, and `{"report", "problems"}` as JSON.
#[wasm_bindgen]
pub struct Migrated {
    snapshot: Vec<u8>,
    report: String,
}

#[wasm_bindgen]
impl Migrated {
    /// The snapshot, to open with `EditorHandle`.
    #[must_use]
    pub fn snapshot(&self) -> Vec<u8> {
        self.snapshot.clone()
    }

    /// `{"report", "problems"}` as JSON.
    #[must_use]
    #[wasm_bindgen(js_name = reportJson)]
    pub fn report_json(&self) -> String {
        self.report.clone()
    }
}

/// The crate version, so the UI can log which build it loaded.
#[wasm_bindgen]
#[must_use]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
