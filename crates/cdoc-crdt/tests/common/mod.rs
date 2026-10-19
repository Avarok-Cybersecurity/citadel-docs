//! What the tests share.

/// Room for everything these tests buffer: the cap is not what they test
/// (`cdoc-crdt/tests/pending_limits.rs` does).
pub const ROOM: cdoc_crdt::PendingLimits = cdoc_crdt::PendingLimits {
    max_ops: usize::MAX,
    max_bytes: usize::MAX,
};
