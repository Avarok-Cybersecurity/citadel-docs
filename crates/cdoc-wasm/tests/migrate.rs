//! Moving a stored ProseMirror document onto a replica: the snapshot opens in a fresh replica
//! and an editor holding the same document, and bad input is refused by name.

use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_wasm::migrate::pm_to_crdt_snapshot;
use serde_json::{json, Value};

fn kitchen_pm() -> String {
    cdoc_pm::to_pm_json(&kitchen_sink())
}

#[test]
fn the_kitchen_sink_becomes_a_snapshot_that_holds_it() {
    let (snapshot, report) = pm_to_crdt_snapshot(&kitchen_pm(), 7).unwrap();
    let report: Value = serde_json::from_str(&report).unwrap();
    assert_eq!(report, json!({"report": {"losses": []}, "problems": []}));
    let doc = cdoc_crdt::Doc::decode_snapshot(&snapshot, cdoc_crdt::ClientId(8)).unwrap();
    let mut want = cdoc_pm::from_pm_json(&kitchen_pm()).unwrap();
    want.normalize();
    assert_eq!(cdoc_crdt::to_model(&doc).unwrap(), want);
    let mut sink = kitchen_sink();
    sink.normalize();
    assert_eq!(want, sink, "cdoc-pm reads the kitchen sink back whole");
    let sv = cdoc_crdt::snapshot_version(&snapshot).unwrap();
    assert_eq!(
        sv.iter().map(|(c, _)| c).collect::<Vec<_>>(),
        [cdoc_crdt::ClientId(7)]
    );
}

#[test]
fn the_snapshot_back_to_pm_is_the_stored_document() {
    let stored = kitchen_pm();
    let (snapshot, _) = pm_to_crdt_snapshot(&stored, 1).unwrap();
    let doc = cdoc_crdt::Doc::decode_snapshot(&snapshot, cdoc_crdt::ClientId(2)).unwrap();
    let back = cdoc_pm::to_pm_json(&cdoc_crdt::to_model(&doc).unwrap());
    let mut normal = kitchen_sink();
    normal.normalize();
    assert_eq!(back, cdoc_pm::to_pm_json(&normal));
}

#[test]
fn bad_json_and_bad_trees_are_refused_by_name() {
    let stored = kitchen_pm();
    let cut = &stored[..stored.len() / 2];
    let err = pm_to_crdt_snapshot(cut, 1).unwrap_err();
    assert!(err.contains("not a ProseMirror node"), "{err}");
    let err = pm_to_crdt_snapshot(r#"{"type":"paragraph"}"#, 1).unwrap_err();
    assert!(
        !err.is_empty() && !err.contains("not a ProseMirror node"),
        "{err}"
    );
    let err = pm_to_crdt_snapshot(r#"{"type":"doc","content":[{"type":"nope"}]}"#, 1).unwrap_err();
    assert!(err.contains("nope"), "{err}");
    let marked = r#""marks":[{"type":"bold""#;
    assert!(stored.contains(marked), "the kitchen sink has bold text");
    let mark = stored.replacen(marked, r#""marks":[{"type":"glitter""#, 1);
    let err = pm_to_crdt_snapshot(&mark, 1).unwrap_err();
    assert!(err.contains("glitter"), "{err}");
}
