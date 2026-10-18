//! The diff cut under a byte limit, for a log that refuses large updates (and for publishing
//! an imported document into an empty one).

use super::{text, EditSession};
use cdoc_edit::wire::encode_chunks;

impl EditSession {
    /// What a peer at state vector `sv` (`None`: a peer holding nothing, such as an empty log)
    /// lacks, as updates of at most `max_bytes` each to apply in order: the chunks record of
    /// `docs/edit-commands.md`.
    pub fn encode_diff_chunked(
        &self,
        sv: Option<&[u8]>,
        max_bytes: usize,
    ) -> Result<Vec<u8>, String> {
        let chunks = self
            .0
            .encode_diff_chunked(sv, max_bytes)
            .map_err(|e| text(&e))?;
        Ok(encode_chunks(&chunks))
    }
}
