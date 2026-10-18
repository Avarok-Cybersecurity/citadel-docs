//! The editor exports: faces sealed for editing and the editor handle. Each method is one line
//! over [`crate::edit_facade`], which is tested natively. Client ids are `u64`, a `BigInt` in
//! JavaScript (`EditorHandle.blank(1n, fonts)`); coordinates are integers in hundredths of a
//! point; records are `Uint8Array`s in the formats of `docs/edit-commands.md`.

use crate::edit_facade::{EditFonts, EditSession, SealedFonts};
use crate::js;
use wasm_bindgen::prelude::*;

/// Faces being gathered for editing.
#[wasm_bindgen]
pub struct EditorFonts(EditFonts);

/// Faces sealed for the page's lifetime; open every editor on one of these.
#[wasm_bindgen]
pub struct EditorSealedFonts(SealedFonts);

#[wasm_bindgen]
impl EditorFonts {
    /// No faces yet; `fallback_family` serves any family not added or aliased.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(fallback_family: &str) -> Self {
        Self(EditFonts::new(fallback_family))
    }

    /// Adds a face (TrueType or OpenType bytes).
    #[wasm_bindgen(js_name = addFont)]
    pub fn add_font(&mut self, family: &str, bold: bool, italic: bool, bytes: &[u8]) {
        self.0.add_font(family, bold, italic, bytes);
    }

    /// Serves `target` whenever a document asks for `name`.
    #[wasm_bindgen(js_name = aliasFont)]
    pub fn alias_font(&mut self, name: &str, target: &str) {
        self.0.alias_font(name, target);
    }

    /// Seals the faces; they are kept for the page's life (seal once, reuse).
    #[must_use]
    pub fn seal(self) -> EditorSealedFonts {
        EditorSealedFonts(self.0.seal())
    }
}

/// An editing session over one document.
#[wasm_bindgen]
pub struct EditorHandle(EditSession);

#[wasm_bindgen]
impl EditorHandle {
    /// Opens a replica from a CRDT snapshot as client `client` (a `BigInt`).
    pub fn open(
        snapshot: &[u8],
        client: u64,
        fonts: &EditorSealedFonts,
    ) -> Result<EditorHandle, JsError> {
        EditSession::open(snapshot, client, fonts.0)
            .map(Self)
            .map_err(|e| js(&e))
    }

    /// A new blank document as client `client` (a `BigInt`).
    pub fn blank(client: u64, fonts: &EditorSealedFonts) -> Result<EditorHandle, JsError> {
        EditSession::blank(client, fonts.0)
            .map(Self)
            .map_err(|e| js(&e))
    }

    /// Runs a request record; returns the result record.
    pub fn request(&mut self, request: &[u8]) -> Result<Vec<u8>, JsError> {
        self.0.request(request).map_err(|e| js(&e))
    }

    /// Applies a peer's update; returns the result record.
    #[wasm_bindgen(js_name = applyRemote)]
    pub fn apply_remote(&mut self, update: &[u8]) -> Result<Vec<u8>, JsError> {
        self.0.apply_remote(update).map_err(|e| js(&e))
    }

    /// The display list of `pages` (zero-based).
    pub fn paint(&self, pages: &[u32]) -> Result<Vec<u8>, JsError> {
        self.0.paint(pages).map_err(|e| js(&e))
    }

    /// The position record nearest `(x, y)` on `page`.
    #[wasm_bindgen(js_name = hitTest)]
    pub fn hit_test(&self, page: u32, x: i32, y: i32) -> Result<Vec<u8>, JsError> {
        self.0.hit_test(page, x, y).map_err(|e| js(&e))
    }

    /// Pages in the document.
    #[wasm_bindgen(js_name = pageCount)]
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.0.page_count()
    }

    /// Page `page`'s `[width, height]`, or nothing past the last page.
    #[wasm_bindgen(js_name = pageSize)]
    #[must_use]
    pub fn page_size(&self, page: u32) -> Option<Vec<i32>> {
        self.0.page_size(page).map(Vec::from)
    }

    /// The full CRDT snapshot.
    #[must_use]
    pub fn snapshot(&self) -> Vec<u8> {
        self.0.snapshot()
    }

    /// The replica's state vector.
    #[wasm_bindgen(js_name = stateVector)]
    #[must_use]
    pub fn state_vector(&self) -> Vec<u8> {
        self.0.state_vector()
    }

    /// What a peer at state vector `sv` lacks.
    #[wasm_bindgen(js_name = encodeDiff)]
    pub fn encode_diff(&self, sv: &[u8]) -> Result<Vec<u8>, JsError> {
        self.0.encode_diff(sv).map_err(|e| js(&e))
    }

    /// Collects the tombstones every peer in `known` (the known-peers record) has seen;
    /// returns the collected record. Call it only when every listed peer is caught up.
    pub fn gc(&mut self, known: &[u8]) -> Result<Vec<u8>, JsError> {
        self.0.gc(known).map_err(|e| js(&e))
    }

    /// Holds an image file of at most `max_bytes` bytes; returns the added-image record.
    #[wasm_bindgen(js_name = addImage)]
    pub fn add_image(&mut self, bytes: &[u8], max_bytes: u32) -> Result<Vec<u8>, JsError> {
        let max = usize::try_from(max_bytes).map_err(|e| js(&e.to_string()))?;
        self.0.add_image(bytes, max).map_err(|e| js(&e))
    }

    /// This replica's awareness record.
    #[must_use]
    pub fn awareness(&self) -> Vec<u8> {
        self.0.awareness()
    }

    /// A peer's awareness record as a peer-view record.
    #[wasm_bindgen(js_name = peerView)]
    pub fn peer_view(&self, record: &[u8]) -> Result<Vec<u8>, JsError> {
        self.0.peer_view(record).map_err(|e| js(&e))
    }
}
