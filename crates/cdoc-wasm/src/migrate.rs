//! Moving a stored document onto a CRDT replica: the one-time migration of a live document to
//! collaborative editing. Two sources: ProseMirror JSON in the model's own schema
//! ([`pm_to_crdt_snapshot`], lossless) and the old chat editor's TipTap `StarterKit` JSON
//! ([`tiptap_to_crdt_snapshot`], through `cdoc-tiptap`, with its loss report).
//!
//! The snapshot is read back into a fresh replica and compared with the document before it is
//! returned, so a bridge defect fails the migration by name instead of losing content
//! silently. Writing is deterministic: the same input and client always give the same bytes,
//! so two devices migrating one document under one client id write snapshots that merge as a
//! no-op. Client 0 is refused (`cdoc_crdt::ClientId::BLANK`).

use cdoc_crdt::{from_model, to_model, ClientId, Doc};
use cdoc_model::{Document, Report};
use cdoc_pm::from_pm_json;

/// ProseMirror JSON to a CRDT snapshot (format v2) written as `client`, and
/// `{"report", "problems"}` as JSON: the import shapes without the `pm` the snapshot replaces.
pub fn pm_to_crdt_snapshot(pm_json: &str, client: u64) -> Result<(Vec<u8>, String), String> {
    let document = from_pm_json(pm_json).map_err(|e| e.to_string())?;
    migrate(document, &Report::empty(), client)
}

/// The chat editor's TipTap `StarterKit` JSON to a CRDT snapshot written as `client`, and
/// `{"report", "problems"}` as JSON, the report naming what the model could not hold.
pub fn tiptap_to_crdt_snapshot(json: &str, client: u64) -> Result<(Vec<u8>, String), String> {
    let (document, report) = cdoc_tiptap::tiptap_to_document(json).map_err(|e| e.to_string())?;
    migrate(document, &report, client)
}

fn migrate(
    mut document: Document,
    report: &Report,
    client: u64,
) -> Result<(Vec<u8>, String), String> {
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
        crate::convert::json(report),
        crate::convert::json(&crate::convert::problems(&document))
    );
    Ok((snapshot, report))
}
