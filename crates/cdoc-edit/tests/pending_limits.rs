//! The editor buffers a peer's waiting operations within the limits its caller passes, and
//! says how many it dropped.

mod common;

use cdoc_crdt::PendingLimits;
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
