//! Migrating the chat editor's documents is deterministic: the corpus migrates to the bytes
//! recorded here in every process, two devices migrating one document under one client write
//! snapshots that merge as a no-op, another client writes other bytes holding the same
//! document, and client 0 is refused. Rerun with `CDOC_BLESS=1` after an intended change and
//! review the diff of `tests/tiptap_corpus/`.

use cdoc_crdt::{diff_snapshots, from_model, to_model, ClientId, Doc, PendingLimits, StateVector};
use cdoc_tiptap::test_support::CORPUS;
use cdoc_wasm::migrate::tiptap_to_crdt_snapshot;
use serde_json::Value;
use std::path::PathBuf;

/// A client as the UI derives one (a hash): large, so its varints take every byte.
const CLIENT: u64 = 0x9E37_79B9_7F4A_7C15;
const LIMITS: PendingLimits = PendingLimits {
    max_ops: 1 << 20,
    max_bytes: 1 << 26,
};

fn fixture(name: &str, ext: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/tiptap_corpus/{name}.{ext}"))
}

fn model_of(snapshot: &[u8]) -> cdoc_model::Document {
    to_model(&Doc::decode_snapshot(snapshot, ClientId(2)).expect("decodes")).expect("folds")
}

#[test]
fn the_corpus_migrates_to_the_recorded_bytes_and_reports() {
    let bless = std::env::var_os("CDOC_BLESS").is_some();
    for (name, json) in CORPUS {
        let (snapshot, report) = tiptap_to_crdt_snapshot(json, CLIENT).expect(name);
        let model = serde_json::to_string_pretty(&model_of(&snapshot)).expect("serialises") + "\n";
        let report: Value = serde_json::from_str(&report).expect("report JSON");
        let report = serde_json::to_string_pretty(&report).expect("serialises") + "\n";
        if bless {
            std::fs::write(fixture(name, "snapshot"), &snapshot).expect("writes");
            std::fs::write(fixture(name, "model.json"), &model).expect("writes");
            std::fs::write(fixture(name, "report.json"), &report).expect("writes");
        }
        let recorded = std::fs::read(fixture(name, "snapshot")).expect("a recorded snapshot");
        assert!(
            recorded == snapshot,
            "{name}: snapshot bytes differ (CDOC_BLESS=1 to rerecord)"
        );
        let recorded = std::fs::read_to_string(fixture(name, "model.json")).expect("a model");
        assert_eq!(recorded, model, "{name}: model differs");
        let recorded = std::fs::read_to_string(fixture(name, "report.json")).expect("a report");
        assert_eq!(recorded, report, "{name}: report differs");
    }
}

#[test]
fn repeated_runs_write_the_same_bytes() {
    for (name, json) in CORPUS {
        let first = tiptap_to_crdt_snapshot(json, CLIENT).expect(name);
        for _ in 0..3 {
            assert_eq!(
                tiptap_to_crdt_snapshot(json, CLIENT).expect(name),
                first,
                "{name}"
            );
        }
    }
}

#[test]
fn two_devices_migrating_one_document_merge_as_a_no_op() {
    for (name, json) in CORPUS {
        let (a, _) = tiptap_to_crdt_snapshot(json, CLIENT).expect(name);
        let (b, _) = tiptap_to_crdt_snapshot(json, CLIENT).expect(name);
        let mut device_a = Doc::decode_snapshot(&a, ClientId(11)).expect("decodes");
        let device_b = Doc::decode_snapshot(&b, ClientId(12)).expect("decodes");
        let everything = device_b.encode_diff(&StateVector::empty());
        let applied = device_a.apply_update(&everything, LIMITS).expect("applies");
        assert!(applied.missing.is_empty() && applied.dropped == 0, "{name}");
        assert!(
            device_a.encode_snapshot() == a,
            "{name}: merging changed the snapshot"
        );
        assert_eq!(to_model(&device_a).expect("folds"), model_of(&a), "{name}");
        let between = diff_snapshots(&a, &b).expect("diffs");
        let applied = device_a.apply_update(&between, LIMITS).expect("applies");
        assert!(
            applied.changes.is_empty(),
            "{name}: the diff between them carries something"
        );
    }
}

#[test]
fn an_edit_on_one_device_reaches_the_other_once() {
    let (_, json) = CORPUS
        .iter()
        .find(|(n, _)| *n == "meeting-notes")
        .expect("in the corpus");
    let (a, _) = tiptap_to_crdt_snapshot(json, CLIENT).expect("migrates");
    let (b, _) = tiptap_to_crdt_snapshot(json, CLIENT).expect("migrates");
    let mut device_a = Doc::decode_snapshot(&a, ClientId(11)).expect("decodes");
    let mut device_b = Doc::decode_snapshot(&b, ClientId(12)).expect("decodes");
    let before = device_a.state_vector();
    let at = device_a
        .visible_text()
        .chars()
        .take_while(|c| *c != 'W')
        .count();
    device_a.insert_text(at, "EDIT ").expect("inserts");
    device_b
        .apply_update(&device_a.encode_diff(&before), LIMITS)
        .expect("applies");
    let edited = to_model(&device_b).expect("folds");
    assert_eq!(edited, to_model(&device_a).expect("folds"));
    let text = cdoc_tiptap::test_support::model_text(&edited);
    assert!(text.starts_with("EDIT Weekly sync"), "{text}");
    assert_eq!(text.matches("Weekly sync").count(), 1, "{text}");
}

#[test]
fn another_client_writes_other_bytes_holding_the_same_document() {
    for (name, json) in CORPUS {
        let (a, report_a) = tiptap_to_crdt_snapshot(json, CLIENT).expect(name);
        let (b, report_b) = tiptap_to_crdt_snapshot(json, CLIENT + 1).expect(name);
        assert!(a != b, "{name}: the client is not in the bytes");
        assert_eq!(model_of(&a), model_of(&b), "{name}");
        assert_eq!(report_a, report_b, "{name}");
    }
}

#[test]
fn the_snapshot_round_trips_through_the_model_to_the_same_bytes() {
    for (name, json) in CORPUS {
        let (snapshot, _) = tiptap_to_crdt_snapshot(json, CLIENT).expect(name);
        let (mapped, _) = cdoc_tiptap::tiptap_to_document(json).expect(name);
        assert_eq!(model_of(&snapshot), mapped, "{name}");
        let again = from_model(&model_of(&snapshot), ClientId(CLIENT)).expect("writes");
        assert!(
            again.encode_snapshot() == snapshot,
            "{name}: model round trip moved bytes"
        );
    }
}

#[test]
fn client_zero_and_non_documents_are_refused_by_name() {
    let (_, json) = CORPUS[2];
    let err = tiptap_to_crdt_snapshot(json, 0).unwrap_err();
    assert!(err.contains("client 0 is reserved"), "{err}");
    let err = tiptap_to_crdt_snapshot("{\"type\":", CLIENT).unwrap_err();
    assert!(err.contains("not JSON"), "{err}");
    let err = tiptap_to_crdt_snapshot("[]", CLIENT).unwrap_err();
    assert!(err.contains("not a TipTap node"), "{err}");
}

#[test]
fn the_report_has_the_import_shape_with_tiers() {
    let (_, json) = CORPUS
        .iter()
        .find(|(n, _)| *n == "unknown-node")
        .expect("in the corpus");
    let (_, report) = tiptap_to_crdt_snapshot(json, CLIENT).expect("migrates");
    let report: Value = serde_json::from_str(&report).expect("JSON");
    assert_eq!(report["problems"], serde_json::json!([]));
    let losses = report["report"]["losses"].as_array().expect("losses");
    for loss in losses {
        assert_eq!(loss["part"], "chat document");
        assert!(["info", "minor", "major"].contains(&loss["severity"].as_str().expect("severity")));
        assert!(loss["label"].as_str().is_some_and(|l| !l.is_empty()));
    }
    assert!(losses.iter().any(|l| l["severity"] == "major"));
}
