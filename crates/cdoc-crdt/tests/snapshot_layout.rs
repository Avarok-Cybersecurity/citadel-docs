//! Snapshot layout 2: units located by position, clocks as gaps, delete ranges by position,
//! Word's built-in styles as one marker. What a loaded replica holds is what the source held
//! (its diff to anyone is byte-identical), the sizes are what the layout promises, and what
//! the layout cannot carry is refused by name. The churn workload is the benchmark's.

mod common;
#[path = "../benches/common/workload.rs"]
mod workload;

use cdoc_crdt::{from_model, to_model, ClientId, DecodeError, Doc, DocAttr, StateVector};
use cdoc_model::{Document, StyleSheet};
use common::ROOM;

/// Loads `d`'s snapshot and checks the replica is `d` again: the same diff for an empty peer
/// (every operation, origin and delete), and the same snapshot.
fn reload(d: &Doc) -> Doc {
    let bytes = d.encode_snapshot();
    let back = Doc::decode_snapshot(&bytes, ClientId(77)).expect("a snapshot loads");
    let empty = StateVector::empty();
    assert_eq!(back.encode_diff(&empty), d.encode_diff(&empty));
    assert_eq!(back.encode_snapshot(), bytes);
    back
}

#[test]
fn the_churned_snapshot_is_under_the_revised_target_and_loads_whole() {
    let d = workload::churn_doc();
    let bytes = d.encode_snapshot();
    assert!(bytes.len() <= 1_150_000, "{} bytes", bytes.len());
    let back = reload(&d);
    assert_eq!(back.visible_text(), d.visible_text());
}

#[test]
fn a_collected_snapshot_names_collected_origins_explicitly_and_keeps_converging() {
    let mut d = workload::churn_doc();
    let mut peer = Doc::decode_snapshot(&d.encode_snapshot(), ClientId(2)).unwrap();
    d.gc(&[d.state_vector(), peer.state_vector()]).unwrap();
    let mut back = reload(&d);
    back.insert_text(1, "after").unwrap();
    peer.insert_text(2, "meanwhile").unwrap();
    peer.apply_update(&back.encode_diff(&peer.state_vector()), ROOM)
        .unwrap();
    back.apply_update(&peer.encode_diff(&back.state_vector()), ROOM)
        .unwrap();
    assert_eq!(back.visible_text(), peer.visible_text());
}

#[test]
fn concurrent_replicas_reload_with_cross_client_origins_and_deletes() {
    let mut a = Doc::new(ClientId(1));
    let mut b = Doc::new(ClientId(900));
    a.insert_text(0, "shared start").unwrap();
    b.apply_update(&a.encode_diff(&b.state_vector()), ROOM)
        .unwrap();
    for i in 0..40 {
        a.insert_text(i % a.len(), "a").unwrap();
        b.insert_text((i * 7) % b.len(), "bb").unwrap();
        if i % 3 == 0 {
            b.delete(i % (b.len() - 2), 2).unwrap();
        }
        if i % 5 == 0 {
            a.apply_update(&b.encode_diff(&a.state_vector()), ROOM)
                .unwrap();
            a.delete(i % (a.len() - 3), 3).unwrap();
            b.apply_update(&a.encode_diff(&b.state_vector()), ROOM)
                .unwrap();
        }
    }
    a.apply_update(&b.encode_diff(&a.state_vector()), ROOM)
        .unwrap();
    b.apply_update(&a.encode_diff(&b.state_vector()), ROOM)
        .unwrap();
    let back = reload(&a);
    assert_eq!(back.visible_text(), b.visible_text());
    assert_eq!(reload(&b).encode_snapshot(), back.encode_snapshot());
}

#[test]
fn the_built_in_styles_are_one_marker_and_stay_live_registers() {
    let mut a = from_model(&Document::blank(), ClientId(1)).unwrap();
    let marked = a.encode_snapshot();
    let mut b = reload(&a);
    assert_eq!(to_model(&b).unwrap(), to_model(&a).unwrap());
    // One style edited on the loaded replica: the register is live, the edit converges.
    let mut sheet = StyleSheet::word_builtin();
    sheet.styles[3].name = "Renamed".to_owned();
    b.set_attr(&DocAttr::Styles(sheet)).unwrap();
    a.apply_update(&b.encode_diff(&a.state_vector()), ROOM)
        .unwrap();
    assert_eq!(to_model(&a).unwrap(), to_model(&b).unwrap());
    assert_eq!(to_model(&a).unwrap().styles.styles[3].name, "Renamed");
    // The run no longer survives whole, so every write is written out and still loads.
    let written = a.encode_snapshot();
    assert!(
        written.len() > marked.len() + 2_000,
        "{} vs {}",
        written.len(),
        marked.len()
    );
    reload(&a);
}

#[test]
fn a_document_without_the_built_in_run_writes_every_style() {
    let mut model = Document::blank();
    model.styles.styles[0].name = "Changed".to_owned();
    let d = from_model(&model, ClientId(1)).unwrap();
    let with_run = from_model(&Document::blank(), ClientId(1)).unwrap();
    assert!(d.encode_snapshot().len() > with_run.encode_snapshot().len() + 2_000);
    assert_eq!(to_model(&reload(&d)).unwrap(), to_model(&d).unwrap());
}

/// `"ab"` typed by client 1: header (kind 2), state vector, empty floor, the text section,
/// one item (client index 0, clock gap 0, head 0: text with no origins, 2 chars, 2 bytes),
/// no operations.
fn ab() -> Vec<u8> {
    [
        b"CDC\x02\x02".as_slice(),
        &[1, 1, 2, 0, 2, b'a', b'b', 1, 0, 0, 0, 2, 2, 0],
    ]
    .concat()
}

#[test]
fn the_layout_is_the_documented_one_and_what_it_cannot_carry_is_refused_by_name() {
    let mut d = Doc::new(ClientId(1));
    d.insert_text(0, "ab").unwrap();
    assert_eq!(d.encode_snapshot(), ab());
    let load = |bytes: &[u8]| Doc::decode_snapshot(bytes, ClientId(2)).map(|_| ());
    assert_eq!(load(&ab()), Ok(()));
    let mut layout_1 = ab();
    layout_1[4] = 1;
    let refused = DecodeError::SnapshotLayout {
        found: 1,
        supported: 2,
    };
    assert_eq!(load(&layout_1), Err(refused.clone()));
    assert_eq!(cdoc_crdt::snapshot_version(&layout_1), Err(refused));
    let mut high_bit = ab();
    high_bit[15] = 0x80;
    assert!(matches!(load(&high_bit), Err(DecodeError::BadTag { .. })));
    // A left origin one unit before the first unit there is.
    let mut before_start = ab();
    before_start[15] = 2 << 3;
    before_start.insert(16, 1);
    assert_eq!(
        load(&before_start),
        Err(DecodeError::Inconsistent("an origin outside the snapshot"))
    );
    // A right origin past the last unit.
    let mut past_end = ab();
    past_end[15] = 1 << 5;
    past_end.insert(16, 0);
    assert_eq!(
        load(&past_end),
        Err(DecodeError::Inconsistent(
            "a position outside the snapshot's units"
        ))
    );
}
