//! The wasm-bindgen facade over `cdoc-model`, `cdoc-pm` and the renderers. Thin by design:
//! every export is one line over [`facade`] or [`render`], which hold the logic and are tested
//! natively. Built for
//! `wasm32-unknown-unknown` only; the dependency gate keeps C code and I/O crates out.

#![forbid(unsafe_code)]

pub mod convert;
pub mod facade;
pub mod render;
mod render_bindings;

pub use render_bindings::*;

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

/// Images moving across the boundary: embedded images found by `docx_to_pm` (under
/// provisional ids), and the bytes `pm_to_docx` needs for the ids a document names.
#[wasm_bindgen]
pub struct ImageStore(cdoc_docx::MemoryMedia);

#[wasm_bindgen]
impl ImageStore {
    /// An empty store.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn empty() -> ImageStore {
        ImageStore(convert::new_image_store())
    }

    /// Add an image under a content id.
    pub fn add(&mut self, id: &str, mime: &str, bytes: &[u8]) {
        self.0
            .insert(cdoc_model::ContentId::new(id), mime, bytes.to_vec());
    }

    /// How many images the store holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.ids().len()
    }

    /// Whether the store holds no image.
    #[must_use]
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.0.ids().is_empty()
    }

    /// The content id of image `index`.
    #[must_use]
    pub fn id(&self, index: usize) -> Option<String> {
        self.0.ids().get(index).map(|id| id.as_str().to_owned())
    }

    /// The MIME type of image `index`.
    #[must_use]
    pub fn mime(&self, index: usize) -> Option<String> {
        let image = cdoc_model::ImagePort::image(&self.0, self.0.ids().get(index)?).ok()?;
        Some(image.mime.to_owned())
    }

    /// The bytes of image `index`.
    #[must_use]
    pub fn bytes(&self, index: usize) -> Option<Vec<u8>> {
        let image = cdoc_model::ImagePort::image(&self.0, self.0.ids().get(index)?).ok()?;
        Some(image.bytes.to_vec())
    }
}

/// A written file (`.docx`, `.dotx`, `.odt`, `.ott`) and the report of what it could not carry.
#[wasm_bindgen]
pub struct FileExport {
    bytes: Vec<u8>,
    report: String,
}

#[wasm_bindgen]
impl FileExport {
    /// The file.
    #[must_use]
    pub fn bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    /// The loss report as JSON.
    #[must_use]
    #[wasm_bindgen(js_name = reportJson)]
    pub fn report_json(&self) -> String {
        self.report.clone()
    }
}

/// A `.docx`, `.dotx`, `.docm` or `.dotm` to JSON `{"pm", "report", "problems", "kind"}`;
/// images go into `images`, macros are refused and reported.
#[wasm_bindgen]
pub fn docx_to_pm(bytes: &[u8], images: &mut ImageStore) -> Result<String, JsError> {
    convert::docx_to_pm(bytes, &mut images.0).map_err(|e| js(&e))
}

/// ProseMirror JSON to a `.docx` or `.dotx`; image bytes come from `images`. Options JSON:
/// `{"kind": "document" | "template"}`.
#[wasm_bindgen]
pub fn pm_to_docx(
    pm_json: &str,
    images: &ImageStore,
    options_json: &str,
) -> Result<FileExport, JsError> {
    convert::pm_to_docx(pm_json, &images.0, options_json)
        .map(|(bytes, report)| FileExport { bytes, report })
        .map_err(|e| js(&e))
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

/// The crate version, so the UI can log which build it loaded.
#[wasm_bindgen]
#[must_use]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
