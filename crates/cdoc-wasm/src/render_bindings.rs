//! The rendering exports: faces and images handed in from JavaScript, and the PDF, SVG
//! preview, display list, glyph outlines and face metrics drawn with them. Each is one line
//! over [`crate::render`], which is tested natively.

use crate::{js, render};
use wasm_bindgen::prelude::*;

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

/// ProseMirror JSON → the display list of every page: the binary format of
/// `docs/display-list.md`, returned as a `Uint8Array` the caller can transfer to the painter.
#[wasm_bindgen]
pub fn pm_to_display_list(pm_json: &str, assets: &RenderAssets) -> Result<Vec<u8>, JsError> {
    render::pm_to_display_list(pm_json, &assets.0).map_err(|e| js(&e))
}

/// ProseMirror JSON → a display list of only `pages` (zero-based), for repainting the pages
/// an edit changed.
#[wasm_bindgen]
pub fn pm_to_display_list_pages(
    pm_json: &str,
    assets: &RenderAssets,
    pages: &[u32],
) -> Result<Vec<u8>, JsError> {
    render::pm_to_display_list_pages(pm_json, &assets.0, pages).map_err(|e| js(&e))
}

/// The outline of glyph `glyph_id` in the face a display list's face table names, as a
/// glyph-path record (font units, y up).
#[wasm_bindgen]
pub fn glyph_path(
    assets: &RenderAssets,
    family: &str,
    bold: bool,
    italic: bool,
    glyph_id: u16,
) -> Result<Vec<u8>, JsError> {
    render::glyph_path(&assets.0, family, bold, italic, glyph_id).map_err(|e| js(&e))
}

/// That face's metrics as the 8-byte record (`unitsPerEm`, ascender, descender, line gap).
#[wasm_bindgen]
pub fn face_metrics(
    assets: &RenderAssets,
    family: &str,
    bold: bool,
    italic: bool,
) -> Result<Vec<u8>, JsError> {
    render::face_metrics(&assets.0, family, bold, italic).map_err(|e| js(&e))
}
