//! Writes the golden display lists the format-stability test pins, and prints their checksums:
//! `cargo run -p cdoc-paint --example emit_golden -- crates/cdoc-paint/tests/data`.

#[path = "../tests/support/mod.rs"]
mod support;

fn main() {
    let out = std::env::args().nth(1).expect("an output directory");
    let every = support::every_op().to_bytes().expect("writes");
    let kitchen = cdoc_paint::encode(&support::kitchen(), &support::fonts()).expect("encodes");
    for (name, bytes) in [("every-op.bin", every), ("kitchen-sink.bin", kitchen)] {
        std::fs::write(format!("{out}/{name}"), &bytes).expect("writes");
        println!(
            "{name}: {} bytes, fnv1a {:#018x}",
            bytes.len(),
            support::fnv1a(&bytes)
        );
    }
}
