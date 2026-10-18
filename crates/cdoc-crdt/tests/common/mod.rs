//! What the generated-document properties share.

use std::collections::BTreeSet;

/// Whether every comment, style, numbering definition and numbering instance id is distinct.
/// The model's strategies can, rarely, name one id twice, which the model's own validation
/// rejects (`ValidationError::DuplicateComment` and the like) and the CRDT's maps by id refuse
/// (`EditError::DuplicateId`): such a document is not one to round trip.
pub fn distinct_ids(d: &cdoc_model::Document) -> bool {
    fn distinct<K: Ord>(ids: impl Iterator<Item = K>) -> bool {
        let mut seen = BTreeSet::new();
        ids.into_iter().all(|k| seen.insert(k))
    }
    distinct(d.comments.iter().map(|c| c.id.as_str()))
        && distinct(d.styles.styles.iter().map(|s| s.id.as_str()))
        && distinct(d.numbering.definitions.iter().map(|n| n.id))
        && distinct(d.numbering.instances.iter().map(|n| n.id))
}
