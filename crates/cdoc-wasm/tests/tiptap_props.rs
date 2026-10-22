//! Random chat documents migrate: never refused, the same bytes every time, a snapshot that
//! holds exactly what the mapper read, that reads back to the same bytes through the model,
//! and that merges with an identical migration as a no-op.

use cdoc_crdt::{from_model, to_model, ClientId, Doc, PendingLimits, StateVector};
use cdoc_tiptap::test_support::strategies::{foreign_doc, starter_kit_doc};
use cdoc_tiptap::tiptap_to_document;
use cdoc_wasm::migrate::tiptap_to_crdt_snapshot;
use proptest::prelude::*;
use serde_json::Value;

const LIMITS: PendingLimits = PendingLimits {
    max_ops: 1 << 20,
    max_bytes: 1 << 26,
};

fn check(doc: &Value, client: u64) -> Result<(), TestCaseError> {
    let json = doc.to_string();
    let (snapshot, report) = tiptap_to_crdt_snapshot(&json, client).expect("migrates");
    prop_assert_eq!(
        tiptap_to_crdt_snapshot(&json, client).expect("migrates again"),
        (snapshot.clone(), report.clone())
    );
    let report: Value = serde_json::from_str(&report).expect("report JSON");
    prop_assert_eq!(&report["problems"], &serde_json::json!([]));
    let replica = Doc::decode_snapshot(&snapshot, ClientId(client)).expect("decodes");
    let model = to_model(&replica).expect("folds");
    prop_assert_eq!(&model, &tiptap_to_document(&json).expect("reads").0);
    let json_model = serde_json::to_string(&model).expect("serialises");
    let reread: cdoc_model::Document = serde_json::from_str(&json_model).expect("parses");
    prop_assert_eq!(&reread, &model);
    let rewritten = from_model(&reread, ClientId(client)).expect("writes");
    prop_assert!(
        rewritten.encode_snapshot() == snapshot,
        "the model round trip moved bytes"
    );
    let mut merged = Doc::decode_snapshot(&snapshot, ClientId(if client == 7 { 8 } else { 7 }))
        .expect("decodes");
    merged
        .apply_update(&rewritten.encode_diff(&StateVector::empty()), LIMITS)
        .expect("applies");
    prop_assert!(
        merged.encode_snapshot() == snapshot,
        "an identical migration duplicated"
    );
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn starter_kit_documents_migrate_deterministically(
        doc in starter_kit_doc(),
        client in 1..u64::MAX,
    ) {
        check(&doc, client)?;
    }

    #[test]
    fn foreign_documents_migrate_deterministically(doc in foreign_doc(), client in 1..u64::MAX) {
        check(&doc, client)?;
    }
}
