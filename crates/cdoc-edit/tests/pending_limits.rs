//! The editor buffers a peer's waiting operations within the limits its caller passes, and
//! says how many it dropped; its result record carries the first unit waited for and that
//! count, and a local request's record carries neither.

mod common;

use cdoc_crdt::PendingLimits;
use cdoc_crdt::{ClientId, Id};
use cdoc_edit::wire::{decode_result, encode_result, Remote};
use cdoc_edit::Command;
use common::{caret, editor, paragraphs, peer, ROOM};

#[test]
fn a_remote_update_over_the_limits_reports_what_it_dropped() {
    let mut a = editor(&paragraphs(&["one"]), 1);
    let mut b = peer(&a, 2);
    caret(&mut b, 1);
    let held_back = b.apply(Command::InsertText("x".into())).unwrap();
    let waits = b.apply(Command::InsertText("y".into())).unwrap();
    let none = PendingLimits {
        max_ops: 0,
        max_bytes: 0,
    };
    let r = a.apply_remote(&waits.update, none).unwrap();
    assert_eq!((r.dropped, r.missing), (1, vec![]));
    let r = a.apply_remote(&waits.update, ROOM).unwrap();
    assert_eq!((r.dropped, r.missing.len()), (0, 1));
    a.apply_remote(&held_back.update, ROOM).unwrap();
    assert_eq!(a.model(), b.model());
}

#[test]
fn the_result_record_says_what_a_remote_update_left_waiting() {
    let mut a = editor(&paragraphs(&["one"]), 1);
    let mut b = peer(&a, 2);
    caret(&mut b, 1);
    let held_back = b.apply(Command::InsertText("x".into())).unwrap();
    let waits = b.apply(Command::InsertText("y".into())).unwrap();
    let record = |a: &mut cdoc_edit::Editor<'_>, update: &[u8], limits| {
        let applied = a.apply_remote(update, limits).unwrap();
        let outcome = a.remote_outcome(applied).unwrap();
        let back = decode_result(&encode_result(&outcome)).unwrap();
        assert_eq!(back, outcome);
        back.remote
    };
    let none = PendingLimits {
        max_ops: 0,
        max_bytes: 0,
    };
    // b's first unit, its "x", which a lacks.
    let lacked = Id::new(ClientId(2), 0);
    assert_eq!(
        record(&mut a, &waits.update, none),
        Some(Remote {
            missing: None,
            dropped: 1
        })
    );
    assert_eq!(
        record(&mut a, &waits.update, ROOM),
        Some(Remote {
            missing: Some(lacked),
            dropped: 0
        })
    );
    assert_eq!(
        record(&mut a, &held_back.update, ROOM),
        Some(Remote {
            missing: None,
            dropped: 0
        })
    );
    let local = a.outcome(Vec::new(), Vec::new(), Vec::new()).unwrap();
    assert_eq!(local.remote, None);
}
