//! `EditorHandle.encodeDiffChunked`, one line over [`crate::edit_facade`].

use super::EditorHandle;
use crate::js;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl EditorHandle {
    /// What a peer at `sv` (a `stateVector()`, or `null` for a peer holding nothing, such as
    /// an empty log) lacks, as updates of at most `max_bytes` each: the chunks record.
    #[wasm_bindgen(js_name = encodeDiffChunked)]
    // An optional byte array crosses the boundary owned; wasm-bindgen has no borrowed form.
    #[allow(clippy::needless_pass_by_value)]
    pub fn encode_diff_chunked(
        &self,
        sv: Option<Vec<u8>>,
        max_bytes: u32,
    ) -> Result<Vec<u8>, JsError> {
        let max = usize::try_from(max_bytes).map_err(|e| js(&e.to_string()))?;
        self.0
            .encode_diff_chunked(sv.as_deref(), max)
            .map_err(|e| js(&e))
    }
}
