//! Format stability: the bytes are pinned. `every-op.bin` is the layout-independent pin (one
//! of every opcode, hand-built); `kitchen-sink.bin` pins the whole path from the kitchen-sink
//! layout. Each file's checksum is pinned here too, so a regenerated file is a visible diff.
//!
//! If `every-op.bin` fails, the byte format changed: bump `format::VERSION`, update
//! `docs/display-list.md`, then regenerate. If only `kitchen-sink.bin` fails, the layout (or
//! lowering) changed what is drawn; check that is intended, then regenerate with
//! `cargo run -p cdoc-paint --example emit_golden -- crates/cdoc-paint/tests/data` and update
//! the checksums below.

mod support;

const EVERY_OP: &[u8] = include_bytes!("data/every-op.bin");
const KITCHEN: &[u8] = include_bytes!("data/kitchen-sink.bin");
const EVERY_OP_FNV: u64 = 0x5df7_07b6_afa7_e302;
const KITCHEN_FNV: u64 = 0x8ab4_a1bc_585c_6960;

#[test]
fn the_every_op_bytes_are_pinned() {
    assert_eq!(
        support::fnv1a(EVERY_OP),
        EVERY_OP_FNV,
        "the checked-in file changed"
    );
    let bytes = support::every_op().to_bytes().expect("writes");
    assert!(
        bytes == EVERY_OP,
        "the encoding of every opcode changed: a format change needs a version bump"
    );
}

#[test]
fn the_kitchen_sink_bytes_are_pinned() {
    assert_eq!(
        support::fnv1a(KITCHEN),
        KITCHEN_FNV,
        "the checked-in file changed"
    );
    let bytes = cdoc_paint::encode(&support::kitchen(), &support::fonts()).expect("encodes");
    assert!(
        bytes == KITCHEN,
        "the kitchen sink's display list changed ({} bytes, golden {})",
        bytes.len(),
        KITCHEN.len()
    );
}

#[test]
fn the_golden_files_decode_with_this_version() {
    for bytes in [EVERY_OP, KITCHEN] {
        let view = cdoc_paint::decode(bytes).expect("decodes");
        view.to_owned().expect("every page");
    }
}
