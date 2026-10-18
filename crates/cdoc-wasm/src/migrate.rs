//! Moving a document stored as ProseMirror JSON onto a CRDT replica: the one-time migration of
//! a live document to collaborative editing.
//!
//! The ProseMirror mapping and the model bridge are both lossless, so the report is empty
//! whenever a snapshot comes back; the snapshot is read back into a fresh replica and compared
//! with the document before it is returned, so a bridge defect fails the migration by name
//! instead of losing content silently.

use cdoc_crdt::{from_model, to_model, ClientId, Doc};
use cdoc_model::Report;
use cdoc_pm::from_pm_json;

/// ProseMirror JSON to a CRDT snapshot (format v2) written as `client`, and
/// `{"report", "problems"}` as JSON: the import shapes without the `pm` the snapshot replaces.
pub fn pm_to_crdt_snapshot(pm_json: &str, client: u64) -> Result<(Vec<u8>, String), String> {
    let mut document = from_pm_json(pm_json).map_err(|e| e.to_string())?;
    let client = ClientId(client);
    let snapshot = from_model(&document, client)
        .map_err(|e| format!("the document does not fit a replica: {e}"))?
        .encode_snapshot();
    let reread = Doc::decode_snapshot(&snapshot, client)
        .map_err(|e| format!("the snapshot does not read back: {e}"))
        .and_then(|doc| {
            to_model(&doc).map_err(|e| format!("the snapshot does not read back: {e}"))
        })?;
    document.normalize();
    if reread != document {
        return Err("the snapshot does not hold the document".to_owned());
    }
    let report = format!(
        "{{\"report\":{},\"problems\":{}}}",
        crate::convert::json(&Report::empty()),
        crate::convert::json(&crate::convert::problems(&document))
    );
    Ok((snapshot, report))
}
