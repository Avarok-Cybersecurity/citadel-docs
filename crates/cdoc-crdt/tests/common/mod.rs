//! What the generated-document properties share.

use std::collections::BTreeSet;

/// Whether every comment id is distinct. The model's strategies can, rarely, name one id twice,
/// which the model's own validation rejects (`ValidationError::DuplicateComment`) and the CRDT's
/// comment map refuses (`EditError::DuplicateComment`): such a document is not one to round
/// trip.
pub fn distinct_comment_ids(d: &cdoc_model::Document) -> bool {
    let mut seen = BTreeSet::new();
    d.comments.iter().all(|c| seen.insert(c.id.as_str()))
}
