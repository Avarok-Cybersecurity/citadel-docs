//! The collaboration methods of `EditorHandle`: peers' updates and awareness, each one line
//! over [`crate::edit_facade`].

use super::EditorHandle;
use crate::js;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl EditorHandle {
    /// Applies a peer's update; returns the result record. Buffers at most `max_pending_ops`
    /// waiting operations and `max_pending_bytes` bytes (no default; docs/edit-commands.md).
    #[wasm_bindgen(js_name = applyRemote)]
    pub fn apply_remote(
        &mut self,
        update: &[u8],
        max_pending_ops: usize,
        max_pending_bytes: usize,
    ) -> Result<Vec<u8>, JsError> {
        self.0
            .apply_remote(update, max_pending_ops, max_pending_bytes)
            .map_err(|e| js(&e))
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
