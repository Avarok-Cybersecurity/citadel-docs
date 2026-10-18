//! A diff cut for a transport that caps message size (the kernel's update limit), including
//! the replica's whole state for an empty log.

use super::Editor;
use crate::error::EditError;
use cdoc_crdt::StateVector;

impl Editor<'_> {
    /// What a peer at state vector `sv` (binary; `None` for a peer that holds nothing, such as
    /// an empty log) lacks, as updates of at most `max_bytes` each to apply in order; see
    /// `cdoc_crdt::Doc::encode_diff_chunked`.
    pub fn encode_diff_chunked(
        &self,
        sv: Option<&[u8]>,
        max_bytes: usize,
    ) -> Result<Vec<Vec<u8>>, EditError> {
        let sv = match sv {
            Some(bytes) => StateVector::decode(bytes)?,
            None => StateVector::empty(),
        };
        Ok(self.doc.encode_diff_chunked(&sv, max_bytes)?)
    }
}
